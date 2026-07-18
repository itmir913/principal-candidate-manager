use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use serde::Serialize;

use crate::{
    audit::{Actor, AuditEntry},
    auth::TeacherClaims,
    enums::AuditAction,
    state::AppState,
};

type ApiError = (StatusCode, String);

// ── Response types ─────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ConfirmationResponse {
    pub confirmed: bool,
    pub confirmed_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ConfirmationStatusResponse {
    pub classes: Vec<ClassConfirmation>,
}

#[derive(Debug, Serialize)]
pub struct ClassConfirmation {
    pub grade: i64,
    pub class_no: i64,
    pub teacher_name: Option<String>,
    pub confirmed: bool,
    pub confirmed_at: Option<String>,
}

// ── DB row types ───────────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct ClassConfirmRow {
    grade: i64,
    class_no: i64,
    teacher_name: Option<String>,
    confirmed_at: Option<String>,
}

// ── Teacher: 확정 조회 ──────────────────────────────────────────

pub async fn teacher_get_confirmation(
    State(state): State<AppState>,
    Extension(claims): Extension<TeacherClaims>,
    Path(round_id): Path<i64>,
) -> Result<Json<ConfirmationResponse>, ApiError> {
    let db_err = |e: sqlx::Error| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string());

    let round_exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM rounds WHERE id = ?)")
        .bind(round_id)
        .fetch_one(&state.db)
        .await
        .map_err(db_err)?;

    if !round_exists {
        return Err((StatusCode::NOT_FOUND, "라운드를 찾을 수 없습니다".into()));
    }

    let confirmed_at: Option<String> = sqlx::query_scalar(
        "SELECT confirmed_at FROM round_confirmations \
         WHERE round_id = ? AND grade = ? AND class_no = ?",
    )
    .bind(round_id)
    .bind(claims.grade)
    .bind(claims.class_no)
    .fetch_optional(&state.db)
    .await
    .map_err(db_err)?;

    Ok(Json(ConfirmationResponse {
        confirmed: confirmed_at.is_some(),
        confirmed_at,
    }))
}

// ── Teacher: 확정 ──────────────────────────────────────────────

pub async fn teacher_confirm_round(
    State(state): State<AppState>,
    Extension(claims): Extension<TeacherClaims>,
    Path(round_id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    // OPEN 라운드만 확정 가능
    let status: Option<String> = sqlx::query_scalar("SELECT status FROM rounds WHERE id = ?")
        .bind(round_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match status.as_deref() {
        Some("OPEN") => {}
        Some(_) => return Err((StatusCode::BAD_REQUEST, "OPEN 라운드에서만 확정할 수 있습니다".into())),
        None => return Err((StatusCode::NOT_FOUND, "라운드를 찾을 수 없습니다".into())),
    }

    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let now = chrono::Utc::now().to_rfc3339();

    // 이미 확정된 경우 409
    let already: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM round_confirmations \
         WHERE round_id = ? AND grade = ? AND class_no = ?)",
    )
    .bind(round_id)
    .bind(claims.grade)
    .bind(claims.class_no)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if already {
        return Err((StatusCode::CONFLICT, "이미 확정되었습니다".into()));
    }

    sqlx::query(
        "INSERT INTO round_confirmations (round_id, grade, class_no, confirmed_at) \
         VALUES (?, ?, ?, ?)",
    )
    .bind(round_id)
    .bind(claims.grade)
    .bind(claims.class_no)
    .bind(&now)
    .execute(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    crate::audit::log(
        &mut *tx,
        AuditEntry {
            actor: Actor::Teacher { grade: claims.grade, class_no: claims.class_no },
            action: AuditAction::RoundConfirmed,
            round_id: Some(round_id),
            student_id: None,
            detail: serde_json::json!({}),
        },
    )
    .await?;

    tx.commit()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

// ── Teacher: 확정 취소 ─────────────────────────────────────────

pub async fn teacher_revoke_confirmation(
    State(state): State<AppState>,
    Extension(claims): Extension<TeacherClaims>,
    Path(round_id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    let round_exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM rounds WHERE id = ?)")
        .bind(round_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .unwrap_or(false);

    if !round_exists {
        return Err((StatusCode::NOT_FOUND, "라운드를 찾을 수 없습니다".into()));
    }

    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let affected = sqlx::query(
        "DELETE FROM round_confirmations \
         WHERE round_id = ? AND grade = ? AND class_no = ?",
    )
    .bind(round_id)
    .bind(claims.grade)
    .bind(claims.class_no)
    .execute(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .rows_affected();

    if affected == 0 {
        return Err((StatusCode::NOT_FOUND, "확정 내역이 없습니다".into()));
    }

    crate::audit::log(
        &mut *tx,
        AuditEntry {
            actor: Actor::Teacher { grade: claims.grade, class_no: claims.class_no },
            action: AuditAction::RoundConfirmationRevoked,
            round_id: Some(round_id),
            student_id: None,
            detail: serde_json::json!({ "auto": false }),
        },
    )
    .await?;

    tx.commit()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

// ── Admin: 전 학급 확정 현황 ────────────────────────────────────

pub async fn admin_get_confirmation_status(
    State(state): State<AppState>,
    Path(round_id): Path<i64>,
) -> Result<Json<ConfirmationStatusResponse>, ApiError> {
    let db_err = |e: sqlx::Error| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string());

    let round_exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM rounds WHERE id = ?)")
        .bind(round_id)
        .fetch_one(&state.db)
        .await
        .map_err(db_err)?;

    if !round_exists {
        return Err((StatusCode::NOT_FOUND, "라운드를 찾을 수 없습니다".into()));
    }

    let rows = sqlx::query_as::<_, ClassConfirmRow>(
        "SELECT c.grade, c.class_no, c.teacher_name, rc.confirmed_at
         FROM classes c
         LEFT JOIN round_confirmations rc
               ON rc.round_id = ? AND rc.grade = c.grade AND rc.class_no = c.class_no
         ORDER BY c.grade, c.class_no",
    )
    .bind(round_id)
    .fetch_all(&state.db)
    .await
    .map_err(db_err)?;

    let classes = rows
        .into_iter()
        .map(|r| ClassConfirmation {
            grade: r.grade,
            class_no: r.class_no,
            teacher_name: r.teacher_name,
            confirmed: r.confirmed_at.is_some(),
            confirmed_at: r.confirmed_at,
        })
        .collect();

    Ok(Json(ConfirmationStatusResponse { classes }))
}
