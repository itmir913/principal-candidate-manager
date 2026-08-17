//! 감사 산출물 — 총점·순위 불변식 (proptest).
//!
//! 1단계 인벤토리 U-32 / 명세 공백 M-14 대응: `results.total_score` 가
//! `Σ areas.max_score` 를 넘지 않는다는 사후 불변식을 확인하는 테스트가 0건이었다.
//! 여기서 임의 입력으로 고정한다.
//!
//! 각 테스트의 첫 줄 주석 = 그 테스트가 보장하는 불변식 (증거 등급 E2 요건).

mod common;

use principal_candidate_manager::handlers::scoring::run_calculate_scores_on_conn;
use proptest::prelude::*;
use sqlx::SqlitePool;
use std::collections::HashMap;

/// 전형요소 하나의 무작위 명세: (calc_type 코드 0=NUMERIC,1=CATEGORY,2=MANUAL, max_score)
type AreaSpec = (u8, i64);

/// 학생 하나: (재학 여부, 전형요소별 원시 입력값)
type StudentSpec = (bool, Vec<i64>);

fn scenario()
-> impl Strategy<Value = (Vec<AreaSpec>, Vec<StudentSpec>, bool, bool)> {
    (
        prop::collection::vec((0u8..3, (1i64..=30).prop_map(|v| v * 100_000)), 1..4),
        prop::collection::vec((any::<bool>(), prop::collection::vec(-500_000i64..5_000_000, 3)), 1..7),
        any::<bool>(), // 대학 prioritize_enrolled
        any::<bool>(), // 트랙 prioritize_enrolled (대학=1이면 트리거가 1을 강제하므로 아래서 보정)
    )
}

struct Built {
    round_id: i64,
    /// (student_id, track_id) -> is_enrolled
    apps: Vec<(i64, i64, bool)>,
    area_ids: Vec<i64>,
    max_sum: i64,
    max_by_area: HashMap<i64, i64>,
}

