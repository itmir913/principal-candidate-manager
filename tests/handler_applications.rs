mod common;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use principal_candidate_manager::handlers::applications::{
    abandon_application, teacher_create_application, teacher_delete_application,
    CreateApplicationBody,
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
        Json(CreateApplicationBody { student_id: sid, track_id: tid, round_id: rid }),
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
async fn create_application_duplicate_is_silently_ignored() {
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup(&pool).await;
    let body = || Json(CreateApplicationBody { student_id: sid, track_id: tid, round_id: rid });
    teacher_create_application(
        State(common::make_state(pool.clone())),
        Extension(common::teacher_claims(1, 1)),
        body(),
    )
    .await
    .unwrap();
    let res = teacher_create_application(
        State(common::make_state(pool.clone())),
        Extension(common::teacher_claims(1, 1)),
        body(),
    )
    .await;
    assert!(res.is_ok());
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM applications")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn create_application_closed_round_returns_bad_request() {
    let pool = common::create_test_pool().await;
    let (sid, tid, _) = setup(&pool).await;
    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at, closed_at) \
         VALUES ('CLOSED', '2025-01-01T00:00:00Z', '2025-01-02T00:00:00Z') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let res = teacher_create_application(
        State(common::make_state(pool)),
        Extension(common::teacher_claims(1, 1)),
        Json(CreateApplicationBody { student_id: sid, track_id: tid, round_id: rid }),
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
        Json(CreateApplicationBody { student_id: sid, track_id: tid, round_id: 9999 }),
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
        Json(CreateApplicationBody { student_id: sid, track_id: tid, round_id: rid }),
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
        "INSERT INTO applications (student_id, track_id, round_id, confirmed, abandoned) \
         VALUES (?, ?, ?, 1, 0)",
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
    let (sid, tid, _) = setup(&pool).await;
    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at, closed_at) \
         VALUES ('CLOSED', '2025-01-01T00:00:00Z', '2025-01-02T00:00:00Z') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO applications (student_id, track_id, round_id, confirmed, abandoned) \
         VALUES (?, ?, ?, 1, 0)",
    )
    .bind(sid)
    .bind(tid)
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
        "INSERT INTO applications (student_id, track_id, round_id, confirmed, abandoned) \
         VALUES (?, ?, ?, 1, 0)",
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

// ── abandon_application ───────────────────────────────────────────

#[tokio::test]
async fn abandon_application_sets_abandoned_flag() {
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup(&pool).await;
    sqlx::query(
        "INSERT INTO applications (student_id, track_id, round_id, confirmed, abandoned) \
         VALUES (?, ?, ?, 1, 0)",
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
