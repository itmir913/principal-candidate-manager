//! 감사 라운드 3 — 과제 2 (A-2): `excluded` / `abandoned` 혼재 시 표시 순위와 가드의 일치.
//!
//! 2단계 §5.4 는 "영향 없음(설계대로)"을 **E3 단독**으로 판정했다. 이 파일은 그 판정을
//! 실행(E2)으로 다시 세운다. 검사 대상은 세 곳이 같은 순위 숫자를 쓰는가이다:
//!   ① 화면      `get_results` 의 `track_rank` / `ranking`
//!   ② 수동 가드  `recommend_result` 5b 블로커 CTE (`scoring.rs:1134-1150`)
//!   ③ 정원 집계  `get_quota_stats` 의 `unit_used` (`universities.rs:571`)
//!
//! 명세 근거: `00_spec_round_and_scoring.md`
//!   §5.3:450 "순위 계산(RANK())은 excluded 포함 전원으로 계산 — 화면(get_results)·수동 추천
//!             가드와 동일한 순위를 유지하기 위함"
//!   §6.1:528 정원 집계 영향 — excluded 없음 / abandoned 있음
//! **`abandoned` 가 순위에 포함되는지는 어느 조항에도 없다** → 03-round2-findings.md S-08.

mod common;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use principal_candidate_manager::handlers::scoring::{get_results, recommend_result, ResultQuery};
use principal_candidate_manager::handlers::universities::get_quota_stats;
use sqlx::SqlitePool;

// ── 시드 헬퍼 ────────────────────────────────────────────────────

/// 풀 + `students` FK 가 요구하는 학급 1개.
async fn setup_pool() -> SqlitePool {
    let pool = common::create_test_pool_shared().await;
    common::insert_class(&pool, 3, 1).await;
    pool
}

async fn seed_track(pool: &SqlitePool, total_quota: Option<i64>, unit_quota: Option<i64>) -> i64 {
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name, total_quota) VALUES ('대학A', ?) RETURNING id",
    )
    .bind(total_quota)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name, unit_quota)
         VALUES (?, '모집단위1', ?) RETURNING id",
    )
    .bind(uid)
    .bind(unit_quota)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_round(pool: &SqlitePool, status: &str) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at, closed_at)
         VALUES (?, '2026-01-01T00:00:00Z', '2026-01-02T00:00:00Z') RETURNING id",
    )
    .bind(status)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// 학생 + 지원 + 결과를 한 번에 심는다. 점수가 곧 순위를 정한다.
#[allow(clippy::too_many_arguments)]
async fn seed_applicant(
    pool: &SqlitePool,
    tid: i64,
    rid: i64,
    seq: i64,
    score: i64,
    ranking: i64,
    excluded: bool,
    abandoned: bool,
    recommended: bool,
) -> i64 {
    let sid: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled)
         VALUES (?, ?, 3, 1, ?, 1) RETURNING id",
    )
    .bind(format!("2026{seq:03}"))
    .bind(format!("학생{seq}"))
    .bind(seq)
    .fetch_one(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO applications
           (student_id, track_id, round_id, abandoned, excluded, excluded_reason, department_name)
         VALUES (?, ?, ?, ?, ?, ?, '학과')",
    )
    .bind(sid)
    .bind(tid)
    .bind(rid)
    .bind(abandoned as i64)
    .bind(excluded as i64)
    .bind(if excluded { Some("감사 시나리오: 미선발") } else { None })
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO results
           (student_id, track_id, round_id, score_detail, total_score, ranking, recommended, calculated_at)
         VALUES (?, ?, ?, '{}', ?, ?, ?, '2026-01-02T00:00:00Z')",
    )
    .bind(sid)
    .bind(tid)
    .bind(rid)
    .bind(score)
    .bind(ranking)
    .bind(recommended as i64)
    .execute(pool)
    .await
    .unwrap();
    sid
}

