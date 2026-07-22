mod common;

use axum::{
    extract::{Path, State},
    http::StatusCode,
};
use principal_candidate_manager::enums::{CalcType, CategoryAgg, LookupScope, MatchMode};
use axum::extract::Query;
use principal_candidate_manager::handlers::scoring::{
    calc_area_score, calculate_scores, export_results, export_round_summary, get_results,
    lookup_range_score,
    recommend_result, score_preview, teacher_get_results, unrecommend_result,
    AreaRow, ResultQuery, ResultRow, ScorePreviewQuery, StudentTrackCtx,
};
use principal_candidate_manager::score::Score;

// ── lookup_range_score 순수 함수 ──────────────────────────────────

fn sample_rows() -> Vec<(i64, i64)> {
    vec![(100_000, 50_000), (200_000, 30_000), (300_000, 10_000)]
}

#[test]
fn upper_exact_match() {
    assert_eq!(lookup_range_score(200_000, &sample_rows(), MatchMode::Upper).unwrap(), 30_000);
}

#[test]
fn upper_between_thresholds() {
    assert_eq!(lookup_range_score(150_000, &sample_rows(), MatchMode::Upper).unwrap(), 50_000);
}

#[test]
fn upper_above_all_thresholds() {
    assert_eq!(lookup_range_score(350_000, &sample_rows(), MatchMode::Upper).unwrap(), 10_000);
}

#[test]
fn upper_below_all_thresholds_returns_error() {
    // value가 모든 threshold보다 낮아 매칭 행 없음 → Err
    assert!(lookup_range_score(50_000, &sample_rows(), MatchMode::Upper).is_err());
}

#[test]
fn lower_exact_match() {
    assert_eq!(lookup_range_score(200_000, &sample_rows(), MatchMode::Lower).unwrap(), 30_000);
}

#[test]
fn lower_between_thresholds() {
    assert_eq!(lookup_range_score(150_000, &sample_rows(), MatchMode::Lower).unwrap(), 30_000);
}

#[test]
fn lower_above_all_thresholds() {
    // value가 최대 threshold를 초과하면 최대 threshold 행의 점수 반환 ("5일 이상 → 5점" 케이스)
    assert_eq!(lookup_range_score(400_000, &sample_rows(), MatchMode::Lower).unwrap(), 10_000);
}

#[test]
fn lower_below_all_thresholds() {
    assert_eq!(lookup_range_score(50_000, &sample_rows(), MatchMode::Lower).unwrap(), 50_000);
}

#[test]
fn empty_rows_upper_returns_error() {
    assert!(lookup_range_score(100_000, &[], MatchMode::Upper).is_err());
}

#[test]
fn empty_rows_lower_returns_error() {
    assert!(lookup_range_score(100_000, &[], MatchMode::Lower).is_err());
}

// ── lookup_range_score: EXACT 시나리오 ───────────────────────────────

#[test]
fn exact_hit_returns_score() {
    assert_eq!(lookup_range_score(200_000, &sample_rows(), MatchMode::Exact).unwrap(), 30_000);
}

#[test]
fn exact_miss_returns_error() {
    // 150_000은 구간표에 없음 → Err
    assert!(lookup_range_score(150_000, &sample_rows(), MatchMode::Exact).is_err());
}

#[test]
fn exact_empty_rows_returns_error() {
    assert!(lookup_range_score(100_000, &[], MatchMode::Exact).is_err());
}

// ── lookup_range_score: 봉사시간(UPPER) 시나리오 ──────────────────
//
// 기준표: 40시간 이상→2점, 30시간 이상→1점, 30시간 미만→0점
// DB 저장값(×100_000): threshold 0→0, 3_000_000→100_000, 4_000_000→200_000

fn upper_volunteering_rows() -> Vec<(i64, i64)> {
    vec![(0, 0), (3_000_000, 100_000), (4_000_000, 200_000)]
}

#[test]
fn upper_volunteering_negative_value_returns_error() {
    // 음수 봉사시간(불가 입력) → 어떤 threshold도 통과 못 함 → Err
    assert!(lookup_range_score(-1, &upper_volunteering_rows(), MatchMode::Upper).is_err());
}

#[test]
fn upper_volunteering_exactly_0_hours() {
    // 봉사 0시간 → threshold 0 매칭 → 0점
    assert_eq!(lookup_range_score(0, &upper_volunteering_rows(), MatchMode::Upper).unwrap(), 0);
}

#[test]
fn upper_volunteering_below_30_hours() {
    // 봉사 29시간 → threshold 0만 매칭, max=0 → 0점
    assert_eq!(lookup_range_score(2_900_000, &upper_volunteering_rows(), MatchMode::Upper).unwrap(), 0);
}

#[test]
fn upper_volunteering_exactly_30_hours() {
    // 봉사 30시간 → threshold 0, 3_000_000 매칭, max=3_000_000 → 1점
    assert_eq!(lookup_range_score(3_000_000, &upper_volunteering_rows(), MatchMode::Upper).unwrap(), 100_000);
}

#[test]
fn upper_volunteering_between_30_and_40_hours() {
    // 봉사 35시간 → threshold 0, 3_000_000 매칭, max=3_000_000 → 1점
    assert_eq!(lookup_range_score(3_500_000, &upper_volunteering_rows(), MatchMode::Upper).unwrap(), 100_000);
}

#[test]
fn upper_volunteering_exactly_40_hours() {
    // 봉사 40시간 → 모든 threshold 매칭, max=4_000_000 → 2점
    assert_eq!(lookup_range_score(4_000_000, &upper_volunteering_rows(), MatchMode::Upper).unwrap(), 200_000);
}

#[test]
fn upper_volunteering_above_40_hours() {
    // 봉사 50시간 → 모든 threshold 매칭, max=4_000_000 → 2점
    assert_eq!(lookup_range_score(5_000_000, &upper_volunteering_rows(), MatchMode::Upper).unwrap(), 200_000);
}

// ── lookup_range_score: 결석일수(LOWER) 시나리오 ──────────────────
//
// 기준표: 0일→10점, 1일→9점, 2일→8점, 3일→7점, 4일→6점, 5일 이상→5점
// DB 저장값(×100_000): threshold 0→1_000_000, 100_000→900_000, ..., 500_000→500_000

fn lower_absence_rows() -> Vec<(i64, i64)> {
    vec![
        (        0, 1_000_000),
        (  100_000,   900_000),
        (  200_000,   800_000),
        (  300_000,   700_000),
        (  400_000,   600_000),
        (  500_000,   500_000),
    ]
}

#[test]
fn lower_absence_negative_value_uses_min_threshold() {
    // 음수 결석(불가 입력) → 모든 threshold 통과, min=0 → 10점
    assert_eq!(lookup_range_score(-1, &lower_absence_rows(), MatchMode::Lower).unwrap(), 1_000_000);
}

#[test]
fn lower_absence_exactly_0_days() {
    // 결석 0일 → 모든 threshold 통과, min=0 → 10점
    assert_eq!(lookup_range_score(0, &lower_absence_rows(), MatchMode::Lower).unwrap(), 1_000_000);
}

#[test]
fn lower_absence_exactly_1_day() {
    // 결석 1일 → threshold 1,2,3,4,5 통과, min=1 → 9점
    assert_eq!(lookup_range_score(100_000, &lower_absence_rows(), MatchMode::Lower).unwrap(), 900_000);
}

#[test]
fn lower_absence_exactly_3_days() {
    // 결석 3일 → threshold 3,4,5 통과, min=3 → 7점
    assert_eq!(lookup_range_score(300_000, &lower_absence_rows(), MatchMode::Lower).unwrap(), 700_000);
}

#[test]
fn lower_absence_exactly_5_days() {
    // 결석 5일 → threshold 5만 통과, min=5 → 5점
    assert_eq!(lookup_range_score(500_000, &lower_absence_rows(), MatchMode::Lower).unwrap(), 500_000);
}

#[test]
fn lower_absence_6_days_fallback_to_last_score() {
    // 결석 6일 → 어떤 threshold도 통과 못 함(6 <= 5? No) → fallback: max threshold=5 → 5점
    // "5일 이상이면 5점" 동작의 핵심 케이스
    assert_eq!(lookup_range_score(600_000, &lower_absence_rows(), MatchMode::Lower).unwrap(), 500_000);
}

#[test]
fn lower_absence_far_above_max_threshold_fallback() {
    // 결석 100일(크게 초과) → fallback → 5점
    assert_eq!(lookup_range_score(10_000_000, &lower_absence_rows(), MatchMode::Lower).unwrap(), 500_000);
}

// ── calc_area_score ───────────────────────────────────────────────

async fn insert_student(pool: &sqlx::SqlitePool) -> i64 {
    sqlx::query(
        "INSERT INTO students (student_code, name, is_enrolled, grad_year) \
         VALUES ('S001', '홍길동', 0, 2024)",
    )
    .execute(pool)
    .await
    .unwrap()
    .last_insert_rowid()
}

async fn insert_area(
    pool: &sqlx::SqlitePool,
    calc_type: CalcType,
    direction: Option<MatchMode>,
    agg: Option<CategoryAgg>,
    scope: LookupScope,
) -> i64 {
    // CATEGORY는 복수값 허용(multi_value=1), 그 외 단일값(0)
    let multi_value = if calc_type == CalcType::Category { 1i64 } else { 0i64 };
    sqlx::query(
        "INSERT INTO areas (name, max_score, calc_type, match_mode, category_agg, lookup_scope, multi_value) \
         VALUES ('TestArea', 100000, ?, ?, ?, ?, ?)",
    )
    .bind(calc_type)
    .bind(direction)
    .bind(agg)
    .bind(scope)
    .bind(multi_value)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_rowid()
}

fn dummy_ctx() -> StudentTrackCtx {
    StudentTrackCtx {
        student_code: "S001".to_string(),
        student_name: "홍길동".to_string(),
        univ_name: "한국대".to_string(),
        track_name: "컴퓨터공학".to_string(),
    }
}

async fn insert_university(pool: &sqlx::SqlitePool) -> i64 {
    let univ_id: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name) VALUES ('한국대') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name, unit_quota) VALUES (?, '컴퓨터공학', 5) RETURNING id",
    )
    .bind(univ_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn calc_range_simple_upper() {
    let pool = common::create_test_pool().await;
    let sid = insert_student(&pool).await;
    let aid = insert_area(&pool, CalcType::Numeric, Some(MatchMode::Upper), None, LookupScope::Simple).await;

    for (th, sc) in [(100_000i64, 50_000i64), (200_000, 30_000), (300_000, 10_000)] {
        sqlx::query(
            "INSERT INTO numeric_table (area_id, track_id, threshold, score) VALUES (?, NULL, ?, ?)",
        )
        .bind(aid)
        .bind(th)
        .bind(sc)
        .execute(&pool)
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO base_data (student_id, area_id, track_id, value) VALUES (?, ?, NULL, '125000')",
    )
    .bind(sid)
    .bind(aid)
    .execute(&pool)
    .await
    .unwrap();

    let area = AreaRow {
        id: aid,
        name: "TestArea".to_string(),
        calc_type: CalcType::Numeric,
        max_score: 100_000,
        match_mode: Some(MatchMode::Upper),
        category_agg: None,
        lookup_scope: LookupScope::Simple,
    };
    assert_eq!(calc_area_score(&mut pool.acquire().await.unwrap(), sid, &area, 0, &dummy_ctx()).await.unwrap(), 50_000);
}

#[tokio::test]
async fn calc_range_simple_lower() {
    let pool = common::create_test_pool().await;
    let sid = insert_student(&pool).await;
    let aid = insert_area(&pool, CalcType::Numeric, Some(MatchMode::Lower), None, LookupScope::Simple).await;

    for (th, sc) in [(100_000i64, 50_000i64), (200_000, 30_000), (300_000, 10_000)] {
        sqlx::query(
            "INSERT INTO numeric_table (area_id, track_id, threshold, score) VALUES (?, NULL, ?, ?)",
        )
        .bind(aid)
        .bind(th)
        .bind(sc)
        .execute(&pool)
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO base_data (student_id, area_id, track_id, value) VALUES (?, ?, NULL, '150000')",
    )
    .bind(sid)
    .bind(aid)
    .execute(&pool)
    .await
    .unwrap();

    let area = AreaRow {
        id: aid,
        name: "TestArea".to_string(),
        calc_type: CalcType::Numeric,
        max_score: 100_000,
        match_mode: Some(MatchMode::Lower),
        category_agg: None,
        lookup_scope: LookupScope::Simple,
    };
    assert_eq!(calc_area_score(&mut pool.acquire().await.unwrap(), sid, &area, 0, &dummy_ctx()).await.unwrap(), 30_000);
}

#[tokio::test]
async fn calc_range_composite() {
    let pool = common::create_test_pool().await;
    let sid = insert_student(&pool).await;
    let uid = insert_university(&pool).await;
    let aid = insert_area(&pool, CalcType::Numeric, Some(MatchMode::Upper), None, LookupScope::Composite).await;

    sqlx::query(
        "INSERT INTO numeric_table (area_id, track_id, threshold, score) VALUES (?, ?, 100000, 80000)",
    )
    .bind(aid)
    .bind(uid)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO base_data (student_id, area_id, track_id, value) VALUES (?, ?, ?, '150000')",
    )
    .bind(sid)
    .bind(aid)
    .bind(uid)
    .execute(&pool)
    .await
    .unwrap();

    let area = AreaRow {
        id: aid,
        name: "TestArea".to_string(),
        calc_type: CalcType::Numeric,
        max_score: 100_000,
        match_mode: Some(MatchMode::Upper),
        category_agg: None,
        lookup_scope: LookupScope::Composite,
    };
    assert_eq!(calc_area_score(&mut pool.acquire().await.unwrap(), sid, &area, uid, &dummy_ctx()).await.unwrap(), 80_000);
}

