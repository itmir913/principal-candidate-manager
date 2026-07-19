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
        "INSERT INTO applications (student_id, track_id, round_id, abandoned) \
         VALUES (?, ?, ?, 0)",
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
async fn reopen_round_clears_excluded_applications() {
    let pool = common::create_test_pool().await;
    let hash = bcrypt::hash("pass", 4u32).unwrap();
    sqlx::query("INSERT INTO classes (grade, class_no, password_hash) VALUES (1, 1, ?)")
        .bind(&hash)
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
    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at, closed_at) \
         VALUES ('CLOSED', '2025-01-01T00:00:00Z', '2025-01-02T00:00:00Z') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    // INSERT 시 excluded=1 직접 지정 (INSERT는 트리거 비대상, DB CHECK 만족)
    sqlx::query(
        "INSERT INTO applications (student_id, track_id, round_id, excluded, excluded_reason) \
         VALUES (?, ?, ?, 1, '테스트 미선발 사유')",
    )
    .bind(sid)
    .bind(tid)
    .bind(rid)
    .execute(&pool)
    .await
    .unwrap();

    reopen_round(State(common::make_state(pool.clone())), Path(rid))
        .await
        .unwrap();

    let (excluded, reason): (i64, Option<String>) = sqlx::query_as(
        "SELECT excluded, excluded_reason FROM applications WHERE student_id = ? AND round_id = ?",
    )
    .bind(sid)
    .bind(rid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(excluded, 0, "reopen 후 excluded는 0이어야 함");
    assert!(reason.is_none(), "reopen 후 excluded_reason은 NULL이어야 함");
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
        "INSERT INTO applications (student_id, track_id, round_id, abandoned) \
         VALUES (?, ?, ?, 0)",
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
        "INSERT INTO applications (student_id, track_id, round_id, abandoned) \
         VALUES (?, ?, ?, 0)",
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

// ── finalize_round 정원 초과 검증 ────────────────────────────────

/// 정원 없음(NULL) → finalize 성공 (기존 동작 유지)
#[tokio::test]
async fn finalize_round_no_quota_succeeds() {
    let pool = common::create_test_pool().await;
    let (_, _, rid) = setup_closed_with_result(&pool).await;
    let res = finalize_round(State(common::make_state(pool.clone())), Path(rid)).await;
    assert!(res.is_ok(), "정원 미설정 시 finalize는 항상 성공해야 함");
}

/// 모집단위 정원 이내 → finalize 성공
#[tokio::test]
async fn finalize_round_within_track_quota_succeeds() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    // unit_quota=2, recommended=1
    let univ_id: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name) VALUES ('한국대') RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name, unit_quota) VALUES (?, '컴공', 2) RETURNING id",
    )
    .bind(univ_id).fetch_one(&pool).await.unwrap();
    let sid: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES ('S001', '홍길동', 1, 1, 1, 1) RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();
    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at, closed_at) \
         VALUES ('CLOSED', '2025-01-01T00:00:00Z', '2025-01-02T00:00:00Z') RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO applications (student_id, track_id, round_id, abandoned) VALUES (?, ?, ?, 0)")
        .bind(sid).bind(tid).bind(rid).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO results (student_id, track_id, round_id, score_detail, total_score, ranking, recommended, calculated_at) \
         VALUES (?, ?, ?, '{}', 0, 1, 1, '2025-01-02T00:00:00Z')",
    )
    .bind(sid).bind(tid).bind(rid).execute(&pool).await.unwrap();

    let res = finalize_round(State(common::make_state(pool)), Path(rid)).await;
    assert!(res.is_ok(), "모집단위 정원 이내이면 finalize 성공해야 함");
}

