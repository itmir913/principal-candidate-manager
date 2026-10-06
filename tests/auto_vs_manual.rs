//! 자동 추천 ≡ 수동 가드 — 2026-10-07 채점·추천 정합성 감사(F-1·F-2)의 회귀 방어선.
//!
//! 자동 추천(`run_auto_recommend`)과 수동 추천(`recommend_result` 의 5b·5c 가드)은 같은
//! 규칙("모집단위 순서를 건너뛰지 않는다", "대학 컷은 대학 순위 순")을 두 번 구현한다.
//! 둘이 어긋나면 **자동은 하는데 관리자는 못 하는 결정**(F-1)이나 **수동으로는 누구도
//! 추천할 수 없는 교착**(F-2)이 생긴다. 그날 감사 전까지 이 등가를 직접 보는 테스트가 없었다.
//!
//! 두 부분으로 나눈다.
//! 1. **시나리오** — 감사가 추적한 최소 재현과 경계(동점 × 정원 × 재학생 우선 × 미선발 ×
//!    이전 라운드). 실제 `close_round` 가 쓴 순위 위에서 돈다.
//! 2. **불변식** — 작은 구성을 결정적 의사난수로 많이 만들어
//!    (a) 자동이 확정한 집합을 수동 `recommend_result` 로 **전부** 재현할 수 있는가,
//!    (b) 자동이 수동 판단 항목 없이 끝났다면 수동으로 **더** 추천할 수 있는 후보가 없는가,
//!    (c) 모집단위·대학 정원을 넘지 않는가 를 본다.
//!
//! 판별력의 소재: F-1 수정(`merge_univ_cut_held` 의 보류 덩어리에서 멈춤)을 되돌리면 시나리오 1~3 과
//! 불변식 (a) 가, F-2 수정(5c 블로커의 선두 조건)을 되돌리면 시나리오 4 와 불변식 (a) 가
//! 깨진다 — 변이로 확인한다(커밋 메시지에 결과를 남긴다).

mod common;

use std::collections::BTreeSet;

use axum::extract::{Path, State};
use principal_candidate_manager::{
    handlers::{
        rounds::close_round,
        scoring::{auto_recommend_results, recommend_result},
    },
    state::AppState,
};
use sqlx::SqlitePool;

// ── 구성 ──────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
struct Cand {
    track: usize,
    score: i64,
    enrolled: bool,
    excluded: bool,
}

#[derive(Clone, Debug)]
struct Track {
    quota: Option<i64>,
    prio: bool,
    /// 0 = 첫 대학(`total`·`univ_prio`), 1 = 둘째 대학(`univ2`)
    univ: usize,
}

#[derive(Clone, Debug)]
struct Cfg {
    total: Option<i64>,
    univ_prio: bool,
    tracks: Vec<Track>,
    cands: Vec<Cand>,
    /// 둘째 대학 (total_quota, prioritize_enrolled). 트랙의 `univ == 1` 이 여기 속한다.
    univ2: Option<(Option<i64>, bool)>,
    /// 이전(FINALIZED) 라운드에서 이미 추천된 인원 — 트랙 인덱스 목록. 같은 트랙이 여러 번
    /// 나오면 그만큼. 이번 라운드 후보와는 다른 학생이다. 정원 집계(전 라운드 누적)를 태운다.
    prior: Vec<usize>,
}

struct Built {
    rid: i64,
    /// cands[i] 의 (student_id, track_id)
    keys: Vec<(i64, i64)>,
    track_ids: Vec<i64>,
}

fn st(pool: &SqlitePool) -> State<AppState> {
    State(common::make_state(pool.clone()))
}

