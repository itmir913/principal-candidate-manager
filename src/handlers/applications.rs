use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::{auth::TeacherClaims, state::AppState};

// ── 비밀번호 변경 ──────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ChangePasswordBody {
    pub new_password: String,
}

pub async fn teacher_change_password(
    State(state): State<AppState>,
    Extension(claims): Extension<TeacherClaims>,
    Json(body): Json<ChangePasswordBody>,
) -> Result<StatusCode, ApiError> {
    if body.new_password.len() < 4 {
        return Err((StatusCode::BAD_REQUEST, "비밀번호는 4자 이상이어야 합니다".into()));
    }
    let hash = bcrypt::hash(&body.new_password, bcrypt::DEFAULT_COST)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    sqlx::query("UPDATE classes SET password_hash = ? WHERE grade = ? AND class_no = ?")
        .bind(&hash)
        .bind(claims.grade)
        .bind(claims.class_no)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

type ApiError = (StatusCode, String);

#[derive(Serialize, FromRow)]
pub struct ApplicationRow {
    pub student_id: i64,
    pub track_id: i64,
    pub round_id: i64,
    pub confirmed: bool,
    pub abandoned: bool,
    pub student_code: String,
    pub name: String,
    pub grade: Option<i64>,
    pub class_no: Option<i64>,
    pub seq_no: Option<i64>,
    pub is_enrolled: bool,
    pub univ_name: String,
    pub track_name: String,
}

#[derive(Serialize, FromRow)]
pub struct StudentRow {
    pub id: i64,
    pub student_code: String,
    pub name: String,
    pub grade: Option<i64>,
    pub class_no: Option<i64>,
    pub seq_no: Option<i64>,
    pub is_enrolled: bool,
}

#[derive(Deserialize)]
pub struct ApplicationListQuery {
    pub round_id: Option<i64>,
    pub track_id: Option<i64>,
}

#[derive(Deserialize)]
pub struct TeacherAppListQuery {
    pub round_id: Option<i64>,
}

#[derive(Deserialize)]
pub struct CreateApplicationBody {
    pub student_id: i64,
    pub track_id: i64,
    pub round_id: i64,
}

// ── Admin ──────────────────────────────────────────────────────────

pub async fn admin_list_applications(
    State(state): State<AppState>,
    Query(q): Query<ApplicationListQuery>,
) -> Result<Json<Vec<ApplicationRow>>, ApiError> {
    let rows = sqlx::query_as::<_, ApplicationRow>(
        "SELECT a.student_id, a.track_id, a.round_id, a.confirmed, a.abandoned,
                s.student_code, s.name, s.grade, s.class_no, s.seq_no, s.is_enrolled,
                u.univ_name, ut.track_name
         FROM applications a
         JOIN students s ON a.student_id = s.id
         JOIN univ_tracks ut ON a.track_id = ut.id
         JOIN universities u ON ut.univ_id = u.id
         WHERE (? IS NULL OR a.round_id = ?)
           AND (? IS NULL OR a.track_id = ?)
         ORDER BY u.univ_name, ut.track_name, s.grade, s.class_no, s.seq_no",
    )
    .bind(q.round_id).bind(q.round_id)
    .bind(q.track_id).bind(q.track_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(rows))
}

// URL: /applications/:sid/:tid/:rid/abandon  (sid=student_id, tid=track_id, rid=round_id)
pub async fn abandon_application(
    State(state): State<AppState>,
    Path((sid, tid, rid)): Path<(i64, i64, i64)>,
) -> Result<StatusCode, ApiError> {
    sqlx::query(
        "UPDATE applications SET abandoned = 1
         WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(sid).bind(tid).bind(rid)
    .execute(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

// ── Teacher ────────────────────────────────────────────────────────

pub async fn teacher_list_students(
    State(state): State<AppState>,
    Extension(claims): Extension<TeacherClaims>,
) -> Result<Json<Vec<StudentRow>>, ApiError> {
    let rows = sqlx::query_as::<_, StudentRow>(
        "SELECT id, student_code, name, grade, class_no, seq_no, is_enrolled
         FROM students WHERE grade = ? AND class_no = ?
         ORDER BY seq_no",
    )
    .bind(claims.grade)
    .bind(claims.class_no)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(rows))
}

pub async fn teacher_list_applications(
    State(state): State<AppState>,
    Extension(claims): Extension<TeacherClaims>,
    Query(q): Query<TeacherAppListQuery>,
) -> Result<Json<Vec<ApplicationRow>>, ApiError> {
    let rows = sqlx::query_as::<_, ApplicationRow>(
        "SELECT a.student_id, a.track_id, a.round_id, a.confirmed, a.abandoned,
                s.student_code, s.name, s.grade, s.class_no, s.seq_no, s.is_enrolled,
                u.univ_name, ut.track_name
         FROM applications a
         JOIN students s ON a.student_id = s.id
         JOIN univ_tracks ut ON a.track_id = ut.id
         JOIN universities u ON ut.univ_id = u.id
         WHERE s.grade = ? AND s.class_no = ?
           AND (? IS NULL OR a.round_id = ?)
         ORDER BY s.seq_no, u.univ_name",
    )
    .bind(claims.grade).bind(claims.class_no)
    .bind(q.round_id).bind(q.round_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(rows))
}

pub async fn teacher_create_application(
    State(state): State<AppState>,
    Extension(claims): Extension<TeacherClaims>,
    Json(body): Json<CreateApplicationBody>,
) -> Result<StatusCode, ApiError> {
    let round_status: Option<String> = sqlx::query_scalar(
        "SELECT status FROM rounds WHERE id = ?",
    )
    .bind(body.round_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match round_status.as_deref() {
        Some("OPEN") => {}
        Some(_) => return Err((StatusCode::BAD_REQUEST, "라운드가 OPEN 상태가 아닙니다".into())),
        None => return Err((StatusCode::NOT_FOUND, "라운드를 찾을 수 없습니다".into())),
    }

    let ok: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM students WHERE id = ? AND grade = ? AND class_no = ?)",
    )
    .bind(body.student_id)
    .bind(claims.grade)
    .bind(claims.class_no)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if !ok {
        return Err((StatusCode::FORBIDDEN, "해당 학생은 담당 학급이 아닙니다".into()));
    }

    sqlx::query(
        "INSERT OR IGNORE INTO applications (student_id, track_id, round_id, confirmed, abandoned)
         VALUES (?, ?, ?, 1, 0)",
    )
    .bind(body.student_id)
    .bind(body.track_id)
    .bind(body.round_id)
    .execute(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::CREATED)
}

// URL: /applications/:sid/:tid/:rid  (sid=student_id, tid=track_id, rid=round_id)
pub async fn teacher_delete_application(
    State(state): State<AppState>,
    Extension(claims): Extension<TeacherClaims>,
    Path((sid, tid, rid)): Path<(i64, i64, i64)>,
) -> Result<StatusCode, ApiError> {
    let round_status: Option<String> = sqlx::query_scalar(
        "SELECT status FROM rounds WHERE id = ?",
    )
    .bind(rid)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if round_status.as_deref() != Some("OPEN") {
        return Err((StatusCode::BAD_REQUEST, "OPEN 라운드의 지원만 취소할 수 있습니다".into()));
    }

    let ok: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM students WHERE id = ? AND grade = ? AND class_no = ?)",
    )
    .bind(sid)
    .bind(claims.grade)
    .bind(claims.class_no)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if !ok {
        return Err((StatusCode::FORBIDDEN, "해당 학생은 담당 학급이 아닙니다".into()));
    }

    sqlx::query(
        "DELETE FROM applications WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(sid).bind(tid).bind(rid)
    .execute(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}
