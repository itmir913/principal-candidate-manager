mod common;

use axum::{
    extract::{Path, State},
    http::StatusCode,
};
use principal_candidate_manager::handlers::rounds::{
    close_round, finalize_round, get_current_round, open_round, reopen_round,
};

// ── open_round ────────────────────────────────────────────────────

#[tokio::test]
async fn open_round_creates_open_round() {
    let pool = common::create_test_pool().await;
    let (status, _) = open_round(State(common::make_state(pool.clone()))).await.unwrap();
    assert_eq!(status, StatusCode::CREATED);
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM rounds WHERE status = 'OPEN'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn open_round_when_already_open_returns_conflict() {
    let pool = common::create_test_pool().await;
    let _ = open_round(State(common::make_state(pool.clone()))).await.unwrap();
    let res = open_round(State(common::make_state(pool))).await;
    assert_eq!(res.unwrap_err().0, StatusCode::CONFLICT);
}

#[tokio::test]
async fn open_round_after_close_returns_conflict() {
    // CLOSED 상태에서는 새 라운드를 열 수 없음 — FINALIZED 후에만 가능
    let pool = common::create_test_pool().await;
    let (_, axum::Json(body)) =
        open_round(State(common::make_state(pool.clone()))).await.unwrap();
    let id = body["id"].as_i64().unwrap();
    let _ = close_round(State(common::make_state(pool.clone())), Path(id))
        .await
        .unwrap();
    let res = open_round(State(common::make_state(pool.clone()))).await;
    assert_eq!(res.unwrap_err().0, StatusCode::CONFLICT);
}

#[tokio::test]
async fn open_round_after_finalize_creates_new_round() {
    // FINALIZED 후에는 새 라운드를 열 수 있음
    let pool = common::create_test_pool().await;
    let (_, axum::Json(body)) =
        open_round(State(common::make_state(pool.clone()))).await.unwrap();
    let id = body["id"].as_i64().unwrap();
    let _ = close_round(State(common::make_state(pool.clone())), Path(id))
        .await
        .unwrap();
    let _ = finalize_round(State(common::make_state(pool.clone())), Path(id))
        .await
        .unwrap();
    let res = open_round(State(common::make_state(pool.clone()))).await;
    assert!(res.is_ok());
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM rounds")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 2);
}

// ── close_round ───────────────────────────────────────────────────

#[tokio::test]
async fn close_round_changes_status_to_closed() {
    let pool = common::create_test_pool().await;
    let (_, axum::Json(body)) =
        open_round(State(common::make_state(pool.clone()))).await.unwrap();
    let id = body["id"].as_i64().unwrap();
    let _ = close_round(State(common::make_state(pool.clone())), Path(id))
        .await
        .unwrap();
    let status: String = sqlx::query_scalar("SELECT status FROM rounds WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "CLOSED");
}

#[tokio::test]
async fn close_round_sets_closed_at_timestamp() {
    let pool = common::create_test_pool().await;
    let (_, axum::Json(body)) =
        open_round(State(common::make_state(pool.clone()))).await.unwrap();
    let id = body["id"].as_i64().unwrap();
    let _ = close_round(State(common::make_state(pool.clone())), Path(id))
        .await
        .unwrap();
    let closed_at: Option<String> =
        sqlx::query_scalar("SELECT closed_at FROM rounds WHERE id = ?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(closed_at.is_some());
}

#[tokio::test]
async fn close_nonexistent_round_returns_not_found() {
    let pool = common::create_test_pool().await;
    let res = close_round(State(common::make_state(pool)), Path(9999i64)).await;
    assert_eq!(res.unwrap_err().0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn close_already_closed_round_returns_not_found() {
    let pool = common::create_test_pool().await;
    let (_, axum::Json(body)) =
        open_round(State(common::make_state(pool.clone()))).await.unwrap();
    let id = body["id"].as_i64().unwrap();
    let _ = close_round(State(common::make_state(pool.clone())), Path(id))
        .await
        .unwrap();
    let res = close_round(State(common::make_state(pool)), Path(id)).await;
    assert_eq!(res.unwrap_err().0, StatusCode::NOT_FOUND);
}

// ── get_current_round ─────────────────────────────────────────────

#[tokio::test]
async fn get_current_round_returns_none_when_no_open() {
    let pool = common::create_test_pool().await;
    let axum::Json(result) =
        get_current_round(State(common::make_state(pool))).await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn get_current_round_returns_the_open_round() {
    let pool = common::create_test_pool().await;
    let (_, axum::Json(body)) =
        open_round(State(common::make_state(pool.clone()))).await.unwrap();
    let expected_id = body["id"].as_i64().unwrap();
    let axum::Json(result) =
        get_current_round(State(common::make_state(pool))).await.unwrap();
    assert_eq!(result.unwrap().id, expected_id);
}

// ── reopen_round ──────────────────────────────────────────────────

/// CLOSED 상태의 라운드를 만들고 results에 ranked+recommended 행을 삽입하는 헬퍼
async fn setup_closed_with_result(pool: &sqlx::SqlitePool) -> (i64, i64, i64) {
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
        "INSERT INTO univ_tracks (univ_id, track_name) VALUES (?, '컴공') RETURNING id",
    )
    .bind(univ_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at, closed_at) \
         VALUES ('CLOSED', '2025-01-01T00:00:00Z', '2025-01-02T00:00:00Z') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO applications (student_id, track_id, round_id, confirmed, abandoned) \
         VALUES (?, ?, ?, 1, 0)",
    )
    .bind(sid)
    .bind(tid)
    .bind(rid)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO results \
         (student_id, track_id, round_id, score_detail, total_score, ranking, recommended, calculated_at) \
         VALUES (?, ?, ?, '{}', 500000, 1, 1, '2025-01-02T00:00:00Z')",
    )
    .bind(sid)
    .bind(tid)
    .bind(rid)
    .execute(pool)
    .await
    .unwrap();
    (sid, tid, rid)
}

#[tokio::test]
async fn reopen_round_changes_status_to_open() {
    let pool = common::create_test_pool().await;
    let (_, _, rid) = setup_closed_with_result(&pool).await;
    reopen_round(State(common::make_state(pool.clone())), Path(rid))
        .await
        .unwrap();
    let status: String = sqlx::query_scalar("SELECT status FROM rounds WHERE id = ?")
        .bind(rid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "OPEN");
}

#[tokio::test]
async fn reopen_round_clears_closed_at() {
    let pool = common::create_test_pool().await;
    let (_, _, rid) = setup_closed_with_result(&pool).await;
    reopen_round(State(common::make_state(pool.clone())), Path(rid))
        .await
        .unwrap();
    let closed_at: Option<String> =
        sqlx::query_scalar("SELECT closed_at FROM rounds WHERE id = ?")
            .bind(rid)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(closed_at.is_none(), "reopen 후 closed_at은 NULL이어야 함");
}

#[tokio::test]
async fn reopen_round_resets_results_recommended_and_ranking() {
    let pool = common::create_test_pool().await;
    let (sid, _tid, rid) = setup_closed_with_result(&pool).await;
    // 재개 전: recommended=1, ranking=1 확인
    let (rec_before, rank_before): (i64, Option<i64>) = sqlx::query_as(
        "SELECT recommended, ranking FROM results WHERE student_id = ? AND round_id = ?",
    )
    .bind(sid)
    .bind(rid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(rec_before, 1);
    assert_eq!(rank_before, Some(1));

    reopen_round(State(common::make_state(pool.clone())), Path(rid))
        .await
        .unwrap();

    let (rec_after, rank_after): (i64, Option<i64>) = sqlx::query_as(
        "SELECT recommended, ranking FROM results WHERE student_id = ? AND round_id = ?",
    )
    .bind(sid)
    .bind(rid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(rec_after, 0, "reopen 후 recommended는 0이어야 함");
    assert!(rank_after.is_none(), "reopen 후 ranking은 NULL이어야 함");
}

#[tokio::test]
async fn reopen_nonexistent_round_returns_not_found() {
    let pool = common::create_test_pool().await;
    let res = reopen_round(State(common::make_state(pool)), Path(9999i64)).await;
    assert_eq!(res.unwrap_err().0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn reopen_open_round_returns_not_found() {
    // OPEN 상태 라운드는 CLOSED가 아니므로 404
    let pool = common::create_test_pool().await;
    let (_, axum::Json(body)) =
        open_round(State(common::make_state(pool.clone()))).await.unwrap();
    let id = body["id"].as_i64().unwrap();
    let res = reopen_round(State(common::make_state(pool)), Path(id)).await;
    assert_eq!(res.unwrap_err().0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn reopen_finalized_round_returns_not_found() {
    // FINALIZED 상태는 reopen 불가
    let pool = common::create_test_pool().await;
    let (_, _, rid) = setup_closed_with_result(&pool).await;
    finalize_round(State(common::make_state(pool.clone())), Path(rid))
        .await
        .unwrap();
    let res = reopen_round(State(common::make_state(pool)), Path(rid)).await;
    assert_eq!(res.unwrap_err().0, StatusCode::NOT_FOUND);
}

// ── finalize_round ────────────────────────────────────────────────

#[tokio::test]
async fn finalize_round_changes_status_to_finalized() {
    let pool = common::create_test_pool().await;
    let (_, _, rid) = setup_closed_with_result(&pool).await;
    finalize_round(State(common::make_state(pool.clone())), Path(rid))
        .await
        .unwrap();
    let status: String = sqlx::query_scalar("SELECT status FROM rounds WHERE id = ?")
        .bind(rid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "FINALIZED");
}

#[tokio::test]
async fn finalize_round_sets_finalized_at() {
    let pool = common::create_test_pool().await;
    let (_, _, rid) = setup_closed_with_result(&pool).await;
    finalize_round(State(common::make_state(pool.clone())), Path(rid))
        .await
        .unwrap();
    let finalized_at: Option<String> =
        sqlx::query_scalar("SELECT finalized_at FROM rounds WHERE id = ?")
            .bind(rid)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(finalized_at.is_some(), "finalize 후 finalized_at이 설정되어야 함");
}

#[tokio::test]
async fn finalize_nonexistent_round_returns_not_found() {
    let pool = common::create_test_pool().await;
    let res = finalize_round(State(common::make_state(pool)), Path(9999i64)).await;
    assert_eq!(res.unwrap_err().0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn finalize_open_round_returns_not_found() {
    // OPEN 상태는 FINALIZED 불가 — CLOSED에서만 가능
    let pool = common::create_test_pool().await;
    let (_, axum::Json(body)) =
        open_round(State(common::make_state(pool.clone()))).await.unwrap();
    let id = body["id"].as_i64().unwrap();
    let res = finalize_round(State(common::make_state(pool)), Path(id)).await;
    assert_eq!(res.unwrap_err().0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn finalize_already_finalized_round_returns_not_found() {
    let pool = common::create_test_pool().await;
    let (_, _, rid) = setup_closed_with_result(&pool).await;
    finalize_round(State(common::make_state(pool.clone())), Path(rid))
        .await
        .unwrap();
    // 두 번째 finalize 시도
    let res = finalize_round(State(common::make_state(pool)), Path(rid)).await;
    assert_eq!(res.unwrap_err().0, StatusCode::NOT_FOUND);
}

// ── open_round TOCTOU fix ─────────────────────────────────────────

/// 두 요청이 동시에 open_round를 호출할 때 정확히 하나만 성공해야 한다.
/// INSERT ... WHERE NOT EXISTS 원자적 쿼리로 race condition을 방지한다.
#[tokio::test]
async fn open_round_concurrent_creates_exactly_one() {
    let pool = common::create_test_pool_shared().await;
    let (r1, r2) = tokio::join!(
        open_round(State(common::make_state(pool.clone()))),
        open_round(State(common::make_state(pool.clone()))),
    );
    let statuses: Vec<StatusCode> = [r1, r2]
        .into_iter()
        .map(|r| r.map(|(s, _)| s).unwrap_or_else(|(s, _)| s))
        .collect();
    assert!(
        statuses.contains(&StatusCode::CREATED),
        "동시 요청 중 하나는 CREATED여야 함: {statuses:?}"
    );
    assert!(
        statuses.contains(&StatusCode::CONFLICT),
        "동시 요청 중 하나는 CONFLICT여야 함: {statuses:?}"
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM rounds WHERE status = 'OPEN'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "OPEN 라운드는 정확히 1개여야 함");
}

// ── close_round 트랜잭션 fix ──────────────────────────────────────

/// 기초데이터가 누락된 상태에서 close_round를 호출하면 422를 반환하고
/// 라운드 상태가 OPEN으로 유지되어야 한다.
#[tokio::test]
async fn close_round_with_missing_base_data_returns_unprocessable_and_keeps_open() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;

    // area 하나 생성 (NUMERIC, UPPER, SIMPLE)
    let area_id: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, calc_type, max_score, match_mode, lookup_scope) \
         VALUES ('내신', 'NUMERIC', 10000000, 'UPPER', 'SIMPLE') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // numeric_table 기준 삽입
    sqlx::query(
        "INSERT INTO numeric_table (area_id, track_id, threshold, score) VALUES (?, NULL, 0, 0)",
    )
    .bind(area_id)
    .execute(&pool)
    .await
    .unwrap();

    // 학생·대학·트랙·라운드·지원 설정 (base_data는 의도적으로 누락)
    let sid: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES ('S001', '홍길동', 1, 1, 1, 1) RETURNING id",
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
        "INSERT INTO univ_tracks (univ_id, track_name) VALUES (?, '컴공') RETURNING id",
    )
    .bind(univ_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let (_, axum::Json(body)) =
        open_round(State(common::make_state(pool.clone()))).await.unwrap();
    let rid = body["id"].as_i64().unwrap();
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

    // base_data 없이 close 시도 → 422
    let res = close_round(State(common::make_state(pool.clone())), Path(rid)).await;
    assert_eq!(res.unwrap_err().0, StatusCode::UNPROCESSABLE_ENTITY);

    // 라운드 상태가 OPEN으로 유지되어야 함
    let status: String = sqlx::query_scalar("SELECT status FROM rounds WHERE id = ?")
        .bind(rid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "OPEN", "기초데이터 누락 검증 실패 후 상태는 OPEN이어야 함");
}

/// 기초데이터가 완전히 입력된 경우 close_round가 성공하고
/// 검증과 상태 변경이 원자적으로 처리되어야 한다.
#[tokio::test]
async fn close_round_with_complete_base_data_succeeds_atomically() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;

    // area 하나 (NUMERIC, UPPER, SIMPLE)
    let area_id: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, calc_type, max_score, match_mode, lookup_scope) \
         VALUES ('내신', 'NUMERIC', 10000000, 'UPPER', 'SIMPLE') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    // threshold=0, score=0 — 모든 값(≥0)이 이 구간에 매칭됨
    sqlx::query(
        "INSERT INTO numeric_table (area_id, track_id, threshold, score) VALUES (?, NULL, 0, 0)",
    )
    .bind(area_id)
    .execute(&pool)
    .await
    .unwrap();

    let sid: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES ('S001', '홍길동', 1, 1, 1, 1) RETURNING id",
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
        "INSERT INTO univ_tracks (univ_id, track_name) VALUES (?, '컴공') RETURNING id",
    )
    .bind(univ_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let (_, axum::Json(body)) =
        open_round(State(common::make_state(pool.clone()))).await.unwrap();
    let rid = body["id"].as_i64().unwrap();
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

    // base_data 삽입 (×100000 스케일 저장값 '0')
    sqlx::query(
        "INSERT INTO base_data (student_id, area_id, track_id, value, multi_value) \
         VALUES (?, ?, NULL, '0', 0)",
    )
    .bind(sid)
    .bind(area_id)
    .execute(&pool)
    .await
    .unwrap();

    // close 성공
    let res = close_round(State(common::make_state(pool.clone())), Path(rid)).await;
    assert!(res.is_ok(), "기초데이터 완비 시 close_round는 성공해야 함");

    let status: String = sqlx::query_scalar("SELECT status FROM rounds WHERE id = ?")
        .bind(rid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "CLOSED");
}
