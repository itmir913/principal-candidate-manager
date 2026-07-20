use axum::{
    extract::{ConnectInfo, State},
    http::StatusCode,
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use crate::{
    audit::{self, Actor, AuditEntry},
    auth,
    enums::AuditAction,
    state::AppState,
};

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
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub token: String,
}

#[derive(Debug, Serialize)]
pub struct TeacherTokenResponse {
    pub token: String,
    pub grade: i64,
    pub class_no: i64,
    pub teacher_name: Option<String>,
}

type ApiError = (StatusCode, String);

#[derive(Serialize)]
pub struct AdminStatusResponse {
    pub initialized: bool,
}

pub async fn admin_status(
    State(state): State<AppState>,
) -> Result<Json<AdminStatusResponse>, ApiError> {
    let hash: Option<String> = sqlx::query_scalar(
        "SELECT value FROM app_configs WHERE key = 'admin_password_hash'",
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let initialized = hash.map(|h| !h.is_empty()).unwrap_or(false);
    Ok(Json(AdminStatusResponse { initialized }))
}

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

    // 행이 없거나 빈 문자열이면 미초기화 상태 → 최초 로그인으로 처리
    let hash = hash.unwrap_or_default();

    if hash.is_empty() {
        let new_hash = bcrypt::hash(&body.password, bcrypt::DEFAULT_COST)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        sqlx::query(
            "INSERT INTO app_configs (key, value) VALUES ('admin_password_hash', ?) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
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
) -> Result<Json<TeacherTokenResponse>, ApiError> {
    // 졸업생 로그인: grade=0, class_no=0 → 관리자 비밀번호로 인증
    if body.grade == 0 && body.class_no == 0 {
        let hash: Option<String> = sqlx::query_scalar(
            "SELECT value FROM app_configs WHERE key = 'admin_password_hash'",
        )
        .fetch_optional(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let hash = hash.unwrap_or_default();
        if hash.is_empty() {
            return Err((StatusCode::UNAUTHORIZED, "관리자 비밀번호가 설정되지 않았습니다".into()));
        }

        let ok = bcrypt::verify(&body.password, &hash)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        if !ok {
            return Err((StatusCode::UNAUTHORIZED, "비밀번호가 틀렸습니다".into()));
        }

        let token = auth::encode_teacher_token(0, 0, &state.jwt_secret)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        return Ok(Json(TeacherTokenResponse {
            token,
            grade: 0,
            class_no: 0,
            teacher_name: Some("졸업생".into()),
        }));
    }

    let row: Option<(String, Option<String>)> = sqlx::query_as(
        "SELECT COALESCE(password_hash, ''), teacher_name FROM classes WHERE grade = ? AND class_no = ?",
    )
    .bind(body.grade)
    .bind(body.class_no)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let (hash, teacher_name) = row.ok_or((StatusCode::NOT_FOUND, "해당 반이 없습니다".into()))?;

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
    Ok(Json(TeacherTokenResponse { token, grade: body.grade, class_no: body.class_no, teacher_name }))
}

pub async fn change_admin_password(
    State(state): State<AppState>,
    Extension(_claims): Extension<auth::AdminClaims>,
    ConnectInfo(client): ConnectInfo<SocketAddr>,
    Json(body): Json<ChangePasswordBody>,
) -> Result<StatusCode, ApiError> {
    let current_hash: String = sqlx::query_scalar(
        "SELECT value FROM app_configs WHERE key = 'admin_password_hash'",
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "설정값 없음".into()))?;

    let ok = bcrypt::verify(&body.current_password, &current_hash)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !ok {
        return Err((StatusCode::BAD_REQUEST, "현재 비밀번호가 틀렸습니다".into()));
    }

    if body.new_password.len() < 8 {
        return Err((StatusCode::BAD_REQUEST, "새 비밀번호는 8자 이상이어야 합니다".into()));
    }

    // bcrypt는 CPU 집약 — DB 접근 전 미리 계산
    let new_hash = bcrypt::hash(&body.new_password, bcrypt::DEFAULT_COST)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // UPDATE와 audit log를 같은 트랜잭션으로 묶어 원자성 확보 (2차 감사 소유자 라운드 #6)
    let mut tx = state.db.begin().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    sqlx::query("UPDATE app_configs SET value = ? WHERE key = 'admin_password_hash'")
        .bind(&new_hash)
        .execute(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    audit::log_with_ip(
        &mut tx,
        AuditEntry {
            actor: Actor::Admin,
            action: AuditAction::AdminPasswordChanged,
            round_id: None,
            student_id: None,
            detail: serde_json::json!({}),
        },
        Some(client.ip().to_string()),
    ).await?;
    tx.commit().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}
