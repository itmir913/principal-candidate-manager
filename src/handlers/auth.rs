use axum::{extract::State, http::StatusCode, Extension, Json};
use serde::{Deserialize, Serialize};

use crate::{auth, state::AppState};

#[derive(Deserialize)]
pub struct AdminLoginBody {
    pub password: String,
}

#[derive(Deserialize)]
pub struct TeacherLoginBody {
    pub grade: i64,
    pub class_no: i64,
    pub password: String,
}

#[derive(Deserialize)]
pub struct ChangePasswordBody {
    pub new_password: String,
}

#[derive(Serialize)]
pub struct TokenResponse {
    pub token: String,
}

type ApiError = (StatusCode, String);

pub async fn admin_login(
    State(state): State<AppState>,
    Json(body): Json<AdminLoginBody>,
) -> Result<Json<TokenResponse>, ApiError> {
    let hash: Option<String> = sqlx::query_scalar(
        "SELECT value FROM app_configs WHERE key = 'admin_password_hash'",
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let hash = hash.ok_or((StatusCode::INTERNAL_SERVER_ERROR, "설정값 없음".into()))?;

    if hash.is_empty() {
        let new_hash = bcrypt::hash(&body.password, bcrypt::DEFAULT_COST)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        sqlx::query("UPDATE app_configs SET value = ? WHERE key = 'admin_password_hash'")
            .bind(&new_hash)
            .execute(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    } else {
        let ok = bcrypt::verify(&body.password, &hash)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        if !ok {
            return Err((StatusCode::UNAUTHORIZED, "비밀번호가 틀렸습니다".into()));
        }
    }

    let token = auth::encode_admin_token(&state.jwt_secret)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(TokenResponse { token }))
}

pub async fn teacher_login(
    State(state): State<AppState>,
    Json(body): Json<TeacherLoginBody>,
) -> Result<Json<TokenResponse>, ApiError> {
    let hash: Option<String> = sqlx::query_scalar(
        "SELECT COALESCE(password_hash, '') FROM classes WHERE grade = ? AND class_no = ?",
    )
    .bind(body.grade)
    .bind(body.class_no)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let hash = hash.ok_or((StatusCode::NOT_FOUND, "해당 반이 없습니다".into()))?;

    if hash.is_empty() {
        return Err((StatusCode::UNAUTHORIZED, "비밀번호가 설정되지 않았습니다".into()));
    }

    let ok = bcrypt::verify(&body.password, &hash)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !ok {
        return Err((StatusCode::UNAUTHORIZED, "비밀번호가 틀렸습니다".into()));
    }

    let token = auth::encode_teacher_token(body.grade, body.class_no, &state.jwt_secret)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(TokenResponse { token }))
}

pub async fn change_admin_password(
    State(state): State<AppState>,
    Extension(_claims): Extension<auth::AdminClaims>,
    Json(body): Json<ChangePasswordBody>,
) -> Result<StatusCode, ApiError> {
    let new_hash = bcrypt::hash(&body.new_password, bcrypt::DEFAULT_COST)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    sqlx::query("UPDATE app_configs SET value = ? WHERE key = 'admin_password_hash'")
        .bind(&new_hash)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}
