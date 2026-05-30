use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::score::Score;
use crate::state::AppState;

type ApiError = (StatusCode, String);

#[derive(Serialize, FromRow)]
pub struct AreaRow {
    pub id: i64,
    pub name: String,
    pub max_score: Score,
    pub calc_type: String,
    pub teacher_editable: bool,
    pub lookup_scope: String,
    pub match_mode: Option<String>,
    pub category_agg: Option<String>,
    pub multi_value: bool,
}

#[derive(Deserialize)]
pub struct CreateAreaBody {
    pub name: String,
    pub max_score: Score,
    pub calc_type: String,
    pub teacher_editable: bool,
    pub lookup_scope: String,
    pub match_mode: Option<String>,
    pub category_agg: Option<String>,
    #[serde(default)]
    pub multi_value: bool,
}

#[derive(Deserialize)]
pub struct UpdateAreaBody {
    pub name: Option<String>,
    pub teacher_editable: Option<bool>,
}

pub async fn list_areas(State(state): State<AppState>) -> Result<Json<Vec<AreaRow>>, ApiError> {
    let rows = sqlx::query_as::<_, AreaRow>(
        "SELECT id, name, max_score, calc_type, teacher_editable, lookup_scope,
                match_mode, category_agg, multi_value
         FROM areas ORDER BY id",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(rows))
}

pub async fn create_area(
    State(state): State<AppState>,
    Json(body): Json<CreateAreaBody>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    if body.max_score.raw() < 0 {
        return Err((StatusCode::BAD_REQUEST, "만점은 0 이상이어야 합니다".into()));
    }
    if body.calc_type == "NUMERIC" && body.match_mode.is_none() {
        return Err((StatusCode::BAD_REQUEST, "NUMERIC 전형요소는 match_mode(UPPER/LOWER/EXACT)가 필수입니다".into()));
    }
    if body.calc_type == "CATEGORY" && body.category_agg.is_none() {
        return Err((StatusCode::BAD_REQUEST, "CATEGORY 전형요소는 category_agg(SUM/MAX)가 필수입니다".into()));
    }
    if body.calc_type != "CATEGORY" && body.multi_value {
        return Err((StatusCode::BAD_REQUEST, "multi_value=1은 CATEGORY 전형요소에만 허용됩니다".into()));
    }

    let id: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, teacher_editable, lookup_scope,
                            match_mode, category_agg, multi_value)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)
         RETURNING id",
    )
    .bind(&body.name)
    .bind(body.max_score.raw())
    .bind(&body.calc_type)
    .bind(body.teacher_editable)
    .bind(&body.lookup_scope)
    .bind(&body.match_mode)
    .bind(&body.category_agg)
    .bind(body.multi_value)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

pub async fn update_area(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateAreaBody>,
) -> Result<StatusCode, ApiError> {
    let mut tx = state.db.begin().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if let Some(v) = body.name {
        sqlx::query("UPDATE areas SET name = ? WHERE id = ?")
            .bind(v).bind(id)
            .execute(&mut *tx).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    if let Some(v) = body.teacher_editable {
        sqlx::query("UPDATE areas SET teacher_editable = ? WHERE id = ?")
            .bind(v).bind(id)
            .execute(&mut *tx).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    tx.commit().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn delete_area(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    sqlx::query("DELETE FROM areas WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

