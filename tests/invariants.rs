//! 속성 기반(proptest) 불변식 테스트.
//!
//! 감사 세션 1~4가 코드를 읽어 확인한 불변식을 임의 입력 실행으로 고정한다:
//! - lookup_range_score: 결과는 항상 표 안의 점수, 경계값 정확 선택, 출력 단조성
//! - parse_display_value ↔ fmt_score: 왕복 불변식 + NaN/inf/±10억 초과/6자리 거부
//! - 순위 계산: 표준 경쟁 순위(1,2,2,4), 동점=동순위, 재학생 우선
//! - CATEGORY SUM: 합산이 max_score를 초과해 저장되지 않음

mod common;

use principal_candidate_manager::enums::{CalcType, CategoryAgg, LookupScope, MatchMode};
use principal_candidate_manager::handlers::area_data::{fmt_score, parse_display_value};
use principal_candidate_manager::handlers::scoring::{
    calc_area_score, lookup_range_score, run_calculate_scores_on_conn, AreaRow, StudentTrackCtx,
};
use proptest::prelude::*;

// ── lookup_range_score ────────────────────────────────────────────

/// UPPER용 단조 구간표: threshold 순증가, score 비감소 (import 검증이 강제하는 형태)
fn upper_table() -> impl Strategy<Value = Vec<(i64, i64)>> {
    (
        -1_000_000i64..1_000_000,
        0i64..500_000,
        prop::collection::vec((1i64..100_000, 0i64..100_000), 0..7),
    )
        .prop_map(|(base_th, base_sc, deltas)| {
            let (mut th, mut sc) = (base_th, base_sc);
            let mut rows = vec![(th, sc)];
            for (dt, ds) in deltas {
                th += dt;
                sc += ds;
                rows.push((th, sc));
            }
            rows
        })
}

/// LOWER용 단조 구간표: threshold 순증가, score 비증가
fn lower_table() -> impl Strategy<Value = Vec<(i64, i64)>> {
    (
        -1_000_000i64..1_000_000,
        0i64..500_000,
        prop::collection::vec((1i64..100_000, 0i64..100_000), 0..7),
    )
        .prop_map(|(base_th, base_sc, deltas)| {
            let (mut th, mut sc) = (base_th, base_sc);
            let mut rows = vec![(th, sc)];
            for (dt, ds) in deltas {
                th += dt;
                sc -= ds;
                rows.push((th, sc));
            }
            rows
        })
}

