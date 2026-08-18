//! 감사 라운드 3 — 과제 3: **U-03 (2-51) 자동추천 감사 로그 수치 실행 대조**.
//!
//! 2단계 §5.5 는 `AUTO_RECOMMEND_RUN` detail 의 집계가 "코드 정의상 일치"한다고 E3 로만
//! 판정하고 실행 대조를 남겨 뒀다(§10 미판정). 여기서 실제로 돌려 대조한다(E2).
//!
//! 대조 대상 (`scoring.rs:1855-1870`):
//!   confirmed_students = Σ per_track  vs  `results.recommended=1` 증가분
//!   confirmed_tracks   = 확정이 발생한 서로 다른 track 수
//!   manual_tracks      = 응답 `manual` 항목 수
//!
//! 증가분을 세는 이유: `UPDATE results SET recommended = 1`(`:1829`)은 멱등이라
//! 이미 추천된 행을 다시 UPDATE 해도 카운트가 늘지 않는다. 후보 필터가
//! `recommended == 0`(`:1702`)이므로 정의상 겹칠 수 없지만, 그 정의를 실측으로 확인한다.

mod common;

use axum::extract::{Path, State};
use principal_candidate_manager::handlers::scoring::{auto_recommend_results, AutoRecommendResponse};
use sqlx::SqlitePool;

// ── 시드 헬퍼 ────────────────────────────────────────────────────

async fn new_univ(pool: &SqlitePool, name: &str, total_quota: Option<i64>) -> i64 {
    sqlx::query_scalar("INSERT INTO universities (univ_name, total_quota) VALUES (?, ?) RETURNING id")
        .bind(name)
        .bind(total_quota)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn new_track(pool: &SqlitePool, univ_id: i64, name: &str, unit_quota: Option<i64>) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name, unit_quota) VALUES (?, ?, ?) RETURNING id",
    )
    .bind(univ_id)
    .bind(name)
    .bind(unit_quota)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn new_closed_round(pool: &SqlitePool) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at, closed_at)
         VALUES ('CLOSED', '2026-01-01T00:00:00Z', '2026-01-02T00:00:00Z') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn new_applicant(
    pool: &SqlitePool,
    tid: i64,
    rid: i64,
    seq: i64,
    score: i64,
    ranking: i64,
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
        "INSERT INTO applications (student_id, track_id, round_id, department_name)
         VALUES (?, ?, ?, '학과')",
    )
    .bind(sid)
    .bind(tid)
    .bind(rid)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO results
           (student_id, track_id, round_id, score_detail, total_score, ranking, recommended, calculated_at)
         VALUES (?, ?, ?, '{}', ?, ?, 0, '2026-01-02T00:00:00Z')",
    )
    .bind(sid)
    .bind(tid)
    .bind(rid)
    .bind(score)
    .bind(ranking)
    .execute(pool)
    .await
    .unwrap();
    sid
}

async fn recommended_count(pool: &SqlitePool, rid: i64) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM results WHERE round_id = ? AND recommended = 1")
        .bind(rid)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// 라운드별 (track_id → recommended 인원) 스냅샷.
async fn recommended_by_track(pool: &SqlitePool, rid: i64) -> Vec<(i64, i64)> {
    sqlx::query_as(
        "SELECT track_id, COUNT(*) FROM results
         WHERE round_id = ? AND recommended = 1 GROUP BY track_id ORDER BY track_id",
    )
    .bind(rid)
    .fetch_all(pool)
    .await
    .unwrap()
}

