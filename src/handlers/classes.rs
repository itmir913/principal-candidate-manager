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
pub struct ClassRow {
    pub grade: i64,
    pub class_no: i64,
    pub teacher_name: Option<String>,
}

#[derive(Deserialize)]
pub struct UpsertClassBody {
    pub teacher_name: Option<String>,
    pub password: Option<String>,
}

pub async fn list_classes(
    State(state): State<AppState>,
) -> Result<Json<Vec<ClassRow>>, ApiError> {
    let rows = sqlx::query_as::<_, ClassRow>(
        "SELECT grade, class_no, teacher_name FROM classes ORDER BY grade, class_no",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(rows))
}

pub async fn upsert_class(
    State(state): State<AppState>,
    Path((grade, class_no)): Path<(i64, i64)>,
    Json(body): Json<UpsertClassBody>,
) -> Result<StatusCode, ApiError> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM classes WHERE grade = ? AND class_no = ?",
    )
    .bind(grade)
    .bind(class_no)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if count == 0 {
        let password_hash = if let Some(ref pw) = body.password {
            Some(
                bcrypt::hash(pw, bcrypt::DEFAULT_COST)
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?,
            )
        } else {
            None
        };
        sqlx::query(
            "INSERT INTO classes (grade, class_no, teacher_name, password_hash) VALUES (?, ?, ?, ?)",
        )
        .bind(grade)
        .bind(class_no)
        .bind(body.teacher_name)
        .bind(password_hash)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    } else {
        if let Some(ref name) = body.teacher_name {
            sqlx::query("UPDATE classes SET teacher_name = ? WHERE grade = ? AND class_no = ?")
                .bind(name)
                .bind(grade)
                .bind(class_no)
                .execute(&state.db)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        }
        if let Some(ref pw) = body.password {
            let hash = bcrypt::hash(pw, bcrypt::DEFAULT_COST)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            sqlx::query("UPDATE classes SET password_hash = ? WHERE grade = ? AND class_no = ?")
                .bind(hash)
                .bind(grade)
                .bind(class_no)
                .execute(&state.db)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        }
    }

    Ok(StatusCode::NO_CONTENT)
}