proptest! {
    /// UPPER: 결과는 항상 표 안의 점수이고, 값이 클수록 점수는 비감소
    #[test]
    fn upper_result_in_table_and_monotone(
        rows in upper_table(),
        off1 in 0i64..2_000_000,
        off2 in 0i64..2_000_000,
    ) {
        let min_th = rows[0].0;
        let (v1, v2) = (min_th + off1.min(off2), min_th + off1.max(off2));
        let s1 = lookup_range_score(v1, &rows, MatchMode::Upper).unwrap();
        let s2 = lookup_range_score(v2, &rows, MatchMode::Upper).unwrap();
        prop_assert!(rows.iter().any(|&(_, sc)| sc == s1), "결과 {}는 표 안의 점수여야 함", s1);
        prop_assert!(rows.iter().any(|&(_, sc)| sc == s2), "결과 {}는 표 안의 점수여야 함", s2);
        prop_assert!(s1 <= s2, "UPPER 단조성 위반: v{}→{}점, v{}→{}점", v1, s1, v2, s2);
    }

    /// UPPER: 경계값(threshold 정확히 일치)은 정확히 그 구간의 점수를 선택
    #[test]
    fn upper_exact_threshold_selects_that_row(rows in upper_table()) {
        for &(th, sc) in &rows {
            let got = lookup_range_score(th, &rows, MatchMode::Upper).unwrap();
            prop_assert_eq!(got, sc, "threshold {} 정확 일치 시 그 행의 점수여야 함", th);
        }
    }

    /// UPPER: 모든 threshold보다 낮은 값은 fail-fast Err (silent 0점 금지)
    #[test]
    fn upper_below_min_threshold_errors(rows in upper_table(), gap in 1i64..1_000_000) {
        let below = rows[0].0 - gap;
        prop_assert!(lookup_range_score(below, &rows, MatchMode::Upper).is_err());
    }

    /// LOWER: 결과는 항상 표 안의 점수이고, 값이 클수록 점수는 비증가.
    /// 최대 threshold 초과("5일 이상") 값도 Err 없이 최대 threshold 행으로 처리
    #[test]
    fn lower_result_in_table_and_monotone(
        rows in lower_table(),
        off1 in -1_000_000i64..2_000_000,
        off2 in -1_000_000i64..2_000_000,
    ) {
        let min_th = rows[0].0;
        let (v1, v2) = (min_th + off1.min(off2), min_th + off1.max(off2));
        let s1 = lookup_range_score(v1, &rows, MatchMode::Lower).unwrap();
        let s2 = lookup_range_score(v2, &rows, MatchMode::Lower).unwrap();
        prop_assert!(rows.iter().any(|&(_, sc)| sc == s1), "결과 {}는 표 안의 점수여야 함", s1);
        prop_assert!(rows.iter().any(|&(_, sc)| sc == s2), "결과 {}는 표 안의 점수여야 함", s2);
        prop_assert!(s1 >= s2, "LOWER 단조성 위반: v{}→{}점, v{}→{}점", v1, s1, v2, s2);
    }

    /// LOWER: 경계값(threshold 정확히 일치)은 정확히 그 구간의 점수를 선택
    #[test]
    fn lower_exact_threshold_selects_that_row(rows in lower_table()) {
        for &(th, sc) in &rows {
            let got = lookup_range_score(th, &rows, MatchMode::Lower).unwrap();
            prop_assert_eq!(got, sc, "threshold {} 정확 일치 시 그 행의 점수여야 함", th);
        }
    }

    /// LOWER: 최대 threshold 초과 값은 최대 threshold 행의 점수 (마지막 구간 개방)
    #[test]
    fn lower_above_max_threshold_uses_last_row(rows in lower_table(), gap in 1i64..1_000_000) {
        let &(max_th, last_sc) = rows.last().unwrap();
        let got = lookup_range_score(max_th + gap, &rows, MatchMode::Lower).unwrap();
        prop_assert_eq!(got, last_sc);
    }
}

// ── parse_display_value ↔ fmt_score 왕복 ─────────────────────────

proptest! {
    /// ±10억(raw ±10^14) 이내 임의 raw 값: fmt_score → parse_display_value 왕복 무손실
    #[test]
    fn parse_fmt_roundtrip_is_lossless(raw in -100_000_000_000_000i64..=100_000_000_000_000) {
        let display = fmt_score(raw);
        let back = parse_display_value(&display);
        prop_assert_eq!(back, Ok(raw), "왕복 실패: raw {} → '{}'", raw, display);
    }

    /// |값| > 10억은 거부 (×100000 시 f64 정밀도 한계 방어)
    #[test]
    fn parse_rejects_over_one_billion(v in 1_000_000_001i64..=1_000_000_000_000, neg in any::<bool>()) {
        let s = if neg { format!("-{}", v) } else { v.to_string() };
        let res = parse_display_value(&s);
        prop_assert!(res.is_err(), "'{}'는 거부되어야 함", s);
        prop_assert!(res.unwrap_err().contains("초과"));
    }

    /// 소수점 6자리(마지막 자리 0 아님)는 거부
    #[test]
    fn parse_rejects_six_decimal_places(head in 0i64..1000, mid in 0u32..100_000, last in 1u32..=9) {
        let s = format!("{}.{:05}{}", head, mid, last);
        let res = parse_display_value(&s);
        prop_assert!(res.is_err(), "'{}'는 거부되어야 함", s);
        prop_assert!(res.unwrap_err().contains("소수점"));
    }
}

#[test]
fn parse_rejects_nan_and_infinity() {
    for s in ["nan", "NaN", "-nan", "inf", "-inf", "infinity", "-infinity"] {
        let res = parse_display_value(s);
        assert!(res.is_err(), "'{}'는 거부되어야 함", s);
        assert!(res.unwrap_err().contains("유한한 숫자"), "입력 '{}'", s);
    }
}