/// 구성을 DB 에 만들고 실제 `close_round` 로 점수·순위를 낸다. 미선발은 마감 뒤에 표시한다
/// (CLOSED 에서만 바꿀 수 있는 값이다).
async fn build(pool: &SqlitePool, cfg: &Cfg) -> Built {
    let hash = bcrypt::hash("pass", 4u32).unwrap();
    for (g, c) in [(3i64, 1i64), (0, 0)] {
        sqlx::query("INSERT INTO classes (grade, class_no, password_hash) VALUES (?, ?, ?)")
            .bind(g).bind(c).bind(&hash).execute(pool).await.unwrap();
    }
    let mut univs: Vec<(i64, bool)> = Vec::new();
    let univ_specs: Vec<(Option<i64>, bool)> =
        std::iter::once((cfg.total, cfg.univ_prio)).chain(cfg.univ2).collect();
    for (ui, (total, uprio)) in univ_specs.iter().enumerate() {
        let uid: i64 = sqlx::query_scalar(
            "INSERT INTO universities (univ_name, total_quota, prioritize_enrolled) VALUES (?, ?, ?) RETURNING id",
        )
        .bind(format!("대학{ui}")).bind(*total).bind(*uprio as i64).fetch_one(pool).await.unwrap();
        univs.push((uid, *uprio));
    }
    let mut track_ids = Vec::new();
    for (i, t) in cfg.tracks.iter().enumerate() {
        let (uid, uprio) = univs[t.univ];
        // 불변식: 대학이 재학생 우선이면 모든 트랙도 재학생 우선(스키마 주석 005)
        let prio = t.prio || uprio;
        let tid: i64 = sqlx::query_scalar(
            "INSERT INTO univ_tracks (univ_id, track_name, unit_quota, prioritize_enrolled) \
             VALUES (?, ?, ?, ?) RETURNING id",
        )
        .bind(uid).bind(format!("트랙{i}")).bind(t.quota).bind(prio as i64)
        .fetch_one(pool).await.unwrap();
        track_ids.push(tid);
    }

    // 이전 라운드: 다른 학생들이 이미 추천돼 정원 일부를 쓰고 있다
    if !cfg.prior.is_empty() {
        let prid: i64 = sqlx::query_scalar(
            "INSERT INTO rounds (status, opened_at, closed_at) \
             VALUES ('CLOSED', '2024-01-01T00:00:00Z', '2024-01-02T00:00:00Z') RETURNING id",
        )
        .fetch_one(pool).await.unwrap();
        for (k, &ti) in cfg.prior.iter().enumerate() {
            let code = format!("P{k:02}");
            let sid: i64 = sqlx::query_scalar(
                "INSERT INTO students (student_code, name, grad_year, is_enrolled) \
                 VALUES (?, ?, 2023, 0) RETURNING id",
            )
            .bind(&code).bind(&code).fetch_one(pool).await.unwrap();
            sqlx::query(
                "INSERT INTO applications (student_id, track_id, round_id, department_name) VALUES (?, ?, ?, '학과')",
            )
            .bind(sid).bind(track_ids[ti]).bind(prid).execute(pool).await.unwrap();
            sqlx::query(
                "INSERT INTO results \
                 (student_id, track_id, round_id, score_detail, total_score, ranking, recommended, calculated_at) \
                 VALUES (?, ?, ?, '{}', 9900000, 1, 1, '2024-01-02T00:00:00Z')",
            )
            .bind(sid).bind(track_ids[ti]).bind(prid).execute(pool).await.unwrap();
        }
        sqlx::query("UPDATE rounds SET status = 'FINALIZED', finalized_at = '2024-01-03T00:00:00Z' WHERE id = ?")
            .bind(prid).execute(pool).await.unwrap();
    }

    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at) VALUES ('OPEN', '2025-01-01T00:00:00Z') RETURNING id",
    )
    .fetch_one(pool).await.unwrap();
    let area: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, calc_type, max_score, lookup_scope) \
         VALUES ('면접', 'MANUAL', 10000000, 'SIMPLE') RETURNING id",
    )
    .fetch_one(pool).await.unwrap();

    let mut keys = Vec::new();
    for (i, c) in cfg.cands.iter().enumerate() {
        let code = format!("S{i:02}");
        let sid: i64 = if c.enrolled {
            sqlx::query_scalar(
                "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
                 VALUES (?, ?, 3, 1, ?, 1) RETURNING id",
            )
            .bind(&code).bind(&code).bind(i as i64 + 1).fetch_one(pool).await.unwrap()
        } else {
            sqlx::query_scalar(
                "INSERT INTO students (student_code, name, grad_year, is_enrolled) \
                 VALUES (?, ?, 2024, 0) RETURNING id",
            )
            .bind(&code).bind(&code).fetch_one(pool).await.unwrap()
        };
        sqlx::query(
            "INSERT INTO base_data (student_id, area_id, track_id, value, multi_value) VALUES (?, ?, NULL, ?, 0)",
        )
        .bind(sid).bind(area).bind((c.score * 100_000).to_string())
        .execute(pool).await.unwrap();
        let tid = track_ids[c.track];
        sqlx::query(
            "INSERT INTO applications (student_id, track_id, round_id, department_name) VALUES (?, ?, ?, '학과')",
        )
        .bind(sid).bind(tid).bind(rid).execute(pool).await.unwrap();
        keys.push((sid, tid));
    }

    let _closed = close_round(st(pool), Path(rid)).await.unwrap();

    for (i, c) in cfg.cands.iter().enumerate() {
        if c.excluded {
            sqlx::query(
                "UPDATE applications SET excluded = 1, excluded_reason = '시험' \
                 WHERE student_id = ? AND track_id = ? AND round_id = ?",
            )
            .bind(keys[i].0).bind(keys[i].1).bind(rid).execute(pool).await.unwrap();
        }
    }
    Built { rid, keys, track_ids }
}

