mod common;

use axum::{extract::State, http::StatusCode, Json};
use principal_candidate_manager::enums::{CalcType, CategoryAgg, LookupScope, MatchMode};
use principal_candidate_manager::handlers::areas::{
    create_area, list_areas, update_area, CreateAreaBody, UpdateAreaBody,
};
use principal_candidate_manager::score::Score;

fn manual_area_body(name: &str, max_score: Score) -> CreateAreaBody {
    CreateAreaBody {
        name: name.into(),
        max_score,
        calc_type: CalcType::Manual,
        teacher_editable: true,
        lookup_scope: LookupScope::Simple,
        match_mode: None,
        category_agg: None,
        multi_value: false,
        unit: None,
    }
}

// ── create_area 만점(max_score) 유효성 검증 ──────────────────────

#[tokio::test]
async fn create_area_zero_max_score_succeeds() {
    // 순수 감점 전형요소: max_score=0 (만점이 0점, 점수 기준에서 감점)
    let pool = common::create_test_pool().await;
    let (status, _) = create_area(
        State(common::make_state(pool)),
        Json(manual_area_body("순수감점", Score::from_raw(0))),
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
        Json(manual_area_body("test", Score::from_raw(-100_000))),
    )
    .await;
    assert_eq!(res.unwrap_err().0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_area_valid_max_score_succeeds() {
    let pool = common::create_test_pool().await;
    let res = create_area(
        State(common::make_state(pool)),
        Json(manual_area_body("수기입력", Score::from_raw(1_000_000))),
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
        max_score: Score::from_raw(1_000_000),
        calc_type: CalcType::Numeric,
        teacher_editable: false,
        lookup_scope: LookupScope::Simple,
        match_mode: None,
        category_agg: None,
        multi_value: false,
        unit: None,
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
        max_score: Score::from_raw(1_000_000),
        calc_type: CalcType::Category,
        teacher_editable: false,
        lookup_scope: LookupScope::Simple,
        match_mode: None,
        category_agg: None,
        multi_value: false,
        unit: None,
    };
    let res = create_area(State(common::make_state(pool)), Json(body)).await;
    assert_eq!(res.unwrap_err().0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_area_whitespace_only_name_rejected() {
    // 이름이 공백만 있는 경우 → 400 (trim 후 빈 문자열)
    let pool = common::create_test_pool().await;
    let res = create_area(
        State(common::make_state(pool)),
        Json(manual_area_body("   ", Score::from_raw(1_000_000))),
    )
    .await;
    assert_eq!(res.unwrap_err().0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_area_empty_name_rejected() {
    // 이름이 빈 문자열 → 400
    let pool = common::create_test_pool().await;
    let res = create_area(
        State(common::make_state(pool)),
        Json(manual_area_body("", Score::from_raw(1_000_000))),
    )
    .await;
    assert_eq!(res.unwrap_err().0, StatusCode::BAD_REQUEST);
}

// ── unit 기능 테스트 ──────────────────────────────────────────────

#[tokio::test]
async fn create_area_with_unit_stored_and_returned() {
    // NUMERIC + unit "시간" 생성 → list_areas에서 unit 반환
    let pool = common::create_test_pool().await;
    let _ = create_area(
        State(common::make_state(pool.clone())),
        Json(CreateAreaBody {
            name: "봉사".into(),
            max_score: Score::from_raw(500_000),
            calc_type: CalcType::Numeric,
            teacher_editable: true,
            lookup_scope: LookupScope::Simple,
            match_mode: Some(MatchMode::Upper),
            category_agg: None,
            multi_value: false,
            unit: Some("시간".into()),
        }),
    )
    .await
    .unwrap();

    let Json(rows) = list_areas(State(common::make_state(pool))).await.unwrap();
    let area = rows.iter().find(|r| r.name == "봉사").unwrap();
    assert_eq!(area.unit.as_deref(), Some("시간"));
}

#[tokio::test]
async fn create_area_unit_whitespace_trimmed_to_null() {
    // unit "  " → NULL 저장
    let pool = common::create_test_pool().await;
    let _ = create_area(
        State(common::make_state(pool.clone())),
        Json(CreateAreaBody {
            name: "출결".into(),
            max_score: Score::from_raw(1_000_000),
            calc_type: CalcType::Manual,
            teacher_editable: true,
            lookup_scope: LookupScope::Simple,
            match_mode: None,
            category_agg: None,
            multi_value: false,
            unit: Some("   ".into()),
        }),
    )
    .await
    .unwrap();

    let Json(rows) = list_areas(State(common::make_state(pool))).await.unwrap();
    let area = rows.iter().find(|r| r.name == "출결").unwrap();
    assert_eq!(area.unit, None);
}

#[tokio::test]
async fn create_area_unit_trimmed_stored() {
    // unit "  등급  " → "등급" 저장
    let pool = common::create_test_pool().await;
    let _ = create_area(
        State(common::make_state(pool.clone())),
        Json(CreateAreaBody {
            name: "내신".into(),
            max_score: Score::from_raw(10_000_000),
            calc_type: CalcType::Numeric,
            teacher_editable: false,
            lookup_scope: LookupScope::Simple,
            match_mode: Some(MatchMode::Upper),
            category_agg: None,
            multi_value: false,
            unit: Some("  등급  ".into()),
        }),
    )
    .await
    .unwrap();

    let Json(rows) = list_areas(State(common::make_state(pool))).await.unwrap();
    let area = rows.iter().find(|r| r.name == "내신").unwrap();
    assert_eq!(area.unit.as_deref(), Some("등급"));
}

#[tokio::test]
async fn create_category_area_with_unit_rejected() {
    // CATEGORY + unit → 400
    let pool = common::create_test_pool().await;
    let body = CreateAreaBody {
        name: "활동".into(),
        max_score: Score::from_raw(1_000_000),
        calc_type: CalcType::Category,
        teacher_editable: true,
        lookup_scope: LookupScope::Simple,
        match_mode: None,
        category_agg: Some(CategoryAgg::Max),
        multi_value: false,
        unit: Some("회".into()),
    };
    let res = create_area(State(common::make_state(pool)), Json(body)).await;
    assert_eq!(res.unwrap_err().0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn update_area_unit_set_and_clear() {
    // unit 설정 후 빈 문자열로 업데이트 → NULL
    let pool = common::create_test_pool().await;
    let (_, Json(body)) = create_area(
        State(common::make_state(pool.clone())),
        Json(manual_area_body("면접", Score::from_raw(500_000))),
    )
    .await
    .unwrap();
    let id = body["id"].as_i64().unwrap();

    // unit 설정
    update_area(
        State(common::make_state(pool.clone())),
        axum::extract::Path(id),
        Json(UpdateAreaBody { name: None, teacher_editable: None, unit: Some("점".into()) }),
    )
    .await
    .unwrap();

    let Json(rows) = list_areas(State(common::make_state(pool.clone()))).await.unwrap();
    let area = rows.iter().find(|r| r.id == id).unwrap();
    assert_eq!(area.unit.as_deref(), Some("점"));

    // unit 제거 (빈 문자열 → NULL)
    update_area(
        State(common::make_state(pool.clone())),
        axum::extract::Path(id),
        Json(UpdateAreaBody { name: None, teacher_editable: None, unit: Some("".into()) }),
    )
    .await
    .unwrap();

    let Json(rows) = list_areas(State(common::make_state(pool))).await.unwrap();
    let area = rows.iter().find(|r| r.id == id).unwrap();
    assert_eq!(area.unit, None);
}

#[tokio::test]
async fn update_category_area_with_unit_rejected() {
    // CATEGORY 전형요소에 update로 unit 설정 → 400
    let pool = common::create_test_pool().await;
    let (_, Json(body)) = create_area(
        State(common::make_state(pool.clone())),
        Json(CreateAreaBody {
            name: "생활태도".into(),
            max_score: Score::from_raw(0),
            calc_type: CalcType::Category,
            teacher_editable: true,
            lookup_scope: LookupScope::Simple,
            match_mode: None,
            category_agg: Some(CategoryAgg::Sum),
            multi_value: false,
            unit: None,
        }),
    )
    .await
    .unwrap();
    let id = body["id"].as_i64().unwrap();

    let res = update_area(
        State(common::make_state(pool)),
        axum::extract::Path(id),
        Json(UpdateAreaBody { name: None, teacher_editable: None, unit: Some("항목".into()) }),
    )
    .await;
    assert_eq!(res.unwrap_err().0, StatusCode::BAD_REQUEST);
}
