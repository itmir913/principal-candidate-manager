mod common;

use axum::{
    body::Body,
    extract::{FromRequest, Multipart, Path, State},
    http::{Request, StatusCode},
};
use principal_candidate_manager::enums::{CalcType, CategoryAgg, MatchMode};
use principal_candidate_manager::handlers::area_data::{
    base_data_import, base_data_list, category_map_import, numeric_table_import,
};

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
    calc_type: CalcType,
    match_mode: Option<MatchMode>,
    category_agg: Option<CategoryAgg>,
    multi_value: i64,
) -> i64 {
    sqlx::query(
        "INSERT INTO areas (name, max_score, calc_type, match_mode, category_agg, lookup_scope, multi_value) \
         VALUES (?, 10000000, ?, ?, ?, 'SIMPLE', ?)",
    )
    .bind(format!("{:?}_{multi_value}", calc_type))
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
    let aid = insert_area(&pool, CalcType::Numeric, Some(MatchMode::Upper), None, 0).await;
    let state = common::make_state(pool.clone());

    // S001이 두 번 등장 → 전체 import 거부(422), DB에 아무것도 저장되지 않음
    let csv = "학생코드,값\nS001,30.5\nS001,25.0\n";
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
    let aid = insert_area(&pool, CalcType::Manual, None, None, 0).await;
    let state = common::make_state(pool.clone());

    let csv = "학생코드,값\nS001,85.0\nS001,90.0\n";
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
    let aid = insert_area(&pool, CalcType::Category, None, Some(CategoryAgg::Sum), 1).await;
    let state = common::make_state(pool.clone());

    // CATEGORY multi_value=1: 같은 학생이 서로 다른 범주 → 두 행 모두 삽입
    let csv = "학생코드,값\nS001,회장\nS001,부회장\n";
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
    let aid = insert_area(&pool, CalcType::Numeric, Some(MatchMode::Upper), None, 0).await;
    let state = common::make_state(pool.clone());

    let csv = "학생코드,값\nS001,30.5\nS002,25.0\n";
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
    // 존재하지 않는 학생코드이 포함된 경우에도 전체 import 거부
    let pool = common::create_test_pool_shared().await;
    insert_student(&pool, "S001").await;
    let aid = insert_area(&pool, CalcType::Numeric, Some(MatchMode::Upper), None, 0).await;
    let state = common::make_state(pool.clone());

    let csv = "학생코드,값\nS001,30.5\nS999,25.0\n"; // S999 미등록
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

// ── 음수 점수 허용 (감점 전형요소 지원) ──────────────────────────
// 특정 범주·구간에 해당하는 학생에게 감점을 부여하는 전형요소에서
// 음수 점수가 유효하다. parse_display_value는 음수를 허용해야 한다.

#[tokio::test]
async fn numeric_base_data_import_negative_value_allowed() {
    // NUMERIC base_data 음수 측정값 → 정상 저장 (감점 구간 탐색에 사용)
    let pool = common::create_test_pool_shared().await;
    insert_student(&pool, "S001").await;
    let aid = insert_area(&pool, CalcType::Numeric, Some(MatchMode::Upper), None, 0).await;

    let csv = "학생코드,값\nS001,-1.0\n";
    let (status, axum::Json(result)) =
        base_data_import(State(common::make_state(pool.clone())), Path(aid), build_multipart(csv).await)
            .await.unwrap();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(result.rows, 1);

    let value: String = sqlx::query_scalar("SELECT value FROM base_data WHERE area_id = ?")
        .bind(aid).fetch_one(&pool).await.unwrap();
    assert_eq!(value, "-100000", "-1.0 × 100000 = -100000");
}

#[tokio::test]
async fn manual_base_data_import_negative_value_allowed() {
    // MANUAL base_data 음수 점수 → 정상 저장 (감점 직접 입력)
    let pool = common::create_test_pool_shared().await;
    insert_student(&pool, "S001").await;
    let aid = insert_area(&pool, CalcType::Manual, None, None, 0).await;

    let csv = "학생코드,값\nS001,-5.0\n";
    let (status, axum::Json(result)) =
        base_data_import(State(common::make_state(pool.clone())), Path(aid), build_multipart(csv).await)
            .await.unwrap();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(result.rows, 1);

    let value: String = sqlx::query_scalar("SELECT value FROM base_data WHERE area_id = ?")
        .bind(aid).fetch_one(&pool).await.unwrap();
    assert_eq!(value, "-500000", "-5.0 × 100000 = -500000");
}

#[tokio::test]
async fn numeric_table_import_negative_threshold_allowed() {
    // numeric_table 음수 기준값 → 정상 저장
    let pool = common::create_test_pool_shared().await;
    let aid = insert_area(&pool, CalcType::Numeric, Some(MatchMode::Upper), None, 0).await;

    let csv = "기준값,점수\n-1.0,50.0\n";
    let (status, axum::Json(result)) =
        numeric_table_import(State(common::make_state(pool.clone())), Path(aid), build_multipart(csv).await)
            .await.unwrap();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(result.rows, 1);

    let threshold: i64 = sqlx::query_scalar("SELECT threshold FROM numeric_table WHERE area_id = ?")
        .bind(aid).fetch_one(&pool).await.unwrap();
    assert_eq!(threshold, -100000);
}

#[tokio::test]
async fn numeric_table_import_negative_score_allowed() {
    // numeric_table 음수 점수 → 정상 저장 (감점 구간표)
    let pool = common::create_test_pool_shared().await;
    let aid = insert_area(&pool, CalcType::Numeric, Some(MatchMode::Upper), None, 0).await;

    let csv = "기준값,점수\n1.0,-10.0\n";
    let (status, axum::Json(result)) =
        numeric_table_import(State(common::make_state(pool.clone())), Path(aid), build_multipart(csv).await)
            .await.unwrap();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(result.rows, 1);

    let score: i64 = sqlx::query_scalar("SELECT score FROM numeric_table WHERE area_id = ?")
        .bind(aid).fetch_one(&pool).await.unwrap();
    assert_eq!(score, -1_000_000, "-10.0 × 100000 = -1000000");
}

#[tokio::test]
async fn numeric_table_import_six_decimal_places_rejected() {
    // numeric_table 소수 6자리 → 전체 거부
    let pool = common::create_test_pool_shared().await;
    let aid = insert_area(&pool, CalcType::Numeric, Some(MatchMode::Upper), None, 0).await;

    let csv = "기준값,점수\n1.123456,50.0\n";
    let (status, axum::Json(result)) =
        numeric_table_import(State(common::make_state(pool.clone())), Path(aid), build_multipart(csv).await)
            .await.unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(result.rows, 0);
    assert!(result.errors.iter().any(|e| e.contains("5자리")));
}

#[tokio::test]
async fn category_map_import_negative_score_allowed() {
    // category_map 음수 점수 → 정상 저장 (범주 감점)
    let pool = common::create_test_pool_shared().await;
    let aid = insert_area(&pool, CalcType::Category, None, Some(CategoryAgg::Sum), 1).await;

    let csv = "범주,점수\n규정위반,-3.0\n";
    let (status, axum::Json(result)) =
        category_map_import(State(common::make_state(pool.clone())), Path(aid), build_multipart(csv).await)
            .await.unwrap();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(result.rows, 1);

    let score: i64 = sqlx::query_scalar("SELECT score FROM category_map WHERE area_id = ?")
        .bind(aid).fetch_one(&pool).await.unwrap();
    assert_eq!(score, -300_000, "-3.0 × 100000 = -300000");
}

#[tokio::test]
async fn category_map_import_six_decimal_places_rejected() {
    // category_map 소수 6자리 점수 → 전체 거부
    let pool = common::create_test_pool_shared().await;
    let aid = insert_area(&pool, CalcType::Category, None, Some(CategoryAgg::Sum), 1).await;

    let csv = "범주,점수\n회장,10.123456\n";
    let (status, axum::Json(result)) =
        category_map_import(State(common::make_state(pool.clone())), Path(aid), build_multipart(csv).await)
            .await.unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(result.rows, 0);
    assert!(result.errors.iter().any(|e| e.contains("5자리")));
}

#[tokio::test]
async fn numeric_table_import_valid_five_decimal_places_succeeds() {
    // 소수 5자리는 정상 처리되어야 함
    let pool = common::create_test_pool_shared().await;
    let aid = insert_area(&pool, CalcType::Numeric, Some(MatchMode::Upper), None, 0).await;

    let csv = "기준값,점수\n1.12345,50.00001\n";
    let (status, axum::Json(result)) =
        numeric_table_import(State(common::make_state(pool.clone())), Path(aid), build_multipart(csv).await)
            .await.unwrap();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(result.rows, 1);
    assert!(result.errors.is_empty());

    let threshold: i64 = sqlx::query_scalar("SELECT threshold FROM numeric_table WHERE area_id = ?")
        .bind(aid).fetch_one(&pool).await.unwrap();
    assert_eq!(threshold, 112345, "1.12345 × 100000 = 112345");
}

#[tokio::test]
async fn numeric_table_import_zero_threshold_and_score_succeeds() {
    // 0은 유효한 값 (음수 거부 기준이 엄격히 < 0 임을 검증)
    let pool = common::create_test_pool_shared().await;
    let aid = insert_area(&pool, CalcType::Numeric, Some(MatchMode::Upper), None, 0).await;

    let csv = "기준값,점수\n0,0\n1.0,50.0\n";
    let (status, axum::Json(result)) =
        numeric_table_import(State(common::make_state(pool.clone())), Path(aid), build_multipart(csv).await)
            .await.unwrap();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(result.rows, 2);
}

// ── base_data_list: Numeric/Manual 파싱 Fail-Fast ────────────────

#[tokio::test]
async fn base_data_list_numeric_corrupt_value_returns_500() {
    // Numeric 전형요소의 base_data 값이 정수로 파싱 불가 → 500 반환 (silent fallback 금지)
    let pool = common::create_test_pool().await;
    let sid = insert_student(&pool, "S001").await;
    let aid = insert_area(&pool, CalcType::Numeric, Some(MatchMode::Upper), None, 0).await;

    sqlx::query(
        "INSERT INTO base_data (student_id, area_id, track_id, value) VALUES (?, ?, NULL, 'not_a_number')",
    )
    .bind(sid)
    .bind(aid)
    .execute(&pool)
    .await
    .unwrap();

    let err = base_data_list(State(common::make_state(pool)), Path(aid))
        .await
        .unwrap_err();
    assert_eq!(err.0, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(err.1.contains("파싱"), "오류 메시지에 '파싱' 포함 기대: {}", err.1);
}

#[tokio::test]
async fn base_data_list_manual_corrupt_value_returns_500() {
    // Manual 전형요소의 base_data 값이 정수로 파싱 불가 → 500 반환
    let pool = common::create_test_pool().await;
    let sid = insert_student(&pool, "S001").await;
    let aid = insert_area(&pool, CalcType::Manual, None, None, 0).await;

    sqlx::query(
        "INSERT INTO base_data (student_id, area_id, track_id, value) VALUES (?, ?, NULL, '3.14abc')",
    )
    .bind(sid)
    .bind(aid)
    .execute(&pool)
    .await
    .unwrap();

    let err = base_data_list(State(common::make_state(pool)), Path(aid))
        .await
        .unwrap_err();
    assert_eq!(err.0, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn base_data_list_category_non_numeric_value_is_ok() {
    // Category 전형요소는 문자열 값 그대로 반환 — 파싱 불필요
    let pool = common::create_test_pool().await;
    let sid = insert_student(&pool, "S001").await;
    let aid = insert_area(&pool, CalcType::Category, None, Some(CategoryAgg::Sum), 1).await;

    sqlx::query(
        "INSERT INTO base_data (student_id, area_id, track_id, value, multi_value) VALUES (?, ?, NULL, '회장', 1)",
    )
    .bind(sid)
    .bind(aid)
    .execute(&pool)
    .await
    .unwrap();

    let axum::Json(rows) = base_data_list(State(common::make_state(pool)), Path(aid))
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].value, "회장");
}
