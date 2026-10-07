//! 자동 추천 ≡ 수동 가드 — 2026-10-07 채점·추천 정합성 감사(F-1·F-2)의 회귀 방어선.
//!
//! 자동 추천(`run_auto_recommend`)과 수동 추천(`recommend_result` 의 5b·5c 가드)은 같은
//! 규칙("모집단위 순서를 건너뛰지 않는다", "대학 컷은 대학 순위 순")을 두 번 구현한다.
//! 둘이 어긋나면 **자동은 하는데 관리자는 못 하는 결정**(F-1)이나 **수동으로는 누구도
//! 추천할 수 없는 교착**(F-2)이 생긴다. 그날 감사 전까지 이 등가를 직접 보는 테스트가 없었다.
//!
//! 네 부분으로 나눈다.
//! 1. **시나리오** — 감사가 추적한 최소 재현과 경계(동점 × 정원 × 재학생 우선 × 미선발 ×
//!    이전 라운드), 운영 흐름(수동 결정 뒤 자동 재실행, 미선발 해제, 추천 취소, 재계산,
//!    대학 지정 자동 추천), 1단계 동점 사유가 2단계 뒤 대학 정원이 찼는지를 따르는가
//!    (재감사 C-1), 그리고 동순위 선두가 둘 이상의 모집단위에서 나올 때 2단계가 멈추는가
//!    (`contention_*`, 2026-10-07 소유자 결정 B-merge). 실제 `close_round` 가 쓴 순위 위에서 돈다.
//! 2. **불변식** — 작은 구성을 결정적 의사난수로 많이 만들어
//!    (a) 자동이 확정한 집합을 수동 `recommend_result` 로 **전부** 재현할 수 있는가,
//!    (b) 자동이 수동 판단 항목 없이 끝났다면 수동으로 **더** 추천할 수 있는 후보가 없는가,
//!    (c) 모집단위·대학 정원을 넘지 않는가 를 본다. 새 상태에서 자동을 한 번만 돌린다.
//!    **(a) 의 한계**: 재현 루프는 인덱스 순으로 넣고 안 되는 것만 다시 시도한다 — 순서에
//!    민감하다. 어떤 순서로는 되는 집합을 다른 순서로 먼저 넣어 영영 막을 수 있다(S1 구성에서
//!    {X1, Y1} 은 Y1→X1 순서로는 되지만 X1 을 먼저 넣으면 X2 가 Y1 을 막는다). 그래서 (a) 가
//!    실패하면 "자동≡수동 위반"과 "루프가 순서를 못 찾음"을 구분해야 한다. 4 의 DFS 가 이
//!    한계 없이 같은 질문을 본다 — 이 루프는 싸서 남겨 둔다.
//! 3. **다단계 불변식** — 2 의 구성에 조작열(자동·추천·취소·미선발·해제)을 더한다. 같은
//!    조작열을 두 풀에 똑같이 적용한 뒤 한 풀에서 자동을 한 번 더 돌려,
//!    (a) 그 실행이 새로 확정한 집합을 다른 풀에서 수동으로 재현할 수 있는가,
//!    (b) 그 실행이 판단 항목 없이 끝났다면 수동으로 더 넣을 후보가 없는가,
//!    (c) 조작마다 정원을 넘지 않는가 를 본다. 2 가 보지 않는 "수동 결정이 끼어든 상태"를 덮는다.
//!    재현 루프의 한계는 2 와 같다.
//! 4. **수동 DFS 전수 불변식** — 같은 시작 상태에서 수동 추천만으로 갈 수 있는 상태를 DFS 로
//!    전부 모은다(한 풀에서 `recommend_result` → 재귀 → `unrecommend_result` 로 되돌림, 방문한
//!    집합 메모). 그다음
//!    (a′) 자동 확정 집합이 방문한 상태 가운데 하나인가,
//!    (d) 자동 확정 집합 ⊆ 모든 최대 결과(더 추천할 수 없는 상태)의 교집합인가,
//!    (e) 자동이 판단 항목 없이 끝났으면 최대 결과가 정확히 하나이고 자동 결과와 같은가 를 본다.
//!    생성기는 2 의 `gen_cfg` 와 **편향 모드**(`gen_cfg_biased` — 대학 재학생 우선 0, 일부
//!    모집단위만 재학생 우선, 졸업생 점수를 재학생 이상으로, 재학생끼리 동점이 잦게) 둘 다 쓴다.
//!    편향 구조가 실제로 나왔는지는 "경합 집합 사유가 나온 구성 수 > 0" 과 "최대 결과가 둘
//!    이상인 구성 수 > 0" 으로 센다.
//!
//! 판별력의 소재 — 2026-10-07 이 파일(합친 판)에 변이를 넣어 확인한 결과다. 코드가 바뀌면
//! 다시 확인해야 하는 기록이다. "확인함"은 이 파일을 쓴 사람이 변이를 직접 돌린 것,
//! "예상"은 손으로 추적했을 뿐 돌리지 않은 것이다.
//! (바로 아래 "확인함" 항목들은 커밋 직전 판에서 오케스트레이터가 다시 돌려 같은 결과를 얻었다.)
//! - **확인함**: `merge_univ_cut_held` 의 경합 집합 규칙을 끄면(`group.len() >= 2` 조건을
//!   거짓으로) `contention_s1`~`s4` 시나리오, `track_tie_reason_does_not_prescribe_exclusion`,
//!   DFS 불변식(`auto_matches_manual_dfs_reachability` — (a′): 편향 생성기가 만든 S2 류 구성에서
//!   자동 결과가 수동 도달 불가)이 실패하고, `handler_auto_recommend.rs` 에서는
//!   `merge_held_contention_s1_stops_without_confirming`·`merge_held_contention_s2_rank_and_counts_and_fits_when_room`·
//!   `merge_held_contention_s4_all_rank_one`·`merge_held_contention_when_group_exceeds_room_but_exposure_can_enter`·
//!   `merge_held_contenders_exclude_candidates_that_can_never_enter`·
//!   `merge_held_block_behind_full_chain_counts_for_demand_not_contenders`·
//!   `merge_held_block_seats_count_toward_demand` 가 실패한다. 2·3 의 재현 루프 불변식은 이
//!   변이를 잡지 못했다 — `gen_cfg` 가 그 구조를 만들지 않았거나 루프가 다른 순서로 재현했다
//!   (어느 쪽인지 가르지 않았다). 그래서 4 가 필요했다. 처음 쓴 편향 생성기는 "선두 그룹은
//!   들어가는데 연쇄까지는 못 들어가는" 대학 정원을 거의 뽑지 않아 커버 카운터로만 잡혔다 —
//!   모집단위마다 재학생 80 선두를 먼저 넣고 정원 2~3 을 자주 뽑게 고친 뒤 (a′) 로 잡는다.
//!   (`merge_held_contention_with_no_room_is_clean` 은 규칙을 꺼도 깨끗한 끝이라 통과한다.)
//! - **확인함**: 연쇄를 r 보다 나쁜 후보에서 끊지 않으면(`c.univ_rank <= r` 를 항상 참으로)
//!   `contention_chain_is_cut_at_worse_rank_so_auto_fills` 와 `handler_auto_recommend.rs` 의
//!   `merge_held_chain_is_cut_at_worse_rank`·`merge_equals_fill_when_flags_align`·
//!   `merge_equals_fill_when_track_rank_numbering_differs` 가 실패한다. DFS 불변식은 **구조상**
//!   이 변이를 잡지 못한다 — 변이는 결과를 바꾸는 곳마다 경합 판단 항목을 내므로 (e) 의 전제
//!   (판단 항목 없음)가 서지 않고, (a′)(d) 는 덜 확정하는 쪽을 막지 않는다. 시나리오와 단위
//!   테스트가 방어선이다.
//! - **확인함**: 보류 덩어리의 `seats` 를 수요에 넣지 않으면 `contention_s3_*`,
//!   `track_tie_reason_does_not_prescribe_exclusion`, DFS 불변식((d): 자동이 확정한 후보가 어떤
//!   수동 최대 결과에 없다), `handler_auto_recommend.rs` 의 `merge_held_block_seats_count_toward_demand`
//!   가 실패한다.
//! - **확인함**(검토 뒤 B1·B2 수정 시): 노출만 있으면 `Contention` 으로 내게 하면(`can_enter` 를
//!   항상 참으로) `contention_rule_keeps_plain_tie_when_exposed_candidate_cannot_enter` 와
//!   `handler_auto_recommend.rs` 의 `merge_held_exposure_that_cannot_enter_is_ordinary_tie` 가
//!   실패한다. 경합 인원에 연쇄 전원을 세면(`run_start < free`·`chained < free` 조건 제거)
//!   `merge_held_contenders_exclude_candidates_that_can_never_enter`·
//!   `merge_held_block_behind_full_chain_counts_for_demand_not_contenders` 가 실패한다. 둘 다
//!   확정·정지 결정은 바꾸지 않고 사유 문장만 바꾸는 변이라 DFS 불변식은 잡지 못한다(예상대로).
//! - **확인함**(2026-10-07 마감 감사 수정 — 1단계 동점 사유 세 갈래): (c) 판정을 (a) 로 되돌리면
//!   (`run_auto_recommend` 4-1 의 `Open` 갈래에서 `held_stop_tracks` 조건을 빼 자리가 남으면 전부
//!   "관리자 선택 필요") `track_tie_behind_university_tie_says_its_turn_has_not_come`·
//!   `track_tie_behind_contention_says_its_turn_has_not_come`·
//!   `held_stop_names_only_the_leading_block_in_track_tie_reasons`·`track_tie_reason_does_not_prescribe_exclusion`·
//!   `contention_s3_with_held_blocks_on_both_chains` 와 DFS 불변식 (f)(생성 구성에서 "관리자 선택
//!   필요"인데 덩어리의 누구도 지금 추천되지 않는다)가 실패한다. 정지 덩어리 판정을 "같은 순위면
//!   전부"로 넓히면(`merge_univ_cut_held` 에서 선두가 아닌 덩어리도 `Held::tracks` 에 넣으면)
//!   `held_stop_names_only_the_leading_block_in_track_tie_reasons` 와 `handler_auto_recommend.rs` 의
//!   `merge_held_tracks_name_only_leading_blocks` 가 실패한다 — DFS 불변식은 이 변이를 잡지 못했다.
//!   그 구성("같은 대학 순위의 덩어리 둘 가운데 하나만 선두")을 편향 생성기(`gen_cfg_biased`)는
//!   **구조상 만들 수 없다**: 선두가 아닌 덩어리는 모집단위의 재학생 80 선두 뒤에 선 졸업생(85/90)
//!   둘이어야 하고, 선두인 덩어리는 같은 점수의 둘이어야 하는데(다른 모집단위의 재학생 80 선두는 그
//!   점수가 아니라 못 낀다) 덧붙이는 후보가 최대 3 명이라 4 명이 안 된다. 일반 생성기(`gen_cfg`)는
//!   만들 수 있지만 드물고, 이 표본에서는 나오지 않았다(변이가 통과했다). 시나리오와 단위 테스트가
//!   방어선이다.
//! - **확인함**(2026-10-08 최종 감사 F1 C-1 수정 — 보류 정지 사유가 같은 순위 선두를 적는다):
//!   병합의 `Held::leaders` 를 0 으로 고정하면 `held_tie_equal_to_other_track_leader_is_manual`·
//!   `held_stop_with_seats_left_explains_itself`·`held_stop_contest_compares_block_seats_not_contenders`
//!   와 DFS 불변식 (g), `handler_auto_recommend.rs` 의 `merge_held_stop_reports_real_leaders_at_the_same_rank`·
//!   `merge_held_block_as_leader_stops_before_contention` 이 실패한다. `leaders` 에 선두 뒤 연쇄
//!   (대학 순위 ≤ r)까지 세면 DFS 불변식 (g) 와 같은 두 단위 테스트가 실패한다 — 시나리오는 이 변이를
//!   잡지 못했다. 호출부 문장 갈래를 늘 옛 문장으로 하면 `leaders` 0 고정과 같은 넷이, 늘 새 문장으로
//!   하면 `held_tie_stops_university_cut_until_resolved` 만 실패한다(새 문장이 "0명"을 적어 (g) 는
//!   통과한다). "다툼" 조건을 늘 참으로 하면 `held_stop_with_seats_left_explains_itself`·
//!   `held_stop_contest_compares_block_seats_not_contenders`, 늘 거짓으로 하면
//!   `held_tie_equal_to_other_track_leader_is_manual`, 경합 인원 합(`tie.contenders`)과 비교하면
//!   `held_stop_contest_compares_block_seats_not_contenders` 만 실패한다. (g) 는 "다툼" 문장을 보지
//!   않으므로 이 셋을 잡지 못한다(예상대로).
//! 아래는 합친 판 작성 시점의 기록이다(B-merge 전 코드에서 확인함).
//! - F-1 수정을 되돌리면(`merge_univ_cut_held` 의 `held_head` 가 항상 None) 보류 덩어리가 대학
//!   컷을 막아야 하는 `held_*` 시나리오들, `two_held_blocks_stop_at_the_better_one`,
//!   `univ_reason_counts_include_this_run`, `exclude_then_clear_then_auto_is_consistent_and_finalizable`,
//!   `unrecommend_after_held_resolution_makes_auto_stop_again`, 두 불변식
//!   (`auto_and_manual_agree_on_generated_configs`, `auto_and_manual_agree_after_manual_interleaving`)이
//!   실패한다.
//! - `run_auto_recommend` 의 후보 필터에서 `recommended == 0` 조건을 빼면
//!   `auto_and_manual_agree_after_manual_interleaving` 이 실패한다.
//! - F-2 수정(5c 블로커의 선두 조건)을 되돌리면
//!   `cross_track_guard_ignores_candidates_blocked_in_their_own_track` 과
//!   `auto_and_manual_agree_on_generated_configs` 가 실패한다. 다단계 불변식은 이 변이를 잡지 못했다.
//! - C-1 판정(2단계 뒤 대학 정원이 찼는가)의 경계를 `<= 0` 에서 `< 0` 으로 바꾸거나 판정을
//!   무력화하면 "대학 정원이 찼습니다"를 단언하는 `track_tie_*` 테스트들이 실패한다. 판정을
//!   `remaining_after <= 0 && held_stop_seats > 0`(당시 변수 — 지금은 `UnivCutStop::Held`)으로
//!   좁히면 `track_tie_reason_says_university_full_after_clean_university_cut` 만 실패한다.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;