/// 화면(`get_results`) 이 그 학생에 대해 내보내는 (track_rank, ranking, excluded, abandoned).
async fn rank_of(pool: &SqlitePool, rid: i64, sid: i64) -> (Option<i64>, Option<i64>, bool, bool) {
    let rows = get_results(
        State(common::make_state(pool.clone())),
        Path(rid),
        Query(ResultQuery { track_id: None }),
    )
    .await
    .unwrap()
    .0;
    let v: Vec<serde_json::Value> = rows.iter().map(|r| serde_json::to_value(r).unwrap()).collect();
    let row = v
        .iter()
        .find(|r| r["student_id"].as_i64() == Some(sid))
        .unwrap_or_else(|| panic!("학생 {sid} 행이 화면 응답에 없다"));
    (
        row["track_rank"].as_i64(),
        row["ranking"].as_i64(),
        row["excluded"].as_bool() == Some(true),
        row["abandoned"].as_bool() == Some(true),
    )
}

// ── 케이스 1 — 상위 2명 미선발(excluded) ─────────────────────────

/// 화면의 `track_rank` 는 excluded 를 **포함해** 매겨지고(명세 §5.3:450),
/// 수동 추천 가드는 excluded 를 blocker 에서 **뺀다**(`scoring.rs:1149`).
/// 두 동작이 어긋나지 않는다는 것 = "3위가 3위로 보이면서 추천도 된다".
#[tokio::test]
async fn excluded_top_two_keeps_rank_and_does_not_block_recommend() {
    let pool = setup_pool().await;
    let tid = seed_track(&pool, None, None).await;
    let rid = seed_round(&pool, "CLOSED").await;

    // 점수 100 / 90 / 80 / 70 — 상위 2명만 미선발
    let s1 = seed_applicant(&pool, tid, rid, 1, 10_000_000, 1, true, false, false).await;
    let s2 = seed_applicant(&pool, tid, rid, 2, 9_000_000, 2, true, false, false).await;
    let s3 = seed_applicant(&pool, tid, rid, 3, 8_000_000, 3, false, false, false).await;
    let s4 = seed_applicant(&pool, tid, rid, 4, 7_000_000, 4, false, false, false).await;

    // ① 화면: 미선발자도 행이 남고 순위 숫자를 점유한다
    for (sid, want) in [(s1, 1), (s2, 2), (s3, 3), (s4, 4)] {
        let (tr, rk, _, _) = rank_of(&pool, rid, sid).await;
        assert_eq!(tr, Some(want), "학생{sid} 의 track_rank");
        assert_eq!(rk, Some(want), "학생{sid} 의 ranking");
    }
    let (_, _, ex1, _) = rank_of(&pool, rid, s1).await;
    let (_, _, ex2, _) = rank_of(&pool, rid, s2).await;
    assert!(
        ex1 && ex2,
        "화면 응답이 미선발 표식을 함께 실어야 관리자가 3위 추천을 이해한다"
    );

    // ② 가드: 3위는 막히지 않는다 (1·2위가 excluded 라 blocker 가 아니다)
    let state = common::make_state(pool.clone());
    let ok = recommend_result(State(state.clone()), Path((s3, tid, rid))).await;
    assert!(ok.is_ok(), "3위 추천은 통과해야 한다. 실제: {:?}", ok.as_ref().err());

    // ③ 3위가 확정된 뒤에는 4위도 막을 상위자가 없다
    let next = recommend_result(State(state.clone()), Path((s4, tid, rid))).await;
    assert!(
        next.is_ok(),
        "3위가 방금 추천됐으므로 4위도 통과해야 한다. 실제: {:?}",
        next.as_ref().err()
    );
}

/// 가드가 실제로 순위를 막는지 — 미선발이 **아닌** 상위자가 있으면 하위자는 409.
/// (위 테스트의 통과가 "가드가 아무것도 안 한다"는 뜻이 아님을 증명한다)
#[tokio::test]
async fn non_excluded_higher_rank_still_blocks() {
    let pool = setup_pool().await;
    let tid = seed_track(&pool, None, None).await;
    let rid = seed_round(&pool, "CLOSED").await;

    let _s1 = seed_applicant(&pool, tid, rid, 1, 10_000_000, 1, false, false, false).await;
    let s2 = seed_applicant(&pool, tid, rid, 2, 9_000_000, 2, false, false, false).await;

    let res = recommend_result(State(common::make_state(pool.clone())), Path((s2, tid, rid))).await;
    assert_eq!(
        res.unwrap_err().0,
        StatusCode::CONFLICT,
        "미선발이 아닌 1위가 있으면 2위 추천은 막혀야 한다"
    );
}

