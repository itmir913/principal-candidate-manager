//! E2: 수동 추천(recommend_result)에도 트랙 내부 순서 가드 적용.
//!
//! 자동 추천의 D 규칙("대학 컷은 같은 트랙에서 track_rank 상위자를 건너뛰고 하위자를
//! 선택할 수 없다")이 수동 경로에도 적용되는지 검증한다. 동점끼리는 우열이 정해지지
//! 않은 상태이므로 서로 막지 않는다(관리자 선택 보장).

mod common;

use axum::{extract::{Path, State}, http::StatusCode};
use principal_candidate_manager::handlers::scoring::recommend_result;
use sqlx::SqlitePool;

// ── 픽스처 ───────────────────────────────────────────────────────

async fn setup(pool: &SqlitePool, prioritize: i64) -> (i64, i64) {
    let hash = bcrypt::hash("pass", 4u32).unwrap();
    for (g, c) in [(1i64, 1i64), (0, 0)] {
        sqlx::query("INSERT INTO classes (grade, class_no, password_hash) VALUES (?, ?, ?)")
            .bind(g).bind(c).bind(&hash).execute(pool).await.unwrap();
    }
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name, prioritize_enrolled) VALUES ('한국대', ?) RETURNING id",
    )
    .bind(prioritize).fetch_one(pool).await.unwrap();
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name, prioritize_enrolled) VALUES (?, '컴공', ?) RETURNING id",
    )
    .bind(uid).bind(prioritize).fetch_one(pool).await.unwrap();
    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at, closed_at) \
         VALUES ('CLOSED', '2025-01-01T00:00:00Z', '2025-01-02T00:00:00Z') RETURNING id",
    )
    .fetch_one(pool).await.unwrap();
    (tid, rid)
}

async fn new_student(pool: &SqlitePool, code: &str, seq: i64, enrolled: bool) -> i64 {
    let sql = if enrolled {
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES (?, ?, 1, 1, ?, 1) RETURNING id"
    } else {
        "INSERT INTO students (student_code, name, grad_year, is_enrolled) \
         VALUES (?, ?, 2024, 0) RETURNING id"
    };
    let mut q = sqlx::query_scalar(sql).bind(code).bind(code);
    if enrolled {
        q = q.bind(seq);
    }
    q.fetch_one(pool).await.unwrap()
}

/// 지원 + 결과 행을 한 번에. ranking 은 대학 순위(마감 시점 저장값).
async fn new_candidate(
    pool: &SqlitePool,
    tid: i64,
    rid: i64,
    code: &str,
    seq: i64,
    enrolled: bool,
    score: i64,
    ranking: i64,
    recommended: bool,
    abandoned: bool,
) -> i64 {
    let sid = new_student(pool, code, seq, enrolled).await;
    sqlx::query(
        "INSERT INTO applications (student_id, track_id, round_id, abandoned, department_name) \
         VALUES (?, ?, ?, ?, '학과')",
    )
    .bind(sid).bind(tid).bind(rid).bind(abandoned as i64)
    .execute(pool).await.unwrap();
    sqlx::query(
        "INSERT INTO results \
         (student_id, track_id, round_id, score_detail, total_score, ranking, recommended, calculated_at) \
         VALUES (?, ?, ?, '{}', ?, ?, ?, '2025-01-02T00:00:00Z')",
    )
    .bind(sid).bind(tid).bind(rid).bind(score).bind(ranking).bind(recommended as i64)
    .execute(pool).await.unwrap();
    sid
}

async fn call(pool: &SqlitePool, sid: i64, tid: i64, rid: i64) -> Result<StatusCode, (StatusCode, String)> {
    recommend_result(State(common::make_state(pool.clone())), Path((sid, tid, rid))).await
}

// ── 테스트 ───────────────────────────────────────────────────────

#[tokio::test]
async fn blocks_when_higher_ranked_candidate_not_recommended() {
    let pool = common::create_test_pool().await;
    let (tid, rid) = setup(&pool, 0).await;
    let _top = new_candidate(&pool, tid, rid, "S001", 1, true, 900000, 1, false, false).await;
    let low = new_candidate(&pool, tid, rid, "S002", 2, true, 500000, 2, false, false).await;

    let err = call(&pool, low, tid, rid).await.unwrap_err();
    assert_eq!(err.0, StatusCode::CONFLICT);
    assert!(err.1.contains("1명"), "막고 있는 인원수를 알려야 한다: {}", err.1);
    assert!(err.1.contains("1위"), "최상위 순위를 알려야 한다: {}", err.1);

    let rec: i64 = sqlx::query_scalar(
        "SELECT recommended FROM results WHERE student_id = ?",
    ).bind(low).fetch_one(&pool).await.unwrap();
    assert_eq!(rec, 0, "409 시 추천이 확정되면 안 된다");
}

