mod common;

use axum::{
    body::Body,
    extract::{FromRequest, Multipart, Path, Query, State},
    http::{Request, StatusCode},
};
use principal_candidate_manager::enums::{CalcType, CategoryAgg, MatchMode};
use principal_candidate_manager::handlers::area_data::{
    base_data_import, base_data_list, category_map_import, numeric_table_import,
    BaseDataPageQuery, StudentTypeQuery,
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

fn graduated_query() -> Query<StudentTypeQuery> {
    Query(StudentTypeQuery { student_type: "graduated".to_string() })
}

fn default_page_query() -> Query<BaseDataPageQuery> {
    // insert_student은 is_enrolled=0(졸업생)을 삽입하므로 student_type은 "graduated"
    Query(BaseDataPageQuery { page: 1, per_page: 50, student_type: "graduated".to_string() })
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
    let csv = "학생코드,이름,값\nS001,테스트,30.5\nS001,테스트,25.0\n";
    let (status, axum::Json(result)) =
        base_data_import(State(state), Path(aid), graduated_query(), build_multipart(csv).await)
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

    let csv = "학생코드,이름,값\nS001,테스트,85.0\nS001,테스트,90.0\n";
    let (status, axum::Json(result)) =
        base_data_import(State(state), Path(aid), graduated_query(), build_multipart(csv).await)
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
async fn manual_import_exceeds_max_score_rejects_entire_import() {
    let pool = common::create_test_pool_shared().await;
    insert_student(&pool, "S001").await;
    insert_student(&pool, "S002").await;
    // max_score = 10000000 (100점 × 100000)
    let aid = insert_area(&pool, CalcType::Manual, None, None, 0).await;
    let state = common::make_state(pool.clone());

    // S002의 값이 만점(100) 초과 → 전체 import 거부
    let csv = "학생코드,이름,값\nS001,테스트,85\nS002,테스트,101\n";
    let (status, axum::Json(result)) =
        base_data_import(State(state), Path(aid), graduated_query(), build_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(result.rows, 0);
    assert!(!result.errors.is_empty(), "만점 초과 오류가 errors에 포함되어야 함");
    assert!(result.errors[0].contains("만점"), "오류 메시지에 '만점' 포함");

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM base_data WHERE area_id = ?")
        .bind(aid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "rollback 되어 S001 행도 저장되지 않아야 함");
}

#[tokio::test]
async fn manual_import_at_max_score_is_accepted() {
    let pool = common::create_test_pool_shared().await;
    insert_student(&pool, "S001").await;
    let aid = insert_area(&pool, CalcType::Manual, None, None, 0).await;
    let state = common::make_state(pool.clone());

    // 정확히 만점(100) — 허용
    let csv = "학생코드,이름,값\nS001,테스트,100\n";
    let (status, _) =
        base_data_import(State(state), Path(aid), graduated_query(), build_multipart(csv).await)
            .await
            .unwrap();
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn category_multi_import_allows_multiple_values_per_student() {
    let pool = common::create_test_pool_shared().await;
    insert_student(&pool, "S001").await;
    let aid = insert_area(&pool, CalcType::Category, None, Some(CategoryAgg::Sum), 1).await;
    let state = common::make_state(pool.clone());

    // CATEGORY multi_value=1: 같은 학생이 서로 다른 범주 → 두 행 모두 삽입
    let csv = "학생코드,이름,값\nS001,테스트,회장\nS001,테스트,부회장\n";
    let (status, axum::Json(result)) =
        base_data_import(State(state), Path(aid), graduated_query(), build_multipart(csv).await)
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

    let csv = "학생코드,이름,값\nS001,테스트,30.5\nS002,테스트,25.0\n";
    let (status, axum::Json(result)) =
        base_data_import(State(state), Path(aid), graduated_query(), build_multipart(csv).await)
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

    let csv = "학생코드,이름,값\nS001,테스트,30.5\nS999,테스트,25.0\n"; // S999 미등록
    let (status, axum::Json(result)) =
        base_data_import(State(state), Path(aid), graduated_query(), build_multipart(csv).await)
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

    let csv = "학생코드,이름,값\nS001,테스트,-1.0\n";
    let (status, axum::Json(result)) =
        base_data_import(State(common::make_state(pool.clone())), Path(aid), graduated_query(), build_multipart(csv).await)
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

    let csv = "학생코드,이름,값\nS001,테스트,-5.0\n";
    let (status, axum::Json(result)) =
        base_data_import(State(common::make_state(pool.clone())), Path(aid), graduated_query(), build_multipart(csv).await)
            .await.unwrap();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(result.rows, 1);

    let value: String = sqlx::query_scalar("SELECT value FROM base_data WHERE area_id = ?")
        .bind(aid).fetch_one(&pool).await.unwrap();
    assert_eq!(value, "-500000", "-5.0 × 100000 = -500000");
}

// ── 비유한·초과 크기 값 거부 (Fail-Fast) ─────────────────────────
// Rust f64 파서는 "nan"/"inf"를 허용하고 `as i64` 캐스트는 NaN→0,
// ±∞→i64::MAX로 포화시킨다. 과거에는 이 값들이 오류 없이 저장되어
// NUMERIC UPPER 매칭에서 최상위 점수를 조용히 획득할 수 있었다.

#[tokio::test]
async fn base_data_import_nan_value_rejected() {
    let pool = common::create_test_pool_shared().await;
    insert_student(&pool, "S001").await;
    let aid = insert_area(&pool, CalcType::Numeric, Some(MatchMode::Upper), None, 0).await;

    let csv = "학생코드,이름,값\nS001,테스트,nan\n";
    let (status, axum::Json(result)) =
        base_data_import(State(common::make_state(pool.clone())), Path(aid), graduated_query(), build_multipart(csv).await)
            .await.unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "'nan'이 0으로 저장되면 안 됨");
    assert_eq!(result.rows, 0);
    assert!(!result.errors.is_empty());

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM base_data WHERE area_id = ?")
        .bind(aid).fetch_one(&pool).await.unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn base_data_import_infinity_value_rejected() {
    let pool = common::create_test_pool_shared().await;
    insert_student(&pool, "S001").await;
    let aid = insert_area(&pool, CalcType::Numeric, Some(MatchMode::Upper), None, 0).await;

    let csv = "학생코드,이름,값\nS001,테스트,inf\n";
    let (status, axum::Json(result)) =
        base_data_import(State(common::make_state(pool.clone())), Path(aid), graduated_query(), build_multipart(csv).await)
            .await.unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "'inf'가 i64::MAX로 포화되면 안 됨");
    assert_eq!(result.rows, 0);
}

#[tokio::test]
async fn numeric_table_import_huge_threshold_rejected() {
    // 1e300은 ×100000 시 i64::MAX로 포화 — 기준값 오염 방지
    let pool = common::create_test_pool_shared().await;
    let aid = insert_area(&pool, CalcType::Numeric, Some(MatchMode::Upper), None, 0).await;

    let csv = "기준값,점수\n1e300,50.0\n";
    let (status, axum::Json(result)) =
        numeric_table_import(State(common::make_state(pool.clone())), Path(aid), build_multipart(csv).await)
            .await.unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(result.rows, 0);
    assert!(result.errors.iter().any(|e| e.contains("초과") || e.contains("유한")));
}

#[tokio::test]
async fn base_data_import_magnitude_boundary() {
    // 경계값: ±10억까지 허용, 초과 시 거부 (f64 정밀도 보장 한계)
    let pool = common::create_test_pool_shared().await;
    insert_student(&pool, "S001").await;
    insert_student(&pool, "S002").await;
    let aid = insert_area(&pool, CalcType::Numeric, Some(MatchMode::Upper), None, 0).await;
    let state = common::make_state(pool.clone());

    let csv = "학생코드,이름,값\nS001,테스트,1000000000\n";
    let (status, _) =
        base_data_import(State(state.clone()), Path(aid), graduated_query(), build_multipart(csv).await)
            .await.unwrap();
    assert_eq!(status, StatusCode::OK, "10억(경계값)은 허용");

    let csv = "학생코드,이름,값\nS002,테스트,1000000001\n";
    let (status, axum::Json(result)) =
        base_data_import(State(state), Path(aid), graduated_query(), build_multipart(csv).await)
            .await.unwrap();
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "10억 초과는 거부");
    assert!(result.errors.iter().any(|e| e.contains("초과")));
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

    let err = base_data_list(State(common::make_state(pool)), Path(aid), default_page_query())
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

    let err = base_data_list(State(common::make_state(pool)), Path(aid), default_page_query())
        .await
        .unwrap_err();
    assert_eq!(err.0, StatusCode::INTERNAL_SERVER_ERROR);
}

// ── resolve_track tx 롤백 — import 실패 시 자동 생성 대학/트랙 제거 ──

async fn insert_area_composite(pool: &sqlx::SqlitePool) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, match_mode, lookup_scope, multi_value) \
         VALUES ('외부점수', 10000000, 'NUMERIC', 'UPPER', 'COMPOSITE', 0) RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn numeric_table_import_failure_rolls_back_auto_created_track() {
    // COMPOSITE 전형요소에 신규 대학/트랙을 자동 생성하면서 import.
    // 오류가 포함된 파일이면 tx 전체가 롤백되어 자동 생성된 대학/트랙도 사라져야 한다.
    let pool = common::create_test_pool_shared().await;
    let aid = insert_area_composite(&pool).await;
    let state = common::make_state(pool.clone());

    // "점수" 컬럼에 잘못된 값("abc")을 포함해 오류를 유발
    let csv = "기준값,점수,대학명,모집단위명\n100,abc,신규대학,신규학과";
    let mp = build_multipart(csv).await;
    let res = numeric_table_import(State(state), Path(aid), mp).await.unwrap();
    assert_eq!(res.0, StatusCode::UNPROCESSABLE_ENTITY);

    // import가 실패했으므로 자동 생성된 대학/트랙 행이 남아 있으면 안 됨
    let univ_cnt: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM universities WHERE univ_name = '신규대학'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let track_cnt: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM univ_tracks WHERE track_name = '신규학과'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(univ_cnt, 0, "import 실패 시 자동 생성된 대학이 롤백되어야 함");
    assert_eq!(track_cnt, 0, "import 실패 시 자동 생성된 트랙이 롤백되어야 함");
}

#[tokio::test]
async fn category_map_import_failure_rolls_back_auto_created_track() {
    let pool = common::create_test_pool_shared().await;
    let aid: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, category_agg, lookup_scope, multi_value) \
         VALUES ('봉사', 10000000, 'CATEGORY', 'SUM', 'COMPOSITE', 0) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let state = common::make_state(pool.clone());

    // 점수가 만점 초과 → 오류 발생
    let max_exceeded = 99999999999i64;
    let csv = format!("범주,점수,대학명,모집단위명\n회장,{max_exceeded},신규대학,신규학과");
    let mp = build_multipart(&csv).await;
    let res = category_map_import(State(state), Path(aid), mp).await.unwrap();
    assert_eq!(res.0, StatusCode::UNPROCESSABLE_ENTITY);

    let univ_cnt: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM universities WHERE univ_name = '신규대학'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(univ_cnt, 0, "import 실패 시 자동 생성된 대학이 롤백되어야 함");
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

    let axum::Json(page) = base_data_list(State(common::make_state(pool)), Path(aid), default_page_query())
        .await
        .unwrap();
    assert_eq!(page.rows.len(), 1);
    assert_eq!(page.rows[0].value, "회장");
}