async fn build(
    pool: &SqlitePool,
    areas: &[AreaSpec],
    students: &[StudentSpec],
    prio_univ: bool,
    prio_track: bool,
) -> Built {
    common::insert_class(pool, 1, 1).await;
    let prio_track = prio_univ || prio_track; // 트리거 불변식: 대학=1 → 트랙=1

    let univ_id: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name, total_quota, prioritize_enrolled)
         VALUES ('한국대', NULL, ?) RETURNING id",
    )
    .bind(prio_univ as i64)
    .fetch_one(pool)
    .await
    .unwrap();

    let mut track_ids = Vec::new();
    for name in ["가군", "나군"] {
        let t: i64 = sqlx::query_scalar(
            "INSERT INTO univ_tracks (univ_id, track_name, unit_quota, prioritize_enrolled)
             VALUES (?, ?, NULL, ?) RETURNING id",
        )
        .bind(univ_id)
        .bind(name)
        .bind(prio_track as i64)
        .fetch_one(pool)
        .await
        .unwrap();
        track_ids.push(t);
    }

    let mut area_ids = Vec::new();
    let mut max_sum = 0i64;
    let mut max_by_area = HashMap::new();
    for (i, (kind, max_score)) in areas.iter().enumerate() {
        let (calc, mode, agg, multi) = match kind {
            0 => ("NUMERIC", Some("UPPER"), None, 0i64),
            1 => ("CATEGORY", None, Some("SUM"), 1i64),
            _ => ("MANUAL", None, None, 0i64),
        };
        let aid: i64 = sqlx::query_scalar(
            "INSERT INTO areas (name, calc_type, max_score, lookup_scope, match_mode, category_agg, multi_value)
             VALUES (?, ?, ?, 'SIMPLE', ?, ?, ?) RETURNING id",
        )
        .bind(format!("요소{i}"))
        .bind(calc)
        .bind(*max_score)
        .bind(mode)
        .bind(agg)
        .bind(multi)
        .fetch_one(pool)
        .await
        .unwrap();

        // 점수표 — 만점을 초과하는 점수를 일부러 넣어 캡핑 경로를 태운다
        match kind {
            0 => {
                for (th, sc) in [(0i64, 0i64), (1_000_000, max_score / 2), (2_000_000, max_score * 3)] {
                    sqlx::query("INSERT INTO numeric_table (area_id, track_id, threshold, score) VALUES (?, NULL, ?, ?)")
                        .bind(aid).bind(th).bind(sc)
                        .execute(pool).await.unwrap();
                }
            }
            1 => {
                for (cat, sc) in [("없음", 0i64), ("A", max_score * 2 / 3), ("B", max_score * 2 / 3), ("감점", -max_score / 5)] {
                    sqlx::query("INSERT INTO category_map (area_id, track_id, category, score) VALUES (?, NULL, ?, ?)")
                        .bind(aid).bind(cat).bind(sc)
                        .execute(pool).await.unwrap();
                }
            }
            _ => {}
        }
        area_ids.push(aid);
        max_sum += *max_score;
        max_by_area.insert(aid, *max_score);
    }

    let round_id: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at) VALUES ('OPEN', 'now') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();

    let mut apps = Vec::new();
    for (i, (enrolled, values)) in students.iter().enumerate() {
        let sid: i64 = if *enrolled {
            sqlx::query_scalar(
                "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled)
                 VALUES (?, ?, 1, 1, ?, 1) RETURNING id",
            )
            .bind(format!("E{i:03}")).bind(format!("재학{i}")).bind((i + 1) as i64)
            .fetch_one(pool).await.unwrap()
        } else {
            sqlx::query_scalar(
                "INSERT INTO students (student_code, name, is_enrolled, grad_year)
                 VALUES (?, ?, 0, 2024) RETURNING id",
            )
            .bind(format!("G{i:03}")).bind(format!("졸업{i}"))
            .fetch_one(pool).await.unwrap()
        };

        for (ai, aid) in area_ids.iter().enumerate() {
            let v = values[ai % values.len()];
            match areas[ai].0 {
                0 | 2 => {
                    sqlx::query("INSERT INTO base_data (student_id, area_id, track_id, value, multi_value) VALUES (?, ?, NULL, ?, 0)")
                        .bind(sid).bind(aid).bind(v.max(0).to_string())
                        .execute(pool).await.unwrap();
                }
                _ => {
                    // 복수 범주 — 합이 만점을 넘도록 A, B 를 동시에 넣는 경우 포함
                    let cats: &[&str] = match v.rem_euclid(4) {
                        0 => &["없음"],
                        1 => &["A"],
                        2 => &["A", "B"],
                        _ => &["A", "B", "감점"],
                    };
                    for c in cats {
                        sqlx::query("INSERT INTO base_data (student_id, area_id, track_id, value, multi_value) VALUES (?, ?, NULL, ?, 1)")
                            .bind(sid).bind(aid).bind(*c)
                            .execute(pool).await.unwrap();
                    }
                }
            }
        }

        let tid = track_ids[i % track_ids.len()];
        sqlx::query("INSERT INTO applications (student_id, track_id, round_id) VALUES (?, ?, ?)")
            .bind(sid).bind(tid).bind(round_id)
            .execute(pool).await.unwrap();
        apps.push((sid, tid, *enrolled));
    }

    Built { round_id, apps, area_ids, max_sum, max_by_area }
}

struct ResRow {
    student_id: i64,
    track_id: i64,
    total: i64,
    ranking: i64,
    detail: HashMap<String, i64>,
    is_enrolled: bool,
}

async fn fetch_results(pool: &SqlitePool, round_id: i64) -> Vec<ResRow> {
    let rows: Vec<(i64, i64, i64, Option<i64>, String, bool)> = sqlx::query_as(
        "SELECT r.student_id, r.track_id, r.total_score, r.ranking, r.score_detail, s.is_enrolled
         FROM results r JOIN students s ON s.id = r.student_id
         WHERE r.round_id = ? ORDER BY r.student_id, r.track_id",
    )
    .bind(round_id)
    .fetch_all(pool)
    .await
    .unwrap();
    rows.into_iter()
        .map(|(sid, tid, total, ranking, detail, enrolled)| ResRow {
            student_id: sid,
            track_id: tid,
            total,
            ranking: ranking.expect("ranking must be filled after calculation"),
            detail: serde_json::from_str(&detail).unwrap(),
            is_enrolled: enrolled,
        })
        .collect()
}