/// 모집단위 정원 초과 → 422, 상태 CLOSED 유지
#[tokio::test]
async fn finalize_round_exceeds_track_quota_returns_unprocessable() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    // unit_quota=1 이지만 recommended=2
    let univ_id: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name) VALUES ('한국대') RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name, unit_quota) VALUES (?, '컴공', 1) RETURNING id",
    )
    .bind(univ_id).fetch_one(&pool).await.unwrap();
    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at, closed_at) \
         VALUES ('CLOSED', '2025-01-01T00:00:00Z', '2025-01-02T00:00:00Z') RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();
    for (code, name, seq) in [("S001", "홍길동", 1), ("S002", "김철수", 2)] {
        let sid: i64 = sqlx::query_scalar(
            "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
             VALUES (?, ?, 1, 1, ?, 1) RETURNING id",
        )
        .bind(code).bind(name).bind(seq).fetch_one(&pool).await.unwrap();
        sqlx::query("INSERT INTO applications (student_id, track_id, round_id, abandoned) VALUES (?, ?, ?, 0)")
            .bind(sid).bind(tid).bind(rid).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO results (student_id, track_id, round_id, score_detail, total_score, recommended, calculated_at) \
             VALUES (?, ?, ?, '{}', 0, 1, '2025-01-02T00:00:00Z')",
        )
        .bind(sid).bind(tid).bind(rid).execute(&pool).await.unwrap();
    }

    let res = finalize_round(State(common::make_state(pool.clone())), Path(rid)).await;
    assert_eq!(res.unwrap_err().0, StatusCode::UNPROCESSABLE_ENTITY, "모집단위 정원 초과 시 422여야 함");

    let status: String = sqlx::query_scalar("SELECT status FROM rounds WHERE id = ?")
        .bind(rid).fetch_one(&pool).await.unwrap();
    assert_eq!(status, "CLOSED", "정원 초과 검증 실패 후 상태는 CLOSED 유지");
}

/// 대학 전체 정원 초과 → 422, 상태 CLOSED 유지
#[tokio::test]
async fn finalize_round_exceeds_univ_quota_returns_unprocessable() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    // total_quota=1, 트랙 2개에 각 1명씩 recommended → 합계 2
    let univ_id: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name, total_quota) VALUES ('한국대', 1) RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();
    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at, closed_at) \
         VALUES ('CLOSED', '2025-01-01T00:00:00Z', '2025-01-02T00:00:00Z') RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();
    for (track_name, code, name, seq) in [("컴공", "S001", "홍길동", 1), ("기계", "S002", "김철수", 2)] {
        let tid: i64 = sqlx::query_scalar(
            "INSERT INTO univ_tracks (univ_id, track_name) VALUES (?, ?) RETURNING id",
        )
        .bind(univ_id).bind(track_name).fetch_one(&pool).await.unwrap();
        let sid: i64 = sqlx::query_scalar(
            "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
             VALUES (?, ?, 1, 1, ?, 1) RETURNING id",
        )
        .bind(code).bind(name).bind(seq).fetch_one(&pool).await.unwrap();
        sqlx::query("INSERT INTO applications (student_id, track_id, round_id, abandoned) VALUES (?, ?, ?, 0)")
            .bind(sid).bind(tid).bind(rid).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO results (student_id, track_id, round_id, score_detail, total_score, recommended, calculated_at) \
             VALUES (?, ?, ?, '{}', 0, 1, '2025-01-02T00:00:00Z')",
        )
        .bind(sid).bind(tid).bind(rid).execute(&pool).await.unwrap();
    }

    let res = finalize_round(State(common::make_state(pool.clone())), Path(rid)).await;
    assert_eq!(res.unwrap_err().0, StatusCode::UNPROCESSABLE_ENTITY, "대학 전체 정원 초과 시 422여야 함");

    let status: String = sqlx::query_scalar("SELECT status FROM rounds WHERE id = ?")
        .bind(rid).fetch_one(&pool).await.unwrap();
    assert_eq!(status, "CLOSED", "대학 정원 초과 검증 실패 후 상태는 CLOSED 유지");
}

// ── 세션 4 감사 후속: 비-FINALIZED 라운드 단일성 DB 방어선 ────────

#[tokio::test]
async fn db_rejects_second_active_round() {
    let pool = common::create_test_pool().await;

    sqlx::query("INSERT INTO rounds (status, opened_at, closed_at) VALUES ('CLOSED', 'now', 'now')")
        .execute(&pool)
        .await
        .unwrap();

    // CLOSED가 남아있는 상태에서 새 OPEN 라운드 직접 삽입 → 인덱스가 차단
    let res = sqlx::query("INSERT INTO rounds (status, opened_at) VALUES ('OPEN', 'now')")
        .execute(&pool)
        .await;
    assert!(res.is_err(), "비-FINALIZED 라운드는 동시에 1개만 존재해야 함");
    assert!(res.unwrap_err().to_string().contains("UNIQUE"));
}