// ── 누락된 열 → 400 ────────────────────────────────────────────────

#[tokio::test]
async fn numeric_table_import_missing_required_column_returns_bad_request() {
    // "기준값" 열 없음 → require_cols 실패 → Err(400) 반환
    let pool = common::create_test_pool_shared().await;
    let aid = insert_area(&pool, CalcType::Numeric, Some(MatchMode::Upper), None, 0).await;
    let state = common::make_state(pool.clone());

    let csv = "점수\n50.0\n"; // "기준값" 열 없음
    // require_cols 실패 시 Err((400, ...))을 반환하므로 match 사용
    match numeric_table_import(State(state), Path(aid), build_multipart(csv).await).await {
        Ok(_) => panic!("400 BAD_REQUEST가 예상됨"),
        Err((status, _)) => assert_eq!(status, StatusCode::BAD_REQUEST),
    }
}

#[tokio::test]
async fn category_map_import_missing_required_column_returns_bad_request() {
    // "범주" 열 없음 → require_cols 실패 → Err(400) 반환
    let pool = common::create_test_pool_shared().await;
    let aid = insert_area(&pool, CalcType::Category, None, Some(CategoryAgg::Sum), 1).await;
    let state = common::make_state(pool.clone());

    let csv = "점수\n10.0\n"; // "범주" 열 없음
    match category_map_import(State(state), Path(aid), build_multipart(csv).await).await {
        Ok(_) => panic!("400 BAD_REQUEST가 예상됨"),
        Err((status, _)) => assert_eq!(status, StatusCode::BAD_REQUEST),
    }
}

