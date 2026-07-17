mod common;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use principal_candidate_manager::handlers::applications::{
    abandon_application, admin_list_applications, teacher_abandon_application,
    teacher_create_application, teacher_delete_application, teacher_list_applications,
    ApplicationListQuery, CreateApplicationBody, TeacherAppListQuery,
};

async fn setup(pool: &sqlx::SqlitePool) -> (i64, i64, i64) {
    let hash = bcrypt::hash("pass", 4u32).unwrap();
    sqlx::query("INSERT INTO classes (grade, class_no, password_hash) VALUES (1, 1, ?)")
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

    let univ_id: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name) VALUES ('한국대') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();

    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name, unit_quota) VALUES (?, '컴공', 5) RETURNING id",
    )
    .bind(univ_id)
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

// ── teacher_create_application ────────────────────────────────────

#[tokio::test]
async fn create_application_open_round_ok() {
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup(&pool).await;
    let res = teacher_create_application(
        State(common::make_state(pool.clone())),
        Extension(common::teacher_claims(1, 1)),
        Json(CreateApplicationBody { student_id: sid, track_id: tid, round_id: rid, department_name: "컴퓨터공학과".into(), ..Default::default() }),
    )
    .await;
    assert_eq!(res.unwrap(), StatusCode::CREATED);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM applications")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn create_application_duplicate_upserts_department_name() {
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup(&pool).await;
    teacher_create_application(
        State(common::make_state(pool.clone())),
        Extension(common::teacher_claims(1, 1)),
        Json(CreateApplicationBody {
            student_id: sid, track_id: tid, round_id: rid,
            department_name: "컴퓨터공학과".into(),
            ..Default::default()
        }),
    )
    .await
    .unwrap();
    let res = teacher_create_application(
        State(common::make_state(pool.clone())),
        Extension(common::teacher_claims(1, 1)),
        Json(CreateApplicationBody {
            student_id: sid, track_id: tid, round_id: rid,
            department_name: "전자공학과".into(),
            ..Default::default()
        }),
    )
    .await;
    assert!(res.is_ok());
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM applications")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    let dept: String = sqlx::query_scalar(
        "SELECT department_name FROM applications WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(sid).bind(tid).bind(rid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(dept, "전자공학과");
}