#[tokio::test]
async fn calc_category_sum() {
    let pool = common::create_test_pool().await;
    let sid = insert_student(&pool).await;
    let aid = insert_area(&pool, CalcType::Category, None, Some(CategoryAgg::Sum), LookupScope::Simple).await;

    for (cat, sc) in [("회장", 30_000i64), ("봉사", 20_000)] {
        sqlx::query(
            "INSERT INTO category_map (area_id, track_id, category, score) VALUES (?, NULL, ?, ?)",
        )
        .bind(aid)
        .bind(cat)
        .bind(sc)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO base_data (student_id, area_id, track_id, value, multi_value) VALUES (?, ?, NULL, ?, 1)",
        )
        .bind(sid)
        .bind(aid)
        .bind(cat)
        .execute(&pool)
        .await
        .unwrap();
    }

    let area = AreaRow {
        id: aid,
        name: "TestArea".to_string(),
        calc_type: CalcType::Category,
        max_score: 100_000,
        match_mode: None,
        category_agg: Some(CategoryAgg::Sum),
        lookup_scope: LookupScope::Simple,
    };
    assert_eq!(calc_area_score(&mut pool.acquire().await.unwrap(), sid, &area, 0, &dummy_ctx()).await.unwrap(), 50_000);
}

#[tokio::test]
async fn calc_category_max() {
    let pool = common::create_test_pool().await;
    let sid = insert_student(&pool).await;
    let aid = insert_area(&pool, CalcType::Category, None, Some(CategoryAgg::Max), LookupScope::Simple).await;

    for (cat, sc) in [("회장", 30_000i64), ("부회장", 20_000)] {
        sqlx::query(
            "INSERT INTO category_map (area_id, track_id, category, score) VALUES (?, NULL, ?, ?)",
        )
        .bind(aid)
        .bind(cat)
        .bind(sc)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO base_data (student_id, area_id, track_id, value, multi_value) VALUES (?, ?, NULL, ?, 1)",
        )
        .bind(sid)
        .bind(aid)
        .bind(cat)
        .execute(&pool)
        .await
        .unwrap();
    }

    let area = AreaRow {
        id: aid,
        name: "TestArea".to_string(),
        calc_type: CalcType::Category,
        max_score: 100_000,
        match_mode: None,
        category_agg: Some(CategoryAgg::Max),
        lookup_scope: LookupScope::Simple,
    };
    assert_eq!(calc_area_score(&mut pool.acquire().await.unwrap(), sid, &area, 0, &dummy_ctx()).await.unwrap(), 30_000);
}

#[tokio::test]
async fn calc_manual() {
    let pool = common::create_test_pool().await;
    let sid = insert_student(&pool).await;
    let aid = insert_area(&pool, CalcType::Manual, None, None, LookupScope::Simple).await;

    sqlx::query(
        "INSERT INTO base_data (student_id, area_id, track_id, value) VALUES (?, ?, NULL, '75000')",
    )
    .bind(sid)
    .bind(aid)
    .execute(&pool)
    .await
    .unwrap();

    let area = AreaRow {
        id: aid,
        name: "TestArea".to_string(),
        calc_type: CalcType::Manual,
        max_score: 100_000,
        match_mode: None,
        category_agg: None,
        lookup_scope: LookupScope::Simple,
    };
    assert_eq!(calc_area_score(&mut pool.acquire().await.unwrap(), sid, &area, 0, &dummy_ctx()).await.unwrap(), 75_000);
}

#[tokio::test]
async fn calc_no_base_data_returns_error() {
    // base_data가 없는 학생 → 관리자가 반드시 데이터를 입력해야 함 → Err
    let pool = common::create_test_pool().await;
    let sid = insert_student(&pool).await;
    let aid = insert_area(&pool, CalcType::Numeric, Some(MatchMode::Upper), None, LookupScope::Simple).await;

    sqlx::query(
        "INSERT INTO numeric_table (area_id, track_id, threshold, score) VALUES (?, NULL, 100000, 50000)",
    )
    .bind(aid)
    .execute(&pool)
    .await
    .unwrap();

    let area = AreaRow {
        id: aid,
        name: "TestArea".to_string(),
        calc_type: CalcType::Numeric,
        max_score: 100_000,
        match_mode: Some(MatchMode::Upper),
        category_agg: None,
        lookup_scope: LookupScope::Simple,
    };
    assert!(calc_area_score(&mut pool.acquire().await.unwrap(), sid, &area, 0, &dummy_ctx()).await.is_err());
}

#[tokio::test]
async fn calc_category_sum_capped_at_max_score() {
    // 합산이 max_score를 초과할 때 max_score로 상한 처리되는지 검증
    let pool = common::create_test_pool().await;
    let sid = insert_student(&pool).await;
    let aid = insert_area(&pool, CalcType::Category, None, Some(CategoryAgg::Sum), LookupScope::Simple).await;
    // insert_area는 max_score=100_000으로 삽입. 세 항목 합산=110_000 > 100_000

    for (cat, sc) in [("회장", 50_000i64), ("부회장", 40_000), ("임원", 20_000)] {
        sqlx::query(
            "INSERT INTO category_map (area_id, track_id, category, score) VALUES (?, NULL, ?, ?)",
        )
        .bind(aid).bind(cat).bind(sc).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO base_data (student_id, area_id, track_id, value, multi_value) VALUES (?, ?, NULL, ?, 1)",
        )
        .bind(sid).bind(aid).bind(cat).execute(&pool).await.unwrap();
    }

    let area = AreaRow {
        id: aid,
        name: "TestArea".to_string(),
        calc_type: CalcType::Category,
        max_score: 100_000,
        match_mode: None,
        category_agg: Some(CategoryAgg::Sum),
        lookup_scope: LookupScope::Simple,
    };
    assert_eq!(calc_area_score(&mut pool.acquire().await.unwrap(), sid, &area, 0, &dummy_ctx()).await.unwrap(), 100_000);
}

#[tokio::test]
async fn calc_range_lower_above_max_threshold_uses_last_score() {
    // "결석 5일 이상: 5점" 케이스 — value가 최대 threshold를 초과해도 최대 threshold 점수 반환
    let pool = common::create_test_pool().await;
    let sid = insert_student(&pool).await;
    let aid = insert_area(&pool, CalcType::Numeric, Some(MatchMode::Lower), None, LookupScope::Simple).await;

    for (th, sc) in [(0i64, 100_000i64), (10_000, 80_000), (50_000, 50_000)] {
        sqlx::query(
            "INSERT INTO numeric_table (area_id, track_id, threshold, score) VALUES (?, NULL, ?, ?)",
        )
        .bind(aid).bind(th).bind(sc).execute(&pool).await.unwrap();
    }
    // value=70_000 은 최대 threshold(50_000)를 초과 → 50_000점 반환 기대
    sqlx::query(
        "INSERT INTO base_data (student_id, area_id, track_id, value) VALUES (?, ?, NULL, '70000')",
    )
    .bind(sid).bind(aid).execute(&pool).await.unwrap();

    let area = AreaRow {
        id: aid,
        name: "TestArea".to_string(),
        calc_type: CalcType::Numeric,
        max_score: 100_000,
        match_mode: Some(MatchMode::Lower),
        category_agg: None,
        lookup_scope: LookupScope::Simple,
    };
    assert_eq!(calc_area_score(&mut pool.acquire().await.unwrap(), sid, &area, 0, &dummy_ctx()).await.unwrap(), 50_000);
}

#[tokio::test]
async fn calc_manual_capped_at_max_score() {
    // MANUAL 값이 max_score를 초과할 때 max_score로 상한 처리
    let pool = common::create_test_pool().await;
    let sid = insert_student(&pool).await;
    let aid = insert_area(&pool, CalcType::Manual, None, None, LookupScope::Simple).await;
    // insert_area: max_score=100_000, 저장값 200_000 > 100_000 → 100_000 반환 기대
    sqlx::query(
        "INSERT INTO base_data (student_id, area_id, track_id, value) VALUES (?, ?, NULL, '200000')",
    )
    .bind(sid).bind(aid).execute(&pool).await.unwrap();

    let area = AreaRow {
        id: aid,
        name: "TestArea".to_string(),
        calc_type: CalcType::Manual,
        max_score: 100_000,
        match_mode: None,
        category_agg: None,
        lookup_scope: LookupScope::Simple,
    };
    assert_eq!(calc_area_score(&mut pool.acquire().await.unwrap(), sid, &area, 0, &dummy_ctx()).await.unwrap(), 100_000);
}

#[tokio::test]
async fn calc_range_upper_capped_at_max_score() {
    // RANGE UPPER: numeric_table score가 max_score를 초과할 때 상한 처리
    let pool = common::create_test_pool().await;
    let sid = insert_student(&pool).await;
    let aid = insert_area(&pool, CalcType::Numeric, Some(MatchMode::Upper), None, LookupScope::Simple).await;
    // insert_area: max_score=100_000, 구간표 점수=200_000 > 100_000 → 100_000 반환 기대
    sqlx::query(
        "INSERT INTO numeric_table (area_id, track_id, threshold, score) VALUES (?, NULL, 100000, 200000)",
    )
    .bind(aid).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO base_data (student_id, area_id, track_id, value) VALUES (?, ?, NULL, '100000')",
    )
    .bind(sid).bind(aid).execute(&pool).await.unwrap();

    let area = AreaRow {
        id: aid,
        name: "TestArea".to_string(),
        calc_type: CalcType::Numeric,
        max_score: 100_000,
        match_mode: Some(MatchMode::Upper),
        category_agg: None,
        lookup_scope: LookupScope::Simple,
    };
    assert_eq!(calc_area_score(&mut pool.acquire().await.unwrap(), sid, &area, 0, &dummy_ctx()).await.unwrap(), 100_000);
}

#[tokio::test]
async fn calc_category_max_capped_at_max_score() {
    // CATEGORY MAX: 단일 항목이 max_score를 초과할 때 상한 처리
    let pool = common::create_test_pool().await;
    let sid = insert_student(&pool).await;
    let aid = insert_area(&pool, CalcType::Category, None, Some(CategoryAgg::Max), LookupScope::Simple).await;

    sqlx::query(
        "INSERT INTO category_map (area_id, track_id, category, score) VALUES (?, NULL, '회장', 200000)",
    )
    .bind(aid).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO base_data (student_id, area_id, track_id, value, multi_value) VALUES (?, ?, NULL, '회장', 1)",
    )
    .bind(sid).bind(aid).execute(&pool).await.unwrap();

    let area = AreaRow {
        id: aid,
        name: "TestArea".to_string(),
        calc_type: CalcType::Category,
        max_score: 100_000,
        match_mode: None,
        category_agg: Some(CategoryAgg::Max),
        lookup_scope: LookupScope::Simple,
    };
    assert_eq!(calc_area_score(&mut pool.acquire().await.unwrap(), sid, &area, 0, &dummy_ctx()).await.unwrap(), 100_000);
}

// ── 감점 전형요소 (음수 점수) ─────────────────────────────────────

#[tokio::test]
async fn calc_category_deduction_returns_negative_score() {
    // CATEGORY SUM: 감점 범주(음수 점수)에 해당하는 학생 → 음수 결과
    // 일반 학생은 base_data 없음 → 0점, 위반 학생은 범주 매핑 → 감점
    let pool = common::create_test_pool().await;
    let sid = insert_student(&pool).await;
    let aid = insert_area(&pool, CalcType::Category, None, Some(CategoryAgg::Sum), LookupScope::Simple).await;

    sqlx::query(
        "INSERT INTO category_map (area_id, track_id, category, score) VALUES (?, NULL, '규정위반', -300000)",
    )
    .bind(aid).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO base_data (student_id, area_id, track_id, value, multi_value) VALUES (?, ?, NULL, '규정위반', 1)",
    )
    .bind(sid).bind(aid).execute(&pool).await.unwrap();

    let area = AreaRow {
        id: aid, name: "TestArea".to_string(), calc_type: CalcType::Category, max_score: 1_000_000,
        match_mode: None, category_agg: Some(CategoryAgg::Sum), lookup_scope: LookupScope::Simple,
    };
    let score = calc_area_score(&mut pool.acquire().await.unwrap(), sid, &area, 0, &dummy_ctx()).await.unwrap();
    assert_eq!(score, -300_000, "감점 범주: -3.0점 → -300000");
}

#[tokio::test]
async fn calc_category_no_base_data_returns_error() {
    // base_data 없음 → 위반 여부 불명, 관리자가 명시적으로 0을 입력해야 함 → Err
    let pool = common::create_test_pool().await;
    let sid = insert_student(&pool).await;
    let aid = insert_area(&pool, CalcType::Category, None, Some(CategoryAgg::Sum), LookupScope::Simple).await;

    sqlx::query(
        "INSERT INTO category_map (area_id, track_id, category, score) VALUES (?, NULL, '규정위반', -300000)",
    )
    .bind(aid).execute(&pool).await.unwrap();
    // base_data 없음

    let area = AreaRow {
        id: aid, name: "TestArea".to_string(), calc_type: CalcType::Category, max_score: 1_000_000,
        match_mode: None, category_agg: Some(CategoryAgg::Sum), lookup_scope: LookupScope::Simple,
    };
    assert!(calc_area_score(&mut pool.acquire().await.unwrap(), sid, &area, 0, &dummy_ctx()).await.is_err());
}