#[tokio::test]
async fn base_data_import_graduated_missing_student_code_column_returns_bad_request() {
    // 졸업생 모드: "학생코드" 열 없음 → require_cols 실패 → Err(400) 반환
    let pool = common::create_test_pool_shared().await;
    let aid = insert_area(&pool, CalcType::Manual, None, None, 0).await;
    let state = common::make_state(pool.clone());

    let csv = "이름,값\n홍길동,85.0\n"; // "학생코드" 열 없음
    match base_data_import(State(state), Path(aid), graduated_query(), build_multipart(csv).await).await {
        Ok(_) => panic!("400 BAD_REQUEST가 예상됨"),
        Err((status, _)) => assert_eq!(status, StatusCode::BAD_REQUEST),
    }
}

// ── numeric_table_import: 값 오류 시나리오 ────────────────────────

#[tokio::test]
async fn numeric_table_import_non_numeric_threshold_rejects() {
    // 기준값이 숫자가 아닌 문자 → 전체 거부
    let pool = common::create_test_pool_shared().await;
    let aid = insert_area(&pool, CalcType::Numeric, Some(MatchMode::Upper), None, 0).await;
    let state = common::make_state(pool.clone());

    let csv = "기준값,점수\nabc,50.0\n";
    let (status, axum::Json(result)) =
        numeric_table_import(State(state), Path(aid), build_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(result.rows, 0);
    assert!(!result.errors.is_empty());
}

#[tokio::test]
async fn numeric_table_import_upper_monotonicity_violation_rejects() {
    // UPPER 모드: 기준값↑ → 점수↑ 이어야 하는데 점수가 감소 → 오류
    // 기준표: (10→50점), (20→30점) → 기준값이 높아졌는데 점수가 낮아짐 → 위반
    let pool = common::create_test_pool_shared().await;
    let aid = insert_area(&pool, CalcType::Numeric, Some(MatchMode::Upper), None, 0).await;
    let state = common::make_state(pool.clone());

    let csv = "기준값,점수\n0,0\n10,50\n20,30\n"; // 10→50, 20→30: 점수 역전
    let (status, axum::Json(result)) =
        numeric_table_import(State(state), Path(aid), build_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(result.rows, 0);
    assert!(result.errors.iter().any(|e| e.contains("순서 오류") || e.contains("UPPER")));
}

#[tokio::test]
async fn numeric_table_import_lower_monotonicity_violation_rejects() {
    // LOWER 모드: 기준값↑ → 점수↓ 이어야 하는데 점수가 증가 → 오류
    // 기준표: (10→30점), (20→50점) → 기준값이 높아졌는데 점수도 높아짐 → 위반
    let pool = common::create_test_pool_shared().await;
    let aid = insert_area(&pool, CalcType::Numeric, Some(MatchMode::Lower), None, 0).await;
    let state = common::make_state(pool.clone());

    let csv = "기준값,점수\n10,30\n20,50\n"; // 10→30, 20→50: LOWER 위반
    let (status, axum::Json(result)) =
        numeric_table_import(State(state), Path(aid), build_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(result.rows, 0);
    assert!(result.errors.iter().any(|e| e.contains("순서 오류") || e.contains("LOWER")));
}

#[tokio::test]
async fn numeric_table_import_upper_no_zero_threshold_adds_warning() {
    // UPPER 모드에서 기준값 0 항목 없음 → 삽입 성공(200)이지만 warning 포함
    let pool = common::create_test_pool_shared().await;
    let aid = insert_area(&pool, CalcType::Numeric, Some(MatchMode::Upper), None, 0).await;
    let state = common::make_state(pool.clone());

    // 기준값 0 없이 10부터 시작 → 10 미만 학생은 점수 산출 실패 → warning
    let csv = "기준값,점수\n10,50\n20,80\n30,100\n";
    let (status, axum::Json(result)) =
        numeric_table_import(State(state), Path(aid), build_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(result.rows, 3);
    assert!(result.errors.is_empty());
    assert!(!result.warnings.is_empty(), "기준값 0 없음 → warning 필수");
}

#[tokio::test]
async fn numeric_table_import_duplicate_threshold_rejects() {
    // 같은 기준값이 두 번 등장 → 전체 거부
    let pool = common::create_test_pool_shared().await;
    let aid = insert_area(&pool, CalcType::Numeric, Some(MatchMode::Upper), None, 0).await;
    let state = common::make_state(pool.clone());

    let csv = "기준값,점수\n0,0\n10,50\n10,80\n"; // 기준값 10 중복
    let (status, axum::Json(result)) =
        numeric_table_import(State(state), Path(aid), build_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(result.rows, 0);
    assert!(result.errors.iter().any(|e| e.contains("중복")));
}

// ── category_map_import: 추가 시나리오 ───────────────────────────

#[tokio::test]
async fn category_map_import_requires_zero_score_entry() {
    // 양수 점수만 있고 0점 기준(해당하지 않음) 항목이 없으면 → 422
    let pool = common::create_test_pool_shared().await;
    let aid = insert_area(&pool, CalcType::Category, None, Some(CategoryAgg::Sum), 1).await;
    let state = common::make_state(pool.clone());

    // "회장→10점" 만 있고 "해당없음→0점" 없음
    let csv = "범주,점수\n회장,10.0\n부회장,5.0\n";
    let (status, axum::Json(result)) =
        category_map_import(State(state), Path(aid), build_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(result.rows, 0);
    assert!(result.errors.iter().any(|e| e.contains("0점")));

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM category_map WHERE area_id = ?")
        .bind(aid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "0점 기준 없음 오류 시 rollback 되어야 함");
}

#[tokio::test]
async fn category_map_import_duplicate_category_rejects() {
    // 같은 범주가 두 번 등장 → 전체 거부
    let pool = common::create_test_pool_shared().await;
    let aid = insert_area(&pool, CalcType::Category, None, Some(CategoryAgg::Sum), 1).await;
    let state = common::make_state(pool.clone());

    let csv = "범주,점수\n회장,10.0\n회장,8.0\n일반,0.0\n"; // "회장" 중복
    let (status, axum::Json(result)) =
        category_map_import(State(state), Path(aid), build_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(result.rows, 0);
    assert!(result.errors.iter().any(|e| e.contains("중복")));
}

#[tokio::test]
async fn category_map_import_deduction_only_no_zero_required() {
    // 감점 전용(모든 점수 < 0) 범주표 → 0점 기준 없어도 허용
    let pool = common::create_test_pool_shared().await;
    let aid: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, category_agg, lookup_scope, multi_value) \
         VALUES ('규정위반', 0, 'CATEGORY', 'SUM', 'SIMPLE', 1) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let state = common::make_state(pool.clone());

    // 음수 점수만 — 양수가 없으므로 0점 기준 필수 규칙 적용 안 됨
    let csv = "범주,점수\n규정위반,-3.0\n";
    let (status, axum::Json(result)) =
        category_map_import(State(state), Path(aid), build_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(result.rows, 1);
    assert!(result.errors.is_empty());
}

// ── base_data_import: 엔롤드 모드 누락 열 ─────────────────────────

#[tokio::test]
async fn base_data_import_enrolled_missing_required_column_returns_bad_request() {
    // 재학생 모드: "학년" 열 없음 → require_cols 실패 → Err(400) 반환
    let pool = common::create_test_pool_shared().await;
    let aid = insert_area(&pool, CalcType::Manual, None, None, 0).await;
    let state = common::make_state(pool.clone());

    let csv = "반,번호,값\n1,1,85.0\n"; // "학년" 열 없음

    use axum::extract::Query;
    use principal_candidate_manager::handlers::area_data::StudentTypeQuery;
    let enrolled_query = Query(StudentTypeQuery { student_type: "enrolled".to_string() });

    match base_data_import(State(state), Path(aid), enrolled_query, build_multipart(csv).await).await {
        Ok(_) => panic!("400 BAD_REQUEST가 예상됨"),
        Err((status, _)) => assert_eq!(status, StatusCode::BAD_REQUEST),
    }
}

// ── base_data_import: 값 비어 있음 ───────────────────────────────

#[tokio::test]
async fn base_data_import_empty_value_rejects() {
    // "값" 열이 있지만 내용이 비어 있는 행 → 오류 → 전체 거부
    let pool = common::create_test_pool_shared().await;
    insert_student(&pool, "S001").await;
    let aid = insert_area(&pool, CalcType::Manual, None, None, 0).await;
    let state = common::make_state(pool.clone());

    let csv = "학생코드,이름,값\nS001,테스트,\n"; // 값 비어 있음
    let (status, axum::Json(result)) =
        base_data_import(State(state), Path(aid), graduated_query(), build_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(result.rows, 0);
    assert!(!result.errors.is_empty());
}

// ── base_data_import: 숫자 아닌 값 ───────────────────────────────

#[tokio::test]
async fn base_data_import_manual_non_numeric_value_rejects() {
    // MANUAL 전형요소에 숫자가 아닌 문자열 값 → 오류 → 전체 거부
    let pool = common::create_test_pool_shared().await;
    insert_student(&pool, "S001").await;
    let aid = insert_area(&pool, CalcType::Manual, None, None, 0).await;
    let state = common::make_state(pool.clone());

    let csv = "학생코드,이름,값\nS001,테스트,홍길동\n"; // 숫자 아닌 문자열
    let (status, axum::Json(result)) =
        base_data_import(State(state), Path(aid), graduated_query(), build_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(result.rows, 0);
    assert!(!result.errors.is_empty());
}

// ── 이름 열 누락/빈 셀 검증 ──────────────────────────────────────

#[tokio::test]
async fn base_data_import_graduated_missing_name_column_returns_bad_request() {
    // 졸업생 모드: "이름" 열 없음 → require_cols 실패 → 400
    let pool = common::create_test_pool_shared().await;
    let aid = insert_area(&pool, CalcType::Manual, None, None, 0).await;
    let state = common::make_state(pool.clone());

    let csv = "학생코드,값\nS001,85.0\n"; // "이름" 열 없음
    match base_data_import(State(state), Path(aid), graduated_query(), build_multipart(csv).await).await {
        Ok(_) => panic!("400 BAD_REQUEST가 예상됨"),
        Err((status, _)) => assert_eq!(status, StatusCode::BAD_REQUEST),
    }
}

#[tokio::test]
async fn base_data_import_graduated_empty_name_rejects() {
    // 졸업생 모드: 이름 열이 있지만 비어 있는 행 → 오류 → 전체 거부
    let pool = common::create_test_pool_shared().await;
    insert_student(&pool, "S001").await;
    let aid = insert_area(&pool, CalcType::Manual, None, None, 0).await;
    let state = common::make_state(pool.clone());

    let csv = "학생코드,이름,값\nS001,,85.0\n"; // 이름 비어 있음
    let (status, axum::Json(result)) =
        base_data_import(State(state), Path(aid), graduated_query(), build_multipart(csv).await)
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
    assert_eq!(count, 0);
}

#[tokio::test]
async fn base_data_import_enrolled_missing_name_column_returns_bad_request() {
    // 재학생 모드: "이름" 열 없음 → require_cols 실패 → 400
    let pool = common::create_test_pool_shared().await;
    let aid = insert_area(&pool, CalcType::Manual, None, None, 0).await;
    let state = common::make_state(pool.clone());

    let csv = "학년,반,번호,값\n1,1,1,85.0\n"; // "이름" 열 없음
    use axum::extract::Query;
    use principal_candidate_manager::handlers::area_data::StudentTypeQuery;
    let enrolled_query = Query(StudentTypeQuery { student_type: "enrolled".to_string() });

    match base_data_import(State(state), Path(aid), enrolled_query, build_multipart(csv).await).await {
        Ok(_) => panic!("400 BAD_REQUEST가 예상됨"),
        Err((status, _)) => assert_eq!(status, StatusCode::BAD_REQUEST),
    }
}

#[tokio::test]
async fn base_data_import_enrolled_empty_name_rejects() {
    // 재학생 모드: 이름 열이 있지만 비어 있는 행 → 오류 → 전체 거부
    let pool = common::create_test_pool_shared().await;
    common::insert_class(&pool, 1, 1).await;
    sqlx::query(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES ('20250101', '홍길동', 1, 1, 1, 1)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let aid = insert_area(&pool, CalcType::Manual, None, None, 0).await;
    let state = common::make_state(pool.clone());

    let csv = "학년,반,번호,이름,값\n1,1,1,,85.0\n"; // 이름 비어 있음
    use axum::extract::Query;
    use principal_candidate_manager::handlers::area_data::StudentTypeQuery;
    let enrolled_query = Query(StudentTypeQuery { student_type: "enrolled".to_string() });

    let (status, axum::Json(result)) =
        base_data_import(State(state), Path(aid), enrolled_query, build_multipart(csv).await)
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
    assert_eq!(count, 0);
}

// ── base_data_import UPSERT 동작 ─────────────────────────────────

#[tokio::test]
async fn base_data_import_upsert_updates_existing_row() {
    // 같은 학생을 두 번 import하면 두 번째 값으로 업데이트되고 행 수는 1 유지
    let pool = common::create_test_pool_shared().await;
    insert_student(&pool, "S001").await;
    let aid = insert_area(&pool, CalcType::Manual, None, None, 0).await;
    let state = common::make_state(pool.clone());

    let csv1 = "학생코드,이름,값\nS001,테스트,85.0\n";
    let (status, _) =
        base_data_import(State(state), Path(aid), graduated_query(), build_multipart(csv1).await)
            .await.unwrap();
    assert_eq!(status, StatusCode::OK);

    let state2 = common::make_state(pool.clone());
    let csv2 = "학생코드,이름,값\nS001,테스트,90.0\n";
    let (status2, axum::Json(result)) =
        base_data_import(State(state2), Path(aid), graduated_query(), build_multipart(csv2).await)
            .await.unwrap();
    assert_eq!(status2, StatusCode::OK);
    assert_eq!(result.rows, 1);

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM base_data WHERE area_id = ?")
        .bind(aid).fetch_one(&pool).await.unwrap();
    assert_eq!(count, 1, "중복 행이 생기지 않고 업데이트되어야 함");

    let value: String = sqlx::query_scalar("SELECT value FROM base_data WHERE area_id = ?")
        .bind(aid).fetch_one(&pool).await.unwrap();
    assert_eq!(value, "9000000", "두 번째 값(90.0 × 100000)으로 갱신되어야 함");
}

#[tokio::test]
async fn base_data_import_partial_preserves_other_students() {
    // S001+S002 import 후, S001만 재import → S001 값 갱신, S002 데이터 유지
    let pool = common::create_test_pool_shared().await;
    insert_student(&pool, "S001").await;
    insert_student(&pool, "S002").await;
    let aid = insert_area(&pool, CalcType::Manual, None, None, 0).await;
    let state1 = common::make_state(pool.clone());

    let csv1 = "학생코드,이름,값\nS001,테스트,85.0\nS002,테스트,70.0\n";
    let (status, _) =
        base_data_import(State(state1), Path(aid), graduated_query(), build_multipart(csv1).await)
            .await.unwrap();
    assert_eq!(status, StatusCode::OK);

    let state2 = common::make_state(pool.clone());
    let csv2 = "학생코드,이름,값\nS001,테스트,95.0\n";
    let (status2, axum::Json(result)) =
        base_data_import(State(state2), Path(aid), graduated_query(), build_multipart(csv2).await)
            .await.unwrap();
    assert_eq!(status2, StatusCode::OK);
    assert_eq!(result.rows, 1);

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM base_data WHERE area_id = ?")
        .bind(aid).fetch_one(&pool).await.unwrap();
    assert_eq!(count, 2, "S001+S002 두 행이 남아있어야 함");

    let s002_val: String = sqlx::query_scalar(
        "SELECT bd.value FROM base_data bd JOIN students s ON bd.student_id = s.id \
         WHERE bd.area_id = ? AND s.student_code = ?",
    )
    .bind(aid).bind("S002").fetch_one(&pool).await.unwrap();
    assert_eq!(s002_val, "7000000", "S002 데이터는 원래 값(70.0)이 보존되어야 함");
}

#[tokio::test]
async fn base_data_import_multi_value_upsert_replaces_student_values() {
    // multi_value=1: 같은 학생 재import 시 기존 값 집합 교체 — 파일에 없는 값은 삭제됨
    let pool = common::create_test_pool_shared().await;
    insert_student(&pool, "S001").await;
    insert_student(&pool, "S002").await;
    let aid = insert_area(&pool, CalcType::Category, None, Some(CategoryAgg::Sum), 1).await;
    let state1 = common::make_state(pool.clone());

    let csv1 = "학생코드,이름,값\nS001,테스트,회장\nS001,테스트,부회장\nS002,테스트,학생회\n";
    let (status, _) =
        base_data_import(State(state1), Path(aid), graduated_query(), build_multipart(csv1).await)
            .await.unwrap();
    assert_eq!(status, StatusCode::OK);

    // S001을 "부회장"만으로 재import (회장 제거), S002 제외
    let state2 = common::make_state(pool.clone());
    let csv2 = "학생코드,이름,값\nS001,테스트,부회장\n";
    let (status2, axum::Json(result)) =
        base_data_import(State(state2), Path(aid), graduated_query(), build_multipart(csv2).await)
            .await.unwrap();
    assert_eq!(status2, StatusCode::OK);
    assert_eq!(result.rows, 1);

    let s001_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM base_data bd JOIN students s ON bd.student_id = s.id \
         WHERE bd.area_id = ? AND s.student_code = ?",
    )
    .bind(aid).bind("S001").fetch_one(&pool).await.unwrap();
    assert_eq!(s001_count, 1, "S001은 '부회장' 1행만 남아야 함 (회장 제거됨)");

    let s002_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM base_data bd JOIN students s ON bd.student_id = s.id \
         WHERE bd.area_id = ? AND s.student_code = ?",
    )
    .bind(aid).bind("S002").fetch_one(&pool).await.unwrap();
    assert_eq!(s002_count, 1, "S002는 파일에 없으므로 기존 데이터 보존");
}

// ── CLOSED 라운드 지원자 기초데이터 삭제 차단 (DELETE 트리거) ──────
// INSERT OR REPLACE(UPSERT)는 내부 DELETE에 BEFORE DELETE 트리거를 발동시키지 않으므로
// 수정은 자유롭게 허용되고, 명시적 DELETE만 차단된다.

async fn setup_closed_round_application(pool: &sqlx::SqlitePool, student_id: i64) {
    let univ_id: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name) VALUES ('서울대학교') RETURNING id",
    )
    .fetch_one(pool).await.unwrap();
    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name) VALUES (?, '컴퓨터공학과') RETURNING id",
    )
    .bind(univ_id).fetch_one(pool).await.unwrap();
    let round_id: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at, closed_at) VALUES ('CLOSED', '2026-01-01', '2026-01-02') RETURNING id",
    )
    .fetch_one(pool).await.unwrap();
    sqlx::query(
        "INSERT INTO applications (student_id, track_id, round_id, confirmed, abandoned, department_name) \
         VALUES (?, ?, ?, 1, 0, '컴퓨터공학과')",
    )
    .bind(student_id).bind(track_id).bind(round_id)
    .execute(pool).await.unwrap();
}

async fn setup_finalized_round_application(pool: &sqlx::SqlitePool, student_id: i64) {
    let univ_id: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name) VALUES ('연세대학교') RETURNING id",
    )
    .fetch_one(pool).await.unwrap();
    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name) VALUES (?, '전기전자공학과') RETURNING id",
    )
    .bind(univ_id).fetch_one(pool).await.unwrap();
    let round_id: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at, closed_at, finalized_at) VALUES ('FINALIZED', '2026-01-01', '2026-01-02', '2026-01-03') RETURNING id",
    )
    .fetch_one(pool).await.unwrap();
    sqlx::query(
        "INSERT INTO applications (student_id, track_id, round_id, confirmed, abandoned, department_name) \
         VALUES (?, ?, ?, 1, 0, '전기전자공학과')",
    )
    .bind(student_id).bind(track_id).bind(round_id)
    .execute(pool).await.unwrap();
}