#[tokio::test]
async fn db_allows_new_round_after_finalize() {
    let pool = common::create_test_pool().await;

    sqlx::query(
        "INSERT INTO rounds (status, opened_at, closed_at, finalized_at) VALUES ('FINALIZED', 'now', 'now', 'now')",
    )
    .execute(&pool)
    .await
    .unwrap();

    // FINALIZED 라운드가 있어도 새 라운드는 열 수 있어야 함
    sqlx::query("INSERT INTO rounds (status, opened_at) VALUES ('OPEN', 'now')")
        .execute(&pool)
        .await
        .unwrap();

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM rounds")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 2);
}

// ── 세션 4 감사 후속: CLOSED/FINALIZED results 삭제 차단 트리거 ───

#[tokio::test]
async fn db_rejects_result_delete_in_closed_round() {
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup_closed_with_result(&pool).await;

    let res = sqlx::query("DELETE FROM results WHERE student_id = ? AND track_id = ? AND round_id = ?")
        .bind(sid).bind(tid).bind(rid)
        .execute(&pool)
        .await;
    assert!(res.is_err(), "CLOSED 라운드 results 삭제는 차단되어야 함");

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM results WHERE round_id = ?")
        .bind(rid).fetch_one(&pool).await.unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn db_allows_result_delete_in_open_round() {
    // OPEN 라운드는 담임 지원 취소 경로에서 results 동반 삭제가 필요 — 허용 유지
    let pool = common::create_test_pool().await;
    let (sid, tid, rid) = setup_closed_with_result(&pool).await;
    sqlx::query("UPDATE rounds SET status = 'OPEN', closed_at = NULL WHERE id = ?")
        .bind(rid).execute(&pool).await.unwrap();

    sqlx::query("DELETE FROM results WHERE student_id = ? AND track_id = ? AND round_id = ?")
        .bind(sid).bind(tid).bind(rid)
        .execute(&pool)
        .await
        .unwrap();

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM results WHERE round_id = ?")
        .bind(rid).fetch_one(&pool).await.unwrap();
    assert_eq!(count, 0);
}

// ── B단계: 미결정 지원 마감 차단 ─────────────────────────────────

/// 미결정(excluded=0, recommended=0) 지원이 있을 때 finalize_round → 422.
/// T1: 지원 2건 중 1건만 추천, 나머지 방치 → undecided 명단 1건 반환.
#[tokio::test]
async fn finalize_blocked_when_undecided_application_remains() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name) VALUES ('한국대') RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name) VALUES (?, '컴공') RETURNING id",
    )
    .bind(uid).fetch_one(&pool).await.unwrap();
    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at, closed_at) \
         VALUES ('CLOSED', '2025-01-01T00:00:00Z', '2025-01-02T00:00:00Z') RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();

    let sid1: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES ('S001', '홍길동', 1, 1, 1, 1) RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();
    let sid2: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES ('S002', '김철수', 1, 1, 2, 1) RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();

    for sid in [sid1, sid2] {
        sqlx::query("INSERT INTO applications (student_id, track_id, round_id, abandoned) VALUES (?, ?, ?, 0)")
            .bind(sid).bind(tid).bind(rid).execute(&pool).await.unwrap();
    }
    // sid1만 추천 — sid2는 results 행 있고 recommended=0 (미결정)
    for (sid, rec) in [(sid1, 1i64), (sid2, 0)] {
        sqlx::query(
            "INSERT INTO results (student_id, track_id, round_id, score_detail, total_score, ranking, recommended, calculated_at) \
             VALUES (?, ?, ?, '{}', 500000, ?, ?, '2025-01-02T00:00:00Z')",
        )
        .bind(sid).bind(tid).bind(rid).bind(if rec == 1 { 1i64 } else { 2 }).bind(rec)
        .execute(&pool).await.unwrap();
    }

    let err = finalize_round(State(common::make_state(pool.clone())), Path(rid))
        .await
        .unwrap_err();
    assert_eq!(err.0, StatusCode::UNPROCESSABLE_ENTITY);

    let body: serde_json::Value = serde_json::from_str(&err.1).unwrap();
    let undecided = body["undecided"].as_array().unwrap();
    assert_eq!(undecided.len(), 1, "미결정 1건이어야 함");
    assert_eq!(undecided[0]["student_code"].as_str().unwrap(), "S002");
    assert_eq!(undecided[0]["student_name"].as_str().unwrap(), "김철수");
    assert_eq!(undecided[0]["univ_name"].as_str().unwrap(), "한국대");
    assert_eq!(undecided[0]["track_name"].as_str().unwrap(), "컴공");
}