#[tokio::test]
async fn allows_when_higher_ranked_candidate_abandoned() {
    // 철회·포기는 정당한 예외 — 상위자가 빠지면 하위자 추천이 순서를 어기지 않는다
    let pool = common::create_test_pool().await;
    let (tid, rid) = setup(&pool, 0).await;
    let _top = new_candidate(&pool, tid, rid, "S001", 1, true, 900000, 1, false, true).await;
    let low = new_candidate(&pool, tid, rid, "S002", 2, true, 500000, 2, false, false).await;

    assert_eq!(call(&pool, low, tid, rid).await.unwrap(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn allows_when_higher_ranked_candidate_already_recommended() {
    let pool = common::create_test_pool().await;
    let (tid, rid) = setup(&pool, 0).await;
    let _top = new_candidate(&pool, tid, rid, "S001", 1, true, 900000, 1, true, false).await;
    let low = new_candidate(&pool, tid, rid, "S002", 2, true, 500000, 2, false, false).await;

    assert_eq!(call(&pool, low, tid, rid).await.unwrap(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn tied_candidates_do_not_block_each_other() {
    // 동점은 우열이 정해지지 않은 상태 — 관리자가 고를 수 있어야 한다
    let pool = common::create_test_pool().await;
    let (tid, rid) = setup(&pool, 0).await;
    let a = new_candidate(&pool, tid, rid, "S001", 1, true, 500000, 1, false, false).await;
    let b = new_candidate(&pool, tid, rid, "S002", 2, true, 500000, 2, false, false).await;

    // 저장 ranking 이 2위인 쪽을 먼저 확정해도 막히지 않는다 (track_rank 는 동일)
    assert_eq!(call(&pool, b, tid, rid).await.unwrap(), StatusCode::NO_CONTENT);
    assert_eq!(call(&pool, a, tid, rid).await.unwrap(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn other_track_higher_ranked_does_not_block() {
    // 가드는 같은 모집단위 안에서만 작동한다 — 다른 트랙 상위자는 막지 않는다
    let pool = common::create_test_pool().await;
    let (tid, rid) = setup(&pool, 0).await;
    let uid: i64 = sqlx::query_scalar("SELECT univ_id FROM univ_tracks WHERE id = ?")
        .bind(tid).fetch_one(&pool).await.unwrap();
    let other: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name) VALUES (?, '전자') RETURNING id",
    ).bind(uid).fetch_one(&pool).await.unwrap();

    let _top = new_candidate(&pool, other, rid, "S001", 1, true, 900000, 1, false, false).await;
    let low = new_candidate(&pool, tid, rid, "S002", 2, true, 500000, 2, false, false).await;

    assert_eq!(call(&pool, low, tid, rid).await.unwrap(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn guard_uses_track_prioritize_enrolled_ranking() {
    // 재학생 우선 트랙: 저득점 재학생이 track_rank 1위 → 고득점 졸업생 추천은 막힌다.
    // (저장 ranking 이 아니라 자동 추천과 같은 라이브 track_rank 를 쓴다는 확인)
    let pool = common::create_test_pool().await;
    let (tid, rid) = setup(&pool, 1).await;
    let _enrolled = new_candidate(&pool, tid, rid, "S001", 1, true, 100000, 1, false, false).await;
    let graduated = new_candidate(&pool, tid, rid, "S002", 2, false, 900000, 2, false, false).await;

    let err = call(&pool, graduated, tid, rid).await.unwrap_err();
    assert_eq!(err.0, StatusCode::CONFLICT);
}

#[tokio::test]
async fn top_ranked_candidate_is_never_blocked() {
    let pool = common::create_test_pool().await;
    let (tid, rid) = setup(&pool, 0).await;
    let top = new_candidate(&pool, tid, rid, "S001", 1, true, 900000, 1, false, false).await;
    let _low = new_candidate(&pool, tid, rid, "S002", 2, true, 500000, 2, false, false).await;

    assert_eq!(call(&pool, top, tid, rid).await.unwrap(), StatusCode::NO_CONTENT);
}