#[tokio::test]
async fn calc_manual_deduction_returns_negative_score() {
    // MANUAL: 음수 값 직접 입력 → 음수 점수 반환
    let pool = common::create_test_pool().await;
    let sid = insert_student(&pool).await;
    let aid = insert_area(&pool, CalcType::Manual, None, None, LookupScope::Simple).await;

    sqlx::query(
        "INSERT INTO base_data (student_id, area_id, track_id, value) VALUES (?, ?, NULL, '-500000')",
    )
    .bind(sid).bind(aid).execute(&pool).await.unwrap();

    let area = AreaRow {
        id: aid, name: "TestArea".to_string(), calc_type: CalcType::Manual, max_score: 1_000_000,
        match_mode: None, category_agg: None, lookup_scope: LookupScope::Simple,
    };
    assert_eq!(calc_area_score(&mut pool.acquire().await.unwrap(), sid, &area, 0, &dummy_ctx()).await.unwrap(), -500_000, "-5.0점 → -500000");
}

#[tokio::test]
async fn calc_pure_deduction_area_max_score_zero() {
    // 순수 감점 전형요소: max_score=0, 위반 학생은 음수 점수, 일반 학생은 0점
    // raw.min(0): 음수는 그대로 통과, 양수가 있다면 0으로 상한
    let pool = common::create_test_pool().await;
    let sid = insert_student(&pool).await;

    // max_score=0 으로 전형요소 직접 삽입 (insert_area는 100_000 고정이라 직접 삽입)
    let aid: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, category_agg, lookup_scope, multi_value) \
         VALUES ('감점전형', 0, 'CATEGORY', 'SUM', 'SIMPLE', 1) RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();

    sqlx::query(
        "INSERT INTO category_map (area_id, track_id, category, score) VALUES (?, NULL, '위반', -500000)",
    )
    .bind(aid).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO base_data (student_id, area_id, track_id, value, multi_value) VALUES (?, ?, NULL, '위반', 1)",
    )
    .bind(sid).bind(aid).execute(&pool).await.unwrap();

    let area = AreaRow {
        id: aid, name: "TestArea".to_string(), calc_type: CalcType::Category, max_score: 0,
        match_mode: None, category_agg: Some(CategoryAgg::Sum), lookup_scope: LookupScope::Simple,
    };
    // min(-500000, 0) = -500000 → 감점 보존
    assert_eq!(calc_area_score(&mut pool.acquire().await.unwrap(), sid, &area, 0, &dummy_ctx()).await.unwrap(), -500_000);
}

#[tokio::test]
async fn calc_pure_deduction_area_no_base_data_returns_error() {
    // 순수 감점 전형요소라도 base_data 없으면 위반 여부 불명 → Err
    let pool = common::create_test_pool().await;
    let sid = insert_student(&pool).await;

    let aid: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, category_agg, lookup_scope, multi_value) \
         VALUES ('감점전형', 0, 'CATEGORY', 'SUM', 'SIMPLE', 1) RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();

    sqlx::query(
        "INSERT INTO category_map (area_id, track_id, category, score) VALUES (?, NULL, '위반', -500000)",
    )
    .bind(aid).execute(&pool).await.unwrap();
    // base_data 없음 → Err

    let area = AreaRow {
        id: aid, name: "TestArea".to_string(), calc_type: CalcType::Category, max_score: 0,
        match_mode: None, category_agg: Some(CategoryAgg::Sum), lookup_scope: LookupScope::Simple,
    };
    assert!(calc_area_score(&mut pool.acquire().await.unwrap(), sid, &area, 0, &dummy_ctx()).await.is_err());
}

#[tokio::test]
async fn calc_deduction_does_not_cap_at_max_score() {
    // 감점 결과는 max_score 상한에 걸리지 않아야 함
    // raw=-300000, max_score=1000000 → min(-300000, 1000000) = -300000 (감점 유지)
    let pool = common::create_test_pool().await;
    let sid = insert_student(&pool).await;
    let aid = insert_area(&pool, CalcType::Category, None, Some(CategoryAgg::Sum), LookupScope::Simple).await;

    sqlx::query(
        "INSERT INTO category_map (area_id, track_id, category, score) VALUES (?, NULL, '위반', -300000)",
    )
    .bind(aid).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO base_data (student_id, area_id, track_id, value, multi_value) VALUES (?, ?, NULL, '위반', 1)",
    )
    .bind(sid).bind(aid).execute(&pool).await.unwrap();

    let area = AreaRow {
        id: aid, name: "TestArea".to_string(), calc_type: CalcType::Category, max_score: 1_000_000,
        match_mode: None, category_agg: Some(CategoryAgg::Sum), lookup_scope: LookupScope::Simple,
    };
    assert_eq!(calc_area_score(&mut pool.acquire().await.unwrap(), sid, &area, 0, &dummy_ctx()).await.unwrap(), -300_000);
}

#[tokio::test]
async fn calc_range_exact_match_hit() {
    // EXACT: 값이 threshold와 정확히 일치 → 해당 점수 반환
    let pool = common::create_test_pool().await;
    let sid = insert_student(&pool).await;
    let aid = insert_area(&pool, CalcType::Numeric, Some(MatchMode::Exact), None, LookupScope::Simple).await;

    for (th, sc) in [(100_000i64, 50_000i64), (200_000, 30_000), (300_000, 10_000)] {
        sqlx::query(
            "INSERT INTO numeric_table (area_id, track_id, threshold, score) VALUES (?, NULL, ?, ?)",
        )
        .bind(aid).bind(th).bind(sc).execute(&pool).await.unwrap();
    }
    sqlx::query(
        "INSERT INTO base_data (student_id, area_id, track_id, value) VALUES (?, ?, NULL, '200000')",
    )
    .bind(sid).bind(aid).execute(&pool).await.unwrap();

    let area = AreaRow {
        id: aid,
        name: "TestArea".to_string(),
        calc_type: CalcType::Numeric,
        max_score: 100_000,
        match_mode: Some(MatchMode::Exact),
        category_agg: None,
        lookup_scope: LookupScope::Simple,
    };
    assert_eq!(calc_area_score(&mut pool.acquire().await.unwrap(), sid, &area, 0, &dummy_ctx()).await.unwrap(), 30_000);
}

#[tokio::test]
async fn calc_range_exact_match_miss_returns_error() {
    // EXACT: 일치하는 threshold 없음 → Err
    let pool = common::create_test_pool().await;
    let sid = insert_student(&pool).await;
    let aid = insert_area(&pool, CalcType::Numeric, Some(MatchMode::Exact), None, LookupScope::Simple).await;

    for (th, sc) in [(100_000i64, 50_000i64), (200_000, 30_000)] {
        sqlx::query(
            "INSERT INTO numeric_table (area_id, track_id, threshold, score) VALUES (?, NULL, ?, ?)",
        )
        .bind(aid).bind(th).bind(sc).execute(&pool).await.unwrap();
    }
    // 150_000은 구간표에 없음
    sqlx::query(
        "INSERT INTO base_data (student_id, area_id, track_id, value) VALUES (?, ?, NULL, '150000')",
    )
    .bind(sid).bind(aid).execute(&pool).await.unwrap();

    let area = AreaRow {
        id: aid,
        name: "TestArea".to_string(),
        calc_type: CalcType::Numeric,
        max_score: 100_000,
        match_mode: Some(MatchMode::Exact),
        category_agg: None,
        lookup_scope: LookupScope::Simple,
    };
    assert!(calc_area_score(&mut pool.acquire().await.unwrap(), sid, &area, 0, &dummy_ctx()).await.is_err());
}

// ── calculate_scores: 라운드 상태 검증 ────────────────────────────

#[tokio::test]
async fn calculate_scores_open_round_returns_bad_request() {
    // OPEN 상태 라운드 → 점수 계산 불가 (CLOSED에서만 허용)
    let pool = common::create_test_pool().await;
    let (_, _, rid) = setup_full(&pool).await;
    // 라운드는 OPEN 상태로 setup_full에서 생성됨
    let res = calculate_scores(State(common::make_state(pool)), Path(rid)).await;
    assert_eq!(res.unwrap_err().0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn calculate_scores_finalized_round_returns_bad_request() {
    // FINALIZED 상태 라운드도 점수 재계산 불가
    let pool = common::create_test_pool().await;
    let (_, _, rid) = setup_full(&pool).await;
    sqlx::query("UPDATE rounds SET status = 'FINALIZED', closed_at = '2025-01-02', finalized_at = '2025-01-03' WHERE id = ?")
        .bind(rid)
        .execute(&pool)
        .await
        .unwrap();
    let res = calculate_scores(State(common::make_state(pool)), Path(rid)).await;
    assert_eq!(res.unwrap_err().0, StatusCode::BAD_REQUEST);
}

// ── calc_area_score: 새 Err 케이스 ────────────────────────────────

#[tokio::test]
async fn calc_numeric_parse_error_returns_error() {
    // NUMERIC: base_data 값이 정수로 파싱 불가 → Err
    let pool = common::create_test_pool().await;
    let sid = insert_student(&pool).await;
    let aid = insert_area(&pool, CalcType::Numeric, Some(MatchMode::Upper), None, LookupScope::Simple).await;

    sqlx::query(
        "INSERT INTO numeric_table (area_id, track_id, threshold, score) VALUES (?, NULL, 100000, 50000)",
    )
    .bind(aid).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO base_data (student_id, area_id, track_id, value) VALUES (?, ?, NULL, 'abc')",
    )
    .bind(sid).bind(aid).execute(&pool).await.unwrap();

    let area = AreaRow {
        id: aid,
        name: "TestArea".to_string(),
        calc_type: CalcType::Numeric,
        max_score: 100_000,
        match_mode: Some(MatchMode::Upper),
        category_agg: None,
        lookup_scope: LookupScope::Simple,
    };
    assert!(calc_area_score(&mut pool.acquire().await.unwrap(), sid, &area, 0, &dummy_ctx()).await.is_err());
}

#[tokio::test]
async fn calc_category_unknown_category_returns_error() {
    // CATEGORY: base_data에 category_map에 없는 범주 → Err
    let pool = common::create_test_pool().await;
    let sid = insert_student(&pool).await;
    let aid = insert_area(&pool, CalcType::Category, None, Some(CategoryAgg::Sum), LookupScope::Simple).await;

    sqlx::query(
        "INSERT INTO category_map (area_id, track_id, category, score) VALUES (?, NULL, '회장', 30000)",
    )
    .bind(aid).execute(&pool).await.unwrap();
    // base_data에 category_map에 없는 '부회장' 입력
    sqlx::query(
        "INSERT INTO base_data (student_id, area_id, track_id, value, multi_value) VALUES (?, ?, NULL, '부회장', 1)",
    )
    .bind(sid).bind(aid).execute(&pool).await.unwrap();

    let area = AreaRow {
        id: aid,
        name: "TestArea".to_string(),
        calc_type: CalcType::Category,
        max_score: 100_000,
        match_mode: None,
        category_agg: Some(CategoryAgg::Sum),
        lookup_scope: LookupScope::Simple,
    };
    assert!(calc_area_score(&mut pool.acquire().await.unwrap(), sid, &area, 0, &dummy_ctx()).await.is_err());
}

#[tokio::test]
async fn calc_category_missing_agg_returns_error() {
    // CATEGORY: category_agg = None이면 집계 방식 불명 → Err
    let pool = common::create_test_pool().await;
    let sid = insert_student(&pool).await;
    // DB에는 Sum으로 저장, AreaRow만 None으로 테스트
    let aid = insert_area(&pool, CalcType::Category, None, Some(CategoryAgg::Sum), LookupScope::Simple).await;

    sqlx::query(
        "INSERT INTO category_map (area_id, track_id, category, score) VALUES (?, NULL, '회장', 30000)",
    )
    .bind(aid).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO base_data (student_id, area_id, track_id, value, multi_value) VALUES (?, ?, NULL, '회장', 1)",
    )
    .bind(sid).bind(aid).execute(&pool).await.unwrap();

    let area = AreaRow {
        id: aid,
        name: "TestArea".to_string(),
        calc_type: CalcType::Category,
        max_score: 100_000,
        match_mode: None,
        category_agg: None, // 집계 방식 미설정
        lookup_scope: LookupScope::Simple,
    };
    assert!(calc_area_score(&mut pool.acquire().await.unwrap(), sid, &area, 0, &dummy_ctx()).await.is_err());
}

#[tokio::test]
async fn calc_manual_parse_error_returns_error() {
    // MANUAL: base_data 값이 정수로 파싱 불가 → Err
    let pool = common::create_test_pool().await;
    let sid = insert_student(&pool).await;
    let aid = insert_area(&pool, CalcType::Manual, None, None, LookupScope::Simple).await;

    sqlx::query(
        "INSERT INTO base_data (student_id, area_id, track_id, value) VALUES (?, ?, NULL, '3.14')",
    )
    .bind(sid).bind(aid).execute(&pool).await.unwrap();

    let area = AreaRow {
        id: aid,
        name: "TestArea".to_string(),
        calc_type: CalcType::Manual,
        max_score: 100_000,
        match_mode: None,
        category_agg: None,
        lookup_scope: LookupScope::Simple,
    };
    assert!(calc_area_score(&mut pool.acquire().await.unwrap(), sid, &area, 0, &dummy_ctx()).await.is_err());
}

// ── calculate_scores 통합 ─────────────────────────────────────────