/// T2: 전건 추천 → 마감 성공.
#[tokio::test]
async fn finalize_succeeds_when_all_recommended() {
    let pool = common::create_test_pool().await;
    let (_, _, rid) = setup_closed_with_result(&pool).await;
    // setup_closed_with_result이 recommended=1로 결과를 삽입하므로 미결정 없음
    let res = finalize_round(State(common::make_state(pool.clone())), Path(rid)).await;
    assert!(res.is_ok(), "전건 추천 완료 시 finalize는 성공해야 함");
    let status: String = sqlx::query_scalar("SELECT status FROM rounds WHERE id = ?")
        .bind(rid).fetch_one(&pool).await.unwrap();
    assert_eq!(status, "FINALIZED");
}

/// T3: 추천 + 제외 혼합 → 마감 성공.
#[tokio::test]
async fn finalize_succeeds_with_mixed_recommended_and_excluded() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name) VALUES ('한국대') RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name) VALUES (?, '컴공') RETURNING id",
    )
    .bind(uid).fetch_one(&pool).await.unwrap();
    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at, closed_at) \
         VALUES ('CLOSED', '2025-01-01T00:00:00Z', '2025-01-02T00:00:00Z') RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();

    // sid1: 추천 확정, sid2: 제외(결격)
    let sid1: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES ('S001', '홍길동', 1, 1, 1, 1) RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();
    let sid2: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES ('S002', '김철수', 1, 1, 2, 1) RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();

    sqlx::query("INSERT INTO applications (student_id, track_id, round_id, abandoned) VALUES (?, ?, ?, 0)")
        .bind(sid1).bind(tid).bind(rid).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO applications (student_id, track_id, round_id, abandoned, excluded, excluded_reason) \
         VALUES (?, ?, ?, 0, 1, '결격')",
    )
    .bind(sid2).bind(tid).bind(rid).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO results (student_id, track_id, round_id, score_detail, total_score, ranking, recommended, calculated_at) \
         VALUES (?, ?, ?, '{}', 500000, 1, 1, '2025-01-02T00:00:00Z')",
    )
    .bind(sid1).bind(tid).bind(rid).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO results (student_id, track_id, round_id, score_detail, total_score, ranking, recommended, calculated_at) \
         VALUES (?, ?, ?, '{}', 400000, 2, 0, '2025-01-02T00:00:00Z')",
    )
    .bind(sid2).bind(tid).bind(rid).execute(&pool).await.unwrap();

    let res = finalize_round(State(common::make_state(pool)), Path(rid)).await;
    assert!(res.is_ok(), "추천+제외 혼합 시 finalize는 성공해야 함");
}

/// T4: 전건 제외 → 마감 성공 (추천자가 0명이어도 전원 제외면 가능).
#[tokio::test]
async fn finalize_succeeds_when_all_excluded() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name) VALUES ('한국대') RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name) VALUES (?, '컴공') RETURNING id",
    )
    .bind(uid).fetch_one(&pool).await.unwrap();
    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at, closed_at) \
         VALUES ('CLOSED', '2025-01-01T00:00:00Z', '2025-01-02T00:00:00Z') RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();
    let sid: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES ('S001', '홍길동', 1, 1, 1, 1) RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO applications (student_id, track_id, round_id, abandoned, excluded, excluded_reason) \
         VALUES (?, ?, ?, 0, 1, '결격')",
    )
    .bind(sid).bind(tid).bind(rid).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO results (student_id, track_id, round_id, score_detail, total_score, ranking, recommended, calculated_at) \
         VALUES (?, ?, ?, '{}', 500000, 1, 0, '2025-01-02T00:00:00Z')",
    )
    .bind(sid).bind(tid).bind(rid).execute(&pool).await.unwrap();

    let res = finalize_round(State(common::make_state(pool)), Path(rid)).await;
    assert!(res.is_ok(), "전원 제외 시에도 finalize는 성공해야 함");
}

