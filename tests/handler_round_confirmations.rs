mod common;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use principal_candidate_manager::{
    enums::AuditAction,
    handlers::{
        applications::{teacher_create_application, teacher_delete_application, CreateApplicationBody},
        overview::get_overview,
        round_confirmations::{
            admin_get_confirmation_status, teacher_confirm_round, teacher_revoke_confirmation,
        },
        rounds::{close_round, open_round, reopen_round},
    },
    state::AppState,
};

// ── 픽스처 ────────────────────────────────────────────────────────

/// 학급 1-1, 재학생 1명, 대학 1개, 트랙 1개, OPEN 라운드를 생성한다.
/// 반환값: (student_id, track_id, round_id)
async fn setup(pool: &sqlx::SqlitePool) -> (i64, i64, i64) {
    let hash = bcrypt::hash("pass", 4u32).unwrap();
    sqlx::query("INSERT INTO classes (grade, class_no, teacher_name, password_hash) VALUES (1, 1, '김철수', ?)")
        .bind(&hash)
        .execute(pool)
        .await
        .unwrap();

    let sid: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES ('S001', '홍길동', 1, 1, 1, 1) RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();

    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name) VALUES ('한국대') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();

    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name) VALUES (?, '컴공') RETURNING id",
    )
    .bind(uid)
    .fetch_one(pool)
    .await
    .unwrap();

    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at) VALUES ('OPEN', '2025-01-01T00:00:00Z') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();

    (sid, tid, rid)
}

fn app_state(pool: sqlx::SqlitePool) -> State<AppState> {
    State(common::make_state(pool))
}

// ── 1. 확정 성공: row 생성 + 감사 로그 ────────────────────────────

#[tokio::test]
async fn confirm_success_creates_row_and_audit() {
    let pool = common::create_test_pool().await;
    let (_, _, rid) = setup(&pool).await;

    let res = teacher_confirm_round(
        app_state(pool.clone()),
        Extension(common::teacher_claims(1, 1)),
        Path(rid),
    )
    .await;
    assert_eq!(res.unwrap(), StatusCode::NO_CONTENT);

    let row_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM round_confirmations WHERE round_id = ? AND grade = 1 AND class_no = 1",
    )
    .bind(rid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row_count, 1, "확정 행 생성");

    let audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE action = ? AND round_id = ?",
    )
    .bind(AuditAction::RoundConfirmed)
    .bind(rid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit_count, 1, "ROUND_CONFIRMED 감사 로그 1건");
}

// ── 2. 재확정 → 409, 감사 로그 추가 없음 ─────────────────────────

#[tokio::test]
async fn confirm_twice_returns_conflict() {
    let pool = common::create_test_pool().await;
    let (_, _, rid) = setup(&pool).await;

    teacher_confirm_round(app_state(pool.clone()), Extension(common::teacher_claims(1, 1)), Path(rid))
        .await
        .unwrap();

    let res = teacher_confirm_round(
        app_state(pool.clone()),
        Extension(common::teacher_claims(1, 1)),
        Path(rid),
    )
    .await;
    assert_eq!(res.unwrap_err().0, StatusCode::CONFLICT);

    let audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE action = ?",
    )
    .bind(AuditAction::RoundConfirmed)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit_count, 1, "감사 로그 증가 없음");
}

// ── 3. CLOSED 라운드 확정 → 400 / 없는 라운드 → 404 ─────────────

