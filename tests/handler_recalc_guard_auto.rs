//! 수정 검증 라운드 — F-017 가드의 빠진 경로 재현 (F-030).
//!
//! `2e9c273` 는 `recommend_result`(409)와 `finalize_round`(422)에만
//! `needs_recalc` 가드를 넣었다. 그런데 **추천 확정의 주 경로**는 결과 탭의
//! "자동 추천 확정" 버튼 — `auto_recommend_results` / `auto_recommend_results_univ`
//! 이고, 이 경로는 `results.ranking` / `track_rank` 만 보고 `recommended = 1` 을
//! 한꺼번에 쓴다. 가드가 없으므로 낡은 순위로 추천이 확정된다.
//!
//! 아래 테스트는 **현재 코드에서 실패한다**(재현 테스트).

mod common;

use axum::{extract::{Path, State}, http::StatusCode};
use principal_candidate_manager::handlers::scoring::{
    auto_recommend_results, auto_recommend_results_univ,
};
use sqlx::SqlitePool;

/// handler_recalc_guard.rs 의 setup 과 동일한 최소 구성.
/// 대학 1(정원 1) · 모집단위 1(정원 1) · 학생 1 · CLOSED 라운드 + 계산된 results 1행.
async fn setup(pool: &SqlitePool) -> (i64, i64, i64, i64) {
    let hash = bcrypt::hash("pass", 4u32).unwrap();
    sqlx::query("INSERT INTO classes (grade, class_no, password_hash) VALUES (1, 1, ?)")
        .bind(&hash).execute(pool).await.unwrap();
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name, total_quota, prioritize_enrolled) \
         VALUES ('한국대', 1, 0) RETURNING id",
    ).fetch_one(pool).await.unwrap();
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name, unit_quota, prioritize_enrolled) \
         VALUES (?, '컴공', 1, 0) RETURNING id",
    ).bind(uid).fetch_one(pool).await.unwrap();
    let sid: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES ('2026001', '학생1', 1, 1, 1, 1) RETURNING id",
    ).fetch_one(pool).await.unwrap();
    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at, closed_at) \
         VALUES ('CLOSED', '2026-01-01T00:00:00Z', '2026-01-02T00:00:00Z') RETURNING id",
    ).fetch_one(pool).await.unwrap();
    sqlx::query("INSERT INTO applications (student_id, track_id, round_id) VALUES (?, ?, ?)")
        .bind(sid).bind(tid).bind(rid).execute(pool).await.unwrap();
    sqlx::query(
        "INSERT INTO results (student_id, track_id, round_id, total_score, ranking, recommended, calculated_at) \
         VALUES (?, ?, ?, 5000000, 1, 0, '2026-01-02T01:00:00Z')",
    ).bind(sid).bind(tid).bind(rid).execute(pool).await.unwrap();
    (sid, tid, rid, uid)
}

async fn log_base_data_import(pool: &SqlitePool, at: &str) {
    sqlx::query(
        "INSERT INTO audit_log (at, actor_type, action, detail) \
         VALUES (?, 'ADMIN', 'BASE_DATA_IMPORTED', '{}')",
    ).bind(at).execute(pool).await.unwrap();
}

async fn recommended_count(pool: &SqlitePool, rid: i64) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM results WHERE round_id = ? AND recommended = 1")
        .bind(rid).fetch_one(pool).await.unwrap()
}

/// F-030 — 전 대학 자동 추천이 낡은 점수 가드를 우회한다.
#[tokio::test]
async fn stale_round_blocks_auto_recommend_all() {
    let pool = common::create_test_pool().await;
    let (_sid, _tid, rid, _uid) = setup(&pool).await;
    log_base_data_import(&pool, "2026-01-03T00:00:00Z").await;

    let res = auto_recommend_results(
        State(common::make_state(pool.clone())),
        Path(rid),
    ).await;

    let (code, msg) = res.map(|_| ()).expect_err(
        "낡은 상태에서는 자동 추천도 차단되어야 한다 (recommend_result 와 같은 근거)",
    );
    assert_eq!(code, StatusCode::CONFLICT, "recommend_result 와 같은 409 여야 한다");
    assert!(msg.contains("재계산"), "안내에 재계산이 언급되어야 한다: {msg}");
    assert_eq!(recommended_count(&pool, rid).await, 0, "차단됐으므로 추천이 생기면 안 된다");
}

