mod common;

use axum::{
    body::Body,
    extract::{FromRequest, Multipart, Path, State},
    http::{Request, StatusCode},
};
use principal_candidate_manager::handlers::area_data::base_data_import;

async fn build_multipart(csv: &str) -> Multipart {
    let boundary = "boundary42";
    let body = format!(
        "--{boundary}\r\n\
         Content-Disposition: form-data; name=\"file\"; filename=\"data.csv\"\r\n\
         Content-Type: text/csv\r\n\r\n\
         {csv}\r\n\
         --{boundary}--\r\n"
    );
    let req = Request::builder()
        .method("POST")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();
    Multipart::from_request(req, &()).await.unwrap()
}

async fn insert_student(pool: &sqlx::SqlitePool, code: &str) -> i64 {
    sqlx::query(
        "INSERT INTO students (student_code, name, is_enrolled, grad_year) \
         VALUES (?, '테스트', 0, 2024)",
    )
    .bind(code)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_rowid()
}

async fn insert_area(
    pool: &sqlx::SqlitePool,
    calc_type: &str,
    match_mode: Option<&str>,
    category_agg: Option<&str>,
    multi_value: i64,
) -> i64 {
    sqlx::query(
        "INSERT INTO areas (name, max_score, calc_type, match_mode, category_agg, lookup_scope, multi_value) \
         VALUES (?, 100000, ?, ?, ?, 'SIMPLE', ?)",
    )
    .bind(format!("Area_{calc_type}_{multi_value}"))
    .bind(calc_type)
    .bind(match_mode)
    .bind(category_agg)
    .bind(multi_value)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_rowid()
}

// ── base_data_import 중복/에러 거부 ──────────────────────────────

#[tokio::test]
async fn numeric_import_dedup_rejects_entire_import() {
    let pool = common::create_test_pool_shared().await;
    insert_student(&pool, "S001").await;
    let aid = insert_area(&pool, "NUMERIC", Some("UPPER"), None, 0).await;
    let state = common::make_state(pool.clone());

    // S001이 두 번 등장 → 전체 import 거부(422), DB에 아무것도 저장되지 않음
    let csv = "학번,값\nS001,30.5\nS001,25.0\n";
    let (status, axum::Json(result)) =
        base_data_import(State(state), Path(aid), build_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(result.rows, 0);
    assert!(!result.errors.is_empty(), "중복 오류가 errors에 포함되어야 함");
    assert!(result.errors[0].contains("S001"));

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM base_data WHERE area_id = ?")
        .bind(aid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "rollback 되어 DB에 행이 없어야 함");
}

#[tokio::test]
async fn manual_import_dedup_rejects_entire_import() {
    let pool = common::create_test_pool_shared().await;
    insert_student(&pool, "S001").await;
    let aid = insert_area(&pool, "MANUAL", None, None, 0).await;
    let state = common::make_state(pool.clone());

    let csv = "학번,값\nS001,85.0\nS001,90.0\n";
    let (status, axum::Json(result)) =
        base_data_import(State(state), Path(aid), build_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(result.rows, 0);
    assert!(!result.errors.is_empty());

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM base_data WHERE area_id = ?")
        .bind(aid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "rollback 되어 DB에 행이 없어야 함");
}

#[tokio::test]
async fn category_multi_import_allows_multiple_values_per_student() {
    let pool = common::create_test_pool_shared().await;
    insert_student(&pool, "S001").await;
    let aid = insert_area(&pool, "CATEGORY", None, Some("SUM"), 1).await;
    let state = common::make_state(pool.clone());

    // CATEGORY multi_value=1: 같은 학생이 서로 다른 범주 → 두 행 모두 삽입
    let csv = "학번,값\nS001,회장\nS001,부회장\n";
    let (status, axum::Json(result)) =
        base_data_import(State(state), Path(aid), build_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(result.rows, 2);
    assert!(result.errors.is_empty());

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM base_data WHERE area_id = ?")
        .bind(aid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 2);
}

#[tokio::test]
async fn numeric_import_multiple_students_succeeds() {
    let pool = common::create_test_pool_shared().await;
    insert_student(&pool, "S001").await;
    insert_student(&pool, "S002").await;
    let aid = insert_area(&pool, "NUMERIC", Some("UPPER"), None, 0).await;
    let state = common::make_state(pool.clone());

    let csv = "학번,값\nS001,30.5\nS002,25.0\n";
    let (status, axum::Json(result)) =
        base_data_import(State(state), Path(aid), build_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(result.rows, 2);
    assert!(result.errors.is_empty());

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM base_data WHERE area_id = ?")
        .bind(aid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 2);
}

#[tokio::test]
async fn import_unknown_student_rejects_entire_import() {
    // 존재하지 않는 학번이 포함된 경우에도 전체 import 거부
    let pool = common::create_test_pool_shared().await;
    insert_student(&pool, "S001").await;
    let aid = insert_area(&pool, "NUMERIC", Some("UPPER"), None, 0).await;
    let state = common::make_state(pool.clone());

    let csv = "학번,값\nS001,30.5\nS999,25.0\n"; // S999 미등록
    let (status, axum::Json(result)) =
        base_data_import(State(state), Path(aid), build_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(result.rows, 0);
    assert!(!result.errors.is_empty());
    assert!(result.errors.iter().any(|e| e.contains("S999")));

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM base_data WHERE area_id = ?")
        .bind(aid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "S001 행도 rollback 되어야 함");
}