/// T5: results 행이 없는 지원(점수 미계산) → 미결정으로 판정 → 422.
#[tokio::test]
async fn finalize_blocked_when_application_has_no_results_row() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name) VALUES ('한국대') RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name) VALUES (?, '컴공') RETURNING id",
    )
    .bind(uid).fetch_one(&pool).await.unwrap();
    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at, closed_at) \
         VALUES ('CLOSED', '2025-01-01T00:00:00Z', '2025-01-02T00:00:00Z') RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();
    let sid: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES ('S001', '홍길동', 1, 1, 1, 1) RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();
    // applications만 삽입 — results 행 없음(점수 미계산)
    sqlx::query("INSERT INTO applications (student_id, track_id, round_id, abandoned) VALUES (?, ?, ?, 0)")
        .bind(sid).bind(tid).bind(rid).execute(&pool).await.unwrap();

    let err = finalize_round(State(common::make_state(pool)), Path(rid))
        .await
        .unwrap_err();
    assert_eq!(err.0, StatusCode::UNPROCESSABLE_ENTITY, "results 행 없는 지원은 미결정으로 판정되어 422여야 함");
    let body: serde_json::Value = serde_json::from_str(&err.1).unwrap();
    assert!(body.get("undecided").is_some(), "응답 본문에 undecided 키가 있어야 함");
}

/// T6: 미결정 + 정원 초과 동시 → 미결정이 먼저 반환된다.
#[tokio::test]
async fn finalize_returns_undecided_before_quota_violation() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name) VALUES ('한국대') RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();
    // unit_quota=1, 두 학생 모두 recommended=1이면 정원 초과
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name, unit_quota) VALUES (?, '컴공', 1) RETURNING id",
    )
    .bind(uid).fetch_one(&pool).await.unwrap();
    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at, closed_at) \
         VALUES ('CLOSED', '2025-01-01T00:00:00Z', '2025-01-02T00:00:00Z') RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();
    // sid1: 추천(정원 초과 유발), sid2: 미결정
    for (code, name, seq, rec) in [("S001", "홍길동", 1, 1i64), ("S002", "김철수", 2, 0)] {
        let sid: i64 = sqlx::query_scalar(
            "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
             VALUES (?, ?, 1, 1, ?, 1) RETURNING id",
        )
        .bind(code).bind(name).bind(seq).fetch_one(&pool).await.unwrap();
        sqlx::query("INSERT INTO applications (student_id, track_id, round_id, abandoned) VALUES (?, ?, ?, 0)")
            .bind(sid).bind(tid).bind(rid).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO results (student_id, track_id, round_id, score_detail, total_score, ranking, recommended, calculated_at) \
             VALUES (?, ?, ?, '{}', 500000, ?, ?, '2025-01-02T00:00:00Z')",
        )
        .bind(sid).bind(tid).bind(rid).bind(seq).bind(rec)
        .execute(&pool).await.unwrap();
    }

    let err = finalize_round(State(common::make_state(pool)), Path(rid))
        .await
        .unwrap_err();
    assert_eq!(err.0, StatusCode::UNPROCESSABLE_ENTITY);
    let body: serde_json::Value = serde_json::from_str(&err.1).unwrap();
    assert!(body.get("undecided").is_some(), "미결정이 정원 초과보다 먼저 반환되어야 함: undecided 키가 있어야 함");
    assert!(body.get("track_violations").is_none(), "미결정 오류에 track_violations 키가 없어야 함");
    assert!(body.get("univ_violations").is_none(), "미결정 오류에 univ_violations 키가 없어야 함");
}

