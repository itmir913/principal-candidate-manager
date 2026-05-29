/// 모든 import 핸들러의 "오류 시 전체 거부" 원칙을 검증한다.
/// 오류 행이 하나라도 있으면 HTTP 422 + rollback, 성공 시 HTTP 200 + commit.
mod common;

use axum::{extract::State, http::StatusCode};
use principal_candidate_manager::handlers::{
    classes::import_classes,
    students::{import_enrolled, import_graduated, import_students},
};

// ── import_classes ────────────────────────────────────────────────

#[tokio::test]
async fn import_classes_error_rejects_all() {
    let pool = common::create_test_pool().await;
    let state = common::make_state(pool.clone());

    // 2행: 비밀번호가 4자 미만(="ab") → 오류 → 전체 거부
    let csv = "학년,반,비밀번호\n1,1,pass1234\n1,2,ab\n";
    let (status, axum::Json(result)) =
        import_classes(State(state), common::csv_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(result["inserted"], 0);
    assert!(!result["errors"].as_array().unwrap().is_empty());

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM classes")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "rollback — 1행도 저장되면 안 됨");
}

#[tokio::test]
async fn import_classes_success_commits() {
    let pool = common::create_test_pool().await;
    let state = common::make_state(pool.clone());

    let csv = "학년,반,비밀번호\n1,1,pass1234\n1,2,pass5678\n2,1,pass9012\n";
    let (status, axum::Json(result)) =
        import_classes(State(state), common::csv_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["inserted"], 3);

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM classes")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 3);
}

// ── import_students ───────────────────────────────────────────────

#[tokio::test]
async fn import_students_error_rejects_all() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let state = common::make_state(pool.clone());

    // S001: 정상, S002: 재학생인데 학년 누락 → 오류
    let csv = "학생코드,이름,재학여부,학년,반,번호\n\
               S001,홍길동,1,1,1,1\n\
               S002,이순신,1,,1,2\n";
    let (status, axum::Json(result)) =
        import_students(State(state), common::csv_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(result.inserted, 0);
    assert!(!result.errors.is_empty());

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM students")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "rollback — S001도 저장되면 안 됨");
}

#[tokio::test]
async fn import_students_success_commits() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let state = common::make_state(pool.clone());

    let csv = "학생코드,이름,재학여부,학년,반,번호\n\
               S001,홍길동,1,1,1,1\n\
               S002,이순신,1,1,1,2\n";
    let (status, axum::Json(result)) =
        import_students(State(state), common::csv_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(result.inserted, 2);
    assert!(result.errors.is_empty());
}

// ── import_enrolled ───────────────────────────────────────────────

#[tokio::test]
async fn import_enrolled_error_rejects_all() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let state = common::make_state(pool.clone());

    // 이름 누락 행 포함 → 전체 거부
    let csv = "이름,학년,반,번호\n홍길동,1,1,1\n,1,1,2\n";
    let (status, axum::Json(result)) =
        import_enrolled(State(state), common::csv_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(result.inserted, 0);
    assert!(!result.errors.is_empty());

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM students")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "rollback — 홍길동도 저장되면 안 됨");
}

#[tokio::test]
async fn import_enrolled_success_commits() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let state = common::make_state(pool.clone());

    let csv = "이름,학년,반,번호\n홍길동,1,1,1\n이순신,1,1,2\n";
    let (status, axum::Json(result)) =
        import_enrolled(State(state), common::csv_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(result.inserted, 2);
    assert!(result.errors.is_empty());
}

// ── import_graduated ──────────────────────────────────────────────

#[tokio::test]
async fn import_graduated_error_rejects_all() {
    let pool = common::create_test_pool().await;
    let state = common::make_state(pool.clone());

    // 이름 누락 행 포함 → 전체 거부
    let csv = "학생코드,이름,졸업연도\nS001,홍길동,2023\nS002,,2023\n";
    let (status, axum::Json(result)) =
        import_graduated(State(state), common::csv_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(result.inserted, 0);
    assert!(!result.errors.is_empty());

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM students")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "rollback — S001도 저장되면 안 됨");
}

#[tokio::test]
async fn import_graduated_success_commits() {
    let pool = common::create_test_pool().await;
    let state = common::make_state(pool.clone());

    let csv = "학생코드,이름,졸업연도\nS001,홍길동,2023\nS002,이순신,2023\n";
    let (status, axum::Json(result)) =
        import_graduated(State(state), common::csv_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(result.inserted, 2);
    assert!(result.errors.is_empty());
}