async fn recommended(pool: &SqlitePool, rid: i64) -> BTreeSet<(i64, i64)> {
    let rows: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT student_id, track_id FROM results WHERE round_id = ? AND recommended = 1 \
         ORDER BY student_id, track_id",
    )
    .bind(rid).fetch_all(pool).await.unwrap();
    rows.into_iter().collect()
}

/// 이름(= cands 인덱스)으로 확정 집합을 본다 — 단언 메시지를 읽기 쉽게.
fn names(b: &Built, set: &BTreeSet<(i64, i64)>) -> Vec<usize> {
    (0..b.keys.len()).filter(|i| set.contains(&b.keys[*i])).collect()
}

async fn rec_manual(pool: &SqlitePool, b: &Built, i: usize) -> Result<(), String> {
    recommend_result(st(pool), Path((b.keys[i].0, b.keys[i].1, b.rid)))
        .await
        .map(|_| ())
        .map_err(|(s, m)| format!("{s}: {m}"))
}

fn c(track: usize, score: i64, enrolled: bool) -> Cand {
    Cand { track, score, enrolled, excluded: false }
}

fn t(quota: Option<i64>, prio: bool) -> Track {
    Track { quota, prio, univ: 0 }
}

/// 자동 추천을 돌려 (확정 cands 인덱스, 수동 항목 수) 를 돌려준다.
async fn auto(cfg: &Cfg) -> (Vec<usize>, usize) {
    let pool = common::create_test_pool().await;
    let b = build(&pool, cfg).await;
    let resp = auto_recommend_results(st(&pool), Path(b.rid)).await.unwrap().0;
    (names(&b, &recommended(&pool, b.rid).await), resp.manual.len())
}

// ── 1. 시나리오 ───────────────────────────────────────────────────

/// 감사 F-1 최소 재현. X 트랙(1석) 1위 동점 2명(대학 1위 동점)이 보류되면, 대학 3위 Y1 이
/// 대학 정원 1석을 가져가서는 안 된다 — 수동 5c 는 Y1 을 거부한다.
#[tokio::test]
async fn held_track_tie_keeps_its_university_seat() {
    let cfg = Cfg {
        total: Some(1), univ_prio: false,
        tracks: vec![t(Some(1), false), t(Some(1), false)],
        cands: vec![c(0, 90, true), c(0, 90, true), c(1, 80, true)], univ2: None, prior: vec![],
    };
    let (picked, manual) = auto(&cfg).await;
    assert_eq!(picked, Vec::<usize>::new(), "보류된 동점 그룹 대신 대학 순위가 나쁜 후보가 확정됐다");
    assert!(manual >= 1, "동점은 관리자 판단으로 보고돼야 한다");
}

/// F-1 부분 확정 변형. X(2석): X1 95 확정, X2·X3 90 동점 보류(1석). Y(1석): Y1 80.
/// 대학 2석 — 하나는 X1, 나머지 하나는 대학 순위 2위 동점 그룹의 몫이다. Y1 은 아니다.
#[tokio::test]
async fn held_tie_after_partial_fill_still_reserves_seat() {
    let cfg = Cfg {
        total: Some(2), univ_prio: false,
        tracks: vec![t(Some(2), false), t(Some(1), false)],
        cands: vec![c(0, 95, true), c(0, 90, true), c(0, 90, true), c(1, 80, true)], univ2: None, prior: vec![],
    };
    let (picked, manual) = auto(&cfg).await;
    assert_eq!(picked, vec![0], "X1 만 확정돼야 한다");
    assert!(manual >= 1);
}

/// F-1 동순위 변형. 보류 그룹과 Y1 이 **같은 대학 순위**면 셋이 1석을 두고 경합하는 동점이다.
#[tokio::test]
async fn held_tie_equal_to_other_track_leader_is_manual() {
    let cfg = Cfg {
        total: Some(1), univ_prio: false,
        tracks: vec![t(Some(1), false), t(Some(1), false)],
        cands: vec![c(0, 90, true), c(0, 90, true), c(1, 90, true)], univ2: None, prior: vec![],
    };
    let (picked, manual) = auto(&cfg).await;
    assert_eq!(picked, Vec::<usize>::new());
    assert!(manual >= 1);
}

