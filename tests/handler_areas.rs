mod common;

use axum::{extract::{Path, State}, http::StatusCode, Json};
use principal_candidate_manager::handlers::areas::{
    create_area, put_category_map, put_numeric_table, CategoryRow, CreateAreaBody, RangeRow,
};

fn manual_area_body(name: &str, max_score: f64) -> CreateAreaBody {
    CreateAreaBody {
        name: name.into(),
        max_score,
        calc_type: "MANUAL".into(),
        teacher_editable: 1,
        lookup_scope: "SIMPLE".into(),
        match_mode: None,
        category_agg: None,
        multi_value: 0,
    }
}

async fn insert_numeric_area(pool: &sqlx::SqlitePool) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, match_mode, lookup_scope) \
         VALUES ('구간테스트', 1000000, 'NUMERIC', 'UPPER', 'SIMPLE') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn insert_category_area(pool: &sqlx::SqlitePool) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, category_agg, lookup_scope, multi_value) \
         VALUES ('범주테스트', 1000000, 'CATEGORY', 'SUM', 'SIMPLE', 1) RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

// ── create_area 만점(max_score) 유효성 검증 ──────────────────────

#[tokio::test]
async fn create_area_zero_max_score_succeeds() {
    // 순수 감점 전형요소: max_score=0 (만점이 0점, 점수 기준에서 감점)
    let pool = common::create_test_pool().await;
    let (status, _) = create_area(
        State(common::make_state(pool)),
        Json(manual_area_body("순수감점", 0.0)),
    )
    .await
    .unwrap();
    assert_eq!(status, StatusCode::CREATED);
}

#[tokio::test]
async fn create_area_negative_max_score_rejected() {
    let pool = common::create_test_pool().await;
    let res = create_area(
        State(common::make_state(pool)),
        Json(manual_area_body("test", -1.0)),
    )
    .await;
    assert_eq!(res.unwrap_err().0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_area_valid_max_score_succeeds() {
    let pool = common::create_test_pool().await;
    let res = create_area(
        State(common::make_state(pool)),
        Json(manual_area_body("수기입력", 10.0)),
    )
    .await;
    let (status, _) = res.unwrap();
    assert_eq!(status, StatusCode::CREATED);
}

#[tokio::test]
async fn create_numeric_area_without_match_mode_rejected() {
    // NUMERIC 전형요소에 match_mode 없으면 400
    let pool = common::create_test_pool().await;
    let body = CreateAreaBody {
        name: "내신".into(),
        max_score: 10.0,
        calc_type: "NUMERIC".into(),
        teacher_editable: 0,
        lookup_scope: "SIMPLE".into(),
        match_mode: None,
        category_agg: None,
        multi_value: 0,
    };
    let res = create_area(State(common::make_state(pool)), Json(body)).await;
    assert_eq!(res.unwrap_err().0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_category_area_without_category_agg_rejected() {
    // CATEGORY 전형요소에 category_agg 없으면 400
    let pool = common::create_test_pool().await;
    let body = CreateAreaBody {
        name: "활동".into(),
        max_score: 10.0,
        calc_type: "CATEGORY".into(),
        teacher_editable: 0,
        lookup_scope: "SIMPLE".into(),
        match_mode: None,
        category_agg: None,
        multi_value: 0,
    };
    let res = create_area(State(common::make_state(pool)), Json(body)).await;
    assert_eq!(res.unwrap_err().0, StatusCode::BAD_REQUEST);
}

// ── put_numeric_table 음수 허용 (감점 전형요소 지원) ────────────

#[tokio::test]
async fn put_numeric_table_negative_score_allowed() {
    // 감점 전형요소: 음수 점수가 구간표에 저장될 수 있어야 함
    let pool = common::create_test_pool().await;
    let aid = insert_numeric_area(&pool).await;

    let rows = vec![RangeRow { threshold: 100_000, score: -50_000 }];
    let status = put_numeric_table(
        State(common::make_state(pool.clone())),
        Path(aid),
        Json(rows),
    )
    .await
    .unwrap();
    assert_eq!(status, StatusCode::NO_CONTENT);

    let score: i64 =
        sqlx::query_scalar("SELECT score FROM numeric_table WHERE area_id = ?")
            .bind(aid)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(score, -50_000);
}

#[tokio::test]
async fn put_numeric_table_negative_threshold_allowed() {
    // 음수 기준값도 허용
    let pool = common::create_test_pool().await;
    let aid = insert_numeric_area(&pool).await;

    let rows = vec![RangeRow { threshold: -100_000, score: 50_000 }];
    let status = put_numeric_table(
        State(common::make_state(pool.clone())),
        Path(aid),
        Json(rows),
    )
    .await
    .unwrap();
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn put_numeric_table_valid_replaces_data() {
    // 정상 데이터는 기존 삭제 후 교체
    let pool = common::create_test_pool().await;
    let aid = insert_numeric_area(&pool).await;

    let rows = vec![
        RangeRow { threshold: 100_000, score: 50_000 },
        RangeRow { threshold: 200_000, score: 30_000 },
    ];
    let status = put_numeric_table(
        State(common::make_state(pool.clone())),
        Path(aid),
        Json(rows),
    )
    .await
    .unwrap();
    assert_eq!(status, StatusCode::NO_CONTENT);

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM numeric_table WHERE area_id = ?")
            .bind(aid)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 2);
}

// ── put_category_map 음수 허용 (감점 전형요소 지원) ─────────────

#[tokio::test]
async fn put_category_map_negative_score_allowed() {
    // 감점 전형요소: 음수 점수가 범주표에 저장될 수 있어야 함
    let pool = common::create_test_pool().await;
    let aid = insert_category_area(&pool).await;

    let rows = vec![CategoryRow { category: "규정위반".into(), score: -30_000 }];
    let status = put_category_map(
        State(common::make_state(pool.clone())),
        Path(aid),
        Json(rows),
    )
    .await
    .unwrap();
    assert_eq!(status, StatusCode::NO_CONTENT);

    let score: i64 =
        sqlx::query_scalar("SELECT score FROM category_map WHERE area_id = ?")
            .bind(aid)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(score, -30_000);
}

#[tokio::test]
async fn put_category_map_zero_score_succeeds() {
    // 0점은 허용
    let pool = common::create_test_pool().await;
    let aid = insert_category_area(&pool).await;

    let rows = vec![CategoryRow { category: "해당없음".into(), score: 0 }];
    let status = put_category_map(
        State(common::make_state(pool)),
        Path(aid),
        Json(rows),
    )
    .await
    .unwrap();
    assert_eq!(status, StatusCode::NO_CONTENT);
}