use axum::{
    extract::{Path, Query, State},
    Json,
};
use principal_candidate_manager::{
    handlers::{
        applications::{clear_application_exclusion, exclude_application, ExcludeApplicationBody},
        rounds::{close_round, finalize_round},
        scoring::{
            auto_recommend_results, auto_recommend_results_univ, calculate_scores, get_results,
            recommend_result, unrecommend_result, AutoRecommendManualItem, ResultQuery,
        },
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
    /// 대학 인덱스(0 = 첫 대학, 1 = 둘째 대학)의 university id
    univ_ids: Vec<i64>,
    /// 점수를 담은 MANUAL 전형요소 id — 재계산 시나리오가 기초데이터를 바꿀 때 쓴다
    area: i64,
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
    let univ_ids = univs.iter().map(|(id, _)| *id).collect();
    Built { rid, keys, track_ids, univ_ids, area }
}

async fn recommended(pool: &SqlitePool, rid: i64) -> BTreeSet<(i64, i64)> {
    let rows: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT student_id, track_id FROM results WHERE round_id = ? AND recommended = 1 \
         ORDER BY student_id, track_id",
    )
    .bind(rid).fetch_all(pool).await.unwrap();
    rows.into_iter().collect()
}

async fn excluded_now(pool: &SqlitePool, rid: i64) -> BTreeSet<(i64, i64)> {
    let rows: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT student_id, track_id FROM applications WHERE round_id = ? AND excluded = 1 \
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

async fn unrec(pool: &SqlitePool, b: &Built, i: usize) -> Result<(), String> {
    unrecommend_result(st(pool), Path((b.keys[i].0, b.keys[i].1, b.rid)))
        .await
        .map(|_| ())
        .map_err(|(s, m)| format!("{s}: {m}"))
}

async fn excl(pool: &SqlitePool, b: &Built, i: usize) -> Result<(), String> {
    exclude_application(
        st(pool),
        Path((b.keys[i].0, b.keys[i].1, b.rid)),
        Json(ExcludeApplicationBody { reason: "감사".into() }),
    )
    .await
    .map(|_| ())
    .map_err(|(s, m)| format!("{s}: {m}"))
}

async fn clear(pool: &SqlitePool, b: &Built, i: usize) -> Result<(), String> {
    clear_application_exclusion(st(pool), Path((b.keys[i].0, b.keys[i].1, b.rid)))
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

/// 이미 만든 풀에서 자동 추천을 한 번 더 돌려 수동 항목의 사유를 돌려준다.
async fn auto_run(pool: &SqlitePool, b: &Built) -> Vec<String> {
    auto_recommend_results(st(pool), Path(b.rid)).await.unwrap().0.manual
        .into_iter().map(|m| m.reason).collect()
}

/// 정원 불초과 — DB 가 세는 값으로 본다(이전 라운드 포함). 이전 인원이 이미 정원 이상이면
/// 그 값에서 늘어나지 않았어야 한다.
async fn assert_quotas(pool: &SqlitePool, b: &Built, cfg: &Cfg, tag: &str) {
    for (ti, tr) in cfg.tracks.iter().enumerate() {
        if let Some(q) = tr.quota {
            let prior = cfg.prior.iter().filter(|&&p| p == ti).count() as i64;
            let n: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM results r JOIN applications a \
                 ON a.student_id = r.student_id AND a.track_id = r.track_id AND a.round_id = r.round_id \
                 WHERE r.track_id = ? AND r.recommended = 1 AND a.abandoned = 0",
            )
            .bind(b.track_ids[ti]).fetch_one(pool).await.unwrap();
            assert!(n <= q.max(prior), "{tag}: 트랙{ti} 정원 {q}(이전 {prior}) 초과 {n} — {cfg:?}");
        }
    }
    let totals: Vec<Option<i64>> = std::iter::once(cfg.total).chain(cfg.univ2.map(|u| u.0)).collect();
    for (ui, total) in totals.iter().enumerate() {
        if let Some(q) = *total {
            let prior = cfg.prior.iter().filter(|&&p| cfg.tracks[p].univ == ui).count() as i64;
            let n: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM results r JOIN applications a \
                 ON a.student_id = r.student_id AND a.track_id = r.track_id AND a.round_id = r.round_id \
                 JOIN univ_tracks ut ON ut.id = r.track_id \
                 WHERE ut.univ_id = ? AND r.recommended = 1 AND a.abandoned = 0",
            )
            .bind(b.univ_ids[ui]).fetch_one(pool).await.unwrap();
            assert!(n <= q.max(prior), "{tag}: 대학{ui} 정원 {q}(이전 {prior}) 초과 {n} — {cfg:?}");
        }
    }
}

/// `build` 는 후보마다 학생을 만든다 — 같은 학생의 두 지원은 여기서 만든다. 둘째 후보의 지원을
/// 지우고 첫 후보의 학생을 둘째 트랙에도 넣은 뒤 다시 마감한다(점수는 트랙 지정 없는 기초데이터라
/// 두 지원이 같다). 전제: cands[0] 은 트랙 0, cands[1] 은 트랙 1.
async fn first_student_in_both_tracks(pool: &SqlitePool, b: &Built) {
    let rid = b.rid;
    sqlx::query("UPDATE rounds SET status = 'OPEN', closed_at = NULL WHERE id = ?")
        .bind(rid).execute(pool).await.unwrap();
    sqlx::query("DELETE FROM results WHERE round_id = ?").bind(rid).execute(pool).await.unwrap();
    sqlx::query("DELETE FROM applications WHERE student_id = ? AND round_id = ?")
        .bind(b.keys[1].0).bind(rid).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO applications (student_id, track_id, round_id, department_name) VALUES (?, ?, ?, '학과')")
        .bind(b.keys[0].0).bind(b.track_ids[1]).bind(rid).execute(pool).await.unwrap();
    let _closed = close_round(st(pool), Path(rid)).await.unwrap();
}

/// 1단계 모집단위 동점 사유의 세 갈래(2026-10-07 재감사 C-1·마감 감사) — (b) 2단계 뒤 대학 정원이
/// 찼다, (a) 지금 고를 수 있다(무제한이거나 2단계가 바로 그 덩어리에서 멈춤), (c) 자리는 남았는데
/// 2단계가 다른 곳에서 멈춰 아직 차례가 아니다.
const REASON_UNIV_FULL: &str = "대학 정원이 찼습니다";
const REASON_CHOICE: &str = "관리자 선택 필요";
const REASON_NOT_YET: &str = "아직 이 동점의 차례가 오지 않았습니다";
/// 대학 단위 사유 세 갈래를 가르는 조각 — 보류 정지, 경합 집합 정지(B-merge), 일반 동점.
const REASON_HELD_STOP: &str = "정리되지 않아";
const REASON_CONTENTION: &str = "누구를 먼저 추천하느냐";
const REASON_PLAIN_TIE: &str = "명 경합 (경합 대상은";
/// 보류 정지 문장의 두 형태(최종 감사 F1 C-1) — 같은 대학 순위에 다른 모집단위 선두가 없으면 옛
/// 문장("모집단위 동점을 먼저 정리한 뒤"), 있으면 그 인원("…명도 지금 추천할 수 있")을 적고 다음
/// 행동은 중립으로 쓴다. 남은 자리 < 선두 인원 + 덩어리 잔여석이면 다툼("석을 다툽니다")도 적는다.
const REASON_HELD_RESOLVE_FIRST: &str = "모집단위 동점을 먼저 정리한 뒤";
const REASON_HELD_LEADERS: &str = "명도 지금 추천할 수 있";
const REASON_HELD_CONTEST: &str = "석을 다툽니다";

/// 보류 정지 사유가 적은 "같은 대학 순위의 다른 모집단위 지원자 N명"의 N. 그 문장이 없으면 0.
fn held_leaders_of(reason: &str) -> usize {
    let Some(end) = reason.find(REASON_HELD_LEADERS) else { return 0 };
    let head = &reason[..end];
    let digits: String = head.chars().rev().take_while(|c| c.is_ascii_digit()).collect::<Vec<_>>()
        .into_iter().rev().collect();
    digits.parse().unwrap_or_else(|_| panic!("선두 인원을 읽을 수 없다: {reason}"))
}

/// 사유는 상태만 말하고 처방(미선발하라·취소하라)을 하지 않는다 — 명세 §5.4 C-1. "대학 정원이
/// 찼습니다" 문장과 경합 집합 문장을 단언하는 테스트가 함께 부른다.
fn assert_no_prescription(reason: &str) {
    assert!(
        !reason.contains("미선발") && !reason.contains("취소"),
        "상태만 적고 처방하지 않는다: {reason}"
    );
}

/// 대학 단위(track_id 없음) 수동 항목의 사유. 없으면 실패한다.
fn univ_reason(manual: &[AutoRecommendManualItem]) -> String {
    match manual.iter().find(|m| m.track_id.is_none()) {
        Some(m) => m.reason.clone(),
        None => panic!(
            "대학 단위 수동 항목이 없다: {:?}",
            manual.iter().map(|m| m.reason.as_str()).collect::<Vec<_>>()
        ),
    }
}