/// 보류 그룹이 있으면 대학 정원에 자리가 남아도 그 대학 순위에서 멈춘다 — 수동 5c 가
/// "상위 대학 순위 선두가 미결정이고 그 트랙에 빈자리가 있으면" 하위자를 막기 때문이다.
/// (처음 수정은 그룹 몫을 예약하고 계속 병합했는데, 아래 불변식이 자동≠수동으로 잡았다.)
/// 관리자가 동점을 정리하면 다시 돌린 자동 추천이 나머지를 확정한다.
#[tokio::test]
async fn held_tie_stops_university_cut_until_resolved() {
    let cfg = Cfg {
        total: Some(2), univ_prio: false,
        tracks: vec![t(Some(1), false), t(Some(1), false)],
        cands: vec![c(0, 90, true), c(0, 90, true), c(1, 80, true)], univ2: None, prior: vec![],
    };
    let pool = common::create_test_pool().await;
    let b = build(&pool, &cfg).await;
    let resp = auto_recommend_results(st(&pool), Path(b.rid)).await.unwrap().0;
    assert_eq!(names(&b, &recommended(&pool, b.rid).await), Vec::<usize>::new());
    assert!(!resp.manual.is_empty());
    // 수동도 같은 판단 — Y1 은 아직 막힌다
    assert!(rec_manual(&pool, &b, 2).await.is_err(), "자동과 수동이 다르다");

    // 관리자가 동점을 정리(X1 추천)한 뒤 다시 돌리면 Y1 이 확정된다
    rec_manual(&pool, &b, 0).await.expect("동점 중 하나는 관리자가 고를 수 있다");
    let resp = auto_recommend_results(st(&pool), Path(b.rid)).await.unwrap().0;
    assert_eq!(names(&b, &recommended(&pool, b.rid).await), vec![0, 2]);
    assert!(resp.manual.is_empty(), "정리된 뒤에는 판단할 것이 없다: {}", resp.manual.len());
}

/// 보류 그룹보다 대학 순위가 **좋은** 다른 트랙 후보는 막지 않는다.
#[tokio::test]
async fn held_tie_does_not_block_better_ranked_other_track() {
    let cfg = Cfg {
        total: Some(1), univ_prio: false,
        tracks: vec![t(Some(1), false), t(Some(1), false)],
        cands: vec![c(0, 80, true), c(0, 80, true), c(1, 95, true)], univ2: None, prior: vec![],
    };
    let (picked, _) = auto(&cfg).await;
    assert_eq!(picked, vec![2]);
}

/// 감사 F-2 재현. X(1석, 재학생 우선): X1 재학 80, X2 졸업 90. Y(1석): Y1 재학 88.
/// 대학 순위 X2=1, Y1=2, X1=3. X2 는 자기 트랙에서 X1 에 막혀 경쟁하지 않으므로, 자동은
/// Y1 을 확정하고 수동도 Y1 을 받아야 한다(전에는 X2 를 블로커로 세 409 — 교착).
#[tokio::test]
async fn cross_track_guard_ignores_candidates_blocked_in_their_own_track() {
    let cfg = Cfg {
        total: Some(1), univ_prio: false,
        tracks: vec![t(Some(1), true), t(Some(1), false)],
        cands: vec![c(0, 80, true), c(0, 90, false), c(1, 88, true)], univ2: None, prior: vec![],
    };
    let (picked, _) = auto(&cfg).await;
    assert_eq!(picked, vec![2], "자동은 Y1");

    let pool = common::create_test_pool().await;
    let b = build(&pool, &cfg).await;
    rec_manual(&pool, &b, 2).await.expect("수동으로도 Y1 을 추천할 수 있어야 한다");
    assert!(rec_manual(&pool, &b, 0).await.is_err(), "대학 정원 1석이 찼다");
}

/// F-2 의 짝 — 자기 트랙의 **선두**인 상위 대학 순위 후보는 여전히 막는다.
#[tokio::test]
async fn cross_track_guard_still_blocks_on_leader() {
    let cfg = Cfg {
        total: Some(1), univ_prio: false,
        tracks: vec![t(Some(1), false), t(Some(1), false)],
        cands: vec![c(0, 95, true), c(1, 88, true)], univ2: None, prior: vec![],
    };
    let pool = common::create_test_pool().await;
    let b = build(&pool, &cfg).await;
    assert!(rec_manual(&pool, &b, 1).await.is_err(), "대학 1위가 남아 있는데 2위를 먼저 추천했다");
    rec_manual(&pool, &b, 0).await.expect("대학 1위는 추천된다");
}

/// 동점 그룹 안의 미선발자는 경합에서 빠진다. 3명 동점 중 1명 미선발 → 2명이 1석 경합.
#[tokio::test]
async fn excluded_member_leaves_tie_group() {
    let mut cands = vec![c(0, 90, true), c(0, 90, true), c(0, 90, true)];
    cands[1].excluded = true;
    let cfg = Cfg { total: None, univ_prio: false, tracks: vec![t(Some(1), false)], cands, univ2: None, prior: vec![] };
    let (picked, manual) = auto(&cfg).await;
    assert_eq!(picked, Vec::<usize>::new());
    assert_eq!(manual, 1);

    let mut cands = vec![c(0, 90, true), c(0, 90, true), c(0, 90, true)];
    cands[1].excluded = true;
    cands[2].excluded = true;
    let cfg = Cfg { total: None, univ_prio: false, tracks: vec![t(Some(1), false)], cands, univ2: None, prior: vec![] };
    let (picked, manual) = auto(&cfg).await;
    assert_eq!(picked, vec![0], "남은 1명은 확정");
    assert_eq!(manual, 0);
}