#[tokio::test]
async fn confirm_closed_round_returns_bad_request() {
    let pool = common::create_test_pool().await;
    let (_, _, closed_rid) = setup(&pool).await;
    // 기존 OPEN 라운드를 직접 CLOSED로 전환 (새 라운드 INSERT 시 idx_one_active_round 위반)
    sqlx::query("UPDATE rounds SET status = 'CLOSED', closed_at = '2025-01-02T00:00:00Z' WHERE id = ?")
        .bind(closed_rid)
        .execute(&pool)
        .await
        .unwrap();

    let res = teacher_confirm_round(
        app_state(pool.clone()),
        Extension(common::teacher_claims(1, 1)),
        Path(closed_rid),
    )
    .await;
    assert_eq!(res.unwrap_err().0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn confirm_nonexistent_round_returns_not_found() {
    let pool = common::create_test_pool().await;
    setup(&pool).await;

    let res = teacher_confirm_round(
        app_state(pool.clone()),
        Extension(common::teacher_claims(1, 1)),
        Path(9999i64),
    )
    .await;
    assert_eq!(res.unwrap_err().0, StatusCode::NOT_FOUND);
}

// ── 4. 확정 취소 성공: row 삭제 + REVOKED(auto=false) ────────────

#[tokio::test]
async fn revoke_confirmation_success() {
    let pool = common::create_test_pool().await;
    let (_, _, rid) = setup(&pool).await;

    teacher_confirm_round(app_state(pool.clone()), Extension(common::teacher_claims(1, 1)), Path(rid))
        .await
        .unwrap();

    let res = teacher_revoke_confirmation(
        app_state(pool.clone()),
        Extension(common::teacher_claims(1, 1)),
        Path(rid),
    )
    .await;
    assert_eq!(res.unwrap(), StatusCode::NO_CONTENT);

    let row_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM round_confirmations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row_count, 0, "확정 행 삭제");

    let revoked_log: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE action = ?",
    )
    .bind(AuditAction::RoundConfirmationRevoked)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(revoked_log, 1, "REVOKED 로그 1건");

    let detail: String =
        sqlx::query_scalar("SELECT detail FROM audit_log WHERE action = ?")
            .bind(AuditAction::RoundConfirmationRevoked)
            .fetch_one(&pool)
            .await
            .unwrap();
    let v: serde_json::Value = serde_json::from_str(&detail).unwrap();
    assert_eq!(v["auto"], false, "auto=false");
}

// ── 5. 미확정 상태 취소 → 404, 로그 없음 ─────────────────────────

#[tokio::test]
async fn revoke_when_not_confirmed_returns_not_found() {
    let pool = common::create_test_pool().await;
    let (_, _, rid) = setup(&pool).await;

    let res = teacher_revoke_confirmation(
        app_state(pool.clone()),
        Extension(common::teacher_claims(1, 1)),
        Path(rid),
    )
    .await;
    assert_eq!(res.unwrap_err().0, StatusCode::NOT_FOUND);

    let log_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_log")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(log_count, 0, "로그 없음");
}

// ── 6a. 자동 해제: teacher_create_application 후 확정 row 삭제 ────

#[tokio::test]
async fn auto_revoke_on_create_application() {
    let pool = common::create_test_pool_shared().await;
    let (sid, tid, rid) = setup(&pool).await;

    teacher_confirm_round(app_state(pool.clone()), Extension(common::teacher_claims(1, 1)), Path(rid))
        .await
        .unwrap();

    teacher_create_application(
        app_state(pool.clone()),
        Extension(common::teacher_claims(1, 1)),
        Json(CreateApplicationBody {
            student_id: sid,
            track_id: tid,
            round_id: rid,
            department_name: "컴퓨터공학과".into(),
            base_data_entries: vec![],
            prev_track_id: None,
        }),
    )
    .await
    .unwrap();

    let row_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM round_confirmations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row_count, 0, "확정 행 자동 삭제");

    let revoked_log: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE action = ?",
    )
    .bind(AuditAction::RoundConfirmationRevoked)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(revoked_log, 1, "REVOKED(auto=true) 로그 1건");

    let detail: String =
        sqlx::query_scalar("SELECT detail FROM audit_log WHERE action = ?")
            .bind(AuditAction::RoundConfirmationRevoked)
            .fetch_one(&pool)
            .await
            .unwrap();
    let v: serde_json::Value = serde_json::from_str(&detail).unwrap();
    assert_eq!(v["auto"], true, "auto=true");
}

// ── 6b. 자동 해제: teacher_delete_application 후 확정 row 삭제 ───

#[tokio::test]
async fn auto_revoke_on_delete_application() {
    let pool = common::create_test_pool_shared().await;
    let (sid, tid, rid) = setup(&pool).await;

    teacher_create_application(
        app_state(pool.clone()),
        Extension(common::teacher_claims(1, 1)),
        Json(CreateApplicationBody {
            student_id: sid,
            track_id: tid,
            round_id: rid,
            department_name: "컴퓨터공학과".into(),
            base_data_entries: vec![],
            prev_track_id: None,
        }),
    )
    .await
    .unwrap();

    teacher_confirm_round(app_state(pool.clone()), Extension(common::teacher_claims(1, 1)), Path(rid))
        .await
        .unwrap();

    teacher_delete_application(
        app_state(pool.clone()),
        Extension(common::teacher_claims(1, 1)),
        Path((sid, tid, rid)),
    )
    .await
    .unwrap();

    let row_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM round_confirmations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row_count, 0, "확정 행 자동 삭제");

    let revoked_log: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE action = ?",
    )
    .bind(AuditAction::RoundConfirmationRevoked)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(revoked_log, 1, "REVOKED(auto=true) 로그 1건");
}