/// 그 모집단위의 수동 항목 사유. 없으면 실패한다.
fn track_reason(manual: &[AutoRecommendManualItem], track_id: i64) -> String {
    match manual.iter().find(|m| m.track_id == Some(track_id)) {
        Some(m) => m.reason.clone(),
        None => panic!(
            "모집단위 {track_id} 의 수동 항목이 없다: {:?}",
            manual.iter().map(|m| m.reason.as_str()).collect::<Vec<_>>()
        ),
    }
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
///
/// 최종 감사 F1 C-1: 대학 단위 사유가 보류 정지 문장("모집단위 동점을 먼저 정리한 뒤 …")만 적어
/// Y1 을 언급하지 않았다. Y1 도 지금 추천할 수 있고(5c 는 엄격 비교라 동순위를 막지 않는다) X 동점에서
/// 고르면 Y1 은 영영 못 들어오는데, 문장은 결정을 X 쪽으로 유도했다. 지금은 같은 순위의 다른 모집단위
/// 선두 인원(1명)과, 남은 자리(1) < 선두 1 + 덩어리 잔여석 1 이라 같은 자리를 다툰다는 사실을 적고
/// 다음 행동은 중립으로 쓴다.
/// 판별력의 소재: 병합의 `Held::leaders` 를 0 으로 고정하거나 호출부의 문장 갈래 조건을 거짓으로
/// 만들면 옛 문장이 나와 사유 단언이 깨진다. 아래 수동 호출(Y1 통과, X1 뒤 Y1 거부)은 사유가 말하는
/// 사실의 기록이지 방어선이 아니다 — 자동 확정 상태에서 "지금 추천 가능한 비덩어리 후보 수 = 사유의
/// 인원"은 DFS 불변식 (g) 가 생성 구성으로 대조한다.
#[tokio::test]
async fn held_tie_equal_to_other_track_leader_is_manual() {
    let cfg = Cfg {
        total: Some(1), univ_prio: false,
        tracks: vec![t(Some(1), false), t(Some(1), false)],
        cands: vec![c(0, 90, true), c(0, 90, true), c(1, 90, true)], univ2: None, prior: vec![],
    };
    let pool = common::create_test_pool().await;
    let b = build(&pool, &cfg).await;
    let resp = auto_recommend_results(st(&pool), Path(b.rid)).await.unwrap().0;
    assert_eq!(names(&b, &recommended(&pool, b.rid).await), Vec::<usize>::new());
    let u = univ_reason(&resp.manual);
    assert!(u.contains(REASON_HELD_STOP) && u.contains("대학 전체 1위"), "보류 정지 문장: {u}");
    assert_eq!(held_leaders_of(&u), 1, "같은 순위의 다른 모집단위 선두(Y1) 인원을 적는다: {u}");
    assert!(u.contains("대학 잔여 1석을 다툽니다"), "잔여 1 < 선두 1 + 덩어리 1 — 다툼: {u}");
    assert!(!u.contains(REASON_HELD_RESOLVE_FIRST), "덩어리 쪽으로 유도하지 않는다: {u}");
    assert!(!u.contains("명 경합"), "보류 정지 문장은 \"N석에 M명 경합\"을 쓰지 않는다: {u}");
    assert_no_prescription(&u);
    // X 동점은 2단계가 바로 그 덩어리에서 멈췄으므로 (a)
    let rx = track_reason(&resp.manual, b.track_ids[0]);
    assert!(rx.contains(REASON_CHOICE), "{rx}");

    // 기록: Y1 은 지금 수동 추천을 통과한다
    rec_manual(&pool, &b, 2).await.expect("Y1 — 덩어리와 대학 1위 동순위, 5c 가 막지 않는다");
    unrec(&pool, &b, 2).await.expect("되돌리기");
    // 기록: X 동점에서 고르면 대학 정원 1석이 차 Y1 은 들어오지 못한다
    rec_manual(&pool, &b, 0).await.expect("X 동점 중 하나");
    let e = rec_manual(&pool, &b, 2).await.expect_err("X1 뒤에 Y1 이 들어왔다");
    assert!(e.contains("409"), "{e}");
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
    // 같은 순위(대학 1위)에 다른 모집단위 선두가 없다(Y1 은 3위) — 보류 정지 문장은 옛 형태 그대로다
    // (최종 감사 F1 C-1 수정의 반대쪽 경계). 판별력의 소재: 문장 갈래 조건을 항상 참으로 만들면 여기서
    // "0명도 지금 추천할 수 있"이 나와 깨진다.
    let u = univ_reason(&resp.manual);
    assert!(u.contains(REASON_HELD_STOP) && u.contains(REASON_HELD_RESOLVE_FIRST), "{u}");
    assert!(!u.contains(REASON_HELD_LEADERS) && !u.contains(REASON_HELD_CONTEST), "{u}");
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

/// 같은 학생이 같은 대학 두 트랙에 동점 — 행 단위로 세어 1석에 2행 경합
/// (`src/docs/11_release_decisions.md` §11 소유자 결정: 같은 라운드 두 모집단위 지원은 행 단위).
#[tokio::test]
async fn same_student_two_tracks_counts_rows() {
    let cfg = Cfg {
        total: Some(1), univ_prio: false,
        tracks: vec![t(Some(1), false), t(Some(1), false)],
        cands: vec![c(0, 90, true), c(1, 80, true)], univ2: None, prior: vec![],
    };
    let pool = common::create_test_pool().await;
    let b = build(&pool, &cfg).await;
    first_student_in_both_tracks(&pool, &b).await;
    let rid = b.rid;

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
    // Y1 은 덩어리와 같은 대학 1위 선두다 — 그 인원은 적되(최종 감사 F1 C-1), 남은 자리(3) ≥ 선두 1 +
    // 덩어리 잔여석 1 이라 이 순위의 후보를 다 넣어도 자리가 남는다. 다툼을 적으면 거짓이다 — 위 "명
    // 경합" 부재 단언과 같은 의도(보류 정지 문장이 남은 자리와 모순된 경합을 말하지 않는다)다.
    // 판별력의 소재: 다툼 조건을 항상 참으로 만들면 깨진다. 경합 인원 합(선두 1 + 덩어리 2 = 3)과
    // 비교하는 변이는 여기서 3 < 3 이 거짓이라 판별되지 않는다 —
    // `held_stop_contest_compares_block_seats_not_contenders` 가 본다.
    assert_eq!(held_leaders_of(&univ_reason), 1, "Y1 인원: {univ_reason}");
    assert!(!univ_reason.contains(REASON_HELD_CONTEST), "자리가 남는데 다툼을 적었다: {univ_reason}");
    assert_no_prescription(&univ_reason);
    // 2단계가 바로 X 덩어리에서 멈췄고 대학 자리가 남았다 — X 동점은 지금 고를 수 있으므로 (a)
    // "관리자 선택 필요" 그대로다. 아래 X 동점 중 하나를 고르는 수동 호출은 Y1 추천 **뒤** 상태의
    // 기록이다 — 자동 확정 상태에서 지금 고를 수 있다는 판정은 DFS 불변식 (f) 가 한다.
    let r = track_reason(&resp.manual, b.track_ids[0]);
    assert!(r.contains(REASON_CHOICE) && !r.contains(REASON_UNIV_FULL) && !r.contains(REASON_NOT_YET), "{r}");

    rec_manual(&pool, &b, 2).await.expect("Y1 은 대학 1위 동순위 — 관리자가 고를 수 있다");
    assert!(rec_manual(&pool, &b, 3).await.is_err(), "Z1 은 X 동점이 미결정인 동안 막힌다");
    rec_manual(&pool, &b, 0).await.expect("X 동점 중 하나를 고른다");
    let resp = auto_recommend_results(st(&pool), Path(b.rid)).await.unwrap().0;
    assert_eq!(names(&b, &recommended(&pool, b.rid).await), vec![0, 2, 3]);
    assert!(resp.manual.is_empty());
}

/// 보류 정지의 "다툼"은 남은 자리 < 선두 인원 + 덩어리 **잔여석**일 때만 적는다 — 덩어리에서는
/// 모집단위 잔여석까지만 들어온다(최종 감사 F1 C-1 수정). 대학 정원 2, X(1석) 90·90 보류(대학 1위),
/// Y(1석) Y1 90(1위). 남은 자리 2 = 선두 1 + 잔여석 1 이라 Y1 과 X 동점 하나가 다 들어간다.
/// 판별력의 소재: 경합 인원 합(선두 1 + 덩어리 2 = 3)과 비교하면 2 < 3 이라 다툼이 나와 사유 단언이
/// 깨진다. 아래 수동 호출(Y1 뒤에 X 동점 하나도 통과)은 "다툼 없음"이 사실이라는 기록이다.
#[tokio::test]
async fn held_stop_contest_compares_block_seats_not_contenders() {
    let cfg = Cfg {
        total: Some(2), univ_prio: false,
        tracks: vec![t(Some(1), false), t(Some(1), false)],
        cands: vec![c(0, 90, true), c(0, 90, true), c(1, 90, true)], univ2: None, prior: vec![],
    };
    let pool = common::create_test_pool().await;
    let b = build(&pool, &cfg).await;
    let resp = auto_recommend_results(st(&pool), Path(b.rid)).await.unwrap().0;
    assert_eq!(names(&b, &recommended(&pool, b.rid).await), Vec::<usize>::new());
    let u = univ_reason(&resp.manual);
    assert!(u.contains(REASON_HELD_STOP), "{u}");
    assert_eq!(held_leaders_of(&u), 1, "Y1 인원: {u}");
    assert!(!u.contains(REASON_HELD_CONTEST), "선두와 덩어리 몫이 다 들어가는데 다툼을 적었다: {u}");
    assert_no_prescription(&u);
    rec_manual(&pool, &b, 2).await.expect("Y1");
    rec_manual(&pool, &b, 0).await.expect("X 동점 하나 — Y1 뒤에도 대학 자리가 남는다");
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
    // 용어: 화면에서 "제외"는 옛 미선발 용어로 읽힌다. 일반 동점 문장의 괄호는 "세지 않음"으로 쓴다.
    assert!(!univ_reason.contains("제외"), "옛 용어 '제외'가 사유에 남았다: {univ_reason}");
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

// 운영 흐름 — 수동 결정 뒤 자동 재실행, 미선발 해제, 추천 취소, 재계산, 대학 지정 실행
// (2026-10-07 재감사가 손으로 추적한 흐름을 기계로 반복한다).

/// 보류 동점 → 미선발로 정리 → 자동 → 미선발 **해제** → 자동. 해제된 후보는 순위 1위
/// 동점이지만 모집단위가 찼다. 자동은 판단 항목 없이 끝나고, 수동도 못 넣고(정원), 마감은
/// 미결정으로 막히며, 미선발을 다시 하면 마감된다 — 교착 아님.
#[tokio::test]
async fn exclude_then_clear_then_auto_is_consistent_and_finalizable() {
    let cfg = Cfg {
        total: Some(2), univ_prio: false,
        tracks: vec![t(Some(1), false), t(Some(1), false)],
        cands: vec![c(0, 90, true), c(0, 90, true), c(1, 80, true)], univ2: None, prior: vec![],
    };
    let pool = common::create_test_pool().await;
    let b = build(&pool, &cfg).await;

    let m = auto_run(&pool, &b).await;
    assert!(names(&b, &recommended(&pool, b.rid).await).is_empty());
    assert!(!m.is_empty());

    excl(&pool, &b, 1).await.unwrap();
    let m = auto_run(&pool, &b).await;
    assert_eq!(names(&b, &recommended(&pool, b.rid).await), vec![0, 2]);
    assert!(m.is_empty(), "{m:?}");

    clear(&pool, &b, 1).await.unwrap();
    let m = auto_run(&pool, &b).await;
    assert_eq!(names(&b, &recommended(&pool, b.rid).await), vec![0, 2], "해제만으로 추천이 바뀌면 안 된다");
    assert!(m.is_empty(), "자동은 판단할 것이 없다(모집단위 만석): {m:?}");
    assert!(rec_manual(&pool, &b, 1).await.is_err(), "수동도 못 넣는다 — 자동≡수동");
    let fin = finalize_round(st(&pool), Path(b.rid)).await;
    assert!(fin.is_err(), "미결정이 있으면 마감 불가");

    excl(&pool, &b, 1).await.unwrap();
    finalize_round(st(&pool), Path(b.rid)).await.expect("미선발 후 마감");
    assert_quotas(&pool, &b, &cfg, "exclude_clear").await;
}

/// 보류 동점을 수동으로 정리(X1) → 자동이 Y1 확정 → X1 **취소**. 역방향 가드가 없어
/// Y1(대학 3위)이 남은 채 X1·X2(1위)가 미결정이 된다. 자동 재실행은 보류 덩어리에서 다시
/// 멈춰야 하고(F-1), Y1 을 근거로 아무것도 더 확정하지 않으며, 사유 숫자는 Y1 을 센다.
#[tokio::test]
async fn unrecommend_after_held_resolution_makes_auto_stop_again() {
    let cfg = Cfg {
        total: Some(2), univ_prio: false,
        tracks: vec![t(Some(1), false), t(Some(1), false)],
        cands: vec![c(0, 90, true), c(0, 90, true), c(1, 80, true)], univ2: None, prior: vec![],
    };
    let pool = common::create_test_pool().await;
    let b = build(&pool, &cfg).await;

    auto_run(&pool, &b).await;
    rec_manual(&pool, &b, 0).await.unwrap();
    let m = auto_run(&pool, &b).await;
    assert_eq!(names(&b, &recommended(&pool, b.rid).await), vec![0, 2]);
    assert!(m.is_empty());

    unrec(&pool, &b, 0).await.unwrap();
    let m = auto_run(&pool, &b).await;
    assert_eq!(names(&b, &recommended(&pool, b.rid).await), vec![2], "취소 뒤 자동이 새로 확정한 것이 있으면 안 된다");
    let Some(univ) = m.iter().find(|r| r.contains("정리되지 않아")) else {
        panic!("보류 정지 사유가 없다: {m:?}");
    };
    assert!(univ.contains("이번 실행 포함 확정 1명, 잔여 1석"), "{univ}");
    // 관리자는 동점 중 하나를 고를 수 있다(5b 동점 비차단, 5c 상위 없음)
    rec_manual(&pool, &b, 0).await.expect("X1 재추천");
    assert!(rec_manual(&pool, &b, 1).await.is_err(), "X 만석");
    assert_quotas(&pool, &b, &cfg, "unrecommend").await;
}

/// 재계산 끼어들기: 추천 보존(명세 §2.4) 상태에서 순위가 뒤집힌 뒤 자동 재실행이 가드를
/// 건너뛴 추천을 만들지 않는지, 수동과 같은 판단인지.
#[tokio::test]
async fn recalc_between_decisions_keeps_auto_and_manual_aligned() {
    let cfg = Cfg {
        total: Some(2), univ_prio: false,
        tracks: vec![t(Some(1), false), t(Some(1), false)],
        cands: vec![c(0, 90, true), c(0, 90, true), c(1, 80, true)], univ2: None, prior: vec![],
    };
    let pool = common::create_test_pool().await;
    let b = build(&pool, &cfg).await;
    auto_run(&pool, &b).await;
    rec_manual(&pool, &b, 0).await.unwrap();
    auto_run(&pool, &b).await;
    assert_eq!(names(&b, &recommended(&pool, b.rid).await), vec![0, 2]);

    // X2 의 점수가 95 로 바뀌고 재계산 — X1(추천됨, 90)보다 위로 간다
    sqlx::query("UPDATE base_data SET value = ? WHERE student_id = ? AND area_id = ?")
        .bind((95i64 * 100_000).to_string()).bind(b.keys[1].0).bind(b.area)
        .execute(&pool).await.unwrap();
    let _ = calculate_scores(st(&pool), Path(b.rid)).await.expect("재계산");
    assert_eq!(names(&b, &recommended(&pool, b.rid).await), vec![0, 2], "재계산은 추천을 보존한다");

    let m = auto_run(&pool, &b).await;
    assert_eq!(names(&b, &recommended(&pool, b.rid).await), vec![0, 2], "자동은 만석 모집단위에 손대지 않는다");
    assert!(m.is_empty(), "{m:?}");
    assert!(rec_manual(&pool, &b, 1).await.is_err(), "X 만석 — 수동도 거부");
    assert!(finalize_round(st(&pool), Path(b.rid)).await.is_err(), "X2 미결정");

    // 관리자가 바꾸기로 하면: X1 취소 → X2 추천(5b: 상위 미결정 없음) → X1 미선발 → 마감
    unrec(&pool, &b, 0).await.unwrap();
    rec_manual(&pool, &b, 1).await.expect("재계산 뒤 1위 X2");
    excl(&pool, &b, 0).await.unwrap();
    finalize_round(st(&pool), Path(b.rid)).await.expect("마감");
    assert_quotas(&pool, &b, &cfg, "recalc").await;
}

/// 대학 지정 자동 추천(`univ_filter`)은 그 대학에 대해 전체 실행과 같은 결정을 하고, 다른
/// 대학은 건드리지 않는다.
#[tokio::test]
async fn univ_filter_matches_full_run_for_that_university_and_leaves_others() {
    let cfg = Cfg {
        total: Some(2), univ_prio: false,
        tracks: vec![t(Some(1), false), t(Some(1), false), Track { quota: Some(1), prio: true, univ: 1 }, Track { quota: Some(1), prio: false, univ: 1 }],
        cands: vec![c(0, 90, true), c(0, 90, true), c(1, 80, true), c(2, 70, true), c(2, 90, false), c(3, 88, true)],
        univ2: Some((Some(1), false)), prior: vec![],
    };
    let pool_f = common::create_test_pool().await;
    let bf = build(&pool_f, &cfg).await;
    let _ = auto_recommend_results_univ(st(&pool_f), Path((bf.rid, bf.univ_ids[1]))).await.unwrap();
    let filtered = names(&bf, &recommended(&pool_f, bf.rid).await);
    for &i in &filtered {
        assert_eq!(cfg.tracks[cfg.cands[i].track].univ, 1, "지정 대학 밖이 확정됐다: {filtered:?}");
    }

    let pool_a = common::create_test_pool().await;
    let ba = build(&pool_a, &cfg).await;
    let _ = auto_recommend_results(st(&pool_a), Path(ba.rid)).await.unwrap();
    let full: Vec<usize> = names(&ba, &recommended(&pool_a, ba.rid).await)
        .into_iter().filter(|&i| cfg.tracks[cfg.cands[i].track].univ == 1).collect();
    assert_eq!(filtered, full, "대학 지정 실행과 전체 실행이 그 대학에서 다르다");
    assert_eq!(filtered, vec![5], "F-2 구성: 자기 트랙에 막힌 c4 는 경쟁하지 않고 Y1(c5) 확정");
}

/// 같은 학생이 같은 대학 두 모집단위에 지원하면 대학 순위에 두 번 서고, 대학 정원이 허용하면
/// 자동이 **두 행 모두** 확정한다 — 한 학생이 대학 2석. `src/docs/11_release_decisions.md` §11
/// 소유자 결정(2026-10-07): 같은 라운드의 같은 대학 두 모집단위 지원을 앱이 막지 않고 순위·정원은
/// 행 단위로 센다. §8 은 라운드 간 재지원, §11 이 같은 라운드다. 소유자가 결정을 뒤집어
/// "한 학생 한 석"으로 정하면 이 테스트는 그 정책에 맞게 바뀌어야 한다.
#[tokio::test]
async fn same_student_two_tracks_same_univ_takes_two_seats_when_quota_allows() {
    let cfg = Cfg {
        total: Some(2), univ_prio: false,
        tracks: vec![t(Some(1), false), t(Some(1), false)],
        cands: vec![c(0, 90, true), c(1, 80, true)], univ2: None, prior: vec![],
    };
    let pool = common::create_test_pool().await;
    let b = build(&pool, &cfg).await;
    first_student_in_both_tracks(&pool, &b).await;
    let rid = b.rid;

    let ranks: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT track_id, ranking FROM results WHERE round_id = ? AND student_id = ? ORDER BY track_id",
    ).bind(rid).bind(b.keys[0].0).fetch_all(&pool).await.unwrap();
    assert_eq!(ranks.len(), 2);
    assert!(ranks.iter().all(|(_, r)| *r == 1), "두 행 모두 대학 1위: {ranks:?}");

    let resp = auto_recommend_results(st(&pool), Path(rid)).await.unwrap().0;
    let rec = recommended(&pool, rid).await;
    assert_eq!(rec.len(), 2, "한 학생의 두 행이 모두 확정됐다(대학 2석)");
    assert!(resp.manual.is_empty());
    finalize_round(st(&pool), Path(rid)).await.expect("정원 2 = 확정 2 — 마감 허용");
}

/// 1단계 모집단위 동점 + 2단계에서 대학 정원 소진 — 사유는 상태를 말하고, 교착이 아니다.
/// X(1석): X1·X2 90 동점(1석에 2명 → 1단계 보류). Y(1석): Y1 95(대학 1위). 대학 정원 1.
/// 자동은 Y1 을 확정하고, X 동점 항목은 "관리자 선택 필요" 대신 대학 정원이 찼다는 상태를
/// 적는다(2026-10-07 재감사 C-1, 명세 §5.4). X1·X2 추천은 409(대학 정원)이고, 이 구성에서는
/// Y1 을 취소해 자리를 만들어도 5c 가 막는다. 미선발하면 마감된다.
/// 판별력의 소재: 사유가 예전 문장이면 사유 단언이, 동점자 추천이 통과하면 409 단언이, Y1 을
/// 취소해 만든 자리로 동점자를 추천할 수 있게 되면(5c 가 미결정 선두 Y1 을 보지 않으면) 그
/// 단언이, 미선발 뒤에도 마감이 막히면(교착) 마지막 단언이 깨진다.
#[tokio::test]
async fn track_tie_with_university_quota_exhausted_is_not_a_deadlock() {
    let cfg = Cfg {
        total: Some(1), univ_prio: false,
        tracks: vec![t(Some(1), false), t(Some(1), false)],
        cands: vec![c(0, 90, true), c(0, 90, true), c(1, 95, true)], univ2: None, prior: vec![],
    };
    let pool = common::create_test_pool().await;
    let b = build(&pool, &cfg).await;
    let resp = auto_recommend_results(st(&pool), Path(b.rid)).await.unwrap().0;
    assert_eq!(names(&b, &recommended(&pool, b.rid).await), vec![2], "Y1(95) 이 대학 1석");
    let r = track_reason(&resp.manual, b.track_ids[0]);
    assert!(r.contains(REASON_UNIV_FULL) && !r.contains(REASON_CHOICE), "{r}");
    assert!(r.contains("이번 실행 포함 확정 1명 / 정원 1명"), "{r}");
    assert_no_prescription(&r);

    // 동점자 추천은 대학 정원으로 거부되고, 거부 응답이 그 이유를 말한다
    for i in [0, 1] {
        let e = rec_manual(&pool, &b, i).await.expect_err("대학 만석인데 동점자가 추천됐다");
        assert!(e.contains("409") && e.contains("대학 전체 정원"), "{e}");
    }
    // Y1 을 취소해 자리를 만들어도 동점자는 추천되지 않는다 — Y1(대학 1위)이 빈자리 있는
    // 모집단위의 미결정 선두라 5c 가 막는다. Y1 을 되돌려 놓는다.
    unrec(&pool, &b, 2).await.expect("Y1 취소");
    let e = rec_manual(&pool, &b, 0).await.expect_err("Y1 을 건너뛰고 X1 이 추천됐다");
    assert!(e.contains("다른 모집단위"), "{e}");
    rec_manual(&pool, &b, 2).await.expect("Y1 재추천");

    // 미선발하면 마감된다
    excl(&pool, &b, 0).await.expect("X1 미선발");
    excl(&pool, &b, 1).await.expect("X2 미선발");
    finalize_round(st(&pool), Path(b.rid)).await.expect("미선발 뒤 마감 — 교착이 아니다");
    assert_quotas(&pool, &b, &cfg, "track_tie_univ_exhausted").await;
}

/// C-1 이 문구를 바꾼 계기가 된 구성(2026-10-07 재감사). 대학 정원 2, 대학 재학생 우선 0.
/// X(2석, 재학생 우선): X1 재학 70(대학 4위), X2·X3 졸업 90(대학 1위 동점). Y(1석): Y1 재학
/// 85(대학 3위). 자동은 Y1·X1 을 확정하고, X 덩어리가 선두가 될 때 대학 정원이 이미 차 있어
/// 대학 단위 항목 없이 끝난다. X 동점 항목은 대학 정원이 찼다는 상태를 적는다.
/// 끝의 "Y1 취소 → X2 추천 통과"는 **기록이지 방어선이 아니다**: 취소된 Y1 이 X2 보다 대학
/// 순위가 나빠 5c 가 X2 를 막지 않는다. 역방향 가드가 없기 때문이고(명세 §4.3) 소유자가 보류한
/// 결정이다. 예전 문장("관리자 선택 필요")이 관리자를 이 길로 안내했다.
#[tokio::test]
async fn track_tie_reason_says_university_full_when_worse_ranked_took_the_seats() {
    let cfg = Cfg {
        total: Some(2), univ_prio: false,
        tracks: vec![t(Some(2), true), t(Some(1), false)],
        cands: vec![c(0, 70, true), c(0, 90, false), c(0, 90, false), c(1, 85, true)],
        univ2: None, prior: vec![],
    };
    let pool = common::create_test_pool().await;
    let b = build(&pool, &cfg).await;
    let resp = auto_recommend_results(st(&pool), Path(b.rid)).await.unwrap().0;
    assert_eq!(names(&b, &recommended(&pool, b.rid).await), vec![0, 3], "X1·Y1 확정");
    assert!(resp.manual.iter().all(|m| m.track_id.is_some()), "대학 단위 항목은 없다(대학 정원이 찼다)");
    let r = track_reason(&resp.manual, b.track_ids[0]);
    assert!(r.contains(REASON_UNIV_FULL) && !r.contains(REASON_CHOICE), "{r}");
    assert!(r.contains("이번 실행 포함 확정 2명 / 정원 2명"), "{r}");
    assert_no_prescription(&r);

    // 기록: Y1 취소 → X2 추천이 통과한다
    unrec(&pool, &b, 3).await.expect("Y1 취소");
    rec_manual(&pool, &b, 1).await.expect("기록: 취소된 Y1 이 X2 보다 대학 순위가 나빠 5c 가 막지 않는다");
}

/// 2026-10-07 재감사가 A급으로 올린 구성 — 대학 정원 2, 대학 재학생 우선 0. X(2석, 재학생 우선):
/// X1 재학 80, X2·X3 졸업 90(대학 1위 동점, 1단계에서 1석에 2명이라 보류). Y(1석): Y1 재학 80 —
/// X1·Y1 은 대학 3위 동순위 선두. 옛 동작은 X1·Y1 을 통째로 확정하고 X 동점 항목에 "대학 정원이
/// 찼습니다"를 적었다 — 관리자가 X1 을 먼저 고르면 X2(1위)가 Y1 을 앞서므로 그 확정은 순서를
/// 자동이 대신 정한 것이었다. B-merge(소유자 결정, 명세 §5.4) 뒤에는 X 연쇄에 덩어리(1위 ≤ 3위,
/// 1석)가 붙어 수요 3 > 잔여 2 로 **멈춘다**: 확정 없음, 대학 단위 항목은 경합 집합 문장. X 동점
/// 항목은 대학 자리가 남았지만 2단계가 그 덩어리가 아니라 경합 집합에서 멈췄으므로 (c) "아직
/// 차례가 오지 않았습니다" 다 — X2·X3 는 X1 이 미결정인 동안 5b 가 막아 "관리자 선택 필요"는
/// 거짓이다(2026-10-07 마감 감사가 이 경로를 찾았다; 그 전까지 이 테스트가 옛 문장을 단언했다).
/// 모든 수동 최대 결과({X1,X2}·{X1,X3}·{X1,Y1})에 공통인 X1 도 확정하지 않는다(원자 원칙 — 소유자 결정).
/// 사유에 처방(미선발·취소)이 없다는 단언은 그대로 방어선이다. 끝의 수동 진행은 **기록이지
/// 방어선이 아니다**: X1 → X2 는 통과하고(Y1 보다 대학 순위가 좋다), X1 → Y1 은 5c 가 막는다
/// (X2 가 X 의 선두가 되어 Y1 보다 좋다). 관리자가 X1 을 고른 뒤 자동을 다시 돌리면 X 덩어리가
/// 선두로 서서 보류 정지로 멈추고 더 확정하지 않는다.
#[tokio::test]
async fn track_tie_reason_does_not_prescribe_exclusion() {
    let cfg = Cfg {
        total: Some(2), univ_prio: false,
        tracks: vec![t(Some(2), true), t(Some(1), false)],
        cands: vec![c(0, 80, true), c(0, 90, false), c(0, 90, false), c(1, 80, true)],
        univ2: None, prior: vec![],
    };
    let pool = common::create_test_pool().await;
    let b = build(&pool, &cfg).await;
    let resp = auto_recommend_results(st(&pool), Path(b.rid)).await.unwrap().0;
    assert_eq!(names(&b, &recommended(&pool, b.rid).await), Vec::<usize>::new(), "경합 집합 — 확정 없음");
    let u = univ_reason(&resp.manual);
    assert!(u.contains(REASON_CONTENTION) && u.contains("대학 전체 3위"), "{u}");
    assert!(u.contains("잔여 2석에 4명이 경합"), "경합 인원은 X1 + 덩어리 2명 + Y1: {u}");
    assert!(!u.contains(REASON_HELD_STOP) && !u.contains(REASON_PLAIN_TIE), "{u}");
    let r = track_reason(&resp.manual, b.track_ids[0]);
    assert!(r.contains(REASON_NOT_YET) && !r.contains(REASON_CHOICE) && !r.contains(REASON_UNIV_FULL),
        "자리는 남았지만 경합 집합에서 멈춰 X 동점은 아직 차례가 아니다: {r}");
    // 경합 집합 문장과 모집단위 동점 문장 둘 다 처방이 없다. "대학 정원이 찼습니다" 문장의
    // 같은 단언은 그 문장을 단언하는 `track_tie_*` 테스트들에 있다.
    assert_no_prescription(&u);
    assert_no_prescription(&r);
    // (c) 의 근거 — X 동점의 학생은 지금 추천되지 않는다(X1 이 미결정인 동안 5b)
    assert!(rec_manual(&pool, &b, 1).await.is_err() && rec_manual(&pool, &b, 2).await.is_err());

    // 기록: X1 → X2 는 통과하고 X1 → Y1 은 막힌다
    let pool2 = common::create_test_pool().await;
    let b2 = build(&pool2, &cfg).await;
    rec_manual(&pool2, &b2, 0).await.expect("기록: X1");
    assert!(rec_manual(&pool2, &b2, 3).await.is_err(), "기록: X2 가 X 선두로 서서 Y1 을 막는다");
    rec_manual(&pool2, &b2, 1).await.expect("기록: X2 — Y1 보다 대학 순위가 좋다");

    // X1 을 고른 뒤 다시 돌리면 덩어리에서 보류 정지 — 더 확정하지 않는다
    rec_manual(&pool, &b, 0).await.expect("관리자가 X1 을 고른다");
    let m = auto_run(&pool, &b).await;
    assert_eq!(names(&b, &recommended(&pool, b.rid).await), vec![0]);
    assert!(m.iter().any(|r| r.contains(REASON_HELD_STOP)), "{m:?}");
}

/// 대학 컷이 보류 덩어리가 아닌 다른 후보에서 깨끗이 끝나도(StopClean) 대학 정원이 찼으면
/// 1단계 동점 사유는 대학 정원이 찼다는 상태를 적는다. 대학 정원 1. X(2석): X1 95, X2·X3 90
/// 동점(1석에 2명 → 보류). Y(1석): Y1 93. 2단계는 X1(대학 1위)을 확정한 뒤 Y1(2위)에서 정원이
/// 차 깨끗이 끝난다 — X 덩어리(대학 3위)는 X1 뒤에 선두가 되지만, 선두들 가운데 Y1 이 더
/// 좋아서 덩어리에서 멈추지 않는다.
/// 판별력의 소재: "찼는가"를 남은 자리가 아니라 병합이 덩어리에서 멈췄는가(`UnivCutStop::Held`)로
/// 판정하면 이 구성에서 예전 문장이 나와 사유 단언이 깨진다.
#[tokio::test]
async fn track_tie_reason_says_university_full_after_clean_university_cut() {
    let cfg = Cfg {
        total: Some(1), univ_prio: false,
        tracks: vec![t(Some(2), false), t(Some(1), false)],
        cands: vec![c(0, 95, true), c(0, 90, true), c(0, 90, true), c(1, 93, true)],
        univ2: None, prior: vec![],
    };
    let pool = common::create_test_pool().await;
    let b = build(&pool, &cfg).await;
    let resp = auto_recommend_results(st(&pool), Path(b.rid)).await.unwrap().0;
    assert_eq!(names(&b, &recommended(&pool, b.rid).await), vec![0], "X1(95) 만 확정");
    let r = track_reason(&resp.manual, b.track_ids[0]);
    assert!(r.contains(REASON_UNIV_FULL) && !r.contains(REASON_CHOICE), "{r}");
    assert_no_prescription(&r);
}

/// 대학 정원 무제한이면 1단계 동점 사유는 예전 문장("관리자 선택 필요") 그대로다. 자리가 남고
/// 2단계가 그 덩어리에서 멈춘 쪽은 `held_stop_with_seats_left_explains_itself` 가 같은 단언을 한다.
#[tokio::test]
async fn track_tie_reason_keeps_choice_wording_without_university_quota() {
    let cfg = Cfg {
        total: None, univ_prio: false,
        tracks: vec![t(Some(1), false)],
        cands: vec![c(0, 90, true), c(0, 90, true)], univ2: None, prior: vec![],
    };
    let pool = common::create_test_pool().await;
    let b = build(&pool, &cfg).await;
    let resp = auto_recommend_results(st(&pool), Path(b.rid)).await.unwrap().0;
    assert!(recommended(&pool, b.rid).await.is_empty());
    let r = track_reason(&resp.manual, b.track_ids[0]);
    assert!(r.contains(REASON_CHOICE) && !r.contains(REASON_UNIV_FULL) && !r.contains(REASON_NOT_YET), "{r}");
    assert_no_prescription(&r);
    // 무제한이면 2단계 컷이 없어 1단계 확정분이 다 추천되고 5c 도 검사하지 않는다 — 동점자는 지금 고를 수 있다
    rec_manual(&pool, &b, 0).await.expect("무제한 대학의 모집단위 동점 — 관리자가 고를 수 있다");
}

/// 마감 감사(2026-10-07)가 찾은 1단계 동점 사유의 부정확 경로 — 갈래 (c). 대학 정원 1. X(2석): X1 95,
/// X2·X3 85 동점(1석에 2명 → 보류, 대학 3위). Y(1석): Y1 95. 2단계 선두 X1·Y1 이 대학 1위 동순위 —
/// 일반 동점(`Tie`)으로 멈추고 확정 0, 자리 1 남음. 예전 코드는 자리가 남았다고 X 동점에 "관리자
/// 선택 필요"를 적었지만 X2·X3 는 어떤 순서로도 지금 들어오지 못한다(수동 최대 결과는 {X1}·{Y1}) —
/// X1 이 미결정인 동안 5b 가 막는다. 그래서 차례가 오지 않았다는 상태와 대학 단위 항목(X1·Y1 동점)이
/// 먼저라는 것만 적는다. 관리자가 X1 을 고르고 다시 돌리면 대학 정원이 차 (b) 문장으로 바뀐다.
/// 판별력의 소재: (c) 판정을 (a)로 되돌리면(자리가 남으면 전부 "관리자 선택 필요") 사유 단언이 깨진다.
#[tokio::test]
async fn track_tie_behind_university_tie_says_its_turn_has_not_come() {
    let cfg = Cfg {
        total: Some(1), univ_prio: false,
        tracks: vec![t(Some(2), false), t(Some(1), false)],
        cands: vec![c(0, 95, true), c(0, 85, true), c(0, 85, true), c(1, 95, true)],
        univ2: None, prior: vec![],
    };
    let pool = common::create_test_pool().await;
    let b = build(&pool, &cfg).await;
    let resp = auto_recommend_results(st(&pool), Path(b.rid)).await.unwrap().0;
    assert_eq!(names(&b, &recommended(&pool, b.rid).await), Vec::<usize>::new(), "X1·Y1 1위 동순위 — 확정 없음");
    let u = univ_reason(&resp.manual);
    assert!(u.contains(REASON_PLAIN_TIE) && u.contains("대학 전체 1위"), "{u}");
    let r = track_reason(&resp.manual, b.track_ids[0]);
    assert!(r.contains(REASON_NOT_YET), "{r}");
    assert!(!r.contains(REASON_CHOICE) && !r.contains(REASON_UNIV_FULL), "{r}");
    assert!(!r.contains("추천할 수 없습니다"), "단정하지 않는다(관리자가 앞 사람을 미선발하면 달라진다): {r}");
    assert!(r.contains("대학 전체 항목을 먼저"), "대학 단위 항목이 먼저라고 말한다: {r}");
    assert_no_prescription(&r);
    // 동점의 학생은 지금 추천되지 않는다 — X1 이 미결정인 동안 5b 가 막는다
    for i in [1, 2] {
        let e = rec_manual(&pool, &b, i).await.expect_err("X 동점자가 X1 을 건너뛰고 추천됐다");
        assert!(e.contains("409") && e.contains("같은 모집단위"), "{e}");
    }
    // 대학 단위 항목을 정리(X1 추천)하고 다시 돌리면 대학 정원이 차 (b) 문장이다 — 대학 단위 항목은 없다
    rec_manual(&pool, &b, 0).await.expect("X1 — 대학 1위 동순위, 관리자가 고를 수 있다");
    let resp = auto_recommend_results(st(&pool), Path(b.rid)).await.unwrap().0;
    assert_eq!(names(&b, &recommended(&pool, b.rid).await), vec![0]);
    assert!(resp.manual.iter().all(|m| m.track_id.is_some()), "대학 정원이 찼으니 대학 단위 항목은 없다");
    let r = track_reason(&resp.manual, b.track_ids[0]);
    assert!(r.contains(REASON_UNIV_FULL) && !r.contains(REASON_NOT_YET) && !r.contains(REASON_CHOICE), "{r}");
    assert_no_prescription(&r);
}

/// 같은 감사의 둘째 반례 — 2단계가 경합 집합(`Contention`)에서 멈춘 경우. 대학 정원 2, 대학 재학생
/// 우선 0. X(3석, 재학생 우선): X1 재학 80(모집단위 1위, 대학 4위), X2 졸업 95(모집단위 2위, 대학
/// 1위), X3·X4 졸업 90(모집단위 3위 동점, 대학 2위, 1석에 2명 → 보류). Y(1석): Y1 재학 80(4위). 선두
/// X1·Y1 이 4위 동순위, X 연쇄에 X2 와 덩어리가 붙어 수요 4 > 잔여 2 → 경합 정지, 확정 0. X 동점
/// 사유는 (c) — X3·X4 는 X1·X2 가 미결정인 동안 5b 가 막는다.
#[tokio::test]
async fn track_tie_behind_contention_says_its_turn_has_not_come() {
    let cfg = Cfg {
        total: Some(2), univ_prio: false,
        tracks: vec![t(Some(3), true), t(Some(1), false)],
        cands: vec![c(0, 80, true), c(0, 95, false), c(0, 90, false), c(0, 90, false), c(1, 80, true)],
        univ2: None, prior: vec![],
    };
    let pool = common::create_test_pool().await;
    let b = build(&pool, &cfg).await;
    let resp = auto_recommend_results(st(&pool), Path(b.rid)).await.unwrap().0;
    assert_eq!(names(&b, &recommended(&pool, b.rid).await), Vec::<usize>::new(), "경합 집합 — 확정 없음");
    let u = univ_reason(&resp.manual);
    assert!(u.contains(REASON_CONTENTION) && u.contains("대학 전체 4위"), "{u}");
    let r = track_reason(&resp.manual, b.track_ids[0]);
    assert!(r.contains(REASON_NOT_YET) && r.contains("모집단위 3위 동점"), "{r}");
    assert!(!r.contains(REASON_CHOICE) && !r.contains(REASON_UNIV_FULL), "{r}");
    assert_no_prescription(&r);
    for i in [2, 3] {
        let e = rec_manual(&pool, &b, i).await.expect_err("X 동점자가 X1·X2 를 건너뛰고 추천됐다");
        assert!(e.contains("409") && e.contains("같은 모집단위"), "{e}");
    }
}

/// 같은 대학 순위에 선 덩어리가 둘인데 하나만 선두인 경우 — 선두인 덩어리만 (a), 아닌 쪽은 (c).
/// 대학 정원 2, 대학 재학생 우선 0. X(2석, 재학생 우선): X1 재학 70(대학 5위), X2·X3 졸업 90(1위
/// 동점, 1석에 2명 → 보류, X1 뒤). Y(1석): Y1·Y2 재학 90(1위 동점 → 보류, 바로 선두). 2단계는 Y
/// 덩어리에서 보류 정지(잔여 2, 덩어리 잔여석은 Y 몫 1 만). Y 동점은 지금 고를 수 있고(Y1 추천 통과),
/// X 동점은 X1 이 미결정인 동안 5b 가 막는다.
/// 판별력의 소재: 정지 덩어리 판정을 "같은 순위면 전부"로 넓히면(병합의 선두 조건을 빼면) X 동점에도
/// "관리자 선택 필요"가 나오고 대학 단위 문장의 잔여석이 2 가 된다.
#[tokio::test]
async fn held_stop_names_only_the_leading_block_in_track_tie_reasons() {
    let cfg = Cfg {
        total: Some(2), univ_prio: false,
        tracks: vec![t(Some(2), true), t(Some(1), false)],
        cands: vec![c(0, 70, true), c(0, 90, false), c(0, 90, false), c(1, 90, true), c(1, 90, true)],
        univ2: None, prior: vec![],
    };
    let pool = common::create_test_pool().await;
    let b = build(&pool, &cfg).await;
    let resp = auto_recommend_results(st(&pool), Path(b.rid)).await.unwrap().0;
    assert_eq!(names(&b, &recommended(&pool, b.rid).await), Vec::<usize>::new());
    let u = univ_reason(&resp.manual);
    assert!(u.contains(REASON_HELD_STOP) && u.contains("동점(잔여 1석)"), "선두인 Y 덩어리 몫만: {u}");
    let ry = track_reason(&resp.manual, b.track_ids[1]);
    assert!(ry.contains(REASON_CHOICE) && !ry.contains(REASON_NOT_YET), "선두 덩어리는 지금 고를 수 있다: {ry}");
    let rx = track_reason(&resp.manual, b.track_ids[0]);
    assert!(rx.contains(REASON_NOT_YET) && !rx.contains(REASON_CHOICE), "선두가 아닌 덩어리는 차례가 아니다: {rx}");
    assert_no_prescription(&ry);
    assert_no_prescription(&rx);
    let e = rec_manual(&pool, &b, 1).await.expect_err("X2 가 X1 을 건너뛰고 추천됐다");
    assert!(e.contains("409") && e.contains("같은 모집단위"), "{e}");
    rec_manual(&pool, &b, 3).await.expect("Y1 — 선두 덩어리, 관리자가 고를 수 있다");
}

// ── 경합 집합 정지 (2026-10-07 소유자 결정 B-merge, 명세 §5.4) ───────────────
//
// 동순위 선두가 둘 이상의 모집단위에서 나오고, 그 가운데 누구를 먼저 추천하느냐에 따라 같은
// 모집단위의 다음 후보(대학 순위 ≤ r)가 드러나 경합이 남은 자리를 넘으면 2단계는 멈춘다.
// 옛 동작(선두 그룹 원자 Take)은 그 순서를 자동이 대신 정했고, S2 에서는 자동 결과가 수동으로
// 도달 불가했다. 아래 S1~S4 는 오케스트레이터가 실행으로 확인한 결함 구성이다. 기대값은 손으로
// 추적한 뒤 실행으로 맞췄다. 수동 최대 결과는 4 의 DFS 와 같은 뜻이다(더 추천할 수 없는 상태).

/// S1. 대학 정원 2, 대학 재학생 우선 0. X(2석, 재학생 우선): X1 재학 80(대학 2위), X2 졸업
/// 90(대학 1위). Y(1석): Y1 재학 80(대학 2위). 선두 X1·Y1 이 2위 동순위, X 연쇄에 X2 가 붙어
/// 수요 3 > 2. 옛 동작은 {X1, Y1} 을 확정했다 — 수동 최대 결과는 {X1, X2}·{X1, Y1} 둘이고,
/// X1 을 먼저 고르면 X2 가 Y1 을 앞서므로 자동이 순서를 대신 정한 것이었다.
/// 지금은 멈추고(확정 없음, 공통 후보 X1 도 원자 원칙으로 확정하지 않는다) 대학 단위 사유가
/// 경합 집합 문장이다. 관리자가 어느 쪽을 고르든 다시 돌린 자동이 나머지를 결정적으로 채운다.
#[tokio::test]
async fn contention_s1_two_track_tied_leaders_with_exposure_stop() {
    let cfg = Cfg {
        total: Some(2), univ_prio: false,
        tracks: vec![t(Some(2), true), t(Some(1), false)],
        cands: vec![c(0, 80, true), c(0, 90, false), c(1, 80, true)], univ2: None, prior: vec![],
    };
    let pool = common::create_test_pool().await;
    let b = build(&pool, &cfg).await;
    let resp = auto_recommend_results(st(&pool), Path(b.rid)).await.unwrap().0;
    assert_eq!(names(&b, &recommended(&pool, b.rid).await), Vec::<usize>::new(), "경합 집합 — 확정 없음");
    assert_eq!(resp.manual.len(), 1, "대학 단위 항목 하나: {:?}", resp.manual.iter().map(|m| &m.reason).collect::<Vec<_>>());
    let u = univ_reason(&resp.manual);
    assert!(u.contains(REASON_CONTENTION), "{u}");
    assert!(u.contains("대학 전체 2위") && u.contains("잔여 2석에 3명이 경합"), "{u}");
    assert!(u.contains("대학 정원 2명, 이번 실행 포함 확정 0명, 잔여 2석"), "{u}");
    assert!(!u.contains(REASON_HELD_STOP) && !u.contains(REASON_PLAIN_TIE), "{u}");

    // 관리자가 X1 을 고르면 → X2(1위)가 X 선두로 서서 자동이 X2 를 확정, Y1 은 깨끗한 끝
    rec_manual(&pool, &b, 0).await.expect("X1");
    let m = auto_run(&pool, &b).await;
    assert_eq!(names(&b, &recommended(&pool, b.rid).await), vec![0, 1]);
    assert!(m.is_empty(), "{m:?}");

    // 관리자가 Y1 을 고르면 → 자동이 X1 을 확정, X2 는 깨끗한 끝
    let pool2 = common::create_test_pool().await;
    let b2 = build(&pool2, &cfg).await;
    rec_manual(&pool2, &b2, 2).await.expect("Y1 — X2 는 X 선두가 아니라 5c 블로커가 아니다");
    let m = auto_run(&pool2, &b2).await;
    assert_eq!(names(&b2, &recommended(&pool2, b2.rid).await), vec![0, 2]);
    assert!(m.is_empty(), "{m:?}");
    assert_quotas(&pool, &b, &cfg, "s1").await;
}

/// S2. 대학 정원 2. X(2석, 재학생 우선): X1 재학 80(대학 3위), X2 졸업 90(1위). Y 도 같다:
/// Y1 재학 80(3위), Y2 졸업 90(1위). 수동 최대 결과는 {X1, X2}·{Y1, Y2} — 옛 동작의 자동 결과
/// {X1, Y1} 은 **수동으로 도달 불가**였다(X1 다음엔 X2 가 Y1 을 막고, Y1 다음엔 Y2 가 X1 을 막는다).
/// 지금은 멈춘다. 대학 정원 3(S2b)이어도 수요 4 > 3 이라 멈추고, 관리자가 X1 을 고르면 자동이
/// X2·Y1 을 확정한다(수동 최대 결과 {X1, X2, Y1} 과 같다).
#[tokio::test]
async fn contention_s2_old_auto_result_was_unreachable_manually() {
    let cfg = Cfg {
        total: Some(2), univ_prio: false,
        tracks: vec![t(Some(2), true), t(Some(2), true)],
        cands: vec![c(0, 80, true), c(0, 90, false), c(1, 80, true), c(1, 90, false)], univ2: None, prior: vec![],
    };
    let pool = common::create_test_pool().await;
    let b = build(&pool, &cfg).await;
    let resp = auto_recommend_results(st(&pool), Path(b.rid)).await.unwrap().0;
    assert_eq!(names(&b, &recommended(&pool, b.rid).await), Vec::<usize>::new());
    assert_eq!(resp.manual.len(), 1);
    let u = univ_reason(&resp.manual);
    assert!(u.contains(REASON_CONTENTION) && u.contains("대학 전체 3위") && u.contains("잔여 2석에 4명이 경합"), "{u}");

    // 수동 최대 결과 둘 다 도달 가능하고, 옛 자동 결과 {X1, Y1} 은 어느 순서로도 불가
    rec_manual(&pool, &b, 0).await.expect("X1");
    assert!(rec_manual(&pool, &b, 2).await.is_err(), "X1 뒤 Y1 — X2(1위)가 막는다");
    rec_manual(&pool, &b, 1).await.expect("X2");
    assert!(rec_manual(&pool, &b, 2).await.is_err() && rec_manual(&pool, &b, 3).await.is_err(), "대학 만석");
    let pool2 = common::create_test_pool().await;
    let b2 = build(&pool2, &cfg).await;
    rec_manual(&pool2, &b2, 2).await.expect("Y1");
    assert!(rec_manual(&pool2, &b2, 0).await.is_err(), "Y1 뒤 X1 — Y2(1위)가 막는다");
    rec_manual(&pool2, &b2, 3).await.expect("Y2");

    // S2b — 대학 정원 3
    let cfg3 = Cfg { total: Some(3), ..cfg.clone() };
    let pool3 = common::create_test_pool().await;
    let b3 = build(&pool3, &cfg3).await;
    let resp = auto_recommend_results(st(&pool3), Path(b3.rid)).await.unwrap().0;
    assert_eq!(names(&b3, &recommended(&pool3, b3.rid).await), Vec::<usize>::new(), "S2b 도 확정 없음");
    let u = univ_reason(&resp.manual);
    assert!(u.contains(REASON_CONTENTION) && u.contains("잔여 3석에 4명이 경합"), "{u}");
    rec_manual(&pool3, &b3, 0).await.expect("X1");
    let m = auto_run(&pool3, &b3).await;
    assert_eq!(names(&b3, &recommended(&pool3, b3.rid).await), vec![0, 1, 2], "X2(1위) → Y1(3위) 순으로 결정적");
    assert!(m.is_empty(), "{m:?}");
    assert_quotas(&pool3, &b3, &cfg3, "s2b").await;
}

/// S3. S2 에서 졸업생이 모집단위마다 2명 동점(1단계 보류 덩어리). 대학 정원 2. 선두 X1·Y1(대학
/// 5위 동순위), 양쪽 연쇄에 덩어리(1위, 1석에 2명)가 붙어 수요 4 > 2, 경합 6. 멈추고 대학 단위
/// 항목 + 모집단위 동점 항목 둘 — 대학 자리는 남았지만 두 덩어리 다 선두가 아니어서(X1·Y1 이 앞)
/// 모집단위 사유는 (c) "아직 차례가 오지 않았습니다" 다(2026-10-07 마감 감사 전에는 "관리자 선택 필요").
#[tokio::test]
async fn contention_s3_with_held_blocks_on_both_chains() {
    let cfg = Cfg {
        total: Some(2), univ_prio: false,
        tracks: vec![t(Some(2), true), t(Some(2), true)],
        cands: vec![c(0, 80, true), c(0, 90, false), c(0, 90, false), c(1, 80, true), c(1, 90, false), c(1, 90, false)],
        univ2: None, prior: vec![],
    };
    let pool = common::create_test_pool().await;
    let b = build(&pool, &cfg).await;
    let resp = auto_recommend_results(st(&pool), Path(b.rid)).await.unwrap().0;
    assert_eq!(names(&b, &recommended(&pool, b.rid).await), Vec::<usize>::new());
    assert_eq!(resp.manual.len(), 3, "대학 단위 + 모집단위 둘: {:?}", resp.manual.iter().map(|m| &m.reason).collect::<Vec<_>>());
    let u = univ_reason(&resp.manual);
    assert!(u.contains(REASON_CONTENTION) && u.contains("대학 전체 5위") && u.contains("잔여 2석에 6명이 경합"), "{u}");
    for ti in [0, 1] {
        let r = track_reason(&resp.manual, b.track_ids[ti]);
        assert!(r.contains(REASON_NOT_YET) && !r.contains(REASON_CHOICE) && !r.contains(REASON_UNIV_FULL), "{r}");
        assert_no_prescription(&r);
    }
}

/// S4. 셋 다 대학 1위. X(2석, 재학생 우선): X1 재학 80, X2 졸업 80. Y(1석): Y1 재학 80. 대학
/// 정원 2. 옛 동작은 {X1, Y1} 을 확정해 대학 1위 동순위 X2·Y1 가운데 Y1 을 시스템이 골랐다.
/// 지금은 멈춘다. 관리자가 Y1 을 고르면 자동이 X1 을 채운다(X2 는 X1 뒤라 깨끗한 끝). 관리자가
/// X1 을 고르면 남는 X2·Y1 은 1위 동점으로 1석을 다투는 **일반 동점**이라 자동은 다시 멈추고
/// 관리자가 고른다 — 여기서도 시스템이 고르지 않는다.
#[tokio::test]
async fn contention_s4_all_rank_one_is_not_decided_by_the_system() {
    let cfg = Cfg {
        total: Some(2), univ_prio: false,
        tracks: vec![t(Some(2), true), t(Some(1), false)],
        cands: vec![c(0, 80, true), c(0, 80, false), c(1, 80, true)], univ2: None, prior: vec![],
    };
    let pool = common::create_test_pool().await;
    let b = build(&pool, &cfg).await;
    let resp = auto_recommend_results(st(&pool), Path(b.rid)).await.unwrap().0;
    assert_eq!(names(&b, &recommended(&pool, b.rid).await), Vec::<usize>::new());
    assert_eq!(resp.manual.len(), 1);
    let u = univ_reason(&resp.manual);
    assert!(u.contains(REASON_CONTENTION) && u.contains("대학 전체 1위") && u.contains("잔여 2석에 3명이 경합"), "{u}");

    // X1 을 고르면 → X2·Y1 이 1위 동점으로 1석 경합: 일반 동점 문장, 확정 없음, 관리자가 고른다
    rec_manual(&pool, &b, 0).await.expect("X1");
    let m = auto_run(&pool, &b).await;
    assert_eq!(names(&b, &recommended(&pool, b.rid).await), vec![0], "X2 와 Y1 가운데 시스템이 고르면 안 된다");
    assert_eq!(m.len(), 1, "{m:?}");
    assert!(m[0].contains(REASON_PLAIN_TIE) && m[0].contains("1석에 2명 경합"), "{}", m[0]);
    rec_manual(&pool, &b, 1).await.expect("관리자가 X2 를 고른다");
    assert!(auto_run(&pool, &b).await.is_empty());
    assert_eq!(names(&b, &recommended(&pool, b.rid).await), vec![0, 1]);

    // Y1 을 고르면 → 자동이 X1 을 채우고 끝
    let pool2 = common::create_test_pool().await;
    let b2 = build(&pool2, &cfg).await;
    rec_manual(&pool2, &b2, 2).await.expect("Y1");
    let m = auto_run(&pool2, &b2).await;
    assert_eq!(names(&b2, &recommended(&pool2, b2.rid).await), vec![0, 2]);
    assert!(m.is_empty(), "{m:?}");
}

/// 연쇄는 r 보다 **나쁜** 후보에서 끊긴다. X(3석, 재학생 우선): X1 재학 80(대학 2위), X2 재학
/// 70(대학 4위), X3 졸업 90(대학 1위). Y(1석): Y1 재학 80(2위). 대학 정원 2. 선두 X1·Y1 이 2위
/// 동순위지만 X 연쇄는 X2(4위 > 2위)에서 끊겨 X3 를 세지 않으므로 수요 2 ≤ 2 → G 확정, X2 는
/// 깨끗한 끝. 수동 최대 결과도 {X1, Y1} 하나뿐이다(X1 뒤 X2 는 Y1 이 막고, Y1 뒤 X1 은 통과).
/// 판별력의 소재: 연쇄를 끊지 않으면 수요 4(X 3 + Y 1) > 2 로 멈춰 확정이 비고 판단 항목이 생긴다 —
/// 수동 최대 결과가 하나뿐인데 자동이 멈추는 것이므로 자동이 불필요하게 보수적인 쪽의 결함이다.
#[tokio::test]
async fn contention_chain_is_cut_at_worse_rank_so_auto_fills() {
    let cfg = Cfg {
        total: Some(2), univ_prio: false,
        tracks: vec![t(Some(3), true), t(Some(1), false)],
        cands: vec![c(0, 80, true), c(0, 70, true), c(0, 90, false), c(1, 80, true)], univ2: None, prior: vec![],
    };
    let pool = common::create_test_pool().await;
    let b = build(&pool, &cfg).await;
    let resp = auto_recommend_results(st(&pool), Path(b.rid)).await.unwrap().0;
    assert_eq!(names(&b, &recommended(&pool, b.rid).await), vec![0, 3], "X1·Y1 확정");
    assert!(resp.manual.is_empty(), "{:?}", resp.manual.iter().map(|m| &m.reason).collect::<Vec<_>>());

    let pool_m = common::create_test_pool().await;
    let bm = build(&pool_m, &cfg).await;
    let reach = manual_reach(&pool_m, &bm, &cfg).await;
    assert_eq!(reach.maximal.into_iter().collect::<Vec<_>>(), vec![vec![0, 3]], "수동 최대 결과는 하나");
}

/// 노출이 있어도 **들어올 수 없으면** 일반 동점이다(검토 B1). S1 과 같은 구성에 대학 정원만 1:
/// X(2석, 재학생 우선) X1 재학 80(2위)·X2 졸업 90(1위), Y(1석) Y1 재학 80(2위). X2 는 X1 이
/// 추천된 뒤에야 차례인데 그때는 자리가 없다 — 수동 최대 결과는 {X1}·{Y1} 뿐이라 "다음
/// 지원자가 들어올 수 있어 3명 경합"은 거짓이다. 일반 동점 문장 "1석에 2명 경합" 이어야 한다.
#[tokio::test]
async fn contention_rule_keeps_plain_tie_when_exposed_candidate_cannot_enter() {
    let cfg = Cfg {
        total: Some(1), univ_prio: false,
        tracks: vec![t(Some(2), true), t(Some(1), false)],
        cands: vec![c(0, 80, true), c(0, 90, false), c(1, 80, true)], univ2: None, prior: vec![],
    };
    let pool = common::create_test_pool().await;
    let b = build(&pool, &cfg).await;
    let resp = auto_recommend_results(st(&pool), Path(b.rid)).await.unwrap().0;
    assert!(recommended(&pool, b.rid).await.is_empty());
    let u = univ_reason(&resp.manual);
    assert!(u.contains("대학 전체 2위") && u.contains("1석에 2명 경합") && u.contains(REASON_PLAIN_TIE), "{u}");
    assert!(!u.contains(REASON_CONTENTION), "{u}");

    let pool_m = common::create_test_pool().await;
    let bm = build(&pool_m, &cfg).await;
    let reach = manual_reach(&pool_m, &bm, &cfg).await;
    assert_eq!(reach.maximal.into_iter().collect::<Vec<_>>(), vec![vec![0], vec![2]], "X2 는 들어올 수 없다");
}

/// 바뀌면 안 되는 쪽 — 동순위 선두가 둘 이상이어도 **노출이 없으면** 일반 동점 문장 그대로다.
/// X(1석)·Y(1석) 선두가 대학 1위 동점, 각 모집단위에 다음 후보 없음. 대학 정원 1.
#[tokio::test]
async fn contention_rule_leaves_plain_tie_wording_when_nothing_is_exposed() {
    let cfg = Cfg {
        total: Some(1), univ_prio: false,
        tracks: vec![t(Some(1), false), t(Some(1), false)],
        cands: vec![c(0, 90, true), c(1, 90, true)], univ2: None, prior: vec![],
    };
    let pool = common::create_test_pool().await;
    let b = build(&pool, &cfg).await;
    let resp = auto_recommend_results(st(&pool), Path(b.rid)).await.unwrap().0;
    assert!(recommended(&pool, b.rid).await.is_empty());
    let u = univ_reason(&resp.manual);
    assert!(u.contains("1석에 2명 경합") && u.contains(REASON_PLAIN_TIE), "{u}");
    assert!(!u.contains(REASON_CONTENTION) && !u.contains(REASON_HELD_STOP), "{u}");
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
        n_held_stop += resp.manual.iter().any(|m| m.reason.contains(REASON_HELD_STOP)) as u32;

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
        "생성 구성이 갈래를 덮지 못한다: 대학 둘 {n_univ2}, 이전 라운드 {n_prior}, 수동 항목 {n_manual}, 보류 정지 {n_held_stop}");
}

// ── 3. 다단계 불변식 ──────────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
enum Op { Auto, Rec(usize), Unrec(usize), Excl(usize), Clear(usize) }

fn gen_ops(rng: &mut Rng, n_cands: usize) -> Vec<Op> {
    let n = 1 + rng.pick(6) as usize;
    (0..n)
        .map(|_| match rng.pick(5) {
            0 => Op::Auto,
            1 => Op::Rec(rng.pick(n_cands as u64) as usize),
            2 => Op::Unrec(rng.pick(n_cands as u64) as usize),
            3 => Op::Excl(rng.pick(n_cands as u64) as usize),
            _ => Op::Clear(rng.pick(n_cands as u64) as usize),
        })
        .collect()
}

/// 한 조작을 적용하고 결과를 문자열로 — 두 풀에서 같아야 한다(결정성 확인 겸).
async fn apply(pool: &SqlitePool, b: &Built, op: Op) -> String {
    let r: Result<String, String> = match op {
        Op::Auto => auto_recommend_results(st(pool), Path(b.rid)).await
            .map(|r| format!("auto manual={}", r.0.manual.len()))
            .map_err(|(s, m)| format!("{s}: {m}")),
        Op::Rec(i) => rec_manual(pool, b, i).await.map(|_| "ok".into()),
        Op::Unrec(i) => unrec(pool, b, i).await.map(|_| "ok".into()),
        Op::Excl(i) => excl(pool, b, i).await.map(|_| "ok".into()),
        Op::Clear(i) => clear(pool, b, i).await.map(|_| "ok".into()),
    };
    match r { Ok(s) => s, Err(e) => e }
}

/// 자동≡수동 불변식을 **임의의 수동 조작 뒤의 상태**에서 본다. 같은 조작열을 두 풀에 똑같이
/// 적용한 뒤(상태 동일), A 에서 자동을 한 번 더 돌려 이번 실행이 새로 확정한 집합을 M 에서
/// 수동으로 재현한다. (b)·(c) 도 그 상태에서 본다.
///
/// 커버 카운터(`n_rec_ok`·`n_unrec_ok`·`n_clear_ok`)는 조작이 **상태를 실제로 바꾼** 경우만
/// 센다 — 조작 전후의 `recommended` / `excluded_now` 집합을 비교한다. 응답의 Ok 만 세면 안
/// 된다: SQLite UPDATE 는 값이 그대로여도 rows_affected 가 1 이라 `unrecommend_result` 는
/// 추천되지 않은 행에도 Ok 를 돌려주고, `recommend_result` 는 이미 추천된 행에도(정원만 남으면)
/// Ok 를 돌려준다. 그러면 "취소·추천 경로를 탔다"를 과대 집계한다.
#[tokio::test]
async fn auto_and_manual_agree_after_manual_interleaving() {
    const CASES: u64 = 200;
    let (mut n_rec_ok, mut n_unrec_ok, mut n_clear_ok, mut n_new_picks) = (0u32, 0u32, 0u32, 0u32);
    for seed in 1..=CASES {
        let mut rng = Rng(0xA5A5_1234_9E37_79B9 ^ seed.wrapping_mul(0x2545_F491_4F6C_DD1D));
        let cfg = gen_cfg(&mut rng);
        let ops = gen_ops(&mut rng, cfg.cands.len());

        let pool_a = common::create_test_pool().await;
        let ba = build(&pool_a, &cfg).await;
        let pool_m = common::create_test_pool().await;
        let bm = build(&pool_m, &cfg).await;
        assert_eq!(ba.keys, bm.keys, "seed {seed}: 두 풀의 키가 다르다 — 재현 불가");

        for &op in &ops {
            let rec_before = recommended(&pool_a, ba.rid).await;
            let exc_before = excluded_now(&pool_a, ba.rid).await;
            let ra = apply(&pool_a, &ba, op).await;
            let rm = apply(&pool_m, &bm, op).await;
            assert_eq!(ra, rm, "seed {seed}: 같은 조작 {op:?} 이 두 풀에서 다르게 끝났다 — {cfg:?}");
            let rec_changed = recommended(&pool_a, ba.rid).await != rec_before;
            let exc_changed = excluded_now(&pool_a, ba.rid).await != exc_before;
            match op {
                Op::Rec(_) if rec_changed => n_rec_ok += 1,
                Op::Unrec(_) if rec_changed => n_unrec_ok += 1,
                Op::Clear(_) if exc_changed => n_clear_ok += 1,
                _ => {}
            }
            assert_quotas(&pool_a, &ba, &cfg, &format!("seed {seed} after {op:?}")).await;
        }

        let before = recommended(&pool_a, ba.rid).await;
        let resp = auto_recommend_results(st(&pool_a), Path(ba.rid)).await.unwrap().0;
        let after = recommended(&pool_a, ba.rid).await;
        let diff: BTreeSet<(i64, i64)> = after.difference(&before).cloned().collect();
        let new_picks: Vec<usize> = names(&ba, &diff);
        n_new_picks += new_picks.len() as u32;
        assert_quotas(&pool_a, &ba, &cfg, &format!("seed {seed} final auto")).await;
        let excluded = excluded_now(&pool_a, ba.rid).await;
        for &i in &new_picks {
            assert!(!excluded.contains(&ba.keys[i]), "seed {seed}: 미선발 {i} 확정 — {cfg:?}");
        }

        // (a) 이번 실행의 확정분을 M 에서 수동으로 재현 — 순서를 모르므로 되는 것부터 반복
        let mut todo = new_picks.clone();
        let mut last_err = Vec::new();
        loop {
            let n0 = todo.len();
            let mut next = Vec::new();
            last_err.clear();
            for i in todo {
                match rec_manual(&pool_m, &bm, i).await {
                    Ok(()) => {}
                    Err(e) => { last_err.push((i, e)); next.push(i); }
                }
            }
            todo = next;
            if todo.is_empty() || todo.len() == n0 { break; }
        }
        assert!(
            todo.is_empty(),
            "seed {seed}: 수동 조작 {ops:?} 뒤 자동이 확정한 {todo:?} 를 수동으로 재현할 수 없다 {last_err:?}\n새 확정 {new_picks:?}\n{cfg:?}"
        );
        assert_eq!(recommended(&pool_m, bm.rid).await, after, "seed {seed}: 재현 뒤 두 풀의 추천 집합이 다르다");

        // (b) 자동이 판단 없이 끝났다면 수동으로 더 넣을 후보가 없어야 한다
        if resp.manual.is_empty() {
            for i in 0..cfg.cands.len() {
                if after.contains(&bm.keys[i]) || excluded.contains(&bm.keys[i]) { continue; }
                assert!(
                    rec_manual(&pool_m, &bm, i).await.is_err(),
                    "seed {seed}: 수동 조작 {ops:?} 뒤 자동은 판단 없이 끝났는데 수동으로 {i} 를 더 추천할 수 있다\n{cfg:?}"
                );
            }
        }
    }
    assert!(n_rec_ok > 0 && n_unrec_ok > 0 && n_clear_ok > 0 && n_new_picks > 0,
        "조작열이 갈래를 덮지 못한다(상태를 실제로 바꾼 조작만 셈): 추천 {n_rec_ok}, 취소 {n_unrec_ok}, \
         해제 {n_clear_ok}, 재실행 확정 {n_new_picks}");
}

// ── 4. 수동 DFS 전수 불변식 ───────────────────────────────────────

/// 수동 추천만으로 갈 수 있는 상태의 전수. `visited` 는 도달 가능한 추천 집합(cands 인덱스,
/// 오름차순), `maximal` 은 그중 더 추천할 수 없는 상태(= 수동 최대 결과).
#[derive(Default)]
struct Reach {
    visited: BTreeSet<Vec<usize>>,
    maximal: BTreeSet<Vec<usize>>,
    /// 상태(추천 집합, 오름차순) → 그 상태에서 수동 추천이 **통과하는** 후보. 방문한 상태마다
    /// 하나씩 있다 — 1단계 동점 사유의 갈래((a) "지금 고를 수 있다" / (b)(c) "지금은 아니다")를
    /// 자동 확정 상태에서 대조하는 데 쓴다(아래 (f)).
    succ: BTreeMap<Vec<usize>, BTreeSet<usize>>,
}

/// 한 풀에서 추천 → 재귀 → 취소로 되돌리며 훑는다. 같은 집합은 다시 들어가지 않는다 — 어떤
/// 순서로 왔든 상태가 같으면 그 뒤의 가드 판정도 같다(가드는 현재 상태만 본다).
fn dfs<'a>(
    pool: &'a SqlitePool,
    b: &'a Built,
    excluded: &'a [bool],
    state: &'a mut Vec<usize>,
    out: &'a mut Reach,
) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
    Box::pin(async move {
        let mut ok_here: BTreeSet<usize> = BTreeSet::new();
        for i in 0..excluded.len() {
            if excluded[i] || state.contains(&i) {
                continue;
            }
            // 가드 거부(409)만 "갈 수 없는 길"이다 — 다른 오류(500 등)는 삼키지 않고 멈춘다.
            match rec_manual(pool, b, i).await {
                Ok(()) => {}
                Err(e) if e.starts_with("409") => continue,
                Err(e) => panic!("DFS: 후보 {i} 추천이 가드 거부가 아닌 오류로 끝났다: {e}"),
            }
            ok_here.insert(i);
            state.push(i);
            let mut key = state.clone();
            key.sort_unstable();
            if out.visited.insert(key) {
                dfs(pool, b, excluded, state, out).await;
            }
            state.pop();
            unrec(pool, b, i).await.expect("되돌리기");
        }
        let mut key = state.clone();
        key.sort_unstable();
        if ok_here.is_empty() {
            out.maximal.insert(key.clone());
        }
        out.succ.insert(key, ok_here);
    })
}

