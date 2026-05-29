mod common;

use axum::{
    extract::{Path, State},
    http::StatusCode,
};
use principal_candidate_manager::handlers::rounds::{
    close_round, get_current_round, open_round,
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
async fn open_round_after_close_creates_new_round() {
    let pool = common::create_test_pool().await;
    let (_, axum::Json(body)) =
        open_round(State(common::make_state(pool.clone()))).await.unwrap();
    let id = body["id"].as_i64().unwrap();
    close_round(State(common::make_state(pool.clone())), Path(id))
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
    close_round(State(common::make_state(pool.clone())), Path(id))
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
    close_round(State(common::make_state(pool.clone())), Path(id))
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
    close_round(State(common::make_state(pool.clone())), Path(id))
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
