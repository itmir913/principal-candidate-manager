mod common;

use axum::{
    extract::{Path, State},
    http::StatusCode,
};
use principal_candidate_manager::enums::{CalcType, CategoryAgg, LookupScope, MatchMode};
use principal_candidate_manager::handlers::scoring::{
    calc_area_score, calculate_scores, export_results, lookup_range_score, recommend_result,
    AreaRow, ResultRow,
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
        calc_type: CalcType::Numeric,
        max_score: 100_000,
        match_mode: Some(MatchMode::Upper),
        category_agg: None,
        lookup_scope: LookupScope::Simple,
    };
    assert_eq!(calc_area_score(&pool, sid, &area, 0).await.unwrap(), 50_000);
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
        calc_type: CalcType::Numeric,
        max_score: 100_000,
        match_mode: Some(MatchMode::Lower),
        category_agg: None,
        lookup_scope: LookupScope::Simple,
    };
    assert_eq!(calc_area_score(&pool, sid, &area, 0).await.unwrap(), 30_000);
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
        calc_type: CalcType::Numeric,
        max_score: 100_000,
        match_mode: Some(MatchMode::Upper),
        category_agg: None,
        lookup_scope: LookupScope::Composite,
    };
    assert_eq!(calc_area_score(&pool, sid, &area, uid).await.unwrap(), 80_000);
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
        calc_type: CalcType::Category,
        max_score: 100_000,
        match_mode: None,
        category_agg: Some(CategoryAgg::Sum),
        lookup_scope: LookupScope::Simple,
    };
    assert_eq!(calc_area_score(&pool, sid, &area, 0).await.unwrap(), 50_000);
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
        calc_type: CalcType::Category,
        max_score: 100_000,
        match_mode: None,
        category_agg: Some(CategoryAgg::Max),
        lookup_scope: LookupScope::Simple,
    };
    assert_eq!(calc_area_score(&pool, sid, &area, 0).await.unwrap(), 30_000);
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
        calc_type: CalcType::Manual,
        max_score: 100_000,
        match_mode: None,
        category_agg: None,
        lookup_scope: LookupScope::Simple,
    };
    assert_eq!(calc_area_score(&pool, sid, &area, 0).await.unwrap(), 75_000);
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
        calc_type: CalcType::Numeric,
        max_score: 100_000,
        match_mode: Some(MatchMode::Upper),
        category_agg: None,
        lookup_scope: LookupScope::Simple,
    };
    assert!(calc_area_score(&pool, sid, &area, 0).await.is_err());
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
        calc_type: CalcType::Category,
        max_score: 100_000,
        match_mode: None,
        category_agg: Some(CategoryAgg::Sum),
        lookup_scope: LookupScope::Simple,
    };
    assert_eq!(calc_area_score(&pool, sid, &area, 0).await.unwrap(), 100_000);
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
        calc_type: CalcType::Numeric,
        max_score: 100_000,
        match_mode: Some(MatchMode::Lower),
        category_agg: None,
        lookup_scope: LookupScope::Simple,
    };
    assert_eq!(calc_area_score(&pool, sid, &area, 0).await.unwrap(), 50_000);
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
        calc_type: CalcType::Manual,
        max_score: 100_000,
        match_mode: None,
        category_agg: None,
        lookup_scope: LookupScope::Simple,
    };
    assert_eq!(calc_area_score(&pool, sid, &area, 0).await.unwrap(), 100_000);
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
        calc_type: CalcType::Numeric,
        max_score: 100_000,
        match_mode: Some(MatchMode::Upper),
        category_agg: None,
        lookup_scope: LookupScope::Simple,
    };
    assert_eq!(calc_area_score(&pool, sid, &area, 0).await.unwrap(), 100_000);
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
        calc_type: CalcType::Category,
        max_score: 100_000,
        match_mode: None,
        category_agg: Some(CategoryAgg::Max),
        lookup_scope: LookupScope::Simple,
    };
    assert_eq!(calc_area_score(&pool, sid, &area, 0).await.unwrap(), 100_000);
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
        id: aid, calc_type: CalcType::Category, max_score: 1_000_000,
        match_mode: None, category_agg: Some(CategoryAgg::Sum), lookup_scope: LookupScope::Simple,
    };
    let score = calc_area_score(&pool, sid, &area, 0).await.unwrap();
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
        id: aid, calc_type: CalcType::Category, max_score: 1_000_000,
        match_mode: None, category_agg: Some(CategoryAgg::Sum), lookup_scope: LookupScope::Simple,
    };
    assert!(calc_area_score(&pool, sid, &area, 0).await.is_err());
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
        id: aid, calc_type: CalcType::Manual, max_score: 1_000_000,
        match_mode: None, category_agg: None, lookup_scope: LookupScope::Simple,
    };
    assert_eq!(calc_area_score(&pool, sid, &area, 0).await.unwrap(), -500_000, "-5.0점 → -500000");
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
        id: aid, calc_type: CalcType::Category, max_score: 0,
        match_mode: None, category_agg: Some(CategoryAgg::Sum), lookup_scope: LookupScope::Simple,
    };
    // min(-500000, 0) = -500000 → 감점 보존
    assert_eq!(calc_area_score(&pool, sid, &area, 0).await.unwrap(), -500_000);
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
        id: aid, calc_type: CalcType::Category, max_score: 0,
        match_mode: None, category_agg: Some(CategoryAgg::Sum), lookup_scope: LookupScope::Simple,
    };
    assert!(calc_area_score(&pool, sid, &area, 0).await.is_err());
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
        id: aid, calc_type: CalcType::Category, max_score: 1_000_000,
        match_mode: None, category_agg: Some(CategoryAgg::Sum), lookup_scope: LookupScope::Simple,
    };
    assert_eq!(calc_area_score(&pool, sid, &area, 0).await.unwrap(), -300_000);
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
        calc_type: CalcType::Numeric,
        max_score: 100_000,
        match_mode: Some(MatchMode::Exact),
        category_agg: None,
        lookup_scope: LookupScope::Simple,
    };
    assert_eq!(calc_area_score(&pool, sid, &area, 0).await.unwrap(), 30_000);
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
        calc_type: CalcType::Numeric,
        max_score: 100_000,
        match_mode: Some(MatchMode::Exact),
        category_agg: None,
        lookup_scope: LookupScope::Simple,
    };
    assert!(calc_area_score(&pool, sid, &area, 0).await.is_err());
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
        calc_type: CalcType::Numeric,
        max_score: 100_000,
        match_mode: Some(MatchMode::Upper),
        category_agg: None,
        lookup_scope: LookupScope::Simple,
    };
    assert!(calc_area_score(&pool, sid, &area, 0).await.is_err());
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
        calc_type: CalcType::Category,
        max_score: 100_000,
        match_mode: None,
        category_agg: Some(CategoryAgg::Sum),
        lookup_scope: LookupScope::Simple,
    };
    assert!(calc_area_score(&pool, sid, &area, 0).await.is_err());
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
        calc_type: CalcType::Category,
        max_score: 100_000,
        match_mode: None,
        category_agg: None, // 집계 방식 미설정
        lookup_scope: LookupScope::Simple,
    };
    assert!(calc_area_score(&pool, sid, &area, 0).await.is_err());
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
        calc_type: CalcType::Manual,
        max_score: 100_000,
        match_mode: None,
        category_agg: None,
        lookup_scope: LookupScope::Simple,
    };
    assert!(calc_area_score(&pool, sid, &area, 0).await.is_err());
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
    let axum::Json(result) =
        calculate_scores(State(common::make_state(pool)), Path(rid)).await.unwrap();
    assert_eq!(result["calculated"], 0);
}