// ── 7. 미확정 상태에서 지원 저장 → REVOKED 로그 없음 (유령 로그 방지) ──

#[tokio::test]
async fn no_ghost_revoke_log_when_not_confirmed() {
    let pool = common::create_test_pool_shared().await;
    let (sid, tid, rid) = setup(&pool).await;

    teacher_create_application(
        app_state(pool.clone()),
        Extension(common::teacher_claims(1, 1)),
        Json(CreateApplicationBody {
            student_id: sid,
            track_id: tid,
            round_id: rid,
            department_name: "컴퓨터공학과".into(),
            base_data_entries: vec![],
            prev_track_id: None,
        }),
    )
    .await
    .unwrap();

    let revoked_log: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE action = ?",
    )
    .bind(AuditAction::RoundConfirmationRevoked)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(revoked_log, 0, "유령 로그 없음");
}

// ── 8. 졸업생 담당(0/0) 확정 + admin status에 포함 ─────────────────

#[tokio::test]
async fn grad_teacher_confirm_included_in_admin_status() {
    let pool = common::create_test_pool().await;
    let (_, _, rid) = setup(&pool).await;

    let hash = bcrypt::hash("pass", 4u32).unwrap();
    sqlx::query("INSERT INTO classes (grade, class_no, password_hash) VALUES (0, 0, ?)")
        .bind(&hash)
        .execute(&pool)
        .await
        .unwrap();

    teacher_confirm_round(
        app_state(pool.clone()),
        Extension(common::teacher_claims(0, 0)),
        Path(rid),
    )
    .await
    .unwrap();

    let Json(status) = admin_get_confirmation_status(app_state(pool.clone()), Path(rid))
        .await
        .unwrap();

    let grad_cls = status.classes.iter().find(|c| c.grade == 0 && c.class_no == 0);
    assert!(grad_cls.is_some(), "0/0 학급이 결과에 포함");
    assert!(grad_cls.unwrap().confirmed, "0/0 확정 표시");
}

// ── 9. admin confirmation-status: 전 학급 반환 + confirmed 플래그 ──

#[tokio::test]
async fn admin_confirmation_status_all_classes_with_flags() {
    let pool = common::create_test_pool().await;
    let (_, _, rid) = setup(&pool).await;

    let hash = bcrypt::hash("pass", 4u32).unwrap();
    for class_no in [2i64, 3i64] {
        sqlx::query("INSERT INTO classes (grade, class_no, password_hash) VALUES (1, ?, ?)")
            .bind(class_no)
            .bind(&hash)
            .execute(&pool)
            .await
            .unwrap();
    }

    // 1-1만 확정
    teacher_confirm_round(app_state(pool.clone()), Extension(common::teacher_claims(1, 1)), Path(rid))
        .await
        .unwrap();

    let Json(status) = admin_get_confirmation_status(app_state(pool.clone()), Path(rid))
        .await
        .unwrap();

    assert_eq!(status.classes.len(), 3, "전 학급 3개 반환");

    let cls11 = status.classes.iter().find(|c| c.grade == 1 && c.class_no == 1).unwrap();
    assert!(cls11.confirmed, "1-1 확정");
    assert!(cls11.confirmed_at.is_some(), "confirmed_at 있음");

    let cls12 = status.classes.iter().find(|c| c.grade == 1 && c.class_no == 2).unwrap();
    assert!(!cls12.confirmed, "1-2 미확정");
    assert!(cls12.confirmed_at.is_none(), "confirmed_at 없음");
}

#[tokio::test]
async fn admin_confirmation_status_nonexistent_round_returns_not_found() {
    let pool = common::create_test_pool().await;
    let res = admin_get_confirmation_status(app_state(pool), Path(9999i64)).await;
    assert_eq!(res.unwrap_err().0, StatusCode::NOT_FOUND);
}

// ── 10. 미확정 학급 있어도 close_round 성공 ──────────────────────

