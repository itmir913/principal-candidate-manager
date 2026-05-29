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
pub struct AreaRow {
    pub id: i64,
    pub name: String,
    pub max_score: i64,
    pub calc_type: String,
    pub teacher_editable: i64,
    pub lookup_scope: String,
    pub match_mode: Option<String>,
    pub category_agg: Option<String>,
    pub multi_value: i64,
}

#[derive(Deserialize)]
pub struct CreateAreaBody {
    pub name: String,
    pub max_score: i64,
    pub calc_type: String,
    pub teacher_editable: i64,
    pub lookup_scope: String,
    pub match_mode: Option<String>,
    pub category_agg: Option<String>,
    #[serde(default)]
    pub multi_value: i64,
}

#[derive(Deserialize)]
pub struct UpdateAreaBody {
    pub name: Option<String>,
    pub teacher_editable: Option<i64>,
}

#[derive(Serialize, Deserialize, FromRow)]
pub struct RangeRow {
    pub threshold: i64,
    pub score: i64,
}

#[derive(Serialize, Deserialize, FromRow)]
pub struct CategoryRow {
    pub category: String,
    pub score: i64,
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
    if body.max_score < 0 {
        return Err((StatusCode::BAD_REQUEST, "만점은 0 이상이어야 합니다".into()));
    }
    if body.calc_type == "NUMERIC" && body.match_mode.is_none() {
        return Err((StatusCode::BAD_REQUEST, "NUMERIC 전형요소는 match_mode(UPPER/LOWER/EXACT)가 필수입니다".into()));
    }
    if body.calc_type == "CATEGORY" && body.category_agg.is_none() {
        return Err((StatusCode::BAD_REQUEST, "CATEGORY 전형요소는 category_agg(SUM/MAX)가 필수입니다".into()));
    }
    if body.calc_type != "CATEGORY" && body.multi_value != 0 {
        return Err((StatusCode::BAD_REQUEST, "multi_value=1은 CATEGORY 전형요소에만 허용됩니다".into()));
    }

    let id: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, teacher_editable, lookup_scope,
                            match_mode, category_agg, multi_value)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)
         RETURNING id",
    )
    .bind(&body.name)
    .bind(body.max_score)
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

pub async fn get_numeric_table(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Vec<RangeRow>>, ApiError> {
    let rows = sqlx::query_as::<_, RangeRow>(
        "SELECT threshold, score FROM numeric_table WHERE area_id = ? ORDER BY threshold",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(rows))
}

pub async fn put_numeric_table(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(rows): Json<Vec<RangeRow>>,
) -> Result<StatusCode, ApiError> {
    let max_score: i64 = sqlx::query_scalar("SELECT max_score FROM areas WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("전형요소 id={} 없음", id)))?;

    if let Some(row) = rows.iter().find(|r| r.score > max_score) {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("점수({})가 전형요소 만점({})을 초과합니다",
                row.score as f64 / 100_000.0, max_score as f64 / 100_000.0),
        ));
    }

    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    sqlx::query("DELETE FROM numeric_table WHERE area_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    for row in rows {
        sqlx::query("INSERT INTO numeric_table (area_id, threshold, score) VALUES (?, ?, ?)")
            .bind(id)
            .bind(row.threshold)
            .bind(row.score)
            .execute(&mut *tx)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    tx.commit()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_category_map(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Vec<CategoryRow>>, ApiError> {
    let rows = sqlx::query_as::<_, CategoryRow>(
        "SELECT category, score FROM category_map WHERE area_id = ? ORDER BY category",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(rows))
}

pub async fn put_category_map(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(rows): Json<Vec<CategoryRow>>,
) -> Result<StatusCode, ApiError> {
    let max_score: i64 = sqlx::query_scalar("SELECT max_score FROM areas WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("전형요소 id={} 없음", id)))?;

    if let Some(row) = rows.iter().find(|r| r.score > max_score) {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("점수({})가 전형요소 만점({})을 초과합니다",
                row.score as f64 / 100_000.0, max_score as f64 / 100_000.0),
        ));
    }

    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    sqlx::query("DELETE FROM category_map WHERE area_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    for row in rows {
        sqlx::query("INSERT INTO category_map (area_id, category, score) VALUES (?, ?, ?)")
            .bind(id)
            .bind(row.category)
            .bind(row.score)
            .execute(&mut *tx)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    tx.commit()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}