#[tokio::test]
async fn base_data_delete_blocked_for_closed_round_student() {
    // CLOSED 라운드 지원자의 base_data 명시적 DELETE → 트리거로 차단
    let pool = common::create_test_pool_shared().await;
    let sid = insert_student(&pool, "S001").await;
    let aid = insert_area(&pool, CalcType::Manual, None, None, 0).await;

    // base_data 먼저 삽입 (INSERT 트리거 없음 — UPSERT는 허용)
    sqlx::query(
        "INSERT INTO base_data (student_id, area_id, track_id, value, multi_value) VALUES (?, ?, NULL, '8500000', 0)",
    )
    .bind(sid).bind(aid).execute(&pool).await.unwrap();

    // CLOSED 라운드 지원 이력 추가
    setup_closed_round_application(&pool, sid).await;

    // DELETE 시도 → 트리거 차단
    let result = sqlx::query("DELETE FROM base_data WHERE student_id = ? AND area_id = ?")
        .bind(sid).bind(aid).execute(&pool).await;
    assert!(result.is_err(), "CLOSED 라운드 지원자의 base_data 삭제는 차단되어야 함");

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM base_data WHERE student_id = ?")
        .bind(sid).fetch_one(&pool).await.unwrap();
    assert_eq!(count, 1, "삭제 차단으로 데이터 보존");
}

