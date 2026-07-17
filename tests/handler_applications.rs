mod common;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use principal_candidate_manager::handlers::applications::{
    abandon_application, teacher_abandon_application, teacher_create_application,
    teacher_delete_application, CreateApplicationBody,
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