/// 미선발된 상위자 아래의 동점 — 확정 없이 관리자 판단.
#[tokio::test]
async fn excluded_top_then_tie_is_manual() {
    let mut cands = vec![c(0, 95, true), c(0, 90, true), c(0, 90, true)];
    cands[0].excluded = true;
    let cfg = Cfg { total: None, univ_prio: false, tracks: vec![t(Some(1), false)], cands, univ2: None, prior: vec![] };
    let (picked, manual) = auto(&cfg).await;
    assert_eq!(picked, Vec::<usize>::new());
    assert_eq!(manual, 1);
}

/// 대학이 재학생 우선이면 트랙 동점도 재학생이 앞선다.
#[tokio::test]
async fn university_priority_breaks_track_tie() {
    let cfg = Cfg {
        total: Some(2), univ_prio: true,
        tracks: vec![t(None, true), t(None, true)],
        cands: vec![c(0, 90, true), c(0, 90, false), c(1, 85, true)], univ2: None, prior: vec![],
    };
    let (picked, manual) = auto(&cfg).await;
    assert_eq!(picked, vec![0, 2]);
    assert_eq!(manual, 0);
}

/// 대학 정원 0 — 아무도 확정되지 않고 수동도 거부한다.
#[tokio::test]
async fn zero_total_quota_confirms_nobody() {
    let cfg = Cfg {
        total: Some(0), univ_prio: false,
        tracks: vec![t(None, false)],
        cands: vec![c(0, 90, true), c(0, 80, true)], univ2: None, prior: vec![],
    };
    let (picked, manual) = auto(&cfg).await;
    assert_eq!(picked, Vec::<usize>::new());
    assert_eq!(manual, 0);

    let pool = common::create_test_pool().await;
    let b = build(&pool, &cfg).await;
    assert!(rec_manual(&pool, &b, 0).await.is_err());
}

/// 같은 학생이 같은 대학 두 트랙에 동점 — 행 단위로 세어 1석에 2행 경합.
#[tokio::test]
async fn same_student_two_tracks_counts_rows() {
    let cfg = Cfg {
        total: Some(1), univ_prio: false,
        tracks: vec![t(Some(1), false), t(Some(1), false)],
        cands: vec![c(0, 90, true), c(1, 80, true)], univ2: None, prior: vec![],
    };
    // build 는 후보마다 학생을 만든다 — 같은 학생 두 지원은 직접 만든다
    let pool = common::create_test_pool().await;
    let b = build(&pool, &cfg).await;
    // 두 번째 학생의 지원을 지우고, 첫 학생을 두 번째 트랙에도 같은 점수로 넣은 뒤 다시 계산
    let rid = b.rid;
    sqlx::query("UPDATE rounds SET status = 'OPEN', closed_at = NULL WHERE id = ?")
        .bind(rid).execute(&pool).await.unwrap();
    sqlx::query("DELETE FROM results WHERE round_id = ?").bind(rid).execute(&pool).await.unwrap();
    sqlx::query("DELETE FROM applications WHERE student_id = ? AND round_id = ?")
        .bind(b.keys[1].0).bind(rid).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO applications (student_id, track_id, round_id, department_name) VALUES (?, ?, ?, '학과')")
        .bind(b.keys[0].0).bind(b.track_ids[1]).bind(rid).execute(&pool).await.unwrap();
    let _closed = close_round(st(&pool), Path(rid)).await.unwrap();

    let resp = auto_recommend_results(st(&pool), Path(rid)).await.unwrap().0;
    assert!(recommended(&pool, rid).await.is_empty(), "1석에 2행 동점 — 자동 확정 금지");
    assert_eq!(resp.manual.len(), 1, "대학 단위 동점 1건");
    recommend_result(st(&pool), Path((b.keys[0].0, b.track_ids[1], rid))).await
        .expect("관리자는 어느 한쪽을 고를 수 있다");
    assert!(recommend_result(st(&pool), Path((b.keys[0].0, b.track_ids[0], rid))).await.is_err());
}

