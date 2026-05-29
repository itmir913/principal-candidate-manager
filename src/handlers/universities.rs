use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::state::AppState;

type ApiError = (StatusCode, String);

#[derive(Serialize, FromRow)]
pub struct UniversityRow {
    pub id: i64,
    pub univ_name: String,
    pub track_name: String,
    pub capacity: i64,
    pub prioritize_enrolled: i64,
}

#[derive(Deserialize)]
pub struct CreateUniversityBody {
    pub univ_name: String,
    pub track_name: String,
    pub capacity: i64,
    pub prioritize_enrolled: bool,
}

#[derive(Deserialize)]
pub struct UpdateUniversityBody {
    pub univ_name: Option<String>,
    pub track_name: Option<String>,
    pub capacity: Option<i64>,
    pub prioritize_enrolled: Option<bool>,
}

pub async fn list_universities(
    State(state): State<AppState>,
) -> Result<Json<Vec<UniversityRow>>, ApiError> {
    let rows = sqlx::query_as::<_, UniversityRow>(
        "SELECT id, univ_name, track_name, capacity, prioritize_enrolled
         FROM universities ORDER BY id",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(rows))
}

pub async fn create_university(
    State(state): State<AppState>,
    Json(body): Json<CreateUniversityBody>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let enrolled = body.prioritize_enrolled as i64;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name, track_name, capacity, prioritize_enrolled)
         VALUES (?, ?, ?, ?)
         RETURNING id",
    )
    .bind(&body.univ_name)
    .bind(&body.track_name)
    .bind(body.capacity)
    .bind(enrolled)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

pub async fn update_university(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateUniversityBody>,
) -> Result<StatusCode, ApiError> {
    if let Some(v) = body.univ_name {
        sqlx::query("UPDATE universities SET univ_name = ? WHERE id = ?")
            .bind(v)
            .bind(id)
            .execute(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    if let Some(v) = body.track_name {
        sqlx::query("UPDATE universities SET track_name = ? WHERE id = ?")
            .bind(v)
            .bind(id)
            .execute(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    if let Some(v) = body.capacity {
        sqlx::query("UPDATE universities SET capacity = ? WHERE id = ?")
            .bind(v)
            .bind(id)
            .execute(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    if let Some(v) = body.prioritize_enrolled {
        sqlx::query("UPDATE universities SET prioritize_enrolled = ? WHERE id = ?")
            .bind(v as i64)
            .bind(id)
            .execute(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    Ok(StatusCode::NO_CONTENT)
}

pub async fn delete_university(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    sqlx::query("DELETE FROM universities WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}