/// 미선발자 본인은 추천 불가 (§6.3 상호배타). 순위를 점유하는 것과 추천 가능한 것은 별개.
#[tokio::test]
async fn excluded_applicant_itself_cannot_be_recommended() {
    let pool = setup_pool().await;
    let tid = seed_track(&pool, None, None).await;
    let rid = seed_round(&pool, "CLOSED").await;
    let s1 = seed_applicant(&pool, tid, rid, 1, 10_000_000, 1, true, false, false).await;

    let res = recommend_result(State(common::make_state(pool.clone())), Path((s1, tid, rid))).await;
    assert_eq!(res.unwrap_err().0, StatusCode::CONFLICT);
}

// ── 케이스 2 — 추천 확정 후 전원 포기(abandoned) ─────────────────

/// 세 집계 지점이 abandoned 를 각각 어떻게 세는가:
///   track_rank  → 포함(순위 유지)
///   ranking     → 포함(순위 유지)
///   unit_used   → 제외(정원 반환)  ← 명세 §6.1:528 이 규정하는 유일한 항목
#[tokio::test]
async fn all_abandoned_keeps_ranks_but_frees_quota() {
    let pool = setup_pool().await;
    let tid = seed_track(&pool, Some(3), Some(3)).await;
    let rid = seed_round(&pool, "FINALIZED").await;

    let s1 = seed_applicant(&pool, tid, rid, 1, 10_000_000, 1, false, true, true).await;
    let s2 = seed_applicant(&pool, tid, rid, 2, 9_000_000, 2, false, true, true).await;
    let s3 = seed_applicant(&pool, tid, rid, 3, 8_000_000, 3, false, true, true).await;

    // ①② 순위는 그대로 남는다 — 포기해도 "몇 위였다"는 사실은 화면에서 사라지지 않는다
    for (sid, want) in [(s1, 1), (s2, 2), (s3, 3)] {
        let (tr, rk, _, ab) = rank_of(&pool, rid, sid).await;
        assert_eq!(tr, Some(want), "포기해도 track_rank 는 유지된다");
        assert_eq!(rk, Some(want), "포기해도 ranking 은 유지된다");
        assert!(ab, "화면이 포기 표식을 실어야 한다");
    }

    // ③ 정원은 전부 반환된다
    let stats = get_quota_stats(State(common::make_state(pool.clone()))).await.unwrap().0;
    let track = stats.univs.iter().flat_map(|u| &u.tracks).find(|t| t.track_id == tid).unwrap();
    assert_eq!(track.unit_used, 0, "전원 포기 시 unit_used = 0 (정원 반환)");
    assert_eq!(
        stats.univs.iter().find(|u| u.tracks.iter().any(|t| t.track_id == tid)).unwrap().total_used,
        0,
        "대학 전체 집계도 0"
    );
    // F-010 의 all_round_ids 와 같은 필터 — 추천 0건이 된 라운드는 목록에서 빠진다
    assert!(
        stats.all_round_ids.is_empty(),
        "all_round_ids 는 recommended=1 AND abandoned=0 결과에서만 파생된다(F-010). 실제: {:?}",
        stats.all_round_ids
    );
}

/// 일부만 포기 — 순위는 그대로, 정원만 부분 반환.
#[tokio::test]
async fn partial_abandon_frees_only_that_seat() {
    let pool = setup_pool().await;
    let tid = seed_track(&pool, Some(3), Some(3)).await;
    let rid = seed_round(&pool, "FINALIZED").await;

    let s1 = seed_applicant(&pool, tid, rid, 1, 10_000_000, 1, false, true, true).await;
    let s2 = seed_applicant(&pool, tid, rid, 2, 9_000_000, 2, false, false, true).await;

    let (tr1, _, _, _) = rank_of(&pool, rid, s1).await;
    let (tr2, _, _, _) = rank_of(&pool, rid, s2).await;
    assert_eq!(
        (tr1, tr2),
        (Some(1), Some(2)),
        "포기자가 순위를 비워 주지 않는다"
    );

    let stats = get_quota_stats(State(common::make_state(pool.clone()))).await.unwrap().0;
    let track = stats.univs.iter().flat_map(|u| &u.tracks).find(|t| t.track_id == tid).unwrap();
    assert_eq!(track.unit_used, 1, "포기하지 않은 1명만 정원을 점유한다");
}