async fn setup_full(pool: &sqlx::SqlitePool) -> (i64, i64, i64) {
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

#[tokio::test]
async fn calculate_scores_nonexistent_round_returns_not_found() {
    let pool = common::create_test_pool().await;
    let res = calculate_scores(State(common::make_state(pool)), Path(9999i64)).await;
    assert_eq!(res.unwrap_err().0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn calculate_scores_no_applications_returns_zero_count() {
    let pool = common::create_test_pool().await;
    let (_, _, rid) = setup_full(&pool).await;
    sqlx::query("UPDATE rounds SET status = 'CLOSED', closed_at = '2025-01-02T00:00:00Z' WHERE id = ?")
        .bind(rid).execute(&pool).await.unwrap();
    let axum::Json(result) =
        calculate_scores(State(common::make_state(pool)), Path(rid)).await.unwrap();
    assert_eq!(result["calculated"], 0);
}

#[tokio::test]
async fn calculate_scores_creates_result_rows_and_ranking() {
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup_full(&pool).await;

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

    sqlx::query("UPDATE rounds SET status = 'CLOSED', closed_at = '2025-01-02T00:00:00Z' WHERE id = ?")
        .bind(rid).execute(&pool).await.unwrap();

    let axum::Json(result) =
        calculate_scores(State(common::make_state(pool.clone())), Path(rid)).await.unwrap();
    assert_eq!(result["calculated"], 1);

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM results WHERE round_id = ?")
        .bind(rid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);

    let ranking: Option<i64> =
        sqlx::query_scalar("SELECT ranking FROM results WHERE round_id = ?")
            .bind(rid)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(ranking, Some(1));
}

#[tokio::test]
async fn calculate_scores_ranks_higher_score_first() {
    let pool = common::create_test_pool().await;
    let hash = bcrypt::hash("pass", 4u32).unwrap();
    sqlx::query("INSERT INTO classes (grade, class_no, password_hash) VALUES (1, 1, ?)")
        .bind(&hash)
        .execute(&pool)
        .await
        .unwrap();

    let sid1: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES ('S001', '홍길동', 1, 1, 1, 1) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let sid2: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES ('S002', '이순신', 1, 1, 2, 1) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let univ_id: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name) VALUES ('한국대') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name, unit_quota) VALUES (?, '컴공', 5) RETURNING id",
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

    let aid: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, lookup_scope) \
         VALUES ('수동점수', 1000000, 'MANUAL', 'SIMPLE') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO base_data (student_id, area_id, track_id, value) VALUES (?, ?, NULL, '800000')",
    )
    .bind(sid1)
    .bind(aid)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO base_data (student_id, area_id, track_id, value) VALUES (?, ?, NULL, '600000')",
    )
    .bind(sid2)
    .bind(aid)
    .execute(&pool)
    .await
    .unwrap();

    for sid in [sid1, sid2] {
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
    }

    sqlx::query("UPDATE rounds SET status = 'CLOSED', closed_at = '2025-01-02T00:00:00Z' WHERE id = ?")
        .bind(rid).execute(&pool).await.unwrap();

    let _ = calculate_scores(State(common::make_state(pool.clone())), Path(rid)).await.unwrap();

    let rank1: Option<i64> =
        sqlx::query_scalar("SELECT ranking FROM results WHERE student_id = ? AND round_id = ?")
            .bind(sid1)
            .bind(rid)
            .fetch_one(&pool)
            .await
            .unwrap();
    let rank2: Option<i64> =
        sqlx::query_scalar("SELECT ranking FROM results WHERE student_id = ? AND round_id = ?")
            .bind(sid2)
            .bind(rid)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(rank1, Some(1));
    assert_eq!(rank2, Some(2));
}

// ── recommend_result ──────────────────────────────────────────────

#[tokio::test]
async fn recommend_on_open_round_returns_bad_request() {
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup_full(&pool).await;
    let res = recommend_result(State(common::make_state(pool)), Path((sid, tid, rid))).await;
    assert_eq!(res.unwrap_err().0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn recommend_on_closed_round_sets_flag() {
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup_full(&pool).await;

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

    sqlx::query(
        "UPDATE rounds SET status = 'CLOSED', closed_at = '2025-01-02T00:00:00Z' WHERE id = ?",
    )
    .bind(rid)
    .execute(&pool)
    .await
    .unwrap();

    let _ = calculate_scores(State(common::make_state(pool.clone())), Path(rid)).await.unwrap();

    recommend_result(State(common::make_state(pool.clone())), Path((sid, tid, rid)))
        .await
        .unwrap();

    let recommended: i64 = sqlx::query_scalar(
        "SELECT recommended FROM results WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(sid)
    .bind(tid)
    .bind(rid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(recommended, 1);
}

// ── export_results: score_detail Fail-Fast ────────────────────────

/// results 행 직접 삽입 헬퍼 (calculate_scores 우회)
async fn insert_result_raw(pool: &sqlx::SqlitePool, sid: i64, tid: i64, rid: i64, score_detail: &str) {
    sqlx::query(
        "INSERT INTO results (student_id, track_id, round_id, total_score, score_detail, ranking, recommended, calculated_at) \
         VALUES (?, ?, ?, 0, ?, 1, 0, '2025-01-01T00:00:00Z')",
    )
    .bind(sid).bind(tid).bind(rid).bind(score_detail)
    .execute(pool).await.unwrap();
}

#[tokio::test]
async fn export_results_corrupt_score_detail_returns_500() {
    // score_detail이 유효하지 않은 JSON인 경우 → 500 반환 (silent fallback 금지)
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup_full(&pool).await;

    sqlx::query(
        "INSERT INTO applications (student_id, track_id, round_id, abandoned) VALUES (?, ?, ?, 0)",
    )
    .bind(sid).bind(tid).bind(rid).execute(&pool).await.unwrap();

    insert_result_raw(&pool, sid, tid, rid, "{invalid json").await;

    let err = export_results(State(common::make_state(pool)), Path(rid))
        .await
        .unwrap_err();
    assert_eq!(err.0, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(err.1.contains("score_detail"), "오류 메시지에 score_detail 포함 기대: {}", err.1);
}

#[tokio::test]
async fn export_results_area_missing_from_score_detail_returns_500() {
    // area가 DB에 있지만 score_detail에 해당 area_id가 없는 경우 → 500 반환
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup_full(&pool).await;

    sqlx::query(
        "INSERT INTO applications (student_id, track_id, round_id, abandoned) VALUES (?, ?, ?, 0)",
    )
    .bind(sid).bind(tid).bind(rid).execute(&pool).await.unwrap();

    // area를 하나 추가
    let aid: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, lookup_scope) \
         VALUES ('테스트전형요소', 100000, 'MANUAL', 'SIMPLE') RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();

    // score_detail에는 존재하지 않는 area_id(99999)의 점수만 있음
    insert_result_raw(&pool, sid, tid, rid, r#"{"99999": 100000}"#).await;

    let err = export_results(State(common::make_state(pool)), Path(rid))
        .await
        .unwrap_err();
    assert_eq!(err.0, StatusCode::INTERNAL_SERVER_ERROR);
    let msg = err.1;
    assert!(
        msg.contains("점수가 없습니다") || msg.contains(&aid.to_string()),
        "오류 메시지에 area 정보 포함 기대: {}", msg
    );
}

// ── score_detail_as_map: 직렬화 Fail-Fast ────────────────────────

#[test]
fn result_row_corrupt_score_detail_serialization_fails() {
    // score_detail이 유효하지 않은 JSON이면 serde 직렬화 자체가 실패해야 함
    let row = ResultRow {
        student_id: 1,
        track_id: 1,
        round_id: 1,
        total_score: Score::from_raw(0),
        score_detail: "{not valid json".to_string(),
        ranking: None,
        track_rank: None,
        recommended: false,
        abandoned: false,
        excluded: false,
        excluded_reason: None,
        student_code: "S001".to_string(),
        name: "홍길동".to_string(),
        grade: Some(1),
        class_no: Some(1),
        seq_no: Some(1),
        is_enrolled: true,
        univ_name: "한국대".to_string(),
        track_name: "컴공".to_string(),
        department_name: "컴퓨터공학과".to_string(),
    };
    assert!(
        serde_json::to_string(&row).is_err(),
        "유효하지 않은 score_detail은 직렬화 실패를 반환해야 함"
    );
}

#[test]
fn result_row_valid_score_detail_serializes_correctly() {
    // 정상 score_detail은 올바르게 직렬화됨
    let row = ResultRow {
        student_id: 1,
        track_id: 1,
        round_id: 1,
        total_score: Score::from_raw(500_000),
        score_detail: r#"{"1": 300000, "2": 200000}"#.to_string(),
        ranking: Some(1),
        track_rank: None,
        recommended: false,
        abandoned: false,
        excluded: false,
        excluded_reason: None,
        student_code: "S001".to_string(),
        name: "홍길동".to_string(),
        grade: Some(1),
        class_no: Some(1),
        seq_no: Some(1),
        is_enrolled: true,
        univ_name: "한국대".to_string(),
        track_name: "컴공".to_string(),
        department_name: "컴퓨터공학과".to_string(),
    };
    let json = serde_json::to_string(&row).expect("정상 score_detail은 직렬화 성공 기대");
    // score_detail은 Score 타입으로 역변환되어 표시값(÷100000)으로 직렬화됨
    assert!(json.contains("\"1\":3.0") || json.contains("\"1\": 3.0"));
}

// ── recommend_result: 정원 체크 ───────────────────────────────────

/// setup_full과 동일하지만 unit_quota/total_quota를 파라미터로 받는 헬퍼
async fn setup_with_quota(
    pool: &sqlx::SqlitePool,
    unit_quota: Option<i64>,
    total_quota: Option<i64>,
) -> (i64, i64, i64) {
    let hash = bcrypt::hash("pass", 4u32).unwrap();
    sqlx::query("INSERT INTO classes (grade, class_no, password_hash) VALUES (1, 1, ?)")
        .bind(&hash).execute(pool).await.unwrap();
    let sid: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES ('SQ01', '테스트', 1, 1, 1, 1) RETURNING id",
    ).fetch_one(pool).await.unwrap();
    let univ_id: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name, total_quota) VALUES ('quota대', ?) RETURNING id",
    ).bind(total_quota).fetch_one(pool).await.unwrap();
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name, unit_quota) VALUES (?, '테스트학과', ?) RETURNING id",
    ).bind(univ_id).bind(unit_quota).fetch_one(pool).await.unwrap();
    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at) VALUES ('CLOSED', '2025-01-01T00:00:00Z') RETURNING id",
    ).fetch_one(pool).await.unwrap();
    sqlx::query(
        "INSERT INTO applications (student_id, track_id, round_id, abandoned) VALUES (?, ?, ?, 0)",
    ).bind(sid).bind(tid).bind(rid).execute(pool).await.unwrap();
    sqlx::query(
        "INSERT INTO results (student_id, track_id, round_id, score_detail, total_score, ranking, recommended, calculated_at) \
         VALUES (?, ?, ?, '{}', 0, 1, 0, '2025-01-01T00:00:00Z')",
    ).bind(sid).bind(tid).bind(rid).execute(pool).await.unwrap();
    (sid, tid, rid)
}

/// 이미 추천 확정된 더미 학생을 n명 추가 삽입하는 헬퍼
async fn insert_n_recommended(
    pool: &sqlx::SqlitePool,
    tid: i64,
    rid: i64,
    n: usize,
) {
    for i in 0..n {
        let code = format!("DUMMY{:04}", i);
        let sid: i64 = sqlx::query_scalar(
            "INSERT INTO students (student_code, name, is_enrolled, grad_year) VALUES (?, '더미', 0, 2020) RETURNING id",
        ).bind(&code).fetch_one(pool).await.unwrap();
        sqlx::query(
            "INSERT INTO applications (student_id, track_id, round_id, abandoned) VALUES (?, ?, ?, 0)",
        ).bind(sid).bind(tid).bind(rid).execute(pool).await.unwrap();
        sqlx::query(
            "INSERT INTO results (student_id, track_id, round_id, score_detail, total_score, ranking, recommended, calculated_at) \
             VALUES (?, ?, ?, '{}', 0, NULL, 1, '2025-01-01T00:00:00Z')",
        ).bind(sid).bind(tid).bind(rid).execute(pool).await.unwrap();
    }
}

#[tokio::test]
async fn recommend_returns_conflict_when_unit_quota_full() {
    // 모집단위 정원(unit_quota=1)이 이미 찼을 때 → 409 CONFLICT
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup_with_quota(&pool, Some(1), None).await;
    insert_n_recommended(&pool, tid, rid, 1).await;

    let res = recommend_result(State(common::make_state(pool)), Path((sid, tid, rid))).await;
    assert_eq!(res.unwrap_err().0, StatusCode::CONFLICT);
}

#[tokio::test]
async fn recommend_returns_conflict_when_total_quota_full() {
    // 대학 전체 정원(total_quota=1)이 이미 찼을 때 → 409 CONFLICT
    let pool = common::create_test_pool().await;
    // unit_quota=None(무제한), total_quota=1
    let (sid, tid, rid) = setup_with_quota(&pool, None, Some(1)).await;
    insert_n_recommended(&pool, tid, rid, 1).await;

    let res = recommend_result(State(common::make_state(pool)), Path((sid, tid, rid))).await;
    assert_eq!(res.unwrap_err().0, StatusCode::CONFLICT);
}