#[tokio::test]
async fn base_data_delete_allowed_for_finalized_round_student() {
    // FINALIZED 라운드 지원자의 base_data는 삭제 가능 (새 라운드 갱신 허용)
    let pool = common::create_test_pool_shared().await;
    let sid = insert_student(&pool, "S001").await;
    let aid = insert_area(&pool, CalcType::Manual, None, None, 0).await;

    sqlx::query(
        "INSERT INTO base_data (student_id, area_id, track_id, value, multi_value) VALUES (?, ?, NULL, '8500000', 0)",
    )
    .bind(sid).bind(aid).execute(&pool).await.unwrap();

    setup_finalized_round_application(&pool, sid).await;

    // DELETE → 허용
    sqlx::query("DELETE FROM base_data WHERE student_id = ? AND area_id = ?")
        .bind(sid).bind(aid).execute(&pool).await.unwrap();

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM base_data WHERE student_id = ?")
        .bind(sid).fetch_one(&pool).await.unwrap();
    assert_eq!(count, 0, "FINALIZED 라운드는 삭제 허용");
}

#[tokio::test]
async fn base_data_upsert_allowed_for_closed_round_student() {
    // CLOSED 라운드 지원자도 UPSERT(INSERT OR REPLACE)는 허용됨
    // INSERT OR REPLACE는 BEFORE DELETE 트리거를 발동시키지 않으므로 수정은 자유롭게 가능
    let pool = common::create_test_pool_shared().await;
    let sid = insert_student(&pool, "S001").await;
    let aid = insert_area(&pool, CalcType::Manual, None, None, 0).await;

    sqlx::query(
        "INSERT INTO base_data (student_id, area_id, track_id, value, multi_value) VALUES (?, ?, NULL, '8500000', 0)",
    )
    .bind(sid).bind(aid).execute(&pool).await.unwrap();

    setup_closed_round_application(&pool, sid).await;

    // UPSERT → 허용 (트리거 미발동)
    sqlx::query(
        "INSERT OR REPLACE INTO base_data (student_id, area_id, track_id, value, multi_value) VALUES (?, ?, NULL, '9000000', 0)",
    )
    .bind(sid).bind(aid).execute(&pool).await.unwrap();

    let value: String = sqlx::query_scalar("SELECT value FROM base_data WHERE student_id = ? AND area_id = ?")
        .bind(sid).bind(aid).fetch_one(&pool).await.unwrap();
    assert_eq!(value, "9000000", "UPSERT로 값이 갱신되어야 함");
}

