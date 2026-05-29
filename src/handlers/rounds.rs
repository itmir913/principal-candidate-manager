use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Serialize;
use sqlx::FromRow;

use crate::state::AppState;

type ApiError = (StatusCode, String);

#[derive(Serialize, FromRow)]
pub struct RoundRow {
    pub id: i64,
    pub status: String,
    pub opened_at: String,
    pub closed_at: Option<String>,
}

pub async fn list_rounds(
    State(state): State<AppState>,
) -> Result<Json<Vec<RoundRow>>, ApiError> {
    let rows = sqlx::query_as::<_, RoundRow>(
        "SELECT id, status, opened_at, closed_at FROM rounds ORDER BY id DESC",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(rows))
}

pub async fn get_current_round(
    State(state): State<AppState>,
) -> Result<Json<Option<RoundRow>>, ApiError> {
    let row = sqlx::query_as::<_, RoundRow>(
        "SELECT id, status, opened_at, closed_at FROM rounds WHERE status = 'OPEN' LIMIT 1",
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(row))
}

pub async fn open_round(
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let existing: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM rounds WHERE status = 'OPEN' LIMIT 1",
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if existing.is_some() {
        return Err((StatusCode::CONFLICT, "이미 OPEN 상태의 라운드가 있습니다".into()));
    }

    let now = chrono::Utc::now().to_rfc3339();
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at) VALUES ('OPEN', ?) RETURNING id",
    )
    .bind(&now)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

pub async fn close_round(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    let now = chrono::Utc::now().to_rfc3339();
    let affected = sqlx::query(
        "UPDATE rounds SET status = 'CLOSED', closed_at = ? WHERE id = ? AND status = 'OPEN'",
    )
    .bind(&now)
    .bind(id)
    .execute(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .rows_affected();

    if affected == 0 {
        return Err((StatusCode::NOT_FOUND, format!("라운드 id={} 없거나 이미 CLOSED", id)));
    }

    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::create_test_pool, state::AppState};
    use axum::{
        extract::{Path, State},
        http::StatusCode,
    };

    fn make_state(pool: sqlx::SqlitePool) -> AppState {
        AppState { db: pool, jwt_secret: "test".into() }
    }

    // ── open_round ────────────────────────────────────────────────────

    #[tokio::test]
    async fn open_round_creates_open_round() {
        let pool = create_test_pool().await;
        let (status, _) = open_round(State(make_state(pool.clone()))).await.unwrap();
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
        let pool = create_test_pool().await;
        open_round(State(make_state(pool.clone()))).await.unwrap();
        let res = open_round(State(make_state(pool))).await;
        assert_eq!(res.unwrap_err().0, StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn open_round_after_close_creates_new_round() {
        let pool = create_test_pool().await;
        let (_, axum::Json(body)) =
            open_round(State(make_state(pool.clone()))).await.unwrap();
        let id = body["id"].as_i64().unwrap();
        close_round(State(make_state(pool.clone())), Path(id))
            .await
            .unwrap();
        // 이제 새 라운드를 열 수 있어야 함
        let res = open_round(State(make_state(pool.clone()))).await;
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
        let pool = create_test_pool().await;
        let (_, axum::Json(body)) =
            open_round(State(make_state(pool.clone()))).await.unwrap();
        let id = body["id"].as_i64().unwrap();
        close_round(State(make_state(pool.clone())), Path(id))
            .await
            .unwrap();
        let status: String =
            sqlx::query_scalar("SELECT status FROM rounds WHERE id = ?")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(status, "CLOSED");
    }

    #[tokio::test]
    async fn close_round_sets_closed_at_timestamp() {
        let pool = create_test_pool().await;
        let (_, axum::Json(body)) =
            open_round(State(make_state(pool.clone()))).await.unwrap();
        let id = body["id"].as_i64().unwrap();
        close_round(State(make_state(pool.clone())), Path(id))
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
        let pool = create_test_pool().await;
        let res = close_round(State(make_state(pool)), Path(9999i64)).await;
        assert_eq!(res.unwrap_err().0, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn close_already_closed_round_returns_not_found() {
        let pool = create_test_pool().await;
        let (_, axum::Json(body)) =
            open_round(State(make_state(pool.clone()))).await.unwrap();
        let id = body["id"].as_i64().unwrap();
        close_round(State(make_state(pool.clone())), Path(id))
            .await
            .unwrap();
        let res = close_round(State(make_state(pool)), Path(id)).await;
        assert_eq!(res.unwrap_err().0, StatusCode::NOT_FOUND);
    }

    // ── get_current_round ─────────────────────────────────────────────

    #[tokio::test]
    async fn get_current_round_returns_none_when_no_open() {
        let pool = create_test_pool().await;
        let axum::Json(result) =
            get_current_round(State(make_state(pool))).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn get_current_round_returns_the_open_round() {
        let pool = create_test_pool().await;
        let (_, axum::Json(body)) =
            open_round(State(make_state(pool.clone()))).await.unwrap();
        let expected_id = body["id"].as_i64().unwrap();
        let axum::Json(result) =
            get_current_round(State(make_state(pool))).await.unwrap();
        assert_eq!(result.unwrap().id, expected_id);
    }
}
