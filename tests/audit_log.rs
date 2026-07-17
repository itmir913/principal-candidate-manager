mod common;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use principal_candidate_manager::{
    audit::{Actor, AuditEntry},
    enums::AuditAction,
    handlers::{
        applications::{teacher_create_application, CreateApplicationBody},
        rounds::{close_round, open_round},
        scoring::recommend_result,
    },
};

// ── 공통 픽스처 ────────────────────────────────────────────────────

/// area + numeric_table + 학생 + 대학 + 트랙 + 라운드(OPEN) + 지원 + base_data 완비
async fn setup_full(pool: &sqlx::SqlitePool) -> (i64, i64, i64, i64) {
    let hash = bcrypt::hash("pass", 4u32).unwrap();
    sqlx::query(
        "INSERT INTO classes (grade, class_no, teacher_name, password_hash) VALUES (1, 1, '김담임', ?)",
    )
    .bind(&hash)
    .execute(pool)
    .await
    .unwrap();

    let area_id: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, calc_type, max_score, match_mode, lookup_scope) \
         VALUES ('내신', 'NUMERIC', 10000000, 'UPPER', 'SIMPLE') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO numeric_table (area_id, track_id, threshold, score) VALUES (?, NULL, 0, 0)",
    )
    .bind(area_id)
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
        "INSERT INTO univ_tracks (univ_id, track_name) VALUES (?, '컴공') RETURNING id",
    )
    .bind(univ_id)
    .fetch_one(pool)
    .await
    .unwrap();

    let (_, axum::Json(body)) =
        open_round(State(common::make_state(pool.clone()))).await.unwrap();
    let rid = body["id"].as_i64().unwrap();

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

    sqlx::query(
        "INSERT INTO base_data (student_id, area_id, track_id, value, multi_value) \
         VALUES (?, ?, NULL, '0', 0)",
    )
    .bind(sid)
    .bind(area_id)
    .execute(pool)
    .await
    .unwrap();

    (sid, tid, rid, area_id)
}

/// setup_full 후 close_round 성공 → CLOSED + results 행 존재
async fn setup_closed(pool: &sqlx::SqlitePool) -> (i64, i64, i64) {
    let (sid, tid, rid, _) = setup_full(pool).await;
    let _ = close_round(State(common::make_state(pool.clone())), Path(rid))
        .await
        .unwrap();
    (sid, tid, rid)
}

// ── 테스트 1: 불변 트리거 ─────────────────────────────────────────