async fn run_case(
    areas: &[AreaSpec],
    students: &[StudentSpec],
    prio_univ: bool,
    prio_track: bool,
) -> (Built, Vec<ResRow>, Vec<ResRow>) {
    let pool = common::create_test_pool().await;
    let built = build(&pool, areas, students, prio_univ, prio_track).await;

    let mut conn = pool.acquire().await.unwrap();
    run_calculate_scores_on_conn(&mut conn, built.round_id, "2026-08-17T00:00:00Z")
        .await
        .expect("점수 계산 실패");
    drop(conn);
    let first = fetch_results(&pool, built.round_id).await;

    // 결정성 확인용 재계산
    let mut conn = pool.acquire().await.unwrap();
    run_calculate_scores_on_conn(&mut conn, built.round_id, "2026-08-17T00:00:01Z")
        .await
        .expect("재계산 실패");
    drop(conn);
    let second = fetch_results(&pool, built.round_id).await;

    pool.close().await;
    (built, first, second)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    /// 불변식: total_score == Σ score_detail 값,  0 <= 전형요소별 점수 <= 그 전형요소 max_score 의 상한,
    ///         total_score <= Σ areas.max_score,  재계산은 결정적,
    ///         같은 대학 안에서 (우선순위 그룹이 같을 때) 총점이 높을수록 ranking 이 작거나 같고,
    ///         완전 동점자는 같은 ranking 을 받는다.
    #[test]
    fn total_and_ranking_invariants(
        (areas, students, prio_univ, prio_track) in scenario(),
    ) {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let (built, first, second) = rt.block_on(run_case(&areas, &students, prio_univ, prio_track));

        prop_assert_eq!(first.len(), built.apps.len());

        for r in &first {
            // (1) 총점 = 항목 합
            let sum: i64 = r.detail.values().sum();
            prop_assert_eq!(sum, r.total, "총점 != 항목 합 (student {})", r.student_id);

            // (2) 항목별 상한 = 그 전형요소의 max_score
            prop_assert_eq!(r.detail.len(), built.area_ids.len());
            for (aid, sc) in &r.detail {
                let aid: i64 = aid.parse().unwrap();
                let max = built.max_by_area[&aid];
                prop_assert!(*sc <= max, "전형요소 {} 점수 {} > 만점 {}", aid, sc, max);
            }

            // (3) 총점 <= Σ max_score  (U-32 / M-14)
            prop_assert!(r.total <= built.max_sum,
                "총점 {} > Σ 만점 {}", r.total, built.max_sum);
        }

        // (4) 결정성 — 같은 입력 재계산 시 총점·순위 동일
        prop_assert_eq!(first.len(), second.len());
        for (a, b) in first.iter().zip(second.iter()) {
            prop_assert_eq!(a.student_id, b.student_id);
            prop_assert_eq!(a.total, b.total);
            prop_assert_eq!(a.ranking, b.ranking);
        }

        // (5) 순위 단조성 + 동점 그룹 동일 순위 (대학 파티션은 하나뿐인 구성)
        for a in &first {
            for b in &first {
                let a_key = (prio_univ && a.is_enrolled, a.total);
                let b_key = (prio_univ && b.is_enrolled, b.total);
                if a_key > b_key {
                    prop_assert!(a.ranking < b.ranking,
                        "상위자 순위가 하위자보다 크다: {:?}({}) vs {:?}({})",
                        a_key, a.ranking, b_key, b.ranking);
                } else if a_key == b_key {
                    prop_assert_eq!(a.ranking, b.ranking,
                        "동점자 순위 불일치 (student {} vs {})", a.student_id, b.student_id);
                }
            }
        }

        // (6) 순위 값 범위: 1..=행 수, 그리고 표준 경쟁 순위 정의
        //     rank(x) = 1 + |{ y : y 가 x 보다 확실히 상위 }|
        for a in &first {
            let a_key = (prio_univ && a.is_enrolled, a.total);
            let ahead = first.iter().filter(|b| {
                let b_key = (prio_univ && b.is_enrolled, b.total);
                b_key > a_key
            }).count() as i64;
            prop_assert_eq!(a.ranking, ahead + 1,
                "표준 경쟁 순위 위반 (student {}, track {})", a.student_id, a.track_id);
        }
    }
}