/// 보류 덩어리에서 멈췄는데 대학 자리는 남은 경우 — 대학 단위 사유가 "N석에 M명 경합"
/// 처럼 모순된 숫자를 쓰지 않고 멈춘 이유를 말해야 한다(2026-10-07 수정 감사 C-2).
/// 그리고 자동이 보수적일 뿐 틀리지 않음을 확인한다: 관리자는 Y1(같은 1위)을 고를 수 있고
/// Z1(80)은 아직 막히며, 동점을 정리한 뒤 다시 돌리면 Z1 이 확정된다.
#[tokio::test]
async fn held_stop_with_seats_left_explains_itself() {
    let cfg = Cfg {
        total: Some(3), univ_prio: false,
        tracks: vec![t(Some(1), false), t(Some(1), false), t(Some(1), false)],
        cands: vec![c(0, 90, true), c(0, 90, true), c(1, 90, true), c(2, 80, true)],
        univ2: None, prior: vec![],
    };
    let pool = common::create_test_pool().await;
    let b = build(&pool, &cfg).await;
    let resp = auto_recommend_results(st(&pool), Path(b.rid)).await.unwrap().0;
    assert_eq!(names(&b, &recommended(&pool, b.rid).await), Vec::<usize>::new());
    let univ_reason = resp.manual.iter().find(|m| m.track_id.is_none())
        .map(|m| m.reason.clone()).expect("대학 단위 사유가 있어야 한다");
    assert!(univ_reason.contains("정리되지 않아"), "멈춘 이유를 말해야 한다: {univ_reason}");
    assert!(!univ_reason.contains("명 경합"), "남은 자리보다 적은 경합 인원을 적으면 모순: {univ_reason}");

    rec_manual(&pool, &b, 2).await.expect("Y1 은 대학 1위 동순위 — 관리자가 고를 수 있다");
    assert!(rec_manual(&pool, &b, 3).await.is_err(), "Z1 은 X 동점이 미결정인 동안 막힌다");
    rec_manual(&pool, &b, 0).await.expect("X 동점 중 하나를 고른다");
    let resp = auto_recommend_results(st(&pool), Path(b.rid)).await.unwrap().0;
    assert_eq!(names(&b, &recommended(&pool, b.rid).await), vec![0, 2, 3]);
    assert!(resp.manual.is_empty());
}

/// 4차 수정 감사 B-1: 아직 **선두가 아닌** 보류 덩어리가 일반 동점과 같은 대학 순위에 있을 때,
/// 사유가 "모집단위 동점이 정리되지 않아"로 잘못 나왔다. 트랙(재학생 우선)과 대학(점수만)의
/// 플래그가 다를 때 생긴다 — 트랙 0 은 재학생 c0(70)이 선두라 졸업생 동점(90, 대학 1위)은
/// 그 뒤에 보류되고, 트랙 1 의 c3·c4(90, 대학 1위)가 대학 1석을 두고 경합한다.
/// 진짜 원인은 c3·c4 의 대학 단위 동점이다.
#[tokio::test]
async fn ordinary_univ_tie_is_not_reported_as_held_stop() {
    let cfg = Cfg {
        total: Some(1), univ_prio: false,
        tracks: vec![t(Some(2), true), t(Some(2), false)],
        cands: vec![c(0, 70, true), c(0, 90, false), c(0, 90, false), c(1, 90, true), c(1, 90, true)],
        univ2: None, prior: vec![],
    };
    let pool = common::create_test_pool().await;
    let b = build(&pool, &cfg).await;
    let resp = auto_recommend_results(st(&pool), Path(b.rid)).await.unwrap().0;
    let univ_reason = resp.manual.iter().find(|m| m.track_id.is_none())
        .map(|m| m.reason.clone()).expect("대학 단위 동점 사유가 있어야 한다");
    assert!(univ_reason.contains("1석에 2명 경합"), "일반 동점 문장이어야 한다: {univ_reason}");
    assert!(!univ_reason.contains("정리되지 않아"), "보류 정지로 잘못 적었다: {univ_reason}");
}

/// 대학 단위 사유 끝의 정원 숫자는 이번 실행에서 확정한 인원까지 반영한다(5차 수정 감사 C-2).
/// X1 을 확정한 뒤 X 동점에서 멈춘 경우 — 실행 전 값("확정 0명, 잔여 2석")이 아니어야 한다.
#[tokio::test]
async fn univ_reason_counts_include_this_run() {
    let cfg = Cfg {
        total: Some(2), univ_prio: false,
        tracks: vec![t(Some(2), false), t(Some(1), false)],
        cands: vec![c(0, 95, true), c(0, 90, true), c(0, 90, true), c(1, 80, true)],
        univ2: None, prior: vec![],
    };
    let pool = common::create_test_pool().await;
    let b = build(&pool, &cfg).await;
    let resp = auto_recommend_results(st(&pool), Path(b.rid)).await.unwrap().0;
    assert_eq!(names(&b, &recommended(&pool, b.rid).await), vec![0]);
    let univ_reason = resp.manual.iter().find(|m| m.track_id.is_none())
        .map(|m| m.reason.clone()).expect("대학 단위 사유");
    assert!(univ_reason.contains("이번 실행 포함 확정 1명, 잔여 1석"), "{univ_reason}");
}

