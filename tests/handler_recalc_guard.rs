//! F-017: 마감 후 기초데이터가 바뀌면 점수가 낡는다 — 표시 + 차단.
//!
//! `base_data_import` 에는 `guard_no_closed_round` 가 없고, 보호 트리거는 **명시 DELETE만**
//! 막는다(`008-applications.sql:57-59`). 단일값 경로의 `INSERT OR REPLACE` 는 통과하므로
//! CLOSED 라운드에서도 기초데이터가 바뀌고 `results` 는 옛 총점 그대로 남는다.
//!
//! 이 파일이 고정하는 불변식:
//!   1. 계산 직후에는 `needs_recalc = false`
//!   2. 계산 이후 `BASE_DATA_IMPORTED` 감사 로그가 생기면 `needs_recalc = true`
//!   3. 낡은 상태에서는 추천 확정(409)·라운드 마감(422)이 **차단**된다
//!   4. 재계산하면 다시 false 가 되고 추천이 통과한다
//!   5. OPEN 라운드는 계산 자체가 없으므로 항상 false
//!
//! 판정은 `results.calculated_at` 과 감사 로그 시각의 비교로 파생된다(`rounds.rs::needs_recalc_expr`).
//! `base_data` 에 타임스탬프 컬럼이 없고 v1 스키마가 동결이라 택한 방식이다.

mod common;

use axum::{extract::{Path, State}, http::StatusCode};
use principal_candidate_manager::handlers::rounds::{finalize_round, list_rounds};
use principal_candidate_manager::handlers::scoring::recommend_result;
use sqlx::SqlitePool;

/// 대학 1 · 모집단위 1(정원 1) · 학생 1 · CLOSED 라운드 1 + 계산된 results 1행.
async fn setup(pool: &SqlitePool) -> (i64, i64, i64) {
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
    // 점수 계산 결과 — 계산 시각이 기준점이 된다.
    sqlx::query(
        "INSERT INTO results (student_id, track_id, round_id, total_score, ranking, recommended, calculated_at) \
         VALUES (?, ?, ?, 5000000, 1, 0, '2026-01-02T01:00:00Z')",
    ).bind(sid).bind(tid).bind(rid).execute(pool).await.unwrap();
    (sid, tid, rid)
}

/// 기초데이터 변경을 감사 로그로 표현한다 — 판정이 보는 것이 이 로그다.
async fn log_base_data_import(pool: &SqlitePool, at: &str) {
    sqlx::query(
        "INSERT INTO audit_log (at, actor_type, action, detail) \
         VALUES (?, 'ADMIN', 'BASE_DATA_IMPORTED', '{}')",
    ).bind(at).execute(pool).await.unwrap();
}

async fn needs_recalc_of(pool: &SqlitePool, rid: i64) -> bool {
    let rounds = list_rounds(State(common::make_state(pool.clone()))).await.unwrap();
    rounds.0.into_iter().find(|r| r.id == rid).expect("라운드가 목록에 있어야 한다").needs_recalc
}

#[tokio::test]
async fn fresh_round_is_not_stale() {
    let pool = common::create_test_pool().await;
    let (_, _, rid) = setup(&pool).await;
    // 계산 **이전** 시각의 import 는 낡음이 아니다.
    log_base_data_import(&pool, "2026-01-01T12:00:00Z").await;
    assert!(!needs_recalc_of(&pool, rid).await, "계산 이전의 기초데이터 변경은 낡음이 아니다");
}

#[tokio::test]
async fn base_data_import_after_calculation_marks_stale() {
    let pool = common::create_test_pool().await;
    let (_, _, rid) = setup(&pool).await;
    log_base_data_import(&pool, "2026-01-03T00:00:00Z").await;
    assert!(needs_recalc_of(&pool, rid).await, "계산 이후의 기초데이터 변경은 재계산 필요다");
}

#[tokio::test]
async fn stale_round_blocks_recommend() {
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup(&pool).await;
    log_base_data_import(&pool, "2026-01-03T00:00:00Z").await;

    let res = recommend_result(
        State(common::make_state(pool.clone())),
        Path((sid, tid, rid)),
    ).await;
    let (code, msg) = res.expect_err("낡은 상태에서는 추천이 차단되어야 한다");
    assert_eq!(code, StatusCode::CONFLICT);
    assert!(msg.contains("재계산"), "안내에 재계산이 언급되어야 한다: {msg}");

    let recommended: i64 = sqlx::query_scalar(
        "SELECT recommended FROM results WHERE student_id = ? AND track_id = ? AND round_id = ?",
    ).bind(sid).bind(tid).bind(rid).fetch_one(&pool).await.unwrap();
    assert_eq!(recommended, 0, "차단됐으므로 추천 상태가 바뀌면 안 된다");
}

#[tokio::test]
async fn stale_round_blocks_finalize() {
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup(&pool).await;
    // 마감의 다른 사전검증(미결정)을 통과시키기 위해 추천을 먼저 확정해 둔다.
    sqlx::query("UPDATE results SET recommended = 1 WHERE student_id = ? AND track_id = ? AND round_id = ?")
        .bind(sid).bind(tid).bind(rid).execute(&pool).await.unwrap();
    log_base_data_import(&pool, "2026-01-03T00:00:00Z").await;

    let res = finalize_round(State(common::make_state(pool.clone())), Path(rid)).await;
    let (code, msg) = res.expect_err("낡은 상태에서는 마감이 차단되어야 한다");
    assert_eq!(code, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(msg.contains("재계산"), "안내에 재계산이 언급되어야 한다: {msg}");

    let status: String = sqlx::query_scalar("SELECT status FROM rounds WHERE id = ?")
        .bind(rid).fetch_one(&pool).await.unwrap();
    assert_eq!(status, "CLOSED", "차단됐으므로 상태가 바뀌면 안 된다");
}

#[tokio::test]
async fn recalculation_clears_stale_and_unblocks_recommend() {
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup(&pool).await;
    log_base_data_import(&pool, "2026-01-03T00:00:00Z").await;
    assert!(needs_recalc_of(&pool, rid).await);

    // 재계산 = calculated_at 이 import 이후로 갱신되는 것.
    sqlx::query("UPDATE results SET calculated_at = '2026-01-04T00:00:00Z' WHERE round_id = ?")
        .bind(rid).execute(&pool).await.unwrap();

    assert!(!needs_recalc_of(&pool, rid).await, "재계산 후에는 낡음이 해소된다");
    recommend_result(State(common::make_state(pool.clone())), Path((sid, tid, rid)))
        .await
        .expect("재계산 후에는 추천이 통과해야 한다");
}

#[tokio::test]
async fn open_round_is_never_stale() {
    let pool = common::create_test_pool().await;
    sqlx::query("INSERT INTO rounds (status, opened_at) VALUES ('OPEN', '2026-02-01T00:00:00Z')")
        .execute(&pool).await.unwrap();
    let rid: i64 = sqlx::query_scalar("SELECT id FROM rounds WHERE status = 'OPEN'")
        .fetch_one(&pool).await.unwrap();
    log_base_data_import(&pool, "2026-02-02T00:00:00Z").await;
    assert!(!needs_recalc_of(&pool, rid).await, "OPEN 라운드는 계산된 결과가 없어 낡을 수 없다");
}