#[tokio::test]
async fn immutable_trigger_blocks_update() {
    let pool = common::create_test_pool().await;
    sqlx::query(
        "INSERT INTO audit_log (at, actor_type, action, detail) \
         VALUES ('2025-01-01T00:00:00Z', 'ADMIN', 'ROUND_OPENED', '{}')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let res = sqlx::query("UPDATE audit_log SET action = 'ROUND_CLOSED' WHERE id = 1")
        .execute(&pool)
        .await;

    assert!(res.is_err(), "UPDATE는 차단되어야 함");
    let msg = res.unwrap_err().to_string();
    assert!(
        msg.contains("immutable"),
        "오류 메시지에 'immutable' 포함 필요: {msg}"
    );
}

#[tokio::test]
async fn immutable_trigger_blocks_delete() {
    let pool = common::create_test_pool().await;
    sqlx::query(
        "INSERT INTO audit_log (at, actor_type, action, detail) \
         VALUES ('2025-01-01T00:00:00Z', 'ADMIN', 'ROUND_OPENED', '{}')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let res = sqlx::query("DELETE FROM audit_log WHERE id = 1")
        .execute(&pool)
        .await;

    assert!(res.is_err(), "DELETE는 차단되어야 함");
    let msg = res.unwrap_err().to_string();
    assert!(
        msg.contains("immutable"),
        "오류 메시지에 'immutable' 포함 필요: {msg}"
    );
}

// ── 테스트 2: Admin 기록 ──────────────────────────────────────────

#[tokio::test]
async fn audit_log_admin_actor_name_is_null() {
    let pool = common::create_test_pool().await;
    let mut conn = pool.acquire().await.unwrap();

    principal_candidate_manager::audit::log(
        &mut *conn,
        AuditEntry {
            actor: Actor::Admin,
            action: AuditAction::RoundOpened,
            round_id: Some(1),
            student_id: None,
            detail: serde_json::json!({}),
        },
    )
    .await
    .unwrap();
    drop(conn);

    let (actor_type, actor_name): (String, Option<String>) = sqlx::query_as(
        "SELECT actor_type, actor_name FROM audit_log WHERE id = 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(actor_type, "ADMIN");
    assert!(actor_name.is_none(), "ADMIN의 actor_name은 NULL이어야 함");
}

// ── 테스트 3: Teacher 기록 + 스냅샷 불변 ────────────────────────────

#[tokio::test]
async fn audit_log_teacher_snapshots_name_and_stays_immutable() {
    let pool = common::create_test_pool().await;
    let hash = bcrypt::hash("pass", 4u32).unwrap();
    sqlx::query(
        "INSERT INTO classes (grade, class_no, teacher_name, password_hash) VALUES (1, 1, '김담임', ?)",
    )
    .bind(&hash)
    .execute(&pool)
    .await
    .unwrap();

    // Teacher 로그 삽입
    let mut conn = pool.acquire().await.unwrap();
    principal_candidate_manager::audit::log(
        &mut *conn,
        AuditEntry {
            actor: Actor::Teacher { grade: 1, class_no: 1 },
            action: AuditAction::ApplicationSaved,
            round_id: None,
            student_id: None,
            detail: serde_json::json!({}),
        },
    )
    .await
    .unwrap();
    drop(conn);

    // 담임명 변경
    sqlx::query("UPDATE classes SET teacher_name = '이담임' WHERE grade = 1 AND class_no = 1")
        .execute(&pool)
        .await
        .unwrap();

    // 로그는 원래 이름 유지
    let actor_name: Option<String> =
        sqlx::query_scalar("SELECT actor_name FROM audit_log WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(actor_name.as_deref(), Some("김담임"), "로그는 기록 시점 이름을 유지해야 함");
}

// ── 테스트 4: 존재하지 않는 학급 Teacher → Err ────────────────────

#[tokio::test]
async fn audit_log_nonexistent_class_returns_err() {
    let pool = common::create_test_pool().await;
    let mut conn = pool.acquire().await.unwrap();

    let res = principal_candidate_manager::audit::log(
        &mut *conn,
        AuditEntry {
            actor: Actor::Teacher { grade: 99, class_no: 99 },
            action: AuditAction::ApplicationSaved,
            round_id: None,
            student_id: None,
            detail: serde_json::json!({}),
        },
    )
    .await;

    assert!(res.is_err(), "존재하지 않는 학급 → Err 반환 필요");
    let (status, _) = res.unwrap_err();
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

// ── 테스트 5: CHECK 제약 — TEACHER + grade NULL 직접 INSERT 거부 ─────

#[tokio::test]
async fn check_constraint_rejects_teacher_without_grade() {
    let pool = common::create_test_pool().await;
    let res = sqlx::query(
        "INSERT INTO audit_log \
         (at, actor_type, actor_grade, actor_class_no, action, detail) \
         VALUES ('2025-01-01T00:00:00Z', 'TEACHER', NULL, 1, 'APPLICATION_SAVED', '{}')",
    )
    .execute(&pool)
    .await;

    assert!(res.is_err(), "TEACHER + grade NULL은 CHECK 제약으로 거부되어야 함");
}

// ── 테스트 6: close_round 성공 → 로그 1건 / 실패 → 로그 0건 ─────────

#[tokio::test]
async fn close_round_success_writes_audit_log() {
    let pool = common::create_test_pool().await;
    let (_, _, rid) = setup_closed(&pool).await;

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE action = 'ROUND_CLOSED' AND round_id = ?",
    )
    .bind(rid)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(count, 1, "close_round 성공 후 ROUND_CLOSED 로그가 1건이어야 함");

    let round_id_in_log: Option<i64> =
        sqlx::query_scalar("SELECT round_id FROM audit_log WHERE action = 'ROUND_CLOSED'")
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(round_id_in_log, Some(rid));
}

#[tokio::test]
async fn close_round_failure_writes_no_audit_log() {
    let pool = common::create_test_pool().await;

    // 기초데이터 누락 상태 — area + 학생 + 지원은 있지만 base_data 없음
    let hash = bcrypt::hash("pass", 4u32).unwrap();
    sqlx::query(
        "INSERT INTO classes (grade, class_no, password_hash) VALUES (1, 1, ?)",
    )
    .bind(&hash)
    .execute(&pool)
    .await
    .unwrap();

    let area_id: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, calc_type, max_score, match_mode, lookup_scope) \
         VALUES ('내신', 'NUMERIC', 10000000, 'UPPER', 'SIMPLE') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO numeric_table (area_id, track_id, threshold, score) VALUES (?, NULL, 0, 0)",
    )
    .bind(area_id)
    .execute(&pool)
    .await
    .unwrap();

    let sid: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES ('S001', '홍길동', 1, 1, 1, 1) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let univ_id: i64 =
        sqlx::query_scalar("INSERT INTO universities (univ_name) VALUES ('한국대') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();

    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name) VALUES (?, '컴공') RETURNING id",
    )
    .bind(univ_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let (_, axum::Json(body)) =
        open_round(State(common::make_state(pool.clone()))).await.unwrap();
    let rid = body["id"].as_i64().unwrap();

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

    // base_data 없이 close → 422
    let res = close_round(State(common::make_state(pool.clone())), Path(rid)).await;
    assert_eq!(res.unwrap_err().0, StatusCode::UNPROCESSABLE_ENTITY);

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_log")
        .fetch_one(&pool)
        .await
        .unwrap();

    // open_round 로그 1건만 있어야 함 (close 실패 로그 없음)
    assert_eq!(count, 1, "close 실패 시 ROUND_CLOSED 로그가 추가되지 않아야 함");
    let action: String = sqlx::query_scalar("SELECT action FROM audit_log WHERE id = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(action, "ROUND_OPENED");
}

// ── 테스트 7: recommend_result 성공 → RecommendConfirmed 1건 ─────────

#[tokio::test]
async fn recommend_result_writes_audit_log_with_detail() {
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup_closed(&pool).await;

    recommend_result(
        State(common::make_state(pool.clone())),
        Path((sid, tid, rid)),
    )
    .await
    .unwrap();

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE action = 'RECOMMEND_CONFIRMED'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(count, 1, "RECOMMEND_CONFIRMED 로그가 1건이어야 함");

    let detail_str: String =
        sqlx::query_scalar("SELECT detail FROM audit_log WHERE action = 'RECOMMEND_CONFIRMED'")
            .fetch_one(&pool)
            .await
            .unwrap();

    let detail: serde_json::Value = serde_json::from_str(&detail_str).unwrap();
    assert!(
        detail.get("student_code").is_some(),
        "detail에 student_code 필요"
    );
    assert!(
        detail.get("univ_name").is_some(),
        "detail에 univ_name 필요"
    );
}

// ── 테스트 8: teacher_create_application 성공 → ApplicationSaved + 담임 필드 ───

#[tokio::test]
async fn teacher_create_application_writes_audit_log_with_teacher_actor() {
    let pool = common::create_test_pool().await;

    let hash = bcrypt::hash("pass", 4u32).unwrap();
    sqlx::query(
        "INSERT INTO classes (grade, class_no, teacher_name, password_hash) VALUES (1, 1, '김담임', ?)",
    )
    .bind(&hash)
    .execute(&pool)
    .await
    .unwrap();

    let sid: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES ('S001', '홍길동', 1, 1, 1, 1) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let univ_id: i64 =
        sqlx::query_scalar("INSERT INTO universities (univ_name) VALUES ('한국대') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();

    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name) VALUES (?, '컴공') RETURNING id",
    )
    .bind(univ_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at) VALUES ('OPEN', '2025-01-01T00:00:00Z') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    teacher_create_application(
        State(common::make_state(pool.clone())),
        Extension(common::teacher_claims(1, 1)),
        Json(CreateApplicationBody {
            student_id: sid,
            track_id: tid,
            round_id: rid,
            department_name: "컴퓨터공학과".into(),
            base_data_entries: vec![],
        }),
    )
    .await
    .unwrap();

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM audit_log WHERE action = 'APPLICATION_SAVED'")
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(count, 1, "APPLICATION_SAVED 로그가 1건이어야 함");

    #[derive(sqlx::FromRow)]
    struct LogRow {
        actor_type: String,
        actor_grade: Option<i64>,
        actor_class_no: Option<i64>,
        actor_name: Option<String>,
    }

    let row: LogRow =
        sqlx::query_as("SELECT actor_type, actor_grade, actor_class_no, actor_name FROM audit_log WHERE action = 'APPLICATION_SAVED'")
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(row.actor_type, "TEACHER");
    assert_eq!(row.actor_grade, Some(1));
    assert_eq!(row.actor_class_no, Some(1));
    assert_eq!(row.actor_name.as_deref(), Some("김담임"));
}
