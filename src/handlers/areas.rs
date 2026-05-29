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
    pub max_score: Option<i64>,
    pub calc_type: Option<String>,
    pub teacher_editable: Option<i64>,
    pub lookup_scope: Option<String>,
    pub match_mode: Option<String>,
    pub category_agg: Option<String>,
    pub multi_value: Option<i64>,
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
    if body.max_score <= 0 {
        return Err((StatusCode::BAD_REQUEST, "만점은 0보다 커야 합니다".into()));
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
    if let Some(ms) = body.max_score {
        if ms <= 0 {
            return Err((StatusCode::BAD_REQUEST, "만점은 0보다 커야 합니다".into()));
        }
    }

    let mut tx = state.db.begin().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    macro_rules! update_field {
        ($field:expr, $col:literal) => {
            if let Some(v) = $field {
                sqlx::query(concat!("UPDATE areas SET ", $col, " = ? WHERE id = ?"))
                    .bind(v)
                    .bind(id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            }
        };
    }

    update_field!(body.name, "name");
    update_field!(body.max_score, "max_score");
    update_field!(body.calc_type, "calc_type");
    update_field!(body.teacher_editable, "teacher_editable");
    update_field!(body.lookup_scope, "lookup_scope");
    update_field!(body.match_mode, "match_mode");
    update_field!(body.category_agg, "category_agg");
    update_field!(body.multi_value, "multi_value");

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