/// T7: 422 이후 상태 무결성 — 라운드 CLOSED 유지, results 불변.
#[tokio::test]
async fn finalize_422_leaves_round_closed_and_results_unchanged() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name) VALUES ('한국대') RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name) VALUES (?, '컴공') RETURNING id",
    )
    .bind(uid).fetch_one(&pool).await.unwrap();
    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at, closed_at) \
         VALUES ('CLOSED', '2025-01-01T00:00:00Z', '2025-01-02T00:00:00Z') RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();
    let sid: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES ('S001', '홍길동', 1, 1, 1, 1) RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO applications (student_id, track_id, round_id, abandoned) VALUES (?, ?, ?, 0)")
        .bind(sid).bind(tid).bind(rid).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO results (student_id, track_id, round_id, score_detail, total_score, ranking, recommended, calculated_at) \
         VALUES (?, ?, ?, '{}', 500000, 1, 0, '2025-01-02T00:00:00Z')",
    )
    .bind(sid).bind(tid).bind(rid).execute(&pool).await.unwrap();

    let err = finalize_round(State(common::make_state(pool.clone())), Path(rid))
        .await
        .unwrap_err();
    assert_eq!(err.0, StatusCode::UNPROCESSABLE_ENTITY);

    let status: String = sqlx::query_scalar("SELECT status FROM rounds WHERE id = ?")
        .bind(rid).fetch_one(&pool).await.unwrap();
    assert_eq!(status, "CLOSED", "422 후 라운드는 CLOSED 유지");

    let recommended: i64 = sqlx::query_scalar(
        "SELECT recommended FROM results WHERE student_id = ? AND round_id = ?",
    )
    .bind(sid).bind(rid).fetch_one(&pool).await.unwrap();
    assert_eq!(recommended, 0, "422 후 results.recommended는 변하지 않아야 함");
}

/// T8: 트리거 직접 검증 — 핸들러 우회 SQL로 FINALIZED 전환 시도.
/// 미결정 있으면 트리거 ABORT, 미결정 없으면 통과.
#[tokio::test]
async fn trigger_blocks_direct_sql_finalize_when_undecided() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name) VALUES ('한국대') RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name) VALUES (?, '컴공') RETURNING id",
    )
    .bind(uid).fetch_one(&pool).await.unwrap();
    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at, closed_at) \
         VALUES ('CLOSED', '2025-01-01T00:00:00Z', '2025-01-02T00:00:00Z') RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();
    let sid: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES ('S001', '홍길동', 1, 1, 1, 1) RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO applications (student_id, track_id, round_id, abandoned) VALUES (?, ?, ?, 0)")
        .bind(sid).bind(tid).bind(rid).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO results (student_id, track_id, round_id, score_detail, total_score, ranking, recommended, calculated_at) \
         VALUES (?, ?, ?, '{}', 500000, 1, 0, '2025-01-02T00:00:00Z')",
    )
    .bind(sid).bind(tid).bind(rid).execute(&pool).await.unwrap();

    // 미결정(recommended=0) 상태에서 직접 UPDATE → 트리거가 ABORT
    let result = sqlx::query(
        "UPDATE rounds SET status = 'FINALIZED', finalized_at = 'now' WHERE id = ?",
    )
    .bind(rid)
    .execute(&pool)
    .await;
    assert!(result.is_err(), "미결정 지원이 있을 때 직접 FINALIZED 전환은 트리거가 차단해야 함");
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("undecided applications remain"),
        "트리거가 아닌 다른 이유로 실패: {msg}"
    );

    let status: String = sqlx::query_scalar("SELECT status FROM rounds WHERE id = ?")
        .bind(rid).fetch_one(&pool).await.unwrap();
    assert_eq!(status, "CLOSED", "트리거 ABORT 후 라운드는 CLOSED 유지");

    // recommended=1로 변경 후 직접 UPDATE → 통과
    sqlx::query("UPDATE results SET recommended = 1 WHERE student_id = ? AND round_id = ?")
        .bind(sid).bind(rid).execute(&pool).await.unwrap();
    sqlx::query(
        "UPDATE rounds SET status = 'FINALIZED', finalized_at = 'now' WHERE id = ?",
    )
    .bind(rid)
    .execute(&pool)
    .await
    .unwrap();
    let status: String = sqlx::query_scalar("SELECT status FROM rounds WHERE id = ?")
        .bind(rid).fetch_one(&pool).await.unwrap();
    assert_eq!(status, "FINALIZED", "미결정 없으면 직접 SQL FINALIZED 전환이 허용되어야 함");
}