/// 각 후보의 모집단위 순위 — 화면과 같은 값(`get_results` 의 창 함수). 자동 추천 1단계가 쓰는 창
/// 함수와 정의가 같고 분할(라운드 포함 여부)만 다른데 라운드가 하나라 값이 같다.
async fn track_ranks(pool: &SqlitePool, b: &Built) -> Vec<Option<i64>> {
    let rows = get_results(st(pool), Path(b.rid), Query(ResultQuery { track_id: None }))
        .await
        .unwrap()
        .0;
    b.keys
        .iter()
        .map(|&(sid, tid)| {
            rows.iter()
                .find(|r| r.student_id == sid && r.track_id == tid)
                .unwrap_or_else(|| panic!("결과 행이 없다: {sid}/{tid}"))
                .track_rank
        })
        .collect()
}

/// 1단계 동점 사유 "모집단위 {k}위 동점 — …" 에서 k 를 읽는다. 사유 문장이 덩어리를 가리키는
/// 유일한 통로다(응답에 구조화된 순위 필드가 없다).
fn tie_rank_of(reason: &str) -> i64 {
    let rest = reason
        .strip_prefix("모집단위 ")
        .unwrap_or_else(|| panic!("모집단위 동점 사유가 아니다: {reason}"));
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().unwrap_or_else(|_| panic!("모집단위 순위를 읽을 수 없다: {reason}"))
}