#[tokio::test]
async fn recommend_succeeds_when_within_quota() {
    // unit_quota=2, total_quota=3, 현재 1명 추천 확정 → 추가 추천 가능
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup_with_quota(&pool, Some(2), Some(3)).await;
    insert_n_recommended(&pool, tid, rid, 1).await;

    let res = recommend_result(State(common::make_state(pool)), Path((sid, tid, rid))).await;
    assert_eq!(res.unwrap(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn recommend_abandoned_student_does_not_count_toward_quota() {
    // 포기(abandoned=1)한 추천 확정 학생은 잔여석에서 제외(자리 반환)
    // unit_quota=1, 포기 1명이 있으면 잔여석=1 → 새 추천 가능
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup_with_quota(&pool, Some(1), None).await;

    // 포기 학생(recommended=1, abandoned=1) 추가
    let abnd_sid: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, is_enrolled, grad_year) VALUES ('ABND0001', '포기자', 0, 2020) RETURNING id",
    ).fetch_one(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO applications (student_id, track_id, round_id, abandoned) VALUES (?, ?, ?, 1)",
    ).bind(abnd_sid).bind(tid).bind(rid).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO results (student_id, track_id, round_id, score_detail, total_score, ranking, recommended, calculated_at) \
         VALUES (?, ?, ?, '{}', 0, NULL, 1, '2025-01-01T00:00:00Z')",
    ).bind(abnd_sid).bind(tid).bind(rid).execute(&pool).await.unwrap();

    // 포기자가 있어도 unit_quota=1 자리 남아 있음 → 추천 성공 기대
    let res = recommend_result(State(common::make_state(pool)), Path((sid, tid, rid))).await;
    assert_eq!(res.unwrap(), StatusCode::NO_CONTENT);
}

// ── unrecommend_result ────────────────────────────────────────────

#[tokio::test]
async fn unrecommend_on_closed_round_clears_flag() {
    // CLOSED 상태에서 recommended=1 → unrecommend → recommended=0
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup_with_quota(&pool, None, None).await;

    // 먼저 추천 확정
    recommend_result(State(common::make_state(pool.clone())), Path((sid, tid, rid)))
        .await
        .unwrap();
    let rec_before: i64 =
        sqlx::query_scalar("SELECT recommended FROM results WHERE student_id = ? AND round_id = ?")
            .bind(sid)
            .bind(rid)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(rec_before, 1);

    // 추천 취소
    unrecommend_result(State(common::make_state(pool.clone())), Path((sid, tid, rid)))
        .await
        .unwrap();

    let rec_after: i64 =
        sqlx::query_scalar("SELECT recommended FROM results WHERE student_id = ? AND round_id = ?")
            .bind(sid)
            .bind(rid)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(rec_after, 0);
}

#[tokio::test]
async fn unrecommend_on_open_round_returns_bad_request() {
    // OPEN 상태에서는 추천 취소 불가
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup_full(&pool).await;
    // applications → results 순으로 삽입 (FK 제약 준수)
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
    sqlx::query(
        "INSERT INTO results \
         (student_id, track_id, round_id, score_detail, total_score, ranking, recommended, calculated_at) \
         VALUES (?, ?, ?, '{}', 0, 1, 1, '2025-01-01T00:00:00Z')",
    )
    .bind(sid)
    .bind(tid)
    .bind(rid)
    .execute(&pool)
    .await
    .unwrap();
    let res = unrecommend_result(State(common::make_state(pool)), Path((sid, tid, rid))).await;
    assert_eq!(res.unwrap_err().0, StatusCode::BAD_REQUEST);
}

/// 존재하지 않는 (sid,tid,rid) 조합으로 호출 시 404를 반환한다 — recommend_result와 대칭.
/// 대칭화 전에는 UPDATE가 rows_affected=0으로 조용히 성공하고 spurious RecommendCanceled
/// 감사 로그가 남았다 (2차 감사 C 발견 O-1, 소유자 라운드 #7).
#[tokio::test]
async fn unrecommend_nonexistent_result_returns_not_found() {
    let pool = common::create_test_pool().await;
    let (_sid, _tid, rid) = setup_with_quota(&pool, None, None).await;

    // 존재하지 않는 (sid, tid) 조합을 CLOSED 라운드에 대해 unrecommend 시도.
    // 대칭화 전에는 rows_affected=0으로 UPDATE가 조용히 성공해 spurious 감사 로그가 남았다.
    let phantom_sid = 999_999;
    let phantom_tid = 999_999;
    let res = unrecommend_result(State(common::make_state(pool.clone())), Path((phantom_sid, phantom_tid, rid))).await;
    assert_eq!(res.unwrap_err().0, StatusCode::NOT_FOUND);

    // spurious 감사 로그가 남지 않았는지 확인
    let audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE action = 'RecommendCanceled'"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(audit_count, 0, "존재하지 않는 조합에 spurious 감사 로그가 남으면 안 됨");
}

#[tokio::test]
async fn unrecommend_on_finalized_round_returns_bad_request() {
    // FINALIZED 상태에서는 추천 취소 불가
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup_with_quota(&pool, None, None).await;
    // trg_require_all_decided_before_finalize: CLOSED→FINALIZED 직접 SQL은 미결정이 없어야 통과.
    // setup_with_quota가 recommended=0으로 삽입하므로 먼저 결정 완료 상태로 전환한다.
    sqlx::query("UPDATE results SET recommended = 1 WHERE student_id = ? AND round_id = ?")
        .bind(sid).bind(rid).execute(&pool).await.unwrap();
    sqlx::query("UPDATE rounds SET status = 'FINALIZED', finalized_at = '2025-01-03T00:00:00Z' WHERE id = ?")
        .bind(rid)
        .execute(&pool)
        .await
        .unwrap();
    let res = unrecommend_result(State(common::make_state(pool)), Path((sid, tid, rid))).await;
    assert_eq!(res.unwrap_err().0, StatusCode::BAD_REQUEST);
}

// ── teacher_get_results: 졸업생 담당 케이스 ───────────────────────

/// FINALIZED 라운드에 졸업생 결과를 삽입하고 반환값을 검증하는 헬퍼
async fn setup_grad_result(pool: &sqlx::SqlitePool) -> (i64, i64, i64) {
    let grad_sid: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, is_enrolled, grad_year) \
         VALUES ('G001', '졸업생', 0, 2024) RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();

    let univ_id: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name) VALUES ('졸업생대') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();

    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name, unit_quota) VALUES (?, '국문', 3) RETURNING id",
    )
    .bind(univ_id)
    .fetch_one(pool)
    .await
    .unwrap();

    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at, finalized_at) \
         VALUES ('FINALIZED', '2025-01-01T00:00:00Z', '2025-01-10T00:00:00Z') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO applications (student_id, track_id, round_id, abandoned) \
         VALUES (?, ?, ?, 0)",
    )
    .bind(grad_sid)
    .bind(tid)
    .bind(rid)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO results \
         (student_id, track_id, round_id, score_detail, total_score, ranking, recommended, calculated_at) \
         VALUES (?, ?, ?, '{}', 0, 1, 1, '2025-01-09T00:00:00Z')",
    )
    .bind(grad_sid)
    .bind(tid)
    .bind(rid)
    .execute(pool)
    .await
    .unwrap();

    (grad_sid, tid, rid)
}

use axum::Extension;

#[tokio::test]
async fn teacher_get_results_grad_teacher_sees_graduated_students() {
    // 졸업생 담당(grade=0, class_no=0)은 is_enrolled=0 학생의 결과를 조회할 수 있어야 함
    let pool = common::create_test_pool().await;
    let (grad_sid, _, rid) = setup_grad_result(&pool).await;

    let res = teacher_get_results(
        State(common::make_state(pool.clone())),
        Extension(common::teacher_claims(0, 0)),
    )
    .await
    .unwrap();

    assert_eq!(res.0.rounds.len(), 1);
    assert_eq!(res.0.results.len(), 1);
    assert_eq!(res.0.results[0].student_id, grad_sid);
    assert_eq!(res.0.results[0].round_id, rid);
}

#[tokio::test]
async fn teacher_get_results_regular_teacher_does_not_see_graduated_students() {
    // 일반 담임(grade=1, class_no=1)은 졸업생 결과를 볼 수 없어야 함
    let pool = common::create_test_pool().await;
    setup_grad_result(&pool).await;

    let res = teacher_get_results(
        State(common::make_state(pool)),
        Extension(common::teacher_claims(1, 1)),
    )
    .await
    .unwrap();

    // 라운드는 존재하지만 결과 행은 0개여야 함
    assert_eq!(res.0.rounds.len(), 1);
    assert_eq!(res.0.results.len(), 0);
}

/// 담임 결과 조회의 학급 격리. 졸업생 배제 테스트만으로는 `class_no` 필터가
/// 소실되는 회귀를 잡을 수 없다 — 졸업생은 grade 가 NULL 이라 grade 조건만
/// 남아도 계속 배제되기 때문이다. 같은 학년의 다른 학급을 픽스처에 넣어
/// 타 학급 학생의 점수·순위가 새지 않는지 확인한다.
#[tokio::test]
async fn teacher_get_results_excludes_other_class_same_grade() {
    let pool = common::create_test_pool().await;
    let (_grad_sid, tid, rid) = setup_grad_result(&pool).await;

    // 1학년 1반 / 1학년 2반 학생 각 1명 — 같은 모집단위, 같은 라운드
    let hash = bcrypt::hash("pass", 4u32).unwrap();
    for (grade, class_no) in [(1, 1), (1, 2)] {
        sqlx::query("INSERT INTO classes (grade, class_no, password_hash) VALUES (?, ?, ?)")
            .bind(grade).bind(class_no).bind(&hash)
            .execute(&pool).await.unwrap();
    }
    let mut sids = Vec::new();
    for (code, name, class_no) in [("S001", "홍길동", 1), ("S002", "이순신", 2)] {
        let sid: i64 = sqlx::query_scalar(
            "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
             VALUES (?, ?, 1, ?, 1, 1) RETURNING id",
        )
        .bind(code).bind(name).bind(class_no)
        .fetch_one(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO applications (student_id, track_id, round_id, abandoned) VALUES (?, ?, ?, 0)",
        )
        .bind(sid).bind(tid).bind(rid).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO results \
             (student_id, track_id, round_id, score_detail, total_score, ranking, recommended, calculated_at) \
             VALUES (?, ?, ?, '{}', 0, 1, 0, '2025-01-09T00:00:00Z')",
        )
        .bind(sid).bind(tid).bind(rid).execute(&pool).await.unwrap();
        sids.push(sid);
    }

    let res = teacher_get_results(
        State(common::make_state(pool)),
        Extension(common::teacher_claims(1, 1)),
    )
    .await
    .unwrap();

    assert_eq!(
        res.0.results.len(), 1,
        "1반 담임에게는 1반 학생 1명만 보여야 함: {:?}",
        res.0.results.iter().map(|r| (&r.name, r.class_no)).collect::<Vec<_>>(),
    );
    assert_eq!(res.0.results[0].student_id, sids[0], "1반 학생이어야 함");
    assert_eq!(res.0.results[0].class_no, Some(1));
}

// ── Score 산술 연산 ───────────────────────────────────────────────

#[test]
fn score_add_normal() {
    let a = Score::from_raw(100_000);
    let b = Score::from_raw(200_000);
    assert_eq!((a + b).raw(), 300_000);
}

#[test]
fn score_add_zero() {
    let a = Score::from_raw(500_000);
    assert_eq!((a + Score::from_raw(0)).raw(), 500_000);
}

#[test]
fn score_sum_empty() {
    let total: Score = std::iter::empty::<Score>().sum();
    assert_eq!(total.raw(), 0);
}

#[test]
fn score_sum_multiple() {
    let scores = vec![
        Score::from_raw(100_000),
        Score::from_raw(200_000),
        Score::from_raw(300_000),
    ];
    let total: Score = scores.into_iter().sum();
    assert_eq!(total.raw(), 600_000);
}

#[test]
#[should_panic(expected = "Score overflow in Add")]
fn score_add_overflow_panics() {
    let a = Score::from_raw(i64::MAX);
    let b = Score::from_raw(1);
    let _ = a + b;
}

#[test]
#[should_panic(expected = "Score overflow in Sum")]
fn score_sum_overflow_panics() {
    let scores = vec![Score::from_raw(i64::MAX), Score::from_raw(1)];
    let _: Score = scores.into_iter().sum();
}

// ── 대학 전체 순위(univ 파티션) 테스트 ───────────────────────────────

/// 학생 삽입 헬퍼 (is_enrolled=1)
async fn insert_enrolled_student(pool: &sqlx::SqlitePool, code: &str, seq: i64) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES (?, '테스트', 1, 1, ?, 1) RETURNING id",
    )
    .bind(code).bind(seq)
    .fetch_one(pool).await.unwrap()
}