// ── 세션 3 감사 후속: graduated is_enrolled 필터 / 빈 파일 wipe 차단 ──

#[tokio::test]
async fn base_data_import_graduated_rejects_enrolled_student_code() {
    // graduated 업로드에 재학생 student_code가 섞이면 행 오류 — 재학생 데이터 침범 금지
    let pool = common::create_test_pool_shared().await;
    common::insert_class(&pool, 3, 1).await;
    sqlx::query(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES ('E001', '재학생', 3, 1, 5, 1)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let aid = insert_area(&pool, CalcType::Manual, None, None, 0).await;
    let state = common::make_state(pool.clone());

    let csv = "학생코드,이름,값\nE001,재학생,85\n";
    let (status, axum::Json(result)) =
        base_data_import(State(state), Path(aid), graduated_query(), build_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(result.errors[0].contains("졸업생"));

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM base_data WHERE area_id = ?")
        .bind(aid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "재학생 base_data가 생성되면 안 됨");
}

#[tokio::test]
async fn base_data_import_graduated_accepts_graduated_student_code() {
    // 동일 경로 유효값: 졸업생 코드는 정상 저장
    let pool = common::create_test_pool_shared().await;
    insert_student(&pool, "G001").await;
    let aid = insert_area(&pool, CalcType::Manual, None, None, 0).await;
    let state = common::make_state(pool.clone());

    let csv = "학생코드,이름,값\nG001,테스트,85\n";
    let (status, axum::Json(result)) =
        base_data_import(State(state), Path(aid), graduated_query(), build_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(result.rows, 1);
}

#[tokio::test]
async fn numeric_table_import_empty_file_rejected_and_table_preserved() {
    // 헤더만 있는 파일이 기준표 전체를 조용히 비우면 안 됨
    let pool = common::create_test_pool_shared().await;
    let aid = insert_area(&pool, CalcType::Numeric, Some(MatchMode::Upper), None, 0).await;
    sqlx::query("INSERT INTO numeric_table (area_id, track_id, threshold, score) VALUES (?, NULL, 0, 100000)")
        .bind(aid)
        .execute(&pool)
        .await
        .unwrap();
    let state = common::make_state(pool.clone());

    let result = numeric_table_import(State(state), Path(aid), build_multipart("기준값,점수\n").await).await;
    let Err(err) = result else { panic!("오류가 반환되어야 함") };
    assert_eq!(err.0, StatusCode::BAD_REQUEST);

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM numeric_table WHERE area_id = ?")
        .bind(aid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "기존 기준표가 보존되어야 함");
}

#[tokio::test]
async fn category_map_import_empty_file_rejected_and_table_preserved() {
    let pool = common::create_test_pool_shared().await;
    let aid = insert_area(&pool, CalcType::Category, None, Some(CategoryAgg::Max), 0).await;
    sqlx::query("INSERT INTO category_map (area_id, track_id, category, score) VALUES (?, NULL, '해당없음', 0)")
        .bind(aid)
        .execute(&pool)
        .await
        .unwrap();
    let state = common::make_state(pool.clone());

    let result = category_map_import(State(state), Path(aid), build_multipart("범주,점수\n").await).await;
    let Err(err) = result else { panic!("오류가 반환되어야 함") };
    assert_eq!(err.0, StatusCode::BAD_REQUEST);

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM category_map WHERE area_id = ?")
        .bind(aid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "기존 범주표가 보존되어야 함");
}

// ── 세션 4 감사 후속: student_type 검증 (silent fallback 제거) ────

#[tokio::test]
async fn base_data_import_invalid_student_type_returns_400() {
    let pool = common::create_test_pool_shared().await;
    insert_student(&pool, "S001").await;
    let aid = insert_area(&pool, CalcType::Manual, None, None, 0).await;
    let state = common::make_state(pool.clone());

    let csv = "학생코드,이름,값\nS001,테스트,30.5\n";
    let q = Query(StudentTypeQuery { student_type: "Enrolled".to_string() }); // 대문자 오타
    let err = base_data_import(State(state), Path(aid), q, build_multipart(csv).await)
        .await
        .unwrap_err();

    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(err.1.contains("student_type"));

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM base_data WHERE area_id = ?")
        .bind(aid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn base_data_list_invalid_student_type_returns_400() {
    let pool = common::create_test_pool_shared().await;
    let aid = insert_area(&pool, CalcType::Manual, None, None, 0).await;
    let state = common::make_state(pool.clone());

    let q = Query(BaseDataPageQuery { page: 1, per_page: 50, student_type: "all".to_string() });
    let err = base_data_list(State(state), Path(aid), q).await.unwrap_err();

    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(err.1.contains("student_type"));
}

#[tokio::test]
async fn base_data_import_valid_student_types_still_accepted() {
    // 유효값 회귀 확인: graduated는 기존대로 동작
    let pool = common::create_test_pool_shared().await;
    insert_student(&pool, "S001").await;
    let aid = insert_area(&pool, CalcType::Manual, None, None, 0).await;
    let state = common::make_state(pool.clone());

    let csv = "학생코드,이름,값\nS001,테스트,30.5\n";
    let (status, axum::Json(result)) =
        base_data_import(State(state), Path(aid), graduated_query(), build_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(result.rows, 1);
}