// ── 순위 계산 (DB 실행) ──────────────────────────────────────────

/// 임의 학생 목록: (총점 raw — 동점 유도를 위해 0~5점만, 재학 여부)
fn ranking_input() -> impl Strategy<Value = (Vec<(i64, bool)>, bool)> {
    (
        prop::collection::vec(((0i64..=5).prop_map(|v| v * 100_000), any::<bool>()), 1..7),
        any::<bool>(),
    )
}

/// 학생별 (총점, 재학 여부) 입력을 MANUAL 전형요소로 넣고 점수 계산을 실행,
/// results.ranking을 학생 순서대로 반환
async fn run_ranking_case(students: &[(i64, bool)], prioritize: bool) -> Vec<i64> {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;

    let univ_id: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name, prioritize_enrolled) VALUES ('한국대', ?) RETURNING id",
    )
    .bind(prioritize as i64)
    .fetch_one(&pool)
    .await
    .unwrap();
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name) VALUES (?, '컴공') RETURNING id",
    )
    .bind(univ_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let area_id: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, calc_type, max_score, lookup_scope, multi_value) \
         VALUES ('면접', 'MANUAL', 10000000000, 'SIMPLE', 0) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at) VALUES ('OPEN', 'now') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let mut sids: Vec<i64> = Vec::new();
    for (i, (score, enrolled)) in students.iter().enumerate() {
        // 픽스처 규칙: 재학생 위치 유일 — seq_no를 학생마다 다르게
        let sid: i64 = if *enrolled {
            sqlx::query_scalar(
                "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
                 VALUES (?, ?, 1, 1, ?, 1) RETURNING id",
            )
            .bind(format!("E{:03}", i))
            .bind(format!("재학생{}", i))
            .bind((i + 1) as i64)
            .fetch_one(&pool)
            .await
            .unwrap()
        } else {
            sqlx::query_scalar(
                "INSERT INTO students (student_code, name, is_enrolled, grad_year) \
                 VALUES (?, ?, 0, 2024) RETURNING id",
            )
            .bind(format!("G{:03}", i))
            .bind(format!("졸업생{}", i))
            .fetch_one(&pool)
            .await
            .unwrap()
        };
        sqlx::query(
            "INSERT INTO base_data (student_id, area_id, track_id, value, multi_value) \
             VALUES (?, ?, NULL, ?, 0)",
        )
        .bind(sid)
        .bind(area_id)
        .bind(score.to_string())
        .execute(&pool)
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
        sids.push(sid);
    }

    let mut conn = pool.acquire().await.unwrap();
    let count = run_calculate_scores_on_conn(&mut conn, rid, "2025-01-01T00:00:00Z")
        .await
        .unwrap();
    drop(conn);
    assert_eq!(count, students.len());

    let mut rankings = Vec::new();
    for sid in &sids {
        let r: i64 = sqlx::query_scalar(
            "SELECT ranking FROM results WHERE student_id = ? AND track_id = ? AND round_id = ?",
        )
        .bind(sid)
        .bind(tid)
        .bind(rid)
        .fetch_one(&pool)
        .await
        .unwrap();
        rankings.push(r);
    }
    rankings
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// 임의 점수 집합에서 표준 경쟁 순위(1,2,2,4) 오라클과 일치해야 한다:
    /// 순위 = 1 + (나보다 확실히 앞선 학생 수). 동점(같은 그룹·같은 점수)은 자동으로 동순위.
    /// prioritize_enrolled=1이면 재학생 전원이 졸업생 전원보다 앞순위.
    #[test]
    fn ranking_matches_standard_competition_oracle(
        (students, prioritize) in ranking_input(),
    ) {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let rankings = rt.block_on(run_ranking_case(&students, prioritize));

        for (i, &(score_i, enrolled_i)) in students.iter().enumerate() {
            let ahead = students
                .iter()
                .filter(|&&(score_j, enrolled_j)| {
                    if prioritize {
                        (enrolled_j && !enrolled_i)
                            || (enrolled_j == enrolled_i && score_j > score_i)
                    } else {
                        score_j > score_i
                    }
                })
                .count() as i64;
            prop_assert_eq!(
                rankings[i],
                ahead + 1,
                "학생 {}(점수 {}, 재학 {}) 기대 순위 {} ≠ 실제 {} (전체: {:?}, prioritize={})",
                i, score_i, enrolled_i, ahead + 1, rankings[i], students, prioritize
            );
        }

        // 재학생 우선: 재학생 최하위도 졸업생 최상위보다 앞순위
        if prioritize {
            let worst_enrolled = students.iter().zip(&rankings)
                .filter(|((_, e), _)| *e)
                .map(|(_, r)| *r)
                .max();
            let best_graduated = students.iter().zip(&rankings)
                .filter(|((_, e), _)| !*e)
                .map(|(_, r)| *r)
                .min();
            if let (Some(we), Some(bg)) = (worst_enrolled, best_graduated) {
                prop_assert!(we < bg, "재학생 우선 위반: 재학생 최하위 {} ≥ 졸업생 최상위 {}", we, bg);
            }
        }
    }
}