/// 아무것도 추천되지 않은 상태에서 출발한다. 호출 뒤 풀은 출발 상태로 돌아와 있다.
async fn manual_reach(pool: &SqlitePool, b: &Built, cfg: &Cfg) -> Reach {
    let excluded: Vec<bool> = cfg.cands.iter().map(|c| c.excluded).collect();
    let mut out = Reach::default();
    out.visited.insert(vec![]);
    let mut state = Vec::new();
    dfs(pool, b, &excluded, &mut state, &mut out).await;
    assert!(recommended(pool, b.rid).await.is_empty(), "DFS 가 풀을 출발 상태로 되돌리지 못했다");
    out
}

/// 편향 생성기 — 경합 집합이 나오는 구조를 노린다: 대학 재학생 우선 0, 모집단위 대부분 재학생
/// 우선, 모집단위마다 재학생 80 한 명을 먼저 넣어 여러 모집단위의 재학생 선두가 대학 동순위가
/// 되게 하고, 그 뒤에 졸업생을 재학생 이상 점수로 붙여 재학생 우선 모집단위에서 트랙 후순위이면서
/// 대학 순위는 앞서는 후보(연쇄 노출)를 만든다. 대학 정원은 선두 그룹은 들어가되 연쇄까지는 못
/// 들어가는 2~3 을 자주 뽑는다 — 그래야 옛 동작(원자 Take)과 새 동작(정지)이 갈린다.
/// 대학은 하나만 둔다 — 둘째 대학은 경합 구조와 무관하고 `gen_cfg` 가 덮는다.
fn gen_cfg_biased(rng: &mut Rng) -> Cfg {
    let n_tracks = if rng.chance(70) { 2 } else { 3 };
    let total = match rng.pick(10) {
        0 => None,
        1 => Some(1),
        2..=5 => Some(2),
        6..=8 => Some(3),
        _ => Some(4),
    };
    let mut tracks: Vec<Track> = (0..n_tracks)
        .map(|_| Track {
            quota: match rng.pick(5) { 0 => None, 1 => Some(1), 2 | 3 => Some(2), _ => Some(3) },
            prio: rng.chance(70),
            univ: 0,
        })
        .collect();
    if !tracks.iter().any(|t| t.prio) {
        tracks[0].prio = true;
    }
    let prior: Vec<usize> = if rng.chance(10) { vec![rng.pick(n_tracks as u64) as usize] } else { vec![] };
    // 모집단위마다 재학생 80 선두 하나, 그 뒤에 1~3 명을 더 붙인다
    let mut cands: Vec<Cand> = (0..n_tracks)
        .map(|ti| Cand { track: ti, score: 80, enrolled: true, excluded: rng.chance(5) })
        .collect();
    for _ in 0..1 + rng.pick(3) {
        let enrolled = rng.chance(35);
        // 재학생 70 은 선두(80)보다 대학 순위가 나빠 연쇄를 끊는 후보가 된다 — 연쇄 끊김 경로도
        // 생성 구성에 들어가게 한다(그 경로의 변이는 DFS 가 구조상 못 잡는다 — 헤더 참조)
        let score = if enrolled { [70, 80, 85][rng.pick(3) as usize] } else { [85, 90, 90][rng.pick(3) as usize] };
        cands.push(Cand { track: rng.pick(n_tracks as u64) as usize, score, enrolled, excluded: rng.chance(5) });
    }
    Cfg { total, univ_prio: false, tracks, cands, univ2: None, prior }
}