#[tokio::test]
async fn univ_ranking_crosses_track_boundary() {
    // 한국대 2트랙: 전자학생(800점) vs 컴공학생(600점). 같은 대학이므로 전자1위, 컴공2위
    let pool = common::create_test_pool().await;
    let hash = bcrypt::hash("pass", 4u32).unwrap();
    sqlx::query("INSERT INTO classes (grade, class_no, password_hash) VALUES (1, 1, ?)")
        .bind(&hash).execute(&pool).await.unwrap();
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name) VALUES ('한국대') RETURNING id",
    ).fetch_one(&pool).await.unwrap();
    let tid_cs: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name, unit_quota) VALUES (?, '컴공', 5) RETURNING id",
    ).bind(uid).fetch_one(&pool).await.unwrap();
    let tid_ee: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name, unit_quota) VALUES (?, '전자', 5) RETURNING id",
    ).bind(uid).fetch_one(&pool).await.unwrap();
    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at) VALUES ('OPEN', '2025-01-01T00:00:00Z') RETURNING id",
    ).fetch_one(&pool).await.unwrap();
    let area_id: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, lookup_scope) \
         VALUES ('수동점수', 1000000, 'MANUAL', 'SIMPLE') RETURNING id",
    ).fetch_one(&pool).await.unwrap();

    let sid_cs = insert_enrolled_student(&pool, "S001", 1).await;
    let sid_ee = insert_enrolled_student(&pool, "S002", 2).await;

    sqlx::query("INSERT INTO base_data (student_id, area_id, track_id, value) VALUES (?, ?, NULL, '600000')")
        .bind(sid_cs).bind(area_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO base_data (student_id, area_id, track_id, value) VALUES (?, ?, NULL, '800000')")
        .bind(sid_ee).bind(area_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO applications (student_id, track_id, round_id, abandoned) VALUES (?, ?, ?, 0)")
        .bind(sid_cs).bind(tid_cs).bind(rid).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO applications (student_id, track_id, round_id, abandoned) VALUES (?, ?, ?, 0)")
        .bind(sid_ee).bind(tid_ee).bind(rid).execute(&pool).await.unwrap();

    sqlx::query("UPDATE rounds SET status = 'CLOSED', closed_at = '2025-01-02T00:00:00Z' WHERE id = ?")
        .bind(rid).execute(&pool).await.unwrap();

    let _ = calculate_scores(State(common::make_state(pool.clone())), Path(rid)).await.unwrap();

    let rank_cs: Option<i64> = sqlx::query_scalar(
        "SELECT ranking FROM results WHERE student_id = ? AND round_id = ?",
    ).bind(sid_cs).bind(rid).fetch_one(&pool).await.unwrap();
    let rank_ee: Option<i64> = sqlx::query_scalar(
        "SELECT ranking FROM results WHERE student_id = ? AND round_id = ?",
    ).bind(sid_ee).bind(rid).fetch_one(&pool).await.unwrap();

    assert_eq!(rank_ee, Some(1), "전자(높은 점수)가 대학 전체 1위여야 함");
    assert_eq!(rank_cs, Some(2), "컴공(낮은 점수)이 대학 전체 2위여야 함");
}

#[tokio::test]
async fn univ_ranking_ties_get_same_rank() {
    // 두 학생이 같은 점수 → 동점이므로 같은 순위, 다음 순위는 건너뜀
    let pool = common::create_test_pool().await;
    let hash = bcrypt::hash("pass", 4u32).unwrap();
    sqlx::query("INSERT INTO classes (grade, class_no, password_hash) VALUES (1, 1, ?)")
        .bind(&hash).execute(&pool).await.unwrap();
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name) VALUES ('한국대') RETURNING id",
    ).fetch_one(&pool).await.unwrap();
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name) VALUES (?, '컴공') RETURNING id",
    ).bind(uid).fetch_one(&pool).await.unwrap();
    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at) VALUES ('OPEN', '2025-01-01') RETURNING id",
    ).fetch_one(&pool).await.unwrap();
    let area_id: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, lookup_scope) \
         VALUES ('점수', 1000000, 'MANUAL', 'SIMPLE') RETURNING id",
    ).fetch_one(&pool).await.unwrap();

    let sid1 = insert_enrolled_student(&pool, "S001", 1).await;
    let sid2 = insert_enrolled_student(&pool, "S002", 2).await;
    let sid3 = insert_enrolled_student(&pool, "S003", 3).await;

    let same_score = 700_000i64;
    let low_score  = 500_000i64;
    for (sid, sc) in [(sid1, same_score), (sid2, same_score), (sid3, low_score)] {
        sqlx::query("INSERT INTO base_data (student_id, area_id, track_id, value) VALUES (?, ?, NULL, ?)")
            .bind(sid).bind(area_id).bind(sc.to_string()).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO applications (student_id, track_id, round_id, abandoned) VALUES (?, ?, ?, 0)")
            .bind(sid).bind(tid).bind(rid).execute(&pool).await.unwrap();
    }
    sqlx::query("UPDATE rounds SET status = 'CLOSED', closed_at = '2025-01-02T00:00:00Z' WHERE id = ?")
        .bind(rid).execute(&pool).await.unwrap();

    let _ = calculate_scores(State(common::make_state(pool.clone())), Path(rid)).await.unwrap();

    let r1: Option<i64> = sqlx::query_scalar("SELECT ranking FROM results WHERE student_id = ? AND round_id = ?")
        .bind(sid1).bind(rid).fetch_one(&pool).await.unwrap();
    let r2: Option<i64> = sqlx::query_scalar("SELECT ranking FROM results WHERE student_id = ? AND round_id = ?")
        .bind(sid2).bind(rid).fetch_one(&pool).await.unwrap();
    let r3: Option<i64> = sqlx::query_scalar("SELECT ranking FROM results WHERE student_id = ? AND round_id = ?")
        .bind(sid3).bind(rid).fetch_one(&pool).await.unwrap();

    assert_eq!(r1, Some(1), "동점 1위");
    assert_eq!(r2, Some(1), "동점 1위 (같은 점수)");
    assert_eq!(r3, Some(3), "3위 (1,2위 건너뜀 — 표준 경쟁 순위)");
}

#[tokio::test]
async fn get_results_returns_track_rank() {
    // get_results 응답에 track_rank 필드가 포함되어야 함
    let pool = common::create_test_pool().await;
    let hash = bcrypt::hash("pass", 4u32).unwrap();
    sqlx::query("INSERT INTO classes (grade, class_no, password_hash) VALUES (1, 1, ?)")
        .bind(&hash).execute(&pool).await.unwrap();
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name) VALUES ('한국대') RETURNING id",
    ).fetch_one(&pool).await.unwrap();
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name) VALUES (?, '컴공') RETURNING id",
    ).bind(uid).fetch_one(&pool).await.unwrap();
    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at) VALUES ('OPEN', '2025-01-01') RETURNING id",
    ).fetch_one(&pool).await.unwrap();
    let area_id: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, lookup_scope) \
         VALUES ('점수', 1000000, 'MANUAL', 'SIMPLE') RETURNING id",
    ).fetch_one(&pool).await.unwrap();

    let sid1 = insert_enrolled_student(&pool, "S001", 1).await;
    let sid2 = insert_enrolled_student(&pool, "S002", 2).await;

    for (sid, sc) in [(sid1, 800_000i64), (sid2, 600_000i64)] {
        sqlx::query("INSERT INTO base_data (student_id, area_id, track_id, value) VALUES (?, ?, NULL, ?)")
            .bind(sid).bind(area_id).bind(sc.to_string()).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO applications (student_id, track_id, round_id, abandoned) VALUES (?, ?, ?, 0)")
            .bind(sid).bind(tid).bind(rid).execute(&pool).await.unwrap();
    }
    sqlx::query("UPDATE rounds SET status = 'CLOSED', closed_at = '2025-01-02T00:00:00Z' WHERE id = ?")
        .bind(rid).execute(&pool).await.unwrap();

    let _ = calculate_scores(State(common::make_state(pool.clone())), Path(rid)).await.unwrap();

    let axum::Json(rows) = get_results(
        State(common::make_state(pool)),
        Path(rid),
        Query(ResultQuery { track_id: None }),
    ).await.unwrap();

    assert!(rows.iter().all(|r| r.track_rank.is_some()), "모든 행에 track_rank가 있어야 함");
    let row1 = rows.iter().find(|r| r.student_id == sid1).unwrap();
    let row2 = rows.iter().find(|r| r.student_id == sid2).unwrap();
    assert_eq!(row1.track_rank, Some(1), "높은 점수 학생의 track_rank=1");
    assert_eq!(row2.track_rank, Some(2), "낮은 점수 학생의 track_rank=2");
}

#[tokio::test]
async fn export_results_header_contains_univ_rank() {
    // export_results 헤더에 "대학 순위"와 "모집단위 순위"가 포함되어야 함
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup_full(&pool).await;
    sqlx::query("INSERT INTO applications (student_id, track_id, round_id, abandoned) VALUES (?, ?, ?, 0)")
        .bind(sid).bind(tid).bind(rid).execute(&pool).await.unwrap();
    let area_id: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, lookup_scope) \
         VALUES ('점수', 1000000, 'MANUAL', 'SIMPLE') RETURNING id",
    ).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO base_data (student_id, area_id, track_id, value) VALUES (?, ?, NULL, '500000')")
        .bind(sid).bind(area_id).execute(&pool).await.unwrap();
    sqlx::query("UPDATE rounds SET status = 'CLOSED', closed_at = '2025-01-02T00:00:00Z' WHERE id = ?")
        .bind(rid).execute(&pool).await.unwrap();
    let _ = calculate_scores(State(common::make_state(pool.clone())), Path(rid)).await.unwrap();

    let resp = export_results(State(common::make_state(pool)), Path(rid)).await.unwrap();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let rows = principal_candidate_manager::excel::parse_xlsx_all_rows_raw(&bytes).unwrap();
    let header = &rows[0];
    assert!(header.contains(&"대학 순위".to_string()), "헤더에 '대학 순위' 포함: {:?}", header);
    assert!(header.contains(&"모집단위 순위".to_string()), "헤더에 '모집단위 순위' 포함: {:?}", header);

    // 전형요소 점수가 "어느 열 아래에" 실리는지까지 고정한다. 헤더는 areas ORDER BY id
    // 루프로, 데이터는 별도 루프로 쓰므로 두 루프의 순서 일치가 곧 정합성인데,
    // 집합 소속 단언만으로는 그 일치가 깨져도 통과한다.
    let data = &rows[1];
    let col_of = |h: &str| header.iter().position(|c| c == h)
        .unwrap_or_else(|| panic!("헤더에 '{}' 없음: {:?}", h, header));
    for (h, want) in [("점수", "5"), ("총점", "5")] {
        let c = col_of(h);
        assert_eq!(
            data.get(c).map(String::as_str), Some(want),
            "'{}' 열({})의 값이 '{}'이어야 함: {:?}", h, c, want, data,
        );
    }
}

/// `export_round_summary` 의 "지원자결과" 시트 — track_rank_window() 를 r2/ut2/s2 별칭
/// CTE로 쓰는 유일한 경로다. 이 핸들러는 리팩터링 전까지 테스트가 전혀 없어서, 헬퍼에
/// 별칭을 잘못 넘겨도(예: r/ut/s) 전 스위트가 통과했다. SQL이 실제로 실행되는지와
/// 모집단위 순위가 채워지는지를 확인한다.
#[tokio::test]
async fn export_round_summary_applicant_sheet_populates_track_rank() {
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup_full(&pool).await;

    // 같은 트랙에 2명 — 순위가 1, 2로 갈려야 파티션이 동작함을 확인할 수 있다
    let sid2: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES ('S002', '김철수', 1, 1, 2, 1) RETURNING id",
    ).fetch_one(&pool).await.unwrap();

    let area_id: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, lookup_scope) \
         VALUES ('점수', 1000000, 'MANUAL', 'SIMPLE') RETURNING id",
    ).fetch_one(&pool).await.unwrap();

    for (s, v) in [(sid, "900000"), (sid2, "400000")] {
        sqlx::query("INSERT INTO applications (student_id, track_id, round_id, abandoned) VALUES (?, ?, ?, 0)")
            .bind(s).bind(tid).bind(rid).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO base_data (student_id, area_id, track_id, value) VALUES (?, ?, NULL, ?)")
            .bind(s).bind(area_id).bind(v).execute(&pool).await.unwrap();
    }

    sqlx::query("UPDATE rounds SET status = 'CLOSED', closed_at = '2025-01-02T00:00:00Z' WHERE id = ?")
        .bind(rid).execute(&pool).await.unwrap();
    let _ = calculate_scores(State(common::make_state(pool.clone())), Path(rid)).await.unwrap();

    let resp = export_round_summary(State(common::make_state(pool)), Path(rid)).await.unwrap();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    assert!(principal_candidate_manager::excel::is_xlsx(&bytes));

    // 첫 시트가 아니라 "지원자결과" 시트를 직접 열어야 한다
    let rows = principal_candidate_manager::excel::parse_xlsx_sheet_rows(&bytes, "지원자결과").unwrap();
    let header = &rows[0];
    let rank_col = header.iter().position(|h| h == "모집단위 순위")
        .expect(&format!("헤더에 '모집단위 순위' 없음: {header:?}"));
    let name_col = header.iter().position(|h| h == "이름").expect("헤더에 '이름' 없음");

    assert_eq!(rows.len(), 3, "헤더 + 지원자 2행이어야 함: {rows:?}");
    let hong = rows[1..].iter().find(|r| r[name_col] == "홍길동").expect("홍길동 행");
    let kim  = rows[1..].iter().find(|r| r[name_col] == "김철수").expect("김철수 행");
    assert_eq!(hong[rank_col], "1", "고득점자가 모집단위 1위여야 함: {hong:?}");
    assert_eq!(kim[rank_col], "2", "저득점자가 모집단위 2위여야 함: {kim:?}");
}

/// 감점은 `compute_area_score` 단위로만 검증돼 있었고, **여러 전형요소를 합산하는
/// `calculate_scores` 경로에 감점이 섞인 총점**을 단언하는 테스트가 없었다.
/// 합산 시 감점을 0으로 clamp 하거나 부호를 뒤집는 회귀가 초록으로 통과한다.
#[tokio::test]
async fn calculate_scores_total_includes_negative_deduction_area() {
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup_full(&pool).await;
    sqlx::query("INSERT INTO applications (student_id, track_id, round_id, abandoned) VALUES (?, ?, ?, 0)")
        .bind(sid).bind(tid).bind(rid).execute(&pool).await.unwrap();

    // 가점 영역: 만점 100 → 80점 / 감점 영역: 만점 0 → -5점
    let plus: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, lookup_scope) \
         VALUES ('내신', 10000000, 'MANUAL', 'SIMPLE') RETURNING id",
    ).fetch_one(&pool).await.unwrap();
    let minus: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, lookup_scope) \
         VALUES ('감점', 0, 'MANUAL', 'SIMPLE') RETURNING id",
    ).fetch_one(&pool).await.unwrap();
    for (aid, v) in [(plus, "8000000"), (minus, "-500000")] {
        sqlx::query("INSERT INTO base_data (student_id, area_id, track_id, value) VALUES (?, ?, NULL, ?)")
            .bind(sid).bind(aid).bind(v).execute(&pool).await.unwrap();
    }

    sqlx::query("UPDATE rounds SET status = 'CLOSED', closed_at = '2025-01-02' WHERE id = ?")
        .bind(rid).execute(&pool).await.unwrap();
    let _ = calculate_scores(State(common::make_state(pool.clone())), Path(rid)).await.unwrap();

    let (total, detail): (i64, String) = sqlx::query_as(
        "SELECT total_score, score_detail FROM results WHERE round_id = ?",
    ).bind(rid).fetch_one(&pool).await.unwrap();

    // 80 + (-5) = 75 → raw 7_500_000. 감점이 0으로 잘리면 8_000_000 이 되어 실패한다.
    assert_eq!(total, 7_500_000, "총점은 가점−감점이어야 함 (detail={detail})");

    let map: serde_json::Value = serde_json::from_str(&detail).unwrap();
    assert_eq!(map[minus.to_string()], serde_json::json!(-500_000), "감점 영역은 음수로 저장: {detail}");
    assert_eq!(map[plus.to_string()], serde_json::json!(8_000_000), "가점 영역: {detail}");
}