// ── E1: 재오픈 → 재학생 우선 변경 → 재마감 경로가 순위를 갱신 ─────
// CLOSED 중 prioritize 변경은 409로 막히므로(handler_universities.rs), 이 경로가
// 관리자의 유일한 탈출구다. 실제로 저장 순위가 새 설정으로 재계산되는지 확인한다.

#[tokio::test]
async fn reopen_change_prioritize_reclose_recalculates_ranking() {
    let pool = common::create_test_pool().await;
    let st = || State(common::make_state(pool.clone()));

    let area_id: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, calc_type, max_score, match_mode, lookup_scope) \
         VALUES ('내신', 'NUMERIC', 10000000, 'UPPER', 'SIMPLE') RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();
    // threshold 이상이면 그 점수 — 재학생 1점, 졸업생 2점이 되도록 두 구간
    for (threshold, score) in [(0i64, 100000i64), (200000, 200000)] {
        sqlx::query("INSERT INTO numeric_table (area_id, track_id, threshold, score) VALUES (?, NULL, ?, ?)")
            .bind(area_id).bind(threshold).bind(score)
            .execute(&pool).await.unwrap();
    }

    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name, prioritize_enrolled) VALUES ('한국대', 0) RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name) VALUES (?, '컴공') RETURNING id",
    )
    .bind(uid).fetch_one(&pool).await.unwrap();

    // 재학생(낮은 점수) / 졸업생(높은 점수). 졸업생 담당은 특수 계정 0/0
    let hash = bcrypt::hash("pass", 4u32).unwrap();
    for (g, c) in [(1i64, 1i64), (0, 0)] {
        sqlx::query("INSERT INTO classes (grade, class_no, password_hash) VALUES (?, ?, ?)")
            .bind(g).bind(c).bind(&hash).execute(&pool).await.unwrap();
    }
    let enrolled: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES ('S001', '재학생', 1, 1, 1, 1) RETURNING id",
    ).fetch_one(&pool).await.unwrap();
    let graduated: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grad_year, is_enrolled) \
         VALUES ('S002', '졸업생', 2024, 0) RETURNING id",
    ).fetch_one(&pool).await.unwrap();

    let (_, axum::Json(body)) = open_round(st()).await.unwrap();
    let rid = body["id"].as_i64().unwrap();
    for (sid, value) in [(enrolled, "0"), (graduated, "200000")] {
        sqlx::query("INSERT INTO applications (student_id, track_id, round_id, abandoned) VALUES (?, ?, ?, 0)")
            .bind(sid).bind(tid).bind(rid).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO base_data (student_id, area_id, track_id, value, multi_value) VALUES (?, ?, NULL, ?, 0)")
            .bind(sid).bind(area_id).bind(value).execute(&pool).await.unwrap();
    }

    let ranking = |sid: i64| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, Option<i64>>(
                "SELECT ranking FROM results WHERE student_id = ? AND round_id = ?",
            )
            .bind(sid).bind(rid).fetch_one(&pool).await.unwrap()
        }
    };

    // 1차 마감: 재학생 우선 OFF → 점수 높은 졸업생이 1위
    close_round(st(), Path(rid)).await.unwrap();
    assert_eq!(ranking(graduated).await, Some(1), "우선 OFF: 고득점 졸업생이 1위");
    assert_eq!(ranking(enrolled).await, Some(2));

    // 재오픈 → 설정 변경 → 재마감
    reopen_round(st(), Path(rid)).await.unwrap();
    sqlx::query("UPDATE universities SET prioritize_enrolled = 1 WHERE id = ?")
        .bind(uid).execute(&pool).await.unwrap();
    close_round(st(), Path(rid)).await.unwrap();

    assert_eq!(ranking(enrolled).await, Some(1), "재마감 후 재학생 우선이 반영되어야 한다");
    assert_eq!(ranking(graduated).await, Some(2));
}