// ── CATEGORY SUM: max_score 초과 저장 금지 (DB 실행) ──────────────

/// 범주표 점수 목록(전체 등록) + 학생 보유 범주 부분집합 + 전형요소 만점
fn category_sum_input() -> impl Strategy<Value = (Vec<(i64, bool)>, i64)> {
    (
        prop::collection::vec((0i64..=400_000, any::<bool>()), 1..6)
            .prop_filter("학생 보유 범주 최소 1개", |v| v.iter().any(|(_, picked)| *picked)),
        0i64..=1_000_000,
    )
}

async fn run_category_sum_case(categories: &[(i64, bool)], max_score: i64) -> i64 {
    let pool = common::create_test_pool().await;
    let sid: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, is_enrolled, grad_year) \
         VALUES ('G001', '졸업생', 0, 2024) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let area_id: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, calc_type, max_score, category_agg, lookup_scope, multi_value) \
         VALUES ('활동', 'CATEGORY', ?, 'SUM', 'SIMPLE', 1) RETURNING id",
    )
    .bind(max_score)
    .fetch_one(&pool)
    .await
    .unwrap();

    for (i, (score, picked)) in categories.iter().enumerate() {
        let cat = format!("범주{}", i);
        sqlx::query(
            "INSERT INTO category_map (area_id, track_id, category, score) VALUES (?, NULL, ?, ?)",
        )
        .bind(area_id)
        .bind(&cat)
        .bind(score)
        .execute(&pool)
        .await
        .unwrap();
        if *picked {
            sqlx::query(
                "INSERT INTO base_data (student_id, area_id, track_id, value, multi_value) \
                 VALUES (?, ?, NULL, ?, 1)",
            )
            .bind(sid)
            .bind(area_id)
            .bind(&cat)
            .execute(&pool)
            .await
            .unwrap();
        }
    }

    let area = AreaRow {
        id: area_id,
        name: "활동".into(),
        calc_type: CalcType::Category,
        max_score,
        match_mode: None,
        category_agg: Some(CategoryAgg::Sum),
        lookup_scope: LookupScope::Simple,
    };
    let ctx = StudentTrackCtx {
        student_code: "G001".into(),
        student_name: "졸업생".into(),
        univ_name: "한국대".into(),
        track_name: "컴공".into(),
    };
    let mut conn = pool.acquire().await.unwrap();
    calc_area_score(&mut conn, sid, &area, 0, &ctx).await.unwrap()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// 임의 범주 조합: 합산 점수는 min(범주 합, max_score) — 만점 초과 저장 불가
    #[test]
    fn category_sum_never_exceeds_max_score(
        (categories, max_score) in category_sum_input(),
    ) {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let got = rt.block_on(run_category_sum_case(&categories, max_score));

        let sum: i64 = categories.iter().filter(|(_, p)| *p).map(|(s, _)| s).sum();
        prop_assert!(got <= max_score, "만점 초과 저장: {} > {}", got, max_score);
        prop_assert_eq!(got, sum.min(max_score), "기대 min({}, {}) ≠ 실제 {}", sum, max_score, got);
    }
}