/// 대학 순위가 다른 보류 덩어리 둘 — 더 좋은 쪽에서 멈춘다. 그 사이 순위의 Y1 도, 아래
/// 덩어리의 후보도 자동 확정되지 않고, 수동도 둘 다 막는다.
#[tokio::test]
async fn two_held_blocks_stop_at_the_better_one() {
    let cfg = Cfg {
        total: Some(4), univ_prio: false,
        tracks: vec![t(Some(1), false), t(Some(1), false), t(Some(1), false)],
        cands: vec![c(0, 90, true), c(0, 90, true), c(1, 80, true), c(1, 80, true), c(2, 85, true)],
        univ2: None, prior: vec![],
    };
    let pool = common::create_test_pool().await;
    let b = build(&pool, &cfg).await;
    let resp = auto_recommend_results(st(&pool), Path(b.rid)).await.unwrap().0;
    assert_eq!(names(&b, &recommended(&pool, b.rid).await), Vec::<usize>::new());
    assert!(resp.manual.len() >= 2, "두 트랙 동점 + 대학 단위");
    assert!(rec_manual(&pool, &b, 4).await.is_err(), "Y1(85)은 X 동점(90) 미결정으로 막힌다");
    assert!(rec_manual(&pool, &b, 2).await.is_err(), "Z(80)도 막힌다");
}

/// 이전 라운드 추천자가 트랙 정원을 다 쓴 경우 — 그 트랙의 동점은 경합할 자리가 없으므로
/// 보류 덩어리도 아니고, 다른 트랙을 막지도 않는다.
#[tokio::test]
async fn prior_round_fills_track_so_its_tie_blocks_nobody() {
    let cfg = Cfg {
        total: Some(3), univ_prio: false,
        tracks: vec![t(Some(1), false), t(Some(1), false)],
        cands: vec![c(0, 90, true), c(0, 90, true), c(1, 80, true)],
        univ2: None, prior: vec![0],
    };
    let (picked, manual) = auto(&cfg).await;
    assert_eq!(picked, vec![2], "X 는 이전 라운드로 만석 — Y1 확정");
    assert_eq!(manual, 0);
}

// ── 2. 불변식 ─────────────────────────────────────────────────────

/// 결정적 의사난수 (xorshift). 실패한 구성을 그대로 다시 만들 수 있게 시드를 메시지에 남긴다.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn pick(&mut self, n: u64) -> u64 { self.next() % n }
    fn chance(&mut self, pct: u64) -> bool { self.pick(100) < pct }
}

fn gen_cfg(rng: &mut Rng) -> Cfg {
    let quota = |rng: &mut Rng| match rng.pick(5) { 0 => None, 1 => Some(0), 2 => Some(1), 3 => Some(2), _ => Some(3) };
    let n_tracks = 1 + rng.pick(3) as usize;
    let univ_prio = rng.chance(30);
    let total = match rng.pick(5) { 0 => None, 1 => Some(1), 2 => Some(2), 3 => Some(3), _ => Some(4) };
    // 30% 는 대학 둘 — 트랙을 두 대학에 나눠 대학 단위 계산이 섞이지 않는지 본다
    let univ2 = rng.chance(30).then(|| {
        let total2 = match rng.pick(4) { 0 => None, 1 => Some(1), 2 => Some(2), _ => Some(3) };
        (total2, rng.chance(30))
    });
    let tracks: Vec<Track> = (0..n_tracks)
        .map(|_| Track {
            quota: quota(rng),
            prio: rng.chance(40),
            univ: if univ2.is_some() { rng.pick(2) as usize } else { 0 },
        })
        .collect();
    // 30% 는 이전 라운드 추천자 1~2명 — 정원 집계가 라운드를 넘어 누적되는 경로를 태운다
    let prior: Vec<usize> = if rng.chance(30) {
        (0..1 + rng.pick(2)).map(|_| rng.pick(n_tracks as u64) as usize).collect()
    } else {
        vec![]
    };
    let n_cands = 1 + rng.pick(7) as usize;
    let cands = (0..n_cands)
        .map(|_| Cand {
            track: rng.pick(n_tracks as u64) as usize,
            // 세 값만 써서 동점을 자주 만든다
            score: [90, 85, 80][rng.pick(3) as usize],
            enrolled: rng.chance(70),
            excluded: rng.chance(10),
        })
        .collect();
    Cfg { total, univ_prio, tracks, cands, univ2, prior }
}

