mod common;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use principal_candidate_manager::handlers::classes::{delete_class, upsert_class, UpsertClassBody};

// ── upsert_class ──────────────────────────────────────────────────

#[tokio::test]
async fn upsert_new_class_without_password_fails() {
    let pool = common::create_test_pool().await;
    let res = upsert_class(
        State(common::make_state(pool)),
        Path((1i64, 1i64)),
        Json(UpsertClassBody { teacher_name: None, password: None }),
    )
    .await;
    assert_eq!(res.unwrap_err().0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn upsert_new_class_short_password_fails() {
    let pool = common::create_test_pool().await;
    let res = upsert_class(
        State(common::make_state(pool)),
        Path((1i64, 1i64)),
        Json(UpsertClassBody { teacher_name: None, password: Some("abc".into()) }),
    )
    .await;
    assert_eq!(res.unwrap_err().0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn upsert_new_class_with_valid_password_inserts() {
    let pool = common::create_test_pool().await;
    upsert_class(
        State(common::make_state(pool.clone())),
        Path((1i64, 1i64)),
        Json(UpsertClassBody {
            teacher_name: Some("홍길동".into()),
            password: Some("pass1234".into()),
        }),
    )
    .await
    .unwrap();
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM classes WHERE grade = 1 AND class_no = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn upsert_existing_class_updates_teacher_name() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    upsert_class(
        State(common::make_state(pool.clone())),
        Path((1i64, 1i64)),
        Json(UpsertClassBody { teacher_name: Some("새담임".into()), password: None }),
    )
    .await
    .unwrap();
    let name: Option<String> =
        sqlx::query_scalar("SELECT teacher_name FROM classes WHERE grade = 1 AND class_no = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(name.as_deref(), Some("새담임"));
}

#[tokio::test]
async fn upsert_existing_class_without_password_keeps_old_hash() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let old_hash: String =
        sqlx::query_scalar("SELECT password_hash FROM classes WHERE grade = 1 AND class_no = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    upsert_class(
        State(common::make_state(pool.clone())),
        Path((1i64, 1i64)),
        Json(UpsertClassBody { teacher_name: Some("담임".into()), password: None }),
    )
    .await
    .unwrap();
    let new_hash: String =
        sqlx::query_scalar("SELECT password_hash FROM classes WHERE grade = 1 AND class_no = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(old_hash, new_hash);
}

// ── delete_class ──────────────────────────────────────────────────

#[tokio::test]
async fn delete_class_no_students_succeeds() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    delete_class(State(common::make_state(pool.clone())), Path((1i64, 1i64)))
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM classes")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn delete_class_with_students_blocked() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    sqlx::query(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES ('SC001', '홍길동', 1, 1, 1, 1)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let res = delete_class(State(common::make_state(pool)), Path((1i64, 1i64))).await;
    assert_eq!(res.unwrap_err().0, StatusCode::CONFLICT);
}