/// 자동 결과를 수동 **전수**와 대조한다(헤더 4). (a′) 자동 확정 집합은 수동으로 도달 가능한
/// 상태여야 하고, (d) 모든 수동 최대 결과에 들어 있어야 하며(아니면 자동이 동순위 처리 순서를
/// 대신 정한 것이다), (e) 자동이 판단 항목 없이 끝났으면 수동 최대 결과는 하나뿐이고 자동과
/// 같아야 한다. 2 의 재현 루프와 달리 순서를 찾지 못해 실패하는 일이 없다.
///
/// (f) 1단계 모집단위 동점 사유의 갈래가 수동 가드와 맞는가 — 자동 확정 상태에서 그 덩어리
/// (사유에 적힌 모집단위 순위의 미결정 후보)의 학생 가운데 누군가가 지금 바로 수동 추천을 통과하면
/// (a) "관리자 선택 필요" 여야 하고, 아무도 통과하지 않으면 (b) "대학 정원이 찼습니다" 또는
/// (c) "차례가 오지 않았습니다" 여야 한다. DFS 가 상태마다 통과 후보를 적어 둔 것(`Reach::succ`)을
/// 자동 확정 상태에서 읽는다 — (a′) 가 그 상태의 방문을 보장한다.
///
/// (g) 보류 정지 대학 단위 사유가 적은 같은 순위 선두 인원이, 자동 확정 상태에서 지금 추천을
/// 통과하는 그 대학의 비덩어리 후보 수와 같은가(최종 감사 F1 C-1). 같은 `Reach::succ` 를 읽는다.
///
/// 케이스 수는 바이너리 전체가 수십 초 안에 들도록 측정해 정했다(상태 수는 정원에 묶여 작다).
/// 커버 카운터: 경합 집합 사유가 나온 구성, 수동 최대 결과가 둘 이상인 구성, 자동이 판단 없이
/// 끝난 구성, 1단계 동점 사유 세 갈래 각각, 보류 정지 사유의 두 형태(같은 순위 선두 있음·없음)
/// — 0 이면 그 갈래는 검사 밖이다.
#[tokio::test]
async fn auto_matches_manual_dfs_reachability() {
    const CASES_PLAIN: u64 = 150;
    const CASES_BIASED: u64 = 150;
    let (mut n_contention, mut n_multi_max, mut n_manual_free) = (0u32, 0u32, 0u32);
    let (mut n_tie_choice, mut n_tie_not_yet, mut n_tie_full) = (0u32, 0u32, 0u32);
    let (mut n_held_leaders, mut n_held_no_leaders) = (0u32, 0u32);
    let plain = (1..=CASES_PLAIN).map(|s| (false, s));
    let biased = (1..=CASES_BIASED).map(|s| (true, s));
    for (bias, seed) in plain.chain(biased) {
        let salt: u64 = if bias { 0xC3D2_E1F0_1234_5678 } else { 0x7F4A_7C15_9E37_79B9 };
        let mut rng = Rng(salt ^ seed.wrapping_mul(0x2545_F491_4F6C_DD1D));
        let cfg = if bias { gen_cfg_biased(&mut rng) } else { gen_cfg(&mut rng) };
        let tag = format!("{} seed {seed}", if bias { "biased" } else { "plain" });

        let pool_a = common::create_test_pool().await;
        let ba = build(&pool_a, &cfg).await;
        let resp = auto_recommend_results(st(&pool_a), Path(ba.rid)).await.unwrap().0;
        let auto_idx = names(&ba, &recommended(&pool_a, ba.rid).await);
        assert_quotas(&pool_a, &ba, &cfg, &tag).await;
        n_contention += resp.manual.iter().any(|m| m.reason.contains(REASON_CONTENTION)) as u32;

        let pool_m = common::create_test_pool().await;
        let bm = build(&pool_m, &cfg).await;
        assert_eq!(ba.keys, bm.keys, "{tag}: 두 풀의 키가 다르다 — 재현 불가");
        let reach = manual_reach(&pool_m, &bm, &cfg).await;
        n_multi_max += (reach.maximal.len() >= 2) as u32;

        // (a′) 자동 확정 집합은 수동으로 도달 가능한 상태다
        assert!(
            reach.visited.contains(&auto_idx),
            "{tag}: 자동 확정 {auto_idx:?} 는 수동으로 도달할 수 없는 상태다\n수동 최대 결과 {:?}\n{cfg:?}",
            reach.maximal
        );
        // (d) 자동 확정 집합 ⊆ 모든 수동 최대 결과의 교집합
        for m in &reach.maximal {
            for &i in &auto_idx {
                assert!(
                    m.contains(&i),
                    "{tag}: 자동이 확정한 {i} 가 수동 최대 결과 {m:?} 에 없다 — 자동이 동순위 처리 순서를 대신 정했다\n자동 {auto_idx:?}\n수동 최대 결과 {:?}\n{cfg:?}",
                    reach.maximal
                );
            }
        }
        // (e) 판단 항목 없이 끝났으면 수동 최대 결과는 하나뿐이고 자동과 같다
        if resp.manual.is_empty() {
            n_manual_free += 1;
            assert!(
                reach.maximal.len() == 1 && reach.maximal.contains(&auto_idx),
                "{tag}: 자동은 판단 없이 끝났는데 수동 최대 결과가 {:?} 다\n자동 {auto_idx:?}\n{cfg:?}",
                reach.maximal
            );
        }
        // (f) 1단계 모집단위 동점 사유의 갈래 — 자동 확정 상태에서 그 덩어리의 누군가가 지금 추천
        //     통과하는가와 맞아야 한다. 덩어리 = 그 모집단위의 미결정(미추천·미선발 아님) 후보 가운데
        //     사유에 적힌 모집단위 순위인 학생. 통과 여부는 DFS 가 그 상태에서 적어 둔 값이다.
        let succ = reach
            .succ
            .get(&auto_idx)
            .unwrap_or_else(|| panic!("{tag}: 자동 확정 상태 {auto_idx:?} 를 DFS 가 탐색하지 않았다"));
        let ranks = track_ranks(&pool_a, &ba).await;
        for m in resp.manual.iter().filter(|m| m.track_id.is_some() && m.reason.starts_with("모집단위 ")) {
            let tid = m.track_id.expect("필터로 보장");
            let k = tie_rank_of(&m.reason);
            let block: Vec<usize> = (0..cfg.cands.len())
                .filter(|&i| ba.keys[i].1 == tid && !cfg.cands[i].excluded && !auto_idx.contains(&i) && ranks[i] == Some(k))
                .collect();
            assert!(!block.is_empty(), "{tag}: 동점 사유의 덩어리를 찾지 못했다: {}\n{cfg:?}", m.reason);
            let kinds = [REASON_CHOICE, REASON_NOT_YET, REASON_UNIV_FULL]
                .iter()
                .filter(|f| m.reason.contains(*f))
                .count();
            assert_eq!(kinds, 1, "{tag}: 사유의 갈래가 하나가 아니다: {}", m.reason);
            let can_now: Vec<usize> = block.iter().copied().filter(|i| succ.contains(i)).collect();
            if m.reason.contains(REASON_CHOICE) {
                n_tie_choice += 1;
                assert!(
                    !can_now.is_empty(),
                    "{tag}: \"관리자 선택 필요\"인데 덩어리 {block:?} 의 누구도 지금 추천되지 않는다 — {}\n자동 {auto_idx:?}\n{cfg:?}",
                    m.reason
                );
            } else {
                if m.reason.contains(REASON_NOT_YET) { n_tie_not_yet += 1 } else { n_tie_full += 1 }
                assert!(
                    can_now.is_empty(),
                    "{tag}: 덩어리 {block:?} 의 {can_now:?} 는 지금 추천되는데 사유는 — {}\n자동 {auto_idx:?}\n{cfg:?}",
                    m.reason
                );
            }
        }
        // (g) 보류 정지 대학 단위 사유가 적은 "같은 대학 순위의 다른 모집단위 지원자 N명"(그 문장이
        //     없으면 0) = 자동 확정 상태에서 지금 수동 추천을 통과하는 그 대학 후보 가운데, 2단계가 멈춘
        //     바로 그 덩어리((a) "관리자 선택 필요" 사유의 덩어리 — (f) 와 같은 방법으로 찾는다)에 속하지
        //     않는 인원. 최종 감사 F1 C-1 — 그 사유가 같은 순위 선두를 빠뜨려 결정을 덩어리 쪽으로
        //     유도했다. 통과 여부는 (f) 와 같이 DFS 가 적어 둔 값이다. "다툼" 문장은 여기서 보지 않는다
        //     (시나리오 `held_tie_equal_to_other_track_leader_is_manual`·
        //     `held_stop_contest_compares_block_seats_not_contenders` 가 본다).
        for m in resp.manual.iter().filter(|m| m.track_id.is_none() && m.reason.contains(REASON_HELD_STOP)) {
            let ui: usize = m
                .univ_name
                .strip_prefix("대학")
                .and_then(|s| s.parse().ok())
                .unwrap_or_else(|| panic!("{tag}: 대학 이름에서 인덱스를 읽을 수 없다: {}", m.univ_name));
            let mut choice_block: BTreeSet<usize> = BTreeSet::new();
            for tm in resp.manual.iter().filter(|tm| {
                tm.track_id.is_some() && tm.reason.starts_with("모집단위 ") && tm.reason.contains(REASON_CHOICE)
            }) {
                let tid = tm.track_id.expect("필터로 보장");
                let k = tie_rank_of(&tm.reason);
                choice_block.extend((0..cfg.cands.len()).filter(|&i| {
                    ba.keys[i].1 == tid && !cfg.cands[i].excluded && !auto_idx.contains(&i) && ranks[i] == Some(k)
                }));
            }
            let others_now: Vec<usize> = succ
                .iter()
                .copied()
                .filter(|&i| cfg.tracks[cfg.cands[i].track].univ == ui && !choice_block.contains(&i))
                .collect();
            let said = held_leaders_of(&m.reason);
            if said > 0 { n_held_leaders += 1 } else { n_held_no_leaders += 1 }
            assert_eq!(
                said,
                others_now.len(),
                "{tag}: 보류 정지 사유가 적은 같은 순위 선두 인원과 지금 추천되는 비덩어리 후보 {others_now:?} 가 다르다 — {}\n자동 {auto_idx:?}\n{cfg:?}",
                m.reason
            );
        }
    }
    assert!(
        n_contention > 0 && n_multi_max > 0 && n_manual_free > 0,
        "생성 구성이 갈래를 덮지 못한다: 경합 집합 사유 {n_contention}, 최대 결과 둘 이상 {n_multi_max}, 판단 없이 끝남 {n_manual_free}"
    );
    assert!(
        n_tie_choice > 0 && n_tie_not_yet > 0 && n_tie_full > 0,
        "생성 구성이 1단계 동점 사유 갈래를 덮지 못한다: 관리자 선택 필요 {n_tie_choice}, 차례 아님 {n_tie_not_yet}, 대학 정원 참 {n_tie_full}"
    );
    assert!(
        n_held_leaders > 0 && n_held_no_leaders > 0,
        "생성 구성이 보류 정지 사유 두 형태를 덮지 못한다: 같은 순위 선두 있음 {n_held_leaders}, 없음 {n_held_no_leaders}"
    );
}