#[tokio::test]
async fn calculate_scores_creates_result_rows_and_ranking() {
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup_full(&pool).await;

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
        "INSERT INTO base_data (student_id, area_id, track_id, value) VALUES (?, ?, NULL, '8000000')",
    )
    .bind(sid1)
    .bind(aid)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO base_data (student_id, area_id, track_id, value) VALUES (?, ?, NULL, '6000000')",
    )
    .bind(sid2)
    .bind(aid)
    .execute(&pool)
    .await
    .unwrap();

    for sid in [sid1, sid2] {
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
    }

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
        "INSERT INTO applications (student_id, track_id, round_id, confirmed, abandoned) \
         VALUES (?, ?, ?, 1, 0)",
    )
    .bind(sid)
    .bind(tid)
    .bind(rid)
    .execute(&pool)
    .await
    .unwrap();

    let _ = calculate_scores(State(common::make_state(pool.clone())), Path(rid)).await.unwrap();

    sqlx::query(
        "UPDATE rounds SET status = 'CLOSED', closed_at = '2025-01-02T00:00:00Z' WHERE id = ?",
    )
    .bind(rid)
    .execute(&pool)
    .await
    .unwrap();

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
        "INSERT INTO applications (student_id, track_id, round_id, confirmed, abandoned) VALUES (?, ?, ?, 1, 0)",
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
        "INSERT INTO applications (student_id, track_id, round_id, confirmed, abandoned) VALUES (?, ?, ?, 1, 0)",
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
        recommended: false,
        abandoned: false,
        student_code: "S001".to_string(),
        name: "홍길동".to_string(),
        grade: Some(1),
        class_no: Some(1),
        seq_no: Some(1),
        is_enrolled: true,
        univ_name: "한국대".to_string(),
        track_name: "컴공".to_string(),
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
        recommended: false,
        abandoned: false,
        student_code: "S001".to_string(),
        name: "홍길동".to_string(),
        grade: Some(1),
        class_no: Some(1),
        seq_no: Some(1),
        is_enrolled: true,
        univ_name: "한국대".to_string(),
        track_name: "컴공".to_string(),
    };
    let json = serde_json::to_string(&row).expect("정상 score_detail은 직렬화 성공 기대");
    // score_detail은 Score 타입으로 역변환되어 표시값(÷100000)으로 직렬화됨
    assert!(json.contains("\"1\":3.0") || json.contains("\"1\": 3.0"));
}