#[tokio::test]
async fn auto_and_manual_agree_on_generated_configs() {
    const CASES: u64 = 400;
    // 생성기가 갈래(대학 둘·이전 라운드·수동 항목·보류 덩어리에서 멈춘 대학 컷)를 실제로
    // 만드는지 센다 — 0 이면 그 갈래는 검사 밖이다(생성기 수정이 조용히 그 갈래를 끄는 것을 막는다).
    let (mut n_univ2, mut n_prior, mut n_manual, mut n_held_stop) = (0, 0, 0, 0);
    for seed in 1..=CASES {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ seed.wrapping_mul(0x2545_F491_4F6C_DD1D));
        let cfg = gen_cfg(&mut rng);
        n_univ2 += cfg.univ2.is_some() as u32;
        n_prior += (!cfg.prior.is_empty()) as u32;

        // 자동
        let pool_a = common::create_test_pool().await;
        let ba = build(&pool_a, &cfg).await;
        let resp = auto_recommend_results(st(&pool_a), Path(ba.rid)).await.unwrap().0;
        let auto_set = recommended(&pool_a, ba.rid).await;
        let auto_idx = names(&ba, &auto_set);
        n_manual += (!resp.manual.is_empty()) as u32;
        n_held_stop += resp.manual.iter().any(|m| m.reason.contains("정리되지 않아")) as u32;

        // (c) 정원 — 이전 라운드 추천자까지 합쳐 센다. 이전 인원이 이미 정원 이상이면
        //     이번 라운드 확정은 0 이어야 한다(정원 하향 등으로 생길 수 있는 상태).
        for (ti, tr) in cfg.tracks.iter().enumerate() {
            if let Some(q) = tr.quota {
                let prior = cfg.prior.iter().filter(|&&p| p == ti).count() as i64;
                let n = auto_idx.iter().filter(|&&i| cfg.cands[i].track == ti).count() as i64;
                assert!(n <= (q - prior).max(0), "seed {seed}: 트랙{ti} 정원 {q}(이전 {prior}) 초과 {n} — {cfg:?}");
            }
        }
        let univ_totals: Vec<Option<i64>> =
            std::iter::once(cfg.total).chain(cfg.univ2.map(|u| u.0)).collect();
        for (ui, total) in univ_totals.iter().enumerate() {
            if let Some(q) = total {
                let in_univ = |ti: usize| cfg.tracks[ti].univ == ui;
                let prior = cfg.prior.iter().filter(|&&p| in_univ(p)).count() as i64;
                let n = auto_idx.iter().filter(|&&i| in_univ(cfg.cands[i].track)).count() as i64;
                assert!(n <= (q - prior).max(0), "seed {seed}: 대학{ui} 정원 {q}(이전 {prior}) 초과 {n} — {cfg:?}");
            }
        }
        for &i in &auto_idx {
            assert!(!cfg.cands[i].excluded, "seed {seed}: 미선발 {i} 확정 — {cfg:?}");
        }

        // (a) 수동 재현 — 순서를 모르므로 되는 것부터 반복해서 넣는다
        let pool_m = common::create_test_pool().await;
        let bm = build(&pool_m, &cfg).await;
        let mut todo: Vec<usize> = auto_idx.clone();
        let mut last_err = Vec::new();
        loop {
            let before = todo.len();
            let mut next = Vec::new();
            last_err.clear();
            for i in todo {
                match rec_manual(&pool_m, &bm, i).await {
                    Ok(()) => {}
                    Err(e) => { last_err.push((i, e)); next.push(i); }
                }
            }
            todo = next;
            if todo.is_empty() || todo.len() == before {
                break;
            }
        }
        assert!(
            todo.is_empty(),
            "seed {seed}: 자동이 확정한 {todo:?} 를 수동으로 재현할 수 없다 {last_err:?}\n자동 확정 {auto_idx:?}\n{cfg:?}"
        );

        // (b) 자동이 관리자 판단 없이 끝났다면 수동으로 더 넣을 수 있는 후보가 없어야 한다
        if resp.manual.is_empty() {
            for i in 0..cfg.cands.len() {
                if auto_idx.contains(&i) || cfg.cands[i].excluded {
                    continue;
                }
                assert!(
                    rec_manual(&pool_m, &bm, i).await.is_err(),
                    "seed {seed}: 자동은 판단 없이 끝났는데 수동으로 {i} 를 더 추천할 수 있다\n자동 확정 {auto_idx:?}\n{cfg:?}"
                );
            }
        }
    }
    assert!(n_univ2 > 0 && n_prior > 0 && n_manual > 0 && n_held_stop > 0,
        "생성 구성이 갈래를 덮지 못한다: 대학 둘 {n_univ2}, 이전 라운드 {n_prior}, 수동 항목 {n_manual},          보류 정지 {n_held_stop}");
}