#[tokio::test]
async fn close_round_succeeds_with_unconfirmed_classes() {
    let pool = common::create_test_pool_shared().await;
    let (_, _, rid) = setup(&pool).await;

    // 1-1은 확정 안 함, 지원자 없음 → base_data 누락 검증 통과 → CLOSED 성공
    let res = close_round(app_state(pool.clone()), Path(rid)).await;
    assert!(res.is_ok(), "미확정 학급이 있어도 close_round 성공: {:?}", res.err());
    let Json(val) = res.unwrap();
    assert_eq!(val["calculated"], 0);
}

// ── 11. reopen_round 후 확정 유지 확인 ───────────────────────────

#[tokio::test]
async fn reopen_round_does_not_clear_confirmations() {
    let pool = common::create_test_pool().await;
    let (_, _, rid) = setup(&pool).await;

    teacher_confirm_round(app_state(pool.clone()), Extension(common::teacher_claims(1, 1)), Path(rid))
        .await
        .unwrap();

    sqlx::query(
        "UPDATE rounds SET status = 'CLOSED', closed_at = '2025-01-02T00:00:00Z' WHERE id = ?",
    )
    .bind(rid)
    .execute(&pool)
    .await
    .unwrap();

    reopen_round(app_state(pool.clone()), Path(rid)).await.unwrap();

    let row_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM round_confirmations WHERE round_id = ?")
            .bind(rid)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(row_count, 1, "reopen 후 확정 행 유지");
}

// ── 12. delete_class CASCADE → round_confirmations 삭제 ────────────

#[tokio::test]
async fn delete_class_cascades_to_confirmations() {
    let pool = common::create_test_pool().await;
    let (_, _, rid) = setup(&pool).await;

    teacher_confirm_round(app_state(pool.clone()), Extension(common::teacher_claims(1, 1)), Path(rid))
        .await
        .unwrap();

    // 학생 먼저 삭제 (FK 순서)
    sqlx::query("DELETE FROM students WHERE grade = 1 AND class_no = 1")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM classes WHERE grade = 1 AND class_no = 1")
        .execute(&pool)
        .await
        .unwrap();

    let row_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM round_confirmations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row_count, 0, "학급 삭제 시 확정도 CASCADE 삭제");
}

// ── 13. get_overview: confirmed 필드 + graduated ──────────────────

#[tokio::test]
async fn overview_no_grad_students_returns_graduated_none() {
    let pool = common::create_test_pool().await;
    setup(&pool).await;

    let Json(resp) = get_overview(app_state(pool.clone()), Extension(common::admin_claims()))
        .await
        .unwrap();
    assert!(resp.graduated.is_none(), "졸업생 없으면 graduated=None");
}

#[tokio::test]
async fn overview_confirmed_class_and_graduated_fields() {
    let pool = common::create_test_pool().await;
    let (_, _, rid) = setup(&pool).await;

    let hash = bcrypt::hash("pass", 4u32).unwrap();
    sqlx::query("INSERT INTO classes (grade, class_no, teacher_name, password_hash) VALUES (0, 0, '졸업담당', ?)")
        .bind(&hash)
        .execute(&pool)
        .await
        .unwrap();

    let uid: i64 = sqlx::query_scalar("SELECT id FROM universities LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let gtid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name) VALUES (?, '졸업생전형') RETURNING id",
    )
    .bind(uid)
    .fetch_one(&pool)
    .await
    .unwrap();
    let gsid: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, is_enrolled, grad_year) VALUES ('G001', '졸업생', 0, 2024) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO applications (student_id, track_id, round_id, abandoned, department_name) \
         VALUES (?, ?, ?, 0, '테스트학과')",
    )
    .bind(gsid)
    .bind(gtid)
    .bind(rid)
    .execute(&pool)
    .await
    .unwrap();

    // 1-1 확정
    teacher_confirm_round(app_state(pool.clone()), Extension(common::teacher_claims(1, 1)), Path(rid))
        .await
        .unwrap();
    // 0/0 확정
    teacher_confirm_round(app_state(pool.clone()), Extension(common::teacher_claims(0, 0)), Path(rid))
        .await
        .unwrap();

    let Json(resp) = get_overview(app_state(pool.clone()), Extension(common::admin_claims()))
        .await
        .unwrap();

    let cls11 = resp.classes.iter().find(|c| c.grade == 1 && c.class_no == 1).unwrap();
    assert!(cls11.confirmed, "1-1 confirmed=true");
    assert!(cls11.confirmed_at.is_some(), "1-1 confirmed_at 있음");

    let grad = resp.graduated.expect("졸업생 학생 있으면 Some");
    assert_eq!(grad.student_count, 1);
    assert_eq!(grad.submitted, 1, "졸업생 지원 1건");
    assert!(grad.confirmed, "0/0 확정됨");
    assert!(grad.confirmed_at.is_some(), "0/0 confirmed_at 있음");
    assert_eq!(grad.teacher_name.as_deref(), Some("졸업담당"));
}