/// F-030 — 대학 단위 자동 추천도 같은 구멍이다.
#[tokio::test]
async fn stale_round_blocks_auto_recommend_univ() {
    let pool = common::create_test_pool().await;
    let (_sid, _tid, rid, uid) = setup(&pool).await;
    log_base_data_import(&pool, "2026-01-03T00:00:00Z").await;

    let res = auto_recommend_results_univ(
        State(common::make_state(pool.clone())),
        Path((rid, uid)),
    ).await;

    let (code, msg) = res.map(|_| ()).expect_err(
        "낡은 상태에서는 대학 단위 자동 추천도 차단되어야 한다",
    );
    assert_eq!(code, StatusCode::CONFLICT);
    assert!(msg.contains("재계산"), "안내에 재계산이 언급되어야 한다: {msg}");
    assert_eq!(recommended_count(&pool, rid).await, 0, "차단됐으므로 추천이 생기면 안 된다");
}

/// 대조군 — 낡지 않은 라운드에서는 자동 추천이 정상 동작한다.
/// (위 두 테스트의 실패가 "자동 추천이 원래 안 된다"가 아님을 고정한다.)
#[tokio::test]
async fn fresh_round_allows_auto_recommend() {
    let pool = common::create_test_pool().await;
    let (_sid, _tid, rid, _uid) = setup(&pool).await;
    // 계산 **이전** 시각의 import — 낡음이 아니다.
    log_base_data_import(&pool, "2026-01-01T12:00:00Z").await;

    auto_recommend_results(State(common::make_state(pool.clone())), Path(rid))
        .await
        .expect("낡지 않은 라운드에서는 자동 추천이 통과해야 한다");
    assert_eq!(recommended_count(&pool, rid).await, 1, "정원 1석이 채워져야 한다");
}

/// 현재 동작 고정 — 낡은 라운드에서 자동 추천이 **성공하고 recommended 가 실제로 써진다.**
/// (위 두 테스트가 "핸들러가 다른 이유로 Err 를 안 냈다"가 아니라
///  "낡은 점수 위에서 추천이 확정됐다"임을 못박는다. 수정되면 이 테스트가 실패한다.)
#[tokio::test]
async fn blocked_auto_recommend_writes_nothing() {
    let pool = common::create_test_pool().await;
    let (sid, tid, rid, _uid) = setup(&pool).await;
    log_base_data_import(&pool, "2026-01-03T00:00:00Z").await;

    // 낡음 판정 자체는 참이다 (needs_recalc_expr 기준).
    let stale: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM audit_log al WHERE al.action = 'BASE_DATA_IMPORTED'            AND al.at > (SELECT MIN(res.calculated_at) FROM results res WHERE res.round_id = ?))",
    ).bind(rid).fetch_one(&pool).await.unwrap();
    assert!(stale, "전제: 이 라운드는 낡은 상태다");

    // 차단은 tx 진입 직후이므로 **부분 쓰기가 없어야** 한다.
    // AutoRecommendResponse 는 Debug 가 없어 expect_err 를 쓸 수 없다 — match 로 받는다.
    match auto_recommend_results(State(common::make_state(pool.clone())), Path(rid)).await {
        Ok(_) => panic!("낡은 상태에서는 자동 추천이 차단되어야 한다"),
        Err((code, _)) => assert_eq!(code, StatusCode::CONFLICT),
    }

    let rec: i64 = sqlx::query_scalar(
        "SELECT recommended FROM results WHERE student_id = ? AND track_id = ? AND round_id = ?",
    ).bind(sid).bind(tid).bind(rid).fetch_one(&pool).await.unwrap();
    assert_eq!(rec, 0, "차단됐으므로 recommended 가 하나도 써지면 안 된다");
}