#[tokio::test]
async fn create_application_closed_round_returns_bad_request() {
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup(&pool).await;
    // idx_one_active_round: 비-FINALIZED 라운드는 1개만 — 기존 라운드를 CLOSED로 전환
    sqlx::query("UPDATE rounds SET status = 'CLOSED', closed_at = '2025-01-02T00:00:00Z' WHERE id = ?")
        .bind(rid)
        .execute(&pool)
        .await
        .unwrap();
    let res = teacher_create_application(
        State(common::make_state(pool)),
        Extension(common::teacher_claims(1, 1)),
        Json(CreateApplicationBody { student_id: sid, track_id: tid, round_id: rid, ..Default::default() }),
    )
    .await;
    assert_eq!(res.unwrap_err().0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_application_round_not_found_returns_not_found() {
    let pool = common::create_test_pool().await;
    let (sid, tid, _) = setup(&pool).await;
    let res = teacher_create_application(
        State(common::make_state(pool)),
        Extension(common::teacher_claims(1, 1)),
        Json(CreateApplicationBody { student_id: sid, track_id: tid, round_id: 9999, ..Default::default() }),
    )
    .await;
    assert_eq!(res.unwrap_err().0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn create_application_student_not_in_class_returns_forbidden() {
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup(&pool).await;
    let res = teacher_create_application(
        State(common::make_state(pool)),
        Extension(common::teacher_claims(2, 2)),
        Json(CreateApplicationBody { student_id: sid, track_id: tid, round_id: rid, ..Default::default() }),
    )
    .await;
    assert_eq!(res.unwrap_err().0, StatusCode::FORBIDDEN);
}

// ── teacher_create_application: 졸업생 담당 케이스 ───────────────

#[tokio::test]
async fn grad_teacher_can_create_graduated_student_application() {
    // 졸업생 담당(grade=0, class_no=0)은 is_enrolled=0 학생의 지원을 등록할 수 있어야 함
    let pool = common::create_test_pool().await;
    let (_, tid, rid) = setup(&pool).await;

    let grad_sid: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, is_enrolled, grad_year) \
         VALUES ('G001', '졸업생', 0, 2024) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let res = teacher_create_application(
        State(common::make_state(pool.clone())),
        Extension(common::teacher_claims(0, 0)), // 졸업생 담당
        Json(CreateApplicationBody {
            student_id: grad_sid, track_id: tid, round_id: rid,
            department_name: "국어국문학과".into(),
            ..Default::default()
        }),
    )
    .await;
    assert_eq!(res.unwrap(), StatusCode::CREATED);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM applications")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn grad_teacher_cannot_create_enrolled_student_application() {
    // 졸업생 담당은 재학생(is_enrolled=1)의 지원을 등록할 수 없어야 함
    let pool = common::create_test_pool().await;
    let (enrolled_sid, tid, rid) = setup(&pool).await; // setup()의 학생은 is_enrolled=1

    let res = teacher_create_application(
        State(common::make_state(pool)),
        Extension(common::teacher_claims(0, 0)), // 졸업생 담당
        Json(CreateApplicationBody {
            student_id: enrolled_sid, track_id: tid, round_id: rid,
            department_name: "컴퓨터공학과".into(),
            ..Default::default()
        }),
    )
    .await;
    assert_eq!(res.unwrap_err().0, StatusCode::FORBIDDEN);
}

// ── teacher_delete_application ────────────────────────────────────

#[tokio::test]
async fn delete_application_open_round_ok() {
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup(&pool).await;
    sqlx::query(
        "INSERT INTO applications (student_id, track_id, round_id, abandoned) \
         VALUES (?, ?, ?, 0)",
    )
    .bind(sid)
    .bind(tid)
    .bind(rid)
    .execute(&pool)
    .await
    .unwrap();
    teacher_delete_application(
        State(common::make_state(pool.clone())),
        Extension(common::teacher_claims(1, 1)),
        Path((sid, tid, rid)),
    )
    .await
    .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM applications")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn delete_application_closed_round_returns_bad_request() {
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup(&pool).await;
    sqlx::query(
        "INSERT INTO applications (student_id, track_id, round_id, abandoned) \
         VALUES (?, ?, ?, 0)",
    )
    .bind(sid)
    .bind(tid)
    .bind(rid)
    .execute(&pool)
    .await
    .unwrap();
    // idx_one_active_round: 비-FINALIZED 라운드는 1개만 — 기존 라운드를 CLOSED로 전환
    sqlx::query("UPDATE rounds SET status = 'CLOSED', closed_at = '2025-01-02T00:00:00Z' WHERE id = ?")
        .bind(rid)
        .execute(&pool)
        .await
        .unwrap();
    let res = teacher_delete_application(
        State(common::make_state(pool)),
        Extension(common::teacher_claims(1, 1)),
        Path((sid, tid, rid)),
    )
    .await;
    assert_eq!(res.unwrap_err().0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn delete_application_wrong_class_returns_forbidden() {
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup(&pool).await;
    sqlx::query(
        "INSERT INTO applications (student_id, track_id, round_id, abandoned) \
         VALUES (?, ?, ?, 0)",
    )
    .bind(sid)
    .bind(tid)
    .bind(rid)
    .execute(&pool)
    .await
    .unwrap();
    let res = teacher_delete_application(
        State(common::make_state(pool)),
        Extension(common::teacher_claims(2, 2)),
        Path((sid, tid, rid)),
    )
    .await;
    assert_eq!(res.unwrap_err().0, StatusCode::FORBIDDEN);
}

// ── teacher_delete_application: 졸업생 담당 케이스 ───────────────

#[tokio::test]
async fn grad_teacher_can_delete_graduated_student_application() {
    // 졸업생 담당(grade=0, class_no=0)은 is_enrolled=0 학생의 지원을 삭제할 수 있어야 함
    let pool = common::create_test_pool().await;
    let (_, tid, rid) = setup(&pool).await;

    let grad_sid: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, is_enrolled, grad_year) \
         VALUES ('G001', '졸업생', 0, 2024) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    insert_application(&pool, grad_sid, tid, rid).await;

    teacher_delete_application(
        State(common::make_state(pool.clone())),
        Extension(common::teacher_claims(0, 0)), // 졸업생 담당
        Path((grad_sid, tid, rid)),
    )
    .await
    .unwrap();

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM applications")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

// ── abandon_application ───────────────────────────────────────────

/// 테스트용 지원 행 삽입 헬퍼 (setup에서 생성된 sid, tid, rid 사용)
async fn insert_application(pool: &sqlx::SqlitePool, sid: i64, tid: i64, rid: i64) {
    sqlx::query(
        "INSERT INTO applications (student_id, track_id, round_id, abandoned) \
         VALUES (?, ?, ?, 0)",
    )
    .bind(sid)
    .bind(tid)
    .bind(rid)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn abandon_on_open_round_returns_bad_request() {
    // OPEN 상태에서는 포기 불가
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup(&pool).await;
    insert_application(&pool, sid, tid, rid).await;
    let res = abandon_application(State(common::make_state(pool)), Path((sid, tid, rid))).await;
    assert_eq!(res.unwrap_err().0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn abandon_on_closed_round_returns_bad_request() {
    // CLOSED 상태에서도 포기 불가 (FINALIZED에서만 허용)
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup(&pool).await;
    insert_application(&pool, sid, tid, rid).await;
    // idx_one_active_round: 비-FINALIZED 라운드는 1개만 — 기존 라운드를 CLOSED로 전환
    sqlx::query("UPDATE rounds SET status = 'CLOSED', closed_at = '2025-01-02T00:00:00Z' WHERE id = ?")
        .bind(rid)
        .execute(&pool)
        .await
        .unwrap();
    let res = abandon_application(State(common::make_state(pool)), Path((sid, tid, rid))).await;
    assert_eq!(res.unwrap_err().0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn abandon_application_sets_abandoned_flag() {
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup(&pool).await;
    // abandon은 FINALIZED 라운드에서만 허용
    sqlx::query("UPDATE rounds SET status = 'FINALIZED' WHERE id = ?")
        .bind(rid)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO applications (student_id, track_id, round_id, abandoned) \
         VALUES (?, ?, ?, 0)",
    )
    .bind(sid)
    .bind(tid)
    .bind(rid)
    .execute(&pool)
    .await
    .unwrap();
    abandon_application(State(common::make_state(pool.clone())), Path((sid, tid, rid)))
        .await
        .unwrap();
    let abandoned: i64 = sqlx::query_scalar(
        "SELECT abandoned FROM applications \
         WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(sid)
    .bind(tid)
    .bind(rid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(abandoned, 1);
}

#[tokio::test]
async fn abandon_nonexistent_application_returns_not_found() {
    // 존재하지 않는 지원에 대한 포기는 silent no-op(204)이 아니라 404여야 한다
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup(&pool).await;
    sqlx::query("UPDATE rounds SET status = 'FINALIZED' WHERE id = ?")
        .bind(rid)
        .execute(&pool)
        .await
        .unwrap();
    // 지원 행을 삽입하지 않음
    let res = abandon_application(State(common::make_state(pool.clone())), Path((sid, tid, rid))).await;
    assert_eq!(res.unwrap_err().0, StatusCode::NOT_FOUND);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM applications WHERE abandoned = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

// ── teacher_abandon_application ───────────────────────────────────

#[tokio::test]
async fn teacher_abandon_finalized_round_sets_flag() {
    // FINALIZED 라운드에서 담임이 포기 처리 → abandoned=1
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup(&pool).await;
    sqlx::query("UPDATE rounds SET status = 'FINALIZED' WHERE id = ?")
        .bind(rid)
        .execute(&pool)
        .await
        .unwrap();
    insert_application(&pool, sid, tid, rid).await;
    teacher_abandon_application(
        State(common::make_state(pool.clone())),
        Extension(common::teacher_claims(1, 1)),
        Path((sid, tid, rid)),
    )
    .await
    .unwrap();
    let abandoned: i64 = sqlx::query_scalar(
        "SELECT abandoned FROM applications WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(sid)
    .bind(tid)
    .bind(rid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(abandoned, 1);
}

#[tokio::test]
async fn teacher_abandon_open_round_returns_bad_request() {
    // OPEN 상태에서는 포기 불가
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup(&pool).await;
    insert_application(&pool, sid, tid, rid).await;
    let res = teacher_abandon_application(
        State(common::make_state(pool)),
        Extension(common::teacher_claims(1, 1)),
        Path((sid, tid, rid)),
    )
    .await;
    assert_eq!(res.unwrap_err().0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn teacher_abandon_wrong_class_returns_forbidden() {
    // 자신의 반 학생이 아닌 경우 포기 처리 불가
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup(&pool).await;
    sqlx::query("UPDATE rounds SET status = 'FINALIZED' WHERE id = ?")
        .bind(rid)
        .execute(&pool)
        .await
        .unwrap();
    insert_application(&pool, sid, tid, rid).await;
    let res = teacher_abandon_application(
        State(common::make_state(pool)),
        Extension(common::teacher_claims(2, 2)), // 다른 반 담임
        Path((sid, tid, rid)),
    )
    .await;
    assert_eq!(res.unwrap_err().0, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn teacher_abandon_nonexistent_application_returns_not_found() {
    // 담당 학생이지만 지원 행이 없으면 silent no-op(204)이 아니라 404여야 한다
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup(&pool).await;
    sqlx::query("UPDATE rounds SET status = 'FINALIZED' WHERE id = ?")
        .bind(rid)
        .execute(&pool)
        .await
        .unwrap();
    // 지원 행을 삽입하지 않음
    let res = teacher_abandon_application(
        State(common::make_state(pool)),
        Extension(common::teacher_claims(1, 1)),
        Path((sid, tid, rid)),
    )
    .await;
    assert_eq!(res.unwrap_err().0, StatusCode::NOT_FOUND);
}

// ── univ_id 포함 여부 ─────────────────────────────────────────────

#[tokio::test]
async fn teacher_list_applications_includes_univ_id() {
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup(&pool).await;
    insert_application(&pool, sid, tid, rid).await;
    let Json(rows) = teacher_list_applications(
        State(common::make_state(pool)),
        Extension(common::teacher_claims(1, 1)),
        Query(TeacherAppListQuery { round_id: Some(rid) }),
    )
    .await
    .unwrap();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].univ_id > 0);
}

#[tokio::test]
async fn admin_list_applications_includes_univ_id() {
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup(&pool).await;
    insert_application(&pool, sid, tid, rid).await;
    let Json(rows) = admin_list_applications(
        State(common::make_state(pool)),
        Query(ApplicationListQuery { round_id: Some(rid), track_id: None }),
    )
    .await
    .unwrap();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].univ_id > 0);
}

// ── prev_track_id: 모집단위 변경 ─────────────────────────────────

/// 두 번째 모집단위를 추가해 (sid, tid1, tid2, rid) 반환
async fn setup_two_tracks(pool: &sqlx::SqlitePool) -> (i64, i64, i64, i64) {
    let (sid, tid1, rid) = setup(pool).await;
    let univ_id: i64 = sqlx::query_scalar("SELECT univ_id FROM univ_tracks WHERE id = ?")
        .bind(tid1)
        .fetch_one(pool)
        .await
        .unwrap();
    let tid2: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name, unit_quota) VALUES (?, '전자공', 5) RETURNING id",
    )
    .bind(univ_id)
    .fetch_one(pool)
    .await
    .unwrap();
    (sid, tid1, tid2, rid)
}

#[tokio::test]
async fn prev_track_id_change_replaces_application() {
    // track1 지원 → prev_track_id=track1, track_id=track2로 저장 시
    // track1 지원이 사라지고 track2 지원이 생성된다
    let pool = common::create_test_pool().await;
    let (sid, tid1, tid2, rid) = setup_two_tracks(&pool).await;

    // 초기 지원(track1) 등록
    teacher_create_application(
        State(common::make_state(pool.clone())),
        Extension(common::teacher_claims(1, 1)),
        Json(CreateApplicationBody {
            student_id: sid, track_id: tid1, round_id: rid,
            department_name: "컴퓨터공학과".into(),
            ..Default::default()
        }),
    )
    .await
    .unwrap();

    // track1 → track2 변경
    teacher_create_application(
        State(common::make_state(pool.clone())),
        Extension(common::teacher_claims(1, 1)),
        Json(CreateApplicationBody {
            student_id: sid, track_id: tid2, round_id: rid,
            department_name: "전자공학과".into(),
            prev_track_id: Some(tid1),
            ..Default::default()
        }),
    )
    .await
    .unwrap();

    // track1 지원은 사라져야 함
    let track1_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM applications WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(sid).bind(tid1).bind(rid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(track1_count, 0, "이전 트랙 지원이 남아 있어서는 안 된다");

    // track2 지원이 생성돼야 함
    let dept: String = sqlx::query_scalar(
        "SELECT department_name FROM applications WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(sid).bind(tid2).bind(rid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(dept, "전자공학과");

    // track1 results도 사라져야 함
    let track1_results: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM results WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(sid).bind(tid1).bind(rid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(track1_results, 0, "이전 트랙 results가 남아 있어서는 안 된다");

    // track2 results가 생성돼야 함
    let track2_results: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM results WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(sid).bind(tid2).bind(rid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(track2_results, 1, "새 트랙 results가 생성돼야 한다");
}

#[tokio::test]
async fn prev_track_id_nonexistent_returns_not_found_no_db_change() {
    // prev 지원이 존재하지 않으면 404, DB 무변경
    let pool = common::create_test_pool().await;
    let (sid, tid1, tid2, rid) = setup_two_tracks(&pool).await;

    let res = teacher_create_application(
        State(common::make_state(pool.clone())),
        Extension(common::teacher_claims(1, 1)),
        Json(CreateApplicationBody {
            student_id: sid, track_id: tid2, round_id: rid,
            department_name: "전자공학과".into(),
            prev_track_id: Some(tid1), // track1 지원이 없음
            ..Default::default()
        }),
    )
    .await;
    assert_eq!(res.unwrap_err().0, StatusCode::NOT_FOUND);

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM applications")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "rollback — DB에 아무것도 남아서는 안 된다");
}

#[tokio::test]
async fn prev_track_id_target_already_exists_returns_conflict_no_db_change() {
    // 대상 트랙에 이미 지원이 존재하면 409, 이전 지원이 그대로 남아야 한다
    let pool = common::create_test_pool().await;
    let (sid, tid1, tid2, rid) = setup_two_tracks(&pool).await;

    // track1, track2 둘 다 지원
    teacher_create_application(
        State(common::make_state(pool.clone())),
        Extension(common::teacher_claims(1, 1)),
        Json(CreateApplicationBody {
            student_id: sid, track_id: tid1, round_id: rid,
            department_name: "컴퓨터공학과".into(),
            ..Default::default()
        }),
    )
    .await
    .unwrap();
    teacher_create_application(
        State(common::make_state(pool.clone())),
        Extension(common::teacher_claims(1, 1)),
        Json(CreateApplicationBody {
            student_id: sid, track_id: tid2, round_id: rid,
            department_name: "전자공학과".into(),
            ..Default::default()
        }),
    )
    .await
    .unwrap();

    // track1 → track2 변경 시도 (track2에 이미 지원 있음) → 409
    let res = teacher_create_application(
        State(common::make_state(pool.clone())),
        Extension(common::teacher_claims(1, 1)),
        Json(CreateApplicationBody {
            student_id: sid, track_id: tid2, round_id: rid,
            department_name: "충돌".into(),
            prev_track_id: Some(tid1),
            ..Default::default()
        }),
    )
    .await;
    assert_eq!(res.unwrap_err().0, StatusCode::CONFLICT);

    // rollback — track1 지원이 그대로 남아야 함
    let track1_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM applications WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(sid).bind(tid1).bind(rid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(track1_count, 1, "rollback — 이전 지원이 살아 있어야 한다");

    // track2 지원도 원래대로 ("전자공학과")
    let dept: String = sqlx::query_scalar(
        "SELECT department_name FROM applications WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(sid).bind(tid2).bind(rid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(dept, "전자공학과", "충돌 시 track2 department_name이 변경되어서는 안 된다");
}

#[tokio::test]
async fn prev_track_id_same_as_track_id_upserts_normally() {
    // prev_track_id == track_id면 기존 upsert 동작 (지원 1건 유지, department_name 갱신)
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup(&pool).await;

    teacher_create_application(
        State(common::make_state(pool.clone())),
        Extension(common::teacher_claims(1, 1)),
        Json(CreateApplicationBody {
            student_id: sid, track_id: tid, round_id: rid,
            department_name: "컴퓨터공학과".into(),
            ..Default::default()
        }),
    )
    .await
    .unwrap();

    // 같은 트랙으로 재저장 (prev_track_id == track_id)
    teacher_create_application(
        State(common::make_state(pool.clone())),
        Extension(common::teacher_claims(1, 1)),
        Json(CreateApplicationBody {
            student_id: sid, track_id: tid, round_id: rid,
            department_name: "수정된학과".into(),
            prev_track_id: Some(tid), // 동일 트랙
            ..Default::default()
        }),
    )
    .await
    .unwrap();

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM applications")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "upsert — 지원 행이 1건이어야 한다");

    let dept: String = sqlx::query_scalar("SELECT department_name FROM applications")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(dept, "수정된학과");
}

#[tokio::test]
async fn prev_track_id_change_writes_two_audit_logs() {
    // 트랙 변경 1회에 ApplicationDeleted + ApplicationSaved 정확히 2건
    // 초기 등록(ApplicationSaved 1건) 후 변경 → 총 3건, 뒤에서 2건이 Deleted+Saved
    let pool = common::create_test_pool().await;
    let (sid, tid1, tid2, rid) = setup_two_tracks(&pool).await;

    teacher_create_application(
        State(common::make_state(pool.clone())),
        Extension(common::teacher_claims(1, 1)),
        Json(CreateApplicationBody {
            student_id: sid, track_id: tid1, round_id: rid,
            department_name: "컴퓨터공학과".into(),
            ..Default::default()
        }),
    )
    .await
    .unwrap();

    teacher_create_application(
        State(common::make_state(pool.clone())),
        Extension(common::teacher_claims(1, 1)),
        Json(CreateApplicationBody {
            student_id: sid, track_id: tid2, round_id: rid,
            department_name: "전자공학과".into(),
            prev_track_id: Some(tid1),
            ..Default::default()
        }),
    )
    .await
    .unwrap();

    let logs: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT action, round_id, student_id FROM audit_log ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    // 초기 등록 1 + 트랙 변경 2 = 총 3건
    assert_eq!(logs.len(), 3, "초기 등록 1건 + 트랙 변경 2건 = 3건");
    assert_eq!(logs[0].0, "APPLICATION_SAVED");   // 초기 등록
    assert_eq!(logs[1].0, "APPLICATION_DELETED",  "트랙 변경: 이전 지원 삭제 로그");
    assert_eq!(logs[1].1, rid);
    assert_eq!(logs[1].2, sid);
    assert_eq!(logs[2].0, "APPLICATION_SAVED",    "트랙 변경: 새 지원 등록 로그");
    assert_eq!(logs[2].1, rid);
    assert_eq!(logs[2].2, sid);
}

#[tokio::test]
async fn prev_track_id_change_on_closed_round_returns_bad_request() {
    // 라운드가 OPEN이 아니면 트랙 변경 경로에서도 400
    let pool = common::create_test_pool().await;
    let (sid, tid1, tid2, rid) = setup_two_tracks(&pool).await;

    // idx_one_active_round: CLOSED로 전환
    sqlx::query("UPDATE rounds SET status = 'CLOSED', closed_at = '2025-01-02T00:00:00Z' WHERE id = ?")
        .bind(rid)
        .execute(&pool)
        .await
        .unwrap();

    let res = teacher_create_application(
        State(common::make_state(pool)),
        Extension(common::teacher_claims(1, 1)),
        Json(CreateApplicationBody {
            student_id: sid, track_id: tid2, round_id: rid,
            department_name: "전자공학과".into(),
            prev_track_id: Some(tid1),
            ..Default::default()
        }),
    )
    .await;
    assert_eq!(res.unwrap_err().0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn prev_track_id_change_wrong_class_returns_forbidden() {
    // 담당 학급이 아닌 학생의 트랙 변경은 403
    let pool = common::create_test_pool().await;
    let (sid, tid1, tid2, rid) = setup_two_tracks(&pool).await;

    let res = teacher_create_application(
        State(common::make_state(pool)),
        Extension(common::teacher_claims(2, 2)), // 다른 반
        Json(CreateApplicationBody {
            student_id: sid, track_id: tid2, round_id: rid,
            department_name: "전자공학과".into(),
            prev_track_id: Some(tid1),
            ..Default::default()
        }),
    )
    .await;
    assert_eq!(res.unwrap_err().0, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn prev_track_id_change_grad_teacher_wrong_student_returns_forbidden() {
    // 졸업생 담당(0/0)이 재학생 대상 트랙 변경 시 403
    let pool = common::create_test_pool().await;
    let (enrolled_sid, tid1, tid2, rid) = setup_two_tracks(&pool).await;

    let res = teacher_create_application(
        State(common::make_state(pool)),
        Extension(common::teacher_claims(0, 0)), // 졸업생 담당
        Json(CreateApplicationBody {
            student_id: enrolled_sid, // 재학생
            track_id: tid2, round_id: rid,
            department_name: "전자공학과".into(),
            prev_track_id: Some(tid1),
            ..Default::default()
        }),
    )
    .await;
    assert_eq!(res.unwrap_err().0, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn teacher_abandon_grad_teacher_can_abandon_graduated_student() {
    // 졸업생 담당(grade=0, class_no=0)은 is_enrolled=0 학생만 포기 처리 가능
    let pool = common::create_test_pool().await;
    let (_, tid, _) = setup(&pool).await;

    let grad_sid: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, is_enrolled, grad_year) \
         VALUES ('G001', '졸업생', 0, 2024) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at) VALUES ('FINALIZED', '2025-01-01T00:00:00Z') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    insert_application(&pool, grad_sid, tid, rid).await;

    teacher_abandon_application(
        State(common::make_state(pool.clone())),
        Extension(common::teacher_claims(0, 0)), // 졸업생 담당
        Path((grad_sid, tid, rid)),
    )
    .await
    .unwrap();

    let abandoned: i64 = sqlx::query_scalar(
        "SELECT abandoned FROM applications WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(grad_sid)
    .bind(tid)
    .bind(rid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(abandoned, 1);
}