#[tokio::test]
async fn overview_graduated_submitted_zero_when_no_applications() {
    let pool = common::create_test_pool().await;
    setup(&pool).await;

    sqlx::query_scalar::<_, i64>(
        "INSERT INTO students (student_code, name, is_enrolled, grad_year) VALUES ('G001', '졸업생', 0, 2024) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let Json(resp) = get_overview(app_state(pool.clone()), Extension(common::admin_claims()))
        .await
        .unwrap();

    let grad = resp.graduated.expect("졸업생 학생이 있으면 Some");
    assert_eq!(grad.student_count, 1);
    assert_eq!(grad.submitted, 0, "지원 없으면 submitted=0");
    assert!(!grad.confirmed, "미확정");
}

#[tokio::test]
async fn open_round_creates_new_active_round() {
    let pool = common::create_test_pool().await;
    let res = open_round(app_state(pool.clone())).await;
    assert!(res.is_ok());
    let (status, Json(val)) = res.unwrap();
    assert_eq!(status, StatusCode::CREATED);
    assert!(val["id"].is_number());
}

// ── 리뷰 추가: CLOSED 라운드 확정 취소 → 400 (종료 후 사후 변경 차단) ──

#[tokio::test]
async fn revoke_on_closed_round_returns_bad_request() {
    let pool = common::create_test_pool().await;
    let (_, _, rid) = setup(&pool).await;

    teacher_confirm_round(app_state(pool.clone()), Extension(common::teacher_claims(1, 1)), Path(rid))
        .await
        .unwrap();

    sqlx::query("UPDATE rounds SET status = 'CLOSED', closed_at = '2025-01-02T00:00:00Z' WHERE id = ?")
        .bind(rid)
        .execute(&pool)
        .await
        .unwrap();

    let res = teacher_revoke_confirmation(
        app_state(pool.clone()),
        Extension(common::teacher_claims(1, 1)),
        Path(rid),
    )
    .await;
    assert_eq!(res.unwrap_err().0, StatusCode::BAD_REQUEST);

    // 확정 기록은 그대로 보존
    let row_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM round_confirmations WHERE round_id = ?",
    )
    .bind(rid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row_count, 1, "종료된 라운드의 확정은 담임이 취소할 수 없다");

    let revoke_logs: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE action = ?",
    )
    .bind(AuditAction::RoundConfirmationRevoked)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(revoke_logs, 0, "해제 로그 없음");
}

// ── 리뷰 추가: 없는 지원 삭제 → 404, 확정·로그 유령 발생 금지 ─────

#[tokio::test]
async fn delete_nonexistent_application_returns_404_keeps_confirmation() {
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup(&pool).await;

    teacher_confirm_round(app_state(pool.clone()), Extension(common::teacher_claims(1, 1)), Path(rid))
        .await
        .unwrap();

    // 지원이 존재하지 않는 상태에서 삭제 시도
    let res = teacher_delete_application(
        app_state(pool.clone()),
        Extension(common::teacher_claims(1, 1)),
        Path((sid, tid, rid)),
    )
    .await;
    assert_eq!(res.unwrap_err().0, StatusCode::NOT_FOUND);

    // 확정이 유령 해제되지 않아야 함 (rollback)
    let row_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM round_confirmations WHERE round_id = ?",
    )
    .bind(rid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row_count, 1, "없는 지원 삭제 요청이 확정을 해제하면 안 된다");

    let ghost_logs: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE action IN (?, ?)",
    )
    .bind(AuditAction::RoundConfirmationRevoked)
    .bind(AuditAction::ApplicationDeleted)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(ghost_logs, 0, "삭제·해제 유령 로그 없음");
}
