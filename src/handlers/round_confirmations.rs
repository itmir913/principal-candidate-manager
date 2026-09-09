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
    // BEGIN IMMEDIATE: 시작 시점에 쓰기 잠금 획득 — 상태 확인 후 close_round가
    // 끼어들어 CLOSED 라운드에 확정이 삽입되는 TOCTOU 방지
    let mut tx = state
        .db
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // OPEN 라운드만 확정 가능
    let status: Option<String> = sqlx::query_scalar("SELECT status FROM rounds WHERE id = ?")
        .bind(round_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match status.as_deref() {
        Some("OPEN") => {}
        Some(_) => return Err((StatusCode::BAD_REQUEST, "OPEN 라운드에서만 확정할 수 있습니다".into())),
        None => return Err((StatusCode::NOT_FOUND, "라운드를 찾을 수 없습니다".into())),
    }

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

    // 졸업생 담당(0/0)은 classes에 실재하지 않는 논리적 학급이라 그대로 INSERT하면
    // (grade, class_no) → classes FK가 787로 터진다 (이슈 #28). 같은 tx 안에서 sentinel 행을
    // 보장한 뒤 삽입한다 — 확정이 롤백되면 sentinel 생성도 함께 되돌아간다.
    if claims.grade == crate::handlers::classes::GRADUATE_GRADE
        && claims.class_no == crate::handlers::classes::GRADUATE_CLASS_NO
    {
        crate::handlers::classes::ensure_graduate_class(&mut tx).await?;
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
    // BEGIN IMMEDIATE: confirm과 동일 — 상태 확인·삭제가 close_round와 원자적으로 배타
    let mut tx = state
        .db
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // OPEN 라운드만 취소 가능 — 종료된 라운드의 확정 기록은 담임이 사후 변경할 수 없다
    let status: Option<String> = sqlx::query_scalar("SELECT status FROM rounds WHERE id = ?")
        .bind(round_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match status.as_deref() {
        Some("OPEN") => {}
        Some(_) => return Err((StatusCode::BAD_REQUEST, "OPEN 라운드에서만 확정을 취소할 수 있습니다".into())),
        None => return Err((StatusCode::NOT_FOUND, "라운드를 찾을 수 없습니다".into())),
    }

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

    // 0/0 sentinel 행은 진짜 학급이 아니므로 목록에서 빼고, 졸업생 담당은 아래에서 따로 붙인다.
    let rows = sqlx::query_as::<_, ClassConfirmRow>(
        "SELECT c.grade, c.class_no, c.teacher_name, rc.confirmed_at
         FROM classes c
         LEFT JOIN round_confirmations rc
               ON rc.round_id = ? AND rc.grade = c.grade AND rc.class_no = c.class_no
         WHERE NOT (c.grade = 0 AND c.class_no = 0)
         ORDER BY c.grade, c.class_no",
    )
    .bind(round_id)
    .fetch_all(&state.db)
    .await
    .map_err(db_err)?;

    let mut classes: Vec<ClassConfirmation> = rows
        .into_iter()
        .map(|r| ClassConfirmation {
            grade: r.grade,
            class_no: r.class_no,
            teacher_name: r.teacher_name,
            confirmed: r.confirmed_at.is_some(),
            confirmed_at: r.confirmed_at,
        })
        .collect();

    // 졸업생 담당(0/0) — classes 행의 존재 여부와 무관하게 따로 붙인다.
    // 표시 조건은 "졸업생이 있거나(list_classes·overview와 같은 기준) 이미 확정 기록이 있을 때".
    // 확정 기록이 있으면 무조건 보여준다 — 기록이 남았는데 화면에서 사라지는 쪽이 더 나쁘다.
    // 이게 없으면 졸업생 담당이 입력 확정을 해도 관리자 화면에는 아무 흔적이 남지 않는다 (이슈 #28).
    let grad_confirmed_at: Option<String> = sqlx::query_scalar(
        "SELECT confirmed_at FROM round_confirmations          WHERE round_id = ? AND grade = ? AND class_no = ?",
    )
    .bind(round_id)
    .bind(crate::handlers::classes::GRADUATE_GRADE)
    .bind(crate::handlers::classes::GRADUATE_CLASS_NO)
    .fetch_optional(&state.db)
    .await
    .map_err(db_err)?;

    let has_graduates: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM students WHERE is_enrolled = 0)",
    )
    .fetch_one(&state.db)
    .await
    .map_err(db_err)?;

    if has_graduates || grad_confirmed_at.is_some() {
        classes.push(ClassConfirmation {
            grade: crate::handlers::classes::GRADUATE_GRADE,
            class_no: crate::handlers::classes::GRADUATE_CLASS_NO,
            teacher_name: Some("졸업생".into()),
            confirmed: grad_confirmed_at.is_some(),
            confirmed_at: grad_confirmed_at,
        });
    }

    Ok(Json(ConfirmationStatusResponse { classes }))
}