/// `export_round_summary` 의 "라운드결과" 시트 — 관리자가 잔여석을 읽는 산출물인데
/// 지금까지 이 시트를 파싱하는 테스트가 하나도 없었다. `(q - before).max(0)` /
/// `(q - before - this).max(0)` 산술이 조용히 틀려도 전 스위트가 초록이었다.
#[tokio::test]
async fn export_round_summary_track_sheet_computes_remaining_seats() {
    let pool = common::create_test_pool().await;
    let hash = bcrypt::hash("pass", 4u32).unwrap();
    sqlx::query("INSERT INTO classes (grade, class_no, password_hash) VALUES (1, 1, ?)")
        .bind(&hash).execute(&pool).await.unwrap();

    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name, total_quota) VALUES ('한국대', 3) RETURNING id",
    ).fetch_one(&pool).await.unwrap();
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name, unit_quota) VALUES (?, '컴공', 2) RETURNING id",
    ).bind(uid).fetch_one(&pool).await.unwrap();

    let r1: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at, finalized_at) \
         VALUES ('FINALIZED', '2025-01-01', '2025-01-05') RETURNING id",
    ).fetch_one(&pool).await.unwrap();
    let r2: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at, closed_at) \
         VALUES ('CLOSED', '2025-02-01', '2025-02-05') RETURNING id",
    ).fetch_one(&pool).await.unwrap();

    // (학번, 라운드, recommended, abandoned)
    //  이전 라운드 1명 확정 → before_count=1 / 이번 라운드 1명 확정 → this_count=1
    //  포기자는 어느 쪽에도 세지 않아야 한다
    for (seq, (code, rid, rec, aband)) in [
        ("S001", r1, 1, 0),
        ("S002", r2, 1, 0),
        ("S003", r2, 1, 1),
    ].into_iter().enumerate()
    {
        // seq_no 는 학급 내 유일해야 한다 (idx_students_position)
        let sid: i64 = sqlx::query_scalar(
            "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
             VALUES (?, '학생', 1, 1, ?, 1) RETURNING id",
        ).bind(code).bind(seq as i64 + 1).fetch_one(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO applications (student_id, track_id, round_id, abandoned) VALUES (?, ?, ?, ?)",
        ).bind(sid).bind(tid).bind(rid).bind(aband).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO results (student_id, track_id, round_id, score_detail, total_score, \
             ranking, recommended, calculated_at) VALUES (?, ?, ?, '{}', 0, 1, ?, '2025-01-09')",
        ).bind(sid).bind(tid).bind(rid).bind(rec).execute(&pool).await.unwrap();
    }

    let resp = export_round_summary(State(common::make_state(pool)), Path(r2)).await.unwrap();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let rows = principal_candidate_manager::excel::parse_xlsx_sheet_rows(&bytes, "라운드결과").unwrap();

    let header = &rows[0];
    let col_of = |h: &str| header.iter().position(|c| c == h)
        .unwrap_or_else(|| panic!("헤더에 '{}' 없음: {:?}", h, header));
    let data = &rows[1];

    // 트랙 정원 2, 이전 확정 1 → 라운드 전 잔여 1, 이번 1명 → 남은 잔여 0
    // 대학 정원 3, 이전 확정 1 → 라운드 전 잔여 2, 이번 1명 → 남은 잔여 1
    for (h, want) in [
        ("모집단위 정원", "2"),
        ("모집단위 라운드 전 잔여석", "1"),
        ("이번 라운드 추천 인원", "1"),
        ("모집단위 남은 잔여석", "0"),
        ("대학 전체 정원", "3"),
        ("대학 라운드 전 잔여석", "2"),
        ("대학 남은 잔여석", "1"),
    ] {
        let c = col_of(h);
        assert_eq!(
            data.get(c).map(String::as_str), Some(want),
            "'{}' 열({})의 값이 '{}'이어야 함: {:?}", h, c, want, data,
        );
    }
}

/// 정원 무제한(NULL)은 0 이 아니라 "무제한" 문자열로 나가야 한다.
#[tokio::test]
async fn export_round_summary_unlimited_quota_renders_as_text() {
    let pool = common::create_test_pool().await;
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name) VALUES ('무제한대') RETURNING id",
    ).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO univ_tracks (univ_id, track_name) VALUES (?, '자유전공')")
        .bind(uid).execute(&pool).await.unwrap();
    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at, closed_at) \
         VALUES ('CLOSED', '2025-01-01', '2025-01-05') RETURNING id",
    ).fetch_one(&pool).await.unwrap();

    let resp = export_round_summary(State(common::make_state(pool)), Path(rid)).await.unwrap();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let rows = principal_candidate_manager::excel::parse_xlsx_sheet_rows(&bytes, "라운드결과").unwrap();
    let header = &rows[0];
    let col_of = |h: &str| header.iter().position(|c| c == h).unwrap();
    let data = &rows[1];

    for h in [
        "모집단위 정원", "모집단위 라운드 전 잔여석", "모집단위 남은 잔여석",
        "대학 전체 정원", "대학 라운드 전 잔여석", "대학 남은 잔여석",
    ] {
        assert_eq!(data[col_of(h)], "무제한", "'{h}' 열은 '무제한' 이어야 함: {data:?}");
    }
    assert_eq!(data[col_of("이번 라운드 추천 인원")], "0", "추천 인원은 무제한이어도 숫자");
}

/// 내보내기 용어가 화면·매뉴얼의 "미선발"과 일치해야 한다.
/// 화면 라벨만 바꾸고 엑셀 문자열을 빠뜨리면 산출물에서만 옛 용어("제외")가 살아남는다 —
/// 실제로 그렇게 어긋난 적이 있어 헤더와 셀 값을 함께 단언한다.
#[tokio::test]
async fn export_results_uses_unselected_terminology() {
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup_full(&pool).await;
    sqlx::query("INSERT INTO applications (student_id, track_id, round_id, abandoned) VALUES (?, ?, ?, 0)")
        .bind(sid).bind(tid).bind(rid).execute(&pool).await.unwrap();
    let area_id: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, lookup_scope) \
         VALUES ('점수', 1000000, 'MANUAL', 'SIMPLE') RETURNING id",
    ).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO base_data (student_id, area_id, track_id, value) VALUES (?, ?, NULL, '500000')")
        .bind(sid).bind(area_id).execute(&pool).await.unwrap();
    sqlx::query("UPDATE rounds SET status = 'CLOSED', closed_at = '2025-01-02T00:00:00Z' WHERE id = ?")
        .bind(rid).execute(&pool).await.unwrap();
    let _ = calculate_scores(State(common::make_state(pool.clone())), Path(rid)).await.unwrap();

    // 미선발 처리 — 셀 값 확인용
    sqlx::query(
        "UPDATE applications SET excluded = 1, excluded_reason = '정원 미달' \
         WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(sid).bind(tid).bind(rid).execute(&pool).await.unwrap();

    let resp = export_results(State(common::make_state(pool)), Path(rid)).await.unwrap();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let rows = principal_candidate_manager::excel::parse_xlsx_all_rows_raw(&bytes).unwrap();

    let header = &rows[0];
    assert!(header.contains(&"미선발여부".to_string()), "헤더에 '미선발여부' 포함: {:?}", header);
    assert!(header.contains(&"미선발사유".to_string()), "헤더에 '미선발사유' 포함: {:?}", header);
    assert!(!header.iter().any(|h| h.contains("제외")), "옛 용어 '제외'가 헤더에 남아 있음: {:?}", header);

    // 값이 "어느 열 아래에" 있는지까지 단언 — 여부/사유 두 열이 뒤바뀌어도
    // 집합 소속 단언만으로는 통과한다.
    let data = &rows[1];
    let col_of = |h: &str| header.iter().position(|c| c == h)
        .unwrap_or_else(|| panic!("헤더에 '{}' 없음: {:?}", h, header));
    for (h, want) in [("미선발여부", "미선발"), ("미선발사유", "정원 미달")] {
        let c = col_of(h);
        assert_eq!(
            data.get(c).map(String::as_str), Some(want),
            "'{}' 열({})의 값이 '{}'이어야 함: {:?}", h, c, want, data,
        );
    }
}

#[tokio::test]
async fn each_scope_own_prioritize_flag_no_silent_or() {
    // 대학=재학생우선 OFF, 트랙=재학생우선 ON (같은 트랙).
    // 저장 순위(대학 전체, 대학 플래그)=점수만 → 졸업생(고점) 1위.
    // track_rank(트랙 플래그)=재학생 우선 → 재학생(저점) 1위.
    // 과거 `u OR ut` 였다면 저장 순위도 재학생 우선이 됐을 것 — 조용한 규칙 변경 금지 회귀.
    let pool = common::create_test_pool().await;
    let hash = bcrypt::hash("pass", 4u32).unwrap();
    sqlx::query("INSERT INTO classes (grade, class_no, password_hash) VALUES (1, 1, ?)")
        .bind(&hash).execute(&pool).await.unwrap();
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name, prioritize_enrolled) VALUES ('한국대', 0) RETURNING id",
    ).fetch_one(&pool).await.unwrap();
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name, prioritize_enrolled) VALUES (?, '컴공', 1) RETURNING id",
    ).bind(uid).fetch_one(&pool).await.unwrap();
    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at) VALUES ('OPEN', '2025-01-01') RETURNING id",
    ).fetch_one(&pool).await.unwrap();
    let area_id: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, lookup_scope) \
         VALUES ('점수', 1000000, 'MANUAL', 'SIMPLE') RETURNING id",
    ).fetch_one(&pool).await.unwrap();

    let sid_enr = insert_enrolled_student(&pool, "E001", 1).await;
    let sid_grad: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, is_enrolled, grad_year) \
         VALUES ('G001', '졸업', 0, 2024) RETURNING id",
    ).fetch_one(&pool).await.unwrap();

    for (sid, sc) in [(sid_enr, 600_000i64), (sid_grad, 800_000i64)] {
        sqlx::query("INSERT INTO base_data (student_id, area_id, track_id, value) VALUES (?, ?, NULL, ?)")
            .bind(sid).bind(area_id).bind(sc.to_string()).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO applications (student_id, track_id, round_id, abandoned) VALUES (?, ?, ?, 0)")
            .bind(sid).bind(tid).bind(rid).execute(&pool).await.unwrap();
    }
    sqlx::query("UPDATE rounds SET status = 'CLOSED', closed_at = '2025-01-02T00:00:00Z' WHERE id = ?")
        .bind(rid).execute(&pool).await.unwrap();

    let _ = calculate_scores(State(common::make_state(pool.clone())), Path(rid)).await.unwrap();

    let rank_grad: Option<i64> = sqlx::query_scalar(
        "SELECT ranking FROM results WHERE student_id = ? AND round_id = ?",
    ).bind(sid_grad).bind(rid).fetch_one(&pool).await.unwrap();
    let rank_enr: Option<i64> = sqlx::query_scalar(
        "SELECT ranking FROM results WHERE student_id = ? AND round_id = ?",
    ).bind(sid_enr).bind(rid).fetch_one(&pool).await.unwrap();
    assert_eq!(rank_grad, Some(1), "대학=0: 저장 순위는 점수만 → 졸업생(고점) 1위");
    assert_eq!(rank_enr, Some(2), "대학=0: 재학생(저점) 2위 (재학생 우선 미적용)");

    let axum::Json(rows) = get_results(
        State(common::make_state(pool)),
        Path(rid),
        Query(ResultQuery { track_id: None }),
    ).await.unwrap();
    let tr_enr = rows.iter().find(|r| r.student_id == sid_enr).unwrap().track_rank;
    let tr_grad = rows.iter().find(|r| r.student_id == sid_grad).unwrap().track_rank;
    assert_eq!(tr_enr, Some(1), "트랙=1: track_rank 재학생 우선 → 재학생 1위");
    assert_eq!(tr_grad, Some(2), "트랙=1: 졸업생 2위");
}

// ── score_preview (관리자 GET /api/score-preview) ──────────────────
//
// 관리자 화면의 점수 미리보기 진입점인데 이 핸들러를 호출하는 테스트가 하나도
// 없었다. 전형요소 순회·합산·per-area 응답 조립이 통째로 미검증이라, 예를 들어
// 마지막 전형요소를 빠뜨리거나 총점을 detail 과 다르게 만들어도 스위트가 초록이었다.