/// 마지막 `AUTO_RECOMMEND_RUN` 감사 로그의 detail JSON.
async fn last_auto_log(pool: &SqlitePool) -> serde_json::Value {
    let detail: String = sqlx::query_scalar(
        "SELECT detail FROM audit_log WHERE action = 'AUTO_RECOMMEND_RUN' ORDER BY id DESC LIMIT 1",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    serde_json::from_str(&detail).unwrap()
}

async fn call_auto(pool: &SqlitePool, rid: i64) -> AutoRecommendResponse {
    auto_recommend_results(State(common::make_state(pool.clone())), Path(rid))
        .await
        .unwrap()
        .0
}

async fn setup_pool() -> SqlitePool {
    let pool = common::create_test_pool_shared().await;
    common::insert_class(&pool, 3, 1).await;
    pool
}

// ── 대조 ─────────────────────────────────────────────────────────

/// 여러 대학·모집단위가 섞인 상태에서 로그 3개 수치가 전부 실측과 일치하는가.
#[tokio::test]
async fn audit_log_counts_match_actual_recommended_delta() {
    let pool = setup_pool().await;
    let rid = new_closed_round(&pool).await;

    // 대학1: 정원 무제한, 모집단위 A(정원 2) / B(정원 1)
    let u1 = new_univ(&pool, "대학1", None).await;
    let ta = new_track(&pool, u1, "A", Some(2)).await;
    let tb = new_track(&pool, u1, "B", Some(1)).await;
    // 대학2: 모집단위 C(정원 3) — 지원자 2명뿐이라 둘 다 확정
    let u2 = new_univ(&pool, "대학2", None).await;
    let tc = new_track(&pool, u2, "C", Some(3)).await;

    new_applicant(&pool, ta, rid, 1, 10_000_000, 1).await;
    new_applicant(&pool, ta, rid, 2, 9_000_000, 2).await;
    new_applicant(&pool, ta, rid, 3, 8_000_000, 3).await; // 정원 2 초과 → 미확정
    new_applicant(&pool, tb, rid, 4, 7_000_000, 4).await;
    new_applicant(&pool, tb, rid, 5, 6_000_000, 5).await; // 정원 1 초과 → 미확정
    new_applicant(&pool, tc, rid, 6, 5_000_000, 1).await;
    new_applicant(&pool, tc, rid, 7, 4_000_000, 2).await;

    let before = recommended_count(&pool, rid).await;
    let before_by_track = recommended_by_track(&pool, rid).await;
    assert_eq!(before, 0, "사전 조건: 추천 0건");

    let resp = call_auto(&pool, rid).await;

    let after = recommended_count(&pool, rid).await;
    let after_by_track = recommended_by_track(&pool, rid).await;
    let log = last_auto_log(&pool).await;

    // ① confirmed_students == 실제 증가분
    assert_eq!(
        log["confirmed_students"].as_i64(),
        Some(after - before),
        "confirmed_students 가 results.recommended 증가분과 달라졌다. 로그: {log}"
    );

    // ② confirmed_tracks == 실제로 증가가 발생한 서로 다른 track 수
    let before_map: std::collections::HashMap<i64, i64> = before_by_track.into_iter().collect();
    let grown = after_by_track
        .iter()
        .filter(|(tid, cnt)| *cnt > *before_map.get(tid).unwrap_or(&0))
        .count() as i64;
    assert_eq!(
        log["confirmed_tracks"].as_i64(),
        Some(grown),
        "confirmed_tracks 가 실제 증가 트랙 수와 다르다. 로그: {log}"
    );

    // ③ manual_tracks == 응답의 manual 항목 수
    assert_eq!(
        log["manual_tracks"].as_i64(),
        Some(resp.manual.len() as i64),
        "manual_tracks 가 응답 manual 수와 다르다. 로그: {log}"
    );

    // ④ 응답 confirmed 의 count 합도 같은 값이어야 한다 (관리자가 화면에서 읽는 수치)
    let resp_sum: i64 = resp.confirmed.iter().map(|c| c.count).sum();
    assert_eq!(resp_sum, after - before, "응답 confirmed 합계 ≠ 실제 증가분");
    assert_eq!(resp.confirmed.len() as i64, grown, "응답 confirmed 트랙 수 ≠ 실제 증가 트랙 수");

    // 이 시나리오의 기대값 — 회귀 시 어디가 틀렸는지 바로 보이도록 고정
    assert_eq!(after - before, 5, "A 2명 + B 1명 + C 2명 = 5명");
    assert_eq!(grown, 3, "확정이 발생한 모집단위 3개");
}

/// 두 번째 실행은 아무도 확정하지 않는다 — 로그도 0 을 적어야 한다.
/// (`UPDATE ... SET recommended = 1` 이 멱등이라 실제 증가분은 0)
#[tokio::test]
async fn second_run_logs_zero_and_changes_nothing() {
    let pool = setup_pool().await;
    let rid = new_closed_round(&pool).await;
    let u1 = new_univ(&pool, "대학1", None).await;
    let ta = new_track(&pool, u1, "A", Some(2)).await;
    new_applicant(&pool, ta, rid, 1, 10_000_000, 1).await;
    new_applicant(&pool, ta, rid, 2, 9_000_000, 2).await;

    call_auto(&pool, rid).await;
    let after_first = recommended_count(&pool, rid).await;
    assert_eq!(after_first, 2);

    let resp2 = call_auto(&pool, rid).await;
    let after_second = recommended_count(&pool, rid).await;
    let log = last_auto_log(&pool).await;

    assert_eq!(after_second, after_first, "2회차는 아무것도 바꾸지 않는다");
    assert_eq!(log["confirmed_students"].as_i64(), Some(0), "로그: {log}");
    assert_eq!(log["confirmed_tracks"].as_i64(), Some(0), "로그: {log}");
    assert!(resp2.confirmed.is_empty());
}

/// 동점이 정원 경계를 가르면 `manual` 로 빠진다 — 그 수가 로그 `manual_tracks` 와 같은가.
#[tokio::test]
async fn manual_tracks_matches_response_when_tie_at_cutline() {
    let pool = setup_pool().await;
    let rid = new_closed_round(&pool).await;
    let u1 = new_univ(&pool, "대학1", None).await;
    let ta = new_track(&pool, u1, "A", Some(1)).await;

    // 정원 1 인데 1위 동점 2명 → 자동 확정 불가, 관리자 판단
    new_applicant(&pool, ta, rid, 1, 10_000_000, 1).await;
    new_applicant(&pool, ta, rid, 2, 10_000_000, 1).await;

    let resp = call_auto(&pool, rid).await;
    let log = last_auto_log(&pool).await;

    assert_eq!(recommended_count(&pool, rid).await, 0, "동점 그룹은 원자 처리 — 아무도 확정되지 않는다");
    assert_eq!(log["confirmed_students"].as_i64(), Some(0), "로그: {log}");
    assert_eq!(
        log["manual_tracks"].as_i64(),
        Some(resp.manual.len() as i64),
        "로그: {log}"
    );
    assert_eq!(resp.manual.len(), 1, "동점 트랙 1개가 관리자 판단으로 넘어간다");
}