#[tokio::test]
async fn score_preview_returns_total_and_per_area_detail() {
    let pool = common::create_test_pool().await;
    let (sid, tid, _rid) = setup_full(&pool).await;

    // NUMERIC(UPPER): 35시간 → 30시간 구간 3점
    let num_aid: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, lookup_scope, match_mode) \
         VALUES ('봉사시간', 500000, 'NUMERIC', 'SIMPLE', 'UPPER') RETURNING id",
    ).fetch_one(&pool).await.unwrap();
    for (th, sc) in [(1_000_000i64, 100_000i64), (3_000_000, 300_000)] {
        sqlx::query("INSERT INTO numeric_table (area_id, track_id, threshold, score) VALUES (?, NULL, ?, ?)")
            .bind(num_aid).bind(th).bind(sc).execute(&pool).await.unwrap();
    }
    // MANUAL: 4.5점 그대로
    let man_aid: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, lookup_scope) \
         VALUES ('면접점수', 1000000, 'MANUAL', 'SIMPLE') RETURNING id",
    ).fetch_one(&pool).await.unwrap();

    for (aid, v) in [(num_aid, "3500000"), (man_aid, "450000")] {
        sqlx::query("INSERT INTO base_data (student_id, area_id, track_id, value) VALUES (?, ?, NULL, ?)")
            .bind(sid).bind(aid).bind(v).execute(&pool).await.unwrap();
    }

    let axum::Json(resp) = score_preview(
        State(common::make_state(pool)),
        Query(ScorePreviewQuery { student_id: sid, track_id: tid }),
    ).await.unwrap();

    // 총점은 전형요소별 점수의 합이어야 한다 (3점 + 4.5점 = 7.5점)
    assert_eq!(resp.total.raw(), 750_000, "총점 raw");
    assert_eq!(
        resp.detail.iter().map(|d| (d.area_id, d.area_name.as_str(), d.score.raw())).collect::<Vec<_>>(),
        vec![(num_aid, "봉사시간", 300_000), (man_aid, "면접점수", 450_000)],
        "전형요소별 내역은 areas ORDER BY id 순서로 전부 실려야 한다",
    );
    assert_eq!(
        resp.detail.iter().map(|d| d.score.raw()).sum::<i64>(),
        resp.total.raw(),
        "total 과 detail 합이 어긋나면 화면과 확정 점수가 갈린다",
    );
}

/// 기초데이터가 없으면 0점으로 흘리지 않고 즉시 오류여야 한다(Fail-Fast).
/// 미리보기가 조용히 0을 보여주면 관리자가 그 값을 정상으로 오인한다.
#[tokio::test]
async fn score_preview_missing_base_data_returns_500_with_context() {
    let pool = common::create_test_pool().await;
    let (sid, tid, _rid) = setup_full(&pool).await;
    sqlx::query(
        "INSERT INTO areas (name, max_score, calc_type, lookup_scope) \
         VALUES ('면접점수', 1000000, 'MANUAL', 'SIMPLE')",
    ).execute(&pool).await.unwrap();

    let res = score_preview(
        State(common::make_state(pool)),
        Query(ScorePreviewQuery { student_id: sid, track_id: tid }),
    ).await;
    let err = match res {
        Ok(r) => panic!("기초데이터가 없는데 점수가 나왔다: total={}", r.0.total.raw()),
        Err(e) => e,
    };

    assert_eq!(err.0, StatusCode::INTERNAL_SERVER_ERROR);
    // 어느 전형요소·어느 학생·어느 모집단위인지가 메시지에 있어야 관리자가 고칠 수 있다
    for needle in ["면접점수", "홍길동", "S001", "한국대", "컴공"] {
        assert!(err.1.contains(needle), "오류 메시지에 '{}' 가 없다: {}", needle, err.1);
    }
}

// ── 모집단위 순위의 라운드 파티션 ─────────────────────────────────
//
// `track_rank_window(.., partition_by_round = true)` 는 **여러 라운드가 섞이는**
// 쿼리에서만 의미가 있다. get_results 는 WHERE 로 라운드를 이미 고정하므로
// (윈도우 함수는 WHERE 이후에 계산된다) 라운드 파티션이 빠져도 값이 같다 —
// 즉 기존 track_rank 테스트들은 이 인자를 전혀 검증하지 못한다.
// export_round_summary 의 CTE 는 라운드 필터 없이 results 전체를 훑으므로,
// 여기서만 파티션 누락이 드러난다.
#[tokio::test]
async fn export_round_summary_track_rank_restarts_each_round() {
    let pool = common::create_test_pool().await;
    let hash = bcrypt::hash("pass", 4u32).unwrap();
    sqlx::query("INSERT INTO classes (grade, class_no, password_hash) VALUES (1, 1, ?)")
        .bind(&hash).execute(&pool).await.unwrap();

    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name) VALUES ('한국대') RETURNING id",
    ).fetch_one(&pool).await.unwrap();
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name, unit_quota) VALUES (?, '컴공', 5) RETURNING id",
    ).bind(uid).fetch_one(&pool).await.unwrap();

    let r1: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at, finalized_at) \
         VALUES ('FINALIZED', '2025-01-01', '2025-01-05') RETURNING id",
    ).fetch_one(&pool).await.unwrap();
    let r2: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at, closed_at) \
         VALUES ('CLOSED', '2025-02-01', '2025-02-05') RETURNING id",
    ).fetch_one(&pool).await.unwrap();

    // 2차 지원자(C·D)의 점수는 1차 지원자(A·B) 전원보다 낮다.
    // 라운드로 파티션하지 않으면 C·D 는 3위·4위가 된다.
    for (seq, (name, rid, score)) in [
        ("에이", r1, 900_000i64),
        ("비",   r1, 800_000),
        ("씨",   r2, 700_000),
        ("디",   r2, 600_000),
    ].into_iter().enumerate()
    {
        let sid: i64 = sqlx::query_scalar(
            "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
             VALUES (?, ?, 1, 1, ?, 1) RETURNING id",
        )
        .bind(format!("S00{}", seq + 1)).bind(name).bind(seq as i64 + 1)
        .fetch_one(&pool).await.unwrap();
        sqlx::query("INSERT INTO applications (student_id, track_id, round_id, abandoned) VALUES (?, ?, ?, 0)")
            .bind(sid).bind(tid).bind(rid).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO results (student_id, track_id, round_id, score_detail, total_score, \
             ranking, recommended, calculated_at) VALUES (?, ?, ?, '{}', ?, NULL, 0, '2025-02-06')",
        ).bind(sid).bind(tid).bind(rid).bind(score).execute(&pool).await.unwrap();
    }

    let resp = export_round_summary(State(common::make_state(pool)), Path(r2)).await.unwrap();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let rows = principal_candidate_manager::excel::parse_xlsx_sheet_rows(&bytes, "지원자결과").unwrap();

    let header = &rows[0];
    let col_of = |h: &str| header.iter().position(|c| c == h)
        .unwrap_or_else(|| panic!("헤더에 '{}' 없음: {:?}", h, header));
    let (c_name, c_rank) = (col_of("이름"), col_of("모집단위 순위"));

    assert_eq!(rows.len(), 3, "2차 지원자 2명만 실려야 함: {rows:?}");
    let names: Vec<&str> = rows[1..].iter().map(|r| r[c_name].as_str()).collect();
    assert!(!names.contains(&"에이"), "1차 지원자가 2차 시트에 새면 안 됨: {names:?}");

    let c = rows[1..].iter().find(|r| r[c_name] == "씨").expect("씨 행");
    let d = rows[1..].iter().find(|r| r[c_name] == "디").expect("디 행");
    assert_eq!(c[c_rank], "1", "2차 최고점은 그 라운드의 1위여야 함(1차와 합산 금지): {c:?}");
    assert_eq!(d[c_rank], "2", "2차 차점자는 2위: {d:?}");
}

/// 담임 화면의 모집단위 순위는 **학급 필터 이전의 전체 결과** 기준이어야 한다.
/// CTE 를 걷어내고 학급 필터가 걸린 집합에서 순위를 매기면, 타 학급 상위자가
/// 사라져 담임에게 "우리 반 학생이 1위"로 보인다 — 실제 순위와 다른 값이 나간다.
#[tokio::test]
async fn teacher_get_results_track_rank_counts_students_in_other_classes() {
    let pool = common::create_test_pool().await;
    let (_grad_sid, tid, rid) = setup_grad_result(&pool).await;

    let hash = bcrypt::hash("pass", 4u32).unwrap();
    for (grade, class_no) in [(1, 1), (1, 2)] {
        sqlx::query("INSERT INTO classes (grade, class_no, password_hash) VALUES (?, ?, ?)")
            .bind(grade).bind(class_no).bind(&hash)
            .execute(&pool).await.unwrap();
    }
    // 1반 홍길동 5점 / 2반 이순신 9점 — 같은 모집단위, 같은 FINALIZED 라운드
    let mut sids = Vec::new();
    for (code, name, class_no, score) in
        [("S001", "홍길동", 1i64, 500_000i64), ("S002", "이순신", 2, 900_000)]
    {
        let sid: i64 = sqlx::query_scalar(
            "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
             VALUES (?, ?, 1, ?, 1, 1) RETURNING id",
        ).bind(code).bind(name).bind(class_no).fetch_one(&pool).await.unwrap();
        sqlx::query("INSERT INTO applications (student_id, track_id, round_id, abandoned) VALUES (?, ?, ?, 0)")
            .bind(sid).bind(tid).bind(rid).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO results (student_id, track_id, round_id, score_detail, total_score, \
             ranking, recommended, calculated_at) VALUES (?, ?, ?, '{}', ?, NULL, 0, '2025-01-09')",
        ).bind(sid).bind(tid).bind(rid).bind(score).execute(&pool).await.unwrap();
        sids.push(sid);
    }

    let res = teacher_get_results(
        State(common::make_state(pool)),
        Extension(common::teacher_claims(1, 1)),
    ).await.unwrap();

    assert_eq!(res.0.results.len(), 1, "1반 담임에게는 1반 학생만");
    let row = &res.0.results[0];
    assert_eq!(row.student_id, sids[0]);
    assert_eq!(
        row.track_rank, Some(2),
        "2반 이순신(9점)이 위에 있으므로 홍길동은 모집단위 2위여야 한다",
    );
}

/// 정원보다 많이 추천된 상태(정원을 나중에 줄인 경우 등)에서 잔여석이 음수로
/// 나가면 관리자가 "-1석 남음"을 보게 된다. `.max(0)` 클램프가 이를 막는데,
/// 기존 픽스처는 정원과 확정 인원이 정확히 맞아떨어져 클램프가 한 번도 발동하지 않았다.
#[tokio::test]
async fn export_round_summary_clamps_negative_remaining_to_zero() {
    let pool = common::create_test_pool().await;
    let hash = bcrypt::hash("pass", 4u32).unwrap();
    sqlx::query("INSERT INTO classes (grade, class_no, password_hash) VALUES (1, 1, ?)")
        .bind(&hash).execute(&pool).await.unwrap();

    // 정원은 각각 1석뿐인데 1차에서 이미 2명이 확정됐고 2차에서 1명 더 확정된다
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name, total_quota) VALUES ('한국대', 1) RETURNING id",
    ).fetch_one(&pool).await.unwrap();
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name, unit_quota) VALUES (?, '컴공', 1) RETURNING id",
    ).bind(uid).fetch_one(&pool).await.unwrap();

    let r1: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at, finalized_at) \
         VALUES ('FINALIZED', '2025-01-01', '2025-01-05') RETURNING id",
    ).fetch_one(&pool).await.unwrap();
    let r2: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at, closed_at) \
         VALUES ('CLOSED', '2025-02-01', '2025-02-05') RETURNING id",
    ).fetch_one(&pool).await.unwrap();

    for (seq, (code, rid)) in [("S001", r1), ("S002", r1), ("S003", r2)].into_iter().enumerate() {
        let sid: i64 = sqlx::query_scalar(
            "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
             VALUES (?, '학생', 1, 1, ?, 1) RETURNING id",
        ).bind(code).bind(seq as i64 + 1).fetch_one(&pool).await.unwrap();
        sqlx::query("INSERT INTO applications (student_id, track_id, round_id, abandoned) VALUES (?, ?, ?, 0)")
            .bind(sid).bind(tid).bind(rid).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO results (student_id, track_id, round_id, score_detail, total_score, \
             ranking, recommended, calculated_at) VALUES (?, ?, ?, '{}', 0, 1, 1, '2025-01-09')",
        ).bind(sid).bind(tid).bind(rid).execute(&pool).await.unwrap();
    }

    let resp = export_round_summary(State(common::make_state(pool)), Path(r2)).await.unwrap();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let rows = principal_candidate_manager::excel::parse_xlsx_sheet_rows(&bytes, "라운드결과").unwrap();
    let header = &rows[0];
    let col_of = |h: &str| header.iter().position(|c| c == h)
        .unwrap_or_else(|| panic!("헤더에 '{}' 없음: {:?}", h, header));
    let data = &rows[1];

    // 정원 1 - 이전 확정 2 = -1, 여기서 이번 1명 더 = -2 → 둘 다 0 으로 클램프
    for (h, want) in [
        ("모집단위 라운드 전 잔여석", "0"),
        ("모집단위 남은 잔여석", "0"),
        ("대학 라운드 전 잔여석", "0"),
        ("대학 남은 잔여석", "0"),
    ] {
        let c = col_of(h);
        assert_eq!(
            data.get(c).map(String::as_str), Some(want),
            "'{}' 열({})은 음수 대신 '{}' 이어야 함: {:?}", h, c, want, data,
        );
    }
    // 클램프가 "이번 라운드 추천 인원"까지 뭉개면 안 된다 — 이 값은 실제 카운트
    assert_eq!(data[col_of("이번 라운드 추천 인원")], "1", "이번 라운드 확정 1명: {data:?}");
}
