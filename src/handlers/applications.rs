use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::{
    audit::{Actor, AuditEntry},
    auth::TeacherClaims,
    enums::{AuditAction, CalcType, CategoryAgg, LookupScope, RoundStatus},
    handlers::area_data::parse_display_value,
    handlers::scoring::{calc_area_score, AreaRow, StudentTrackCtx},
    state::AppState,
};

// ── 비밀번호 변경 ──────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ChangePasswordBody {
    pub current_password: String,
    pub new_password: String,
}

pub async fn teacher_change_password(
    State(state): State<AppState>,
    Extension(claims): Extension<TeacherClaims>,
    Json(body): Json<ChangePasswordBody>,
) -> Result<StatusCode, ApiError> {
    if is_grad_teacher(&claims) {
        return Err((StatusCode::FORBIDDEN, "졸업생 담당은 비밀번호 변경을 지원하지 않습니다".into()));
    }

    if body.new_password.len() < 4 {
        return Err((StatusCode::BAD_REQUEST, "비밀번호는 4자 이상이어야 합니다".into()));
    }
    let current_hash: String = sqlx::query_scalar(
        "SELECT COALESCE(password_hash, '') FROM classes WHERE grade = ? AND class_no = ?",
    )
    .bind(claims.grade)
    .bind(claims.class_no)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let ok = bcrypt::verify(&body.current_password, &current_hash)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !ok {
        return Err((StatusCode::BAD_REQUEST, "현재 비밀번호가 틀렸습니다".into()));
    }

    // bcrypt는 CPU 집약 — DB 접근 전 미리 계산
    let new_hash = bcrypt::hash(&body.new_password, bcrypt::DEFAULT_COST)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    sqlx::query("UPDATE classes SET password_hash = ? WHERE grade = ? AND class_no = ?")
        .bind(&new_hash)
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
    pub abandoned: bool,
    pub excluded: bool,
    pub excluded_reason: Option<String>,
    pub department_name: String,
    pub student_code: String,
    pub name: String,
    pub grade: Option<i64>,
    pub class_no: Option<i64>,
    pub seq_no: Option<i64>,
    pub is_enrolled: bool,
    pub univ_id: i64,
    pub univ_name: String,
    pub track_name: String,
    pub recommended: Option<bool>,
    pub round_status: String,
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

/// 기초데이터 한 항목. values는 표시용 문자열 목록.
/// NUMERIC/MANUAL: 소수 문자열 1개, CATEGORY: 범주 문자열(단일 또는 복수).
#[derive(Deserialize, Default, Clone)]
pub struct BaseDataEntry {
    pub area_id: i64,
    pub values: Vec<String>,
}

#[derive(Deserialize, Default)]
pub struct CreateApplicationBody {
    pub student_id: i64,
    pub track_id: i64,
    pub round_id: i64,
    #[serde(default)]
    pub department_name: String,
    #[serde(default)]
    pub base_data_entries: Vec<BaseDataEntry>,
    /// 수정 모드에서 기존 지원의 track_id. Some(p) && p != track_id면 모집단위 변경.
    #[serde(default)]
    pub prev_track_id: Option<i64>,
}

// ── Admin ──────────────────────────────────────────────────────────

pub async fn admin_list_applications(
    State(state): State<AppState>,
    Query(q): Query<ApplicationListQuery>,
) -> Result<Json<Vec<ApplicationRow>>, ApiError> {
    let rows = sqlx::query_as::<_, ApplicationRow>(
        "SELECT a.student_id, a.track_id, a.round_id, a.abandoned, a.excluded, a.excluded_reason, a.department_name,
                s.student_code, s.name, s.grade, s.class_no, s.seq_no, s.is_enrolled,
                ut.univ_id, u.univ_name, ut.track_name, r.recommended, rnd.status AS round_status
         FROM applications a
         JOIN students s ON a.student_id = s.id
         JOIN univ_tracks ut ON a.track_id = ut.id
         JOIN universities u ON ut.univ_id = u.id
         JOIN rounds rnd ON rnd.id = a.round_id
         LEFT JOIN results r ON r.student_id = a.student_id AND r.track_id = a.track_id AND r.round_id = a.round_id
         WHERE (? IS NULL OR a.round_id = ?)
           AND (? IS NULL OR a.track_id = ?)
         ORDER BY u.univ_name, ut.track_name, s.grade, s.class_no, s.seq_no",
    )
    .bind(q.round_id)
    .bind(q.round_id)
    .bind(q.track_id)
    .bind(q.track_id)
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
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // FINALIZED 라운드에서만 포기 입력 허용
    let status: Option<RoundStatus> = sqlx::query_scalar(
        "SELECT status FROM rounds WHERE id = ?",
    )
    .bind(rid)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match status {
        Some(RoundStatus::Finalized) => {}
        Some(_) => return Err((StatusCode::BAD_REQUEST, "FINALIZED 라운드에서만 포기 입력이 가능합니다".into())),
        None => return Err((StatusCode::NOT_FOUND, "라운드를 찾을 수 없습니다".into())),
    }

    let detail = crate::audit::application_detail(&mut *tx, sid, tid).await?;

    let affected = sqlx::query(
        "UPDATE applications SET abandoned = 1
         WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(sid)
    .bind(tid)
    .bind(rid)
    .execute(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .rows_affected();

    // silent no-op 방지: 존재하지 않는 지원에 204를 반환하면 포기 처리된 것으로 오인한다
    if affected == 0 {
        return Err((StatusCode::NOT_FOUND, "지원 내역을 찾을 수 없습니다".into()));
    }

    crate::audit::log(
        &mut *tx,
        AuditEntry {
            actor: Actor::Admin,
            action: AuditAction::ApplicationAbandoned,
            round_id: Some(rid),
            student_id: Some(sid),
            detail,
        },
    )
    .await?;

    tx.commit()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

// ── 미선발 처리 ────────────────────────────────────────────────────
// abandoned(포기)와 별개 — CLOSED 전용, 정원 집계는 건드리지 않는다(feedback_...F단계 설계 참조).

#[derive(Deserialize)]
pub struct ExcludeApplicationBody {
    pub reason: String,
}

fn check_round_closed_for_exclusion(status: Option<RoundStatus>) -> Result<(), ApiError> {
    match status {
        Some(RoundStatus::Closed) => Ok(()),
        Some(RoundStatus::Open) => Err((
            StatusCode::BAD_REQUEST,
            "진행 중 라운드는 담임이 지원을 삭제하세요".into(),
        )),
        Some(RoundStatus::Finalized) => Err((
            StatusCode::BAD_REQUEST,
            "마감된 라운드의 지원은 미선발 처리를 변경할 수 없습니다".into(),
        )),
        None => Err((StatusCode::NOT_FOUND, "라운드를 찾을 수 없습니다".into())),
    }
}

// URL: /applications/:sid/:tid/:rid/exclude  (PUT — 제외 설정)
pub async fn exclude_application(
    State(state): State<AppState>,
    Path((sid, tid, rid)): Path<(i64, i64, i64)>,
    Json(body): Json<ExcludeApplicationBody>,
) -> Result<StatusCode, ApiError> {
    let reason = body.reason.trim().to_string();
    if reason.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "미선발 사유는 필수입니다".into()));
    }

    // BEGIN IMMEDIATE: 추천 확정 여부 조회(SELECT) 후 excluded 갱신(UPDATE)까지 원자적으로 처리.
    // DEFERRED면 두 커넥션이 동시에 recommended=0 을 읽고 둘 다 통과해 모순 상태(recommended=1 AND excluded=1)가 된다.
    let mut tx = state
        .db
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let status: Option<RoundStatus> = sqlx::query_scalar("SELECT status FROM rounds WHERE id = ?")
        .bind(rid)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    check_round_closed_for_exclusion(status)?;

    let current_excluded: Option<bool> = sqlx::query_scalar(
        "SELECT excluded = 1 FROM applications WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(sid)
    .bind(tid)
    .bind(rid)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match current_excluded {
        None => return Err((StatusCode::NOT_FOUND, "지원 내역을 찾을 수 없습니다".into())),
        Some(true) => return Err((StatusCode::CONFLICT, "이미 미선발 처리된 지원입니다".into())),
        Some(false) => {}
    }

    // 추천 확정된 지원은 제외할 수 없다 — recommended 와 excluded 는 상호배타.
    // 둘 다 1인 행은 정원 집계(recommended=1 AND abandoned=0)에 그대로 잡혀
    // 결격 학생이 정원을 점유하는 모순 상태가 된다.
    let recommended: Option<bool> = sqlx::query_scalar(
        "SELECT recommended = 1 FROM results
         WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(sid)
    .bind(tid)
    .bind(rid)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if recommended == Some(true) {
        return Err((
            StatusCode::CONFLICT,
            "이미 추천 확정된 지원은 미선발 처리할 수 없습니다. 추천을 먼저 취소한 후 미선발 처리하세요.".to_string(),
        ));
    }

    let mut detail = crate::audit::application_detail(&mut *tx, sid, tid).await?;
    if let serde_json::Value::Object(ref mut map) = detail {
        map.insert("reason".to_string(), serde_json::Value::String(reason.clone()));
    }

    sqlx::query(
        "UPDATE applications SET excluded = 1, excluded_reason = ?
         WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(&reason)
    .bind(sid)
    .bind(tid)
    .bind(rid)
    .execute(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    crate::audit::log(
        &mut *tx,
        AuditEntry {
            actor: Actor::Admin,
            action: AuditAction::ApplicationExcluded,
            round_id: Some(rid),
            student_id: Some(sid),
            detail,
        },
    )
    .await?;

    tx.commit()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

// URL: /applications/:sid/:tid/:rid/exclude  (DELETE — 제외 해제)
pub async fn clear_application_exclusion(
    State(state): State<AppState>,
    Path((sid, tid, rid)): Path<(i64, i64, i64)>,
) -> Result<StatusCode, ApiError> {
    // BEGIN IMMEDIATE: excluded 상태 조회 후 갱신 사이에 recommend_result 가 끼어들 수 있으므로
    // exclude_application 과 동일하게 읽기-쓰기를 원자적으로 처리한다.
    let mut tx = state
        .db
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let status: Option<RoundStatus> = sqlx::query_scalar("SELECT status FROM rounds WHERE id = ?")
        .bind(rid)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    check_round_closed_for_exclusion(status)?;

    let current_excluded: Option<bool> = sqlx::query_scalar(
        "SELECT excluded = 1 FROM applications WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(sid)
    .bind(tid)
    .bind(rid)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match current_excluded {
        None => return Err((StatusCode::NOT_FOUND, "지원 내역을 찾을 수 없습니다".into())),
        Some(false) => return Err((StatusCode::CONFLICT, "미선발 상태가 아닙니다".into())),
        Some(true) => {}
    }

    let detail = crate::audit::application_detail(&mut *tx, sid, tid).await?;

    sqlx::query(
        "UPDATE applications SET excluded = 0, excluded_reason = NULL
         WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(sid)
    .bind(tid)
    .bind(rid)
    .execute(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    crate::audit::log(
        &mut *tx,
        AuditEntry {
            actor: Actor::Admin,
            action: AuditAction::ApplicationExclusionCleared,
            round_id: Some(rid),
            student_id: Some(sid),
            detail,
        },
    )
    .await?;

    tx.commit()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn teacher_abandon_application(
    State(state): State<AppState>,
    Extension(claims): Extension<TeacherClaims>,
    Path((sid, tid, rid)): Path<(i64, i64, i64)>,
) -> Result<StatusCode, ApiError> {
    let status: Option<RoundStatus> = sqlx::query_scalar(
        "SELECT status FROM rounds WHERE id = ?",
    )
    .bind(rid)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match status {
        Some(RoundStatus::Finalized) => {}
        Some(_) => return Err((StatusCode::BAD_REQUEST, "FINALIZED 라운드에서만 포기 입력이 가능합니다".into())),
        None => return Err((StatusCode::NOT_FOUND, "라운드를 찾을 수 없습니다".into())),
    }

    let in_class: Option<i64> = if is_grad_teacher(&claims) {
        sqlx::query_scalar(
            "SELECT id FROM students WHERE id = ? AND is_enrolled = 0",
        )
        .bind(sid)
        .fetch_optional(&state.db)
        .await
    } else {
        sqlx::query_scalar(
            "SELECT id FROM students WHERE id = ? AND grade = ? AND class_no = ?",
        )
        .bind(sid)
        .bind(claims.grade)
        .bind(claims.class_no)
        .fetch_optional(&state.db)
        .await
    }
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if in_class.is_none() {
        return Err((StatusCode::FORBIDDEN, "해당 학생이 이 반 소속이 아닙니다".into()));
    }

    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let detail = crate::audit::application_detail(&mut *tx, sid, tid).await?;

    let affected = sqlx::query(
        "UPDATE applications SET abandoned = 1
         WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(sid)
    .bind(tid)
    .bind(rid)
    .execute(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .rows_affected();

    // silent no-op 방지: 존재하지 않는 지원에 204를 반환하면 포기 처리된 것으로 오인한다
    if affected == 0 {
        return Err((StatusCode::NOT_FOUND, "지원 내역을 찾을 수 없습니다".into()));
    }

    crate::audit::log(
        &mut *tx,
        AuditEntry {
            actor: Actor::Teacher { grade: claims.grade, class_no: claims.class_no },
            action: AuditAction::ApplicationAbandoned,
            round_id: Some(rid),
            student_id: Some(sid),
            detail,
        },
    )
    .await?;

    tx.commit()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

// ── Teacher ────────────────────────────────────────────────────────

fn is_grad_teacher(claims: &TeacherClaims) -> bool {
    claims.grade == 0 && claims.class_no == 0
}

pub async fn teacher_list_students(
    State(state): State<AppState>,
    Extension(claims): Extension<TeacherClaims>,
) -> Result<Json<Vec<StudentRow>>, ApiError> {
    let rows = if is_grad_teacher(&claims) {
        sqlx::query_as::<_, StudentRow>(
            "SELECT id, student_code, name, grade, class_no, seq_no, is_enrolled
             FROM students WHERE is_enrolled = 0
             ORDER BY student_code",
        )
        .fetch_all(&state.db)
        .await
    } else {
        sqlx::query_as::<_, StudentRow>(
            "SELECT id, student_code, name, grade, class_no, seq_no, is_enrolled
             FROM students WHERE grade = ? AND class_no = ?
             ORDER BY seq_no",
        )
        .bind(claims.grade)
        .bind(claims.class_no)
        .fetch_all(&state.db)
        .await
    }
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(rows))
}

pub async fn teacher_list_applications(
    State(state): State<AppState>,
    Extension(claims): Extension<TeacherClaims>,
    Query(q): Query<TeacherAppListQuery>,
) -> Result<Json<Vec<ApplicationRow>>, ApiError> {
    let rows = if is_grad_teacher(&claims) {
        sqlx::query_as::<_, ApplicationRow>(
            "SELECT a.student_id, a.track_id, a.round_id, a.abandoned, a.excluded, a.excluded_reason, a.department_name,
                    s.student_code, s.name, s.grade, s.class_no, s.seq_no, s.is_enrolled,
                    ut.univ_id, u.univ_name, ut.track_name, r.recommended, rnd.status AS round_status
             FROM applications a
             JOIN students s ON a.student_id = s.id
             JOIN univ_tracks ut ON a.track_id = ut.id
             JOIN universities u ON ut.univ_id = u.id
             JOIN rounds rnd ON rnd.id = a.round_id
             LEFT JOIN results r ON r.student_id = a.student_id AND r.track_id = a.track_id AND r.round_id = a.round_id
             WHERE s.is_enrolled = 0
               AND (? IS NULL OR a.round_id = ?)
             ORDER BY s.student_code, u.univ_name",
        )
        .bind(q.round_id)
        .bind(q.round_id)
        .fetch_all(&state.db)
        .await
    } else {
        sqlx::query_as::<_, ApplicationRow>(
            "SELECT a.student_id, a.track_id, a.round_id, a.abandoned, a.excluded, a.excluded_reason, a.department_name,
                    s.student_code, s.name, s.grade, s.class_no, s.seq_no, s.is_enrolled,
                    ut.univ_id, u.univ_name, ut.track_name, r.recommended, rnd.status AS round_status
             FROM applications a
             JOIN students s ON a.student_id = s.id
             JOIN univ_tracks ut ON a.track_id = ut.id
             JOIN universities u ON ut.univ_id = u.id
             JOIN rounds rnd ON rnd.id = a.round_id
             LEFT JOIN results r ON r.student_id = a.student_id AND r.track_id = a.track_id AND r.round_id = a.round_id
             WHERE s.grade = ? AND s.class_no = ?
               AND (? IS NULL OR a.round_id = ?)
             ORDER BY s.seq_no, u.univ_name",
        )
        .bind(claims.grade)
        .bind(claims.class_no)
        .bind(q.round_id)
        .bind(q.round_id)
        .fetch_all(&state.db)
        .await
    }
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(rows))
}

/// POST /api/teacher/applications
///
/// 지원 등록 + 담임 입력 기초데이터 저장을 하나의 트랜잭션으로 처리한다.
/// 동일 (student_id, track_id, round_id) 재전송 시 department_name을 업데이트(upsert).
pub async fn teacher_create_application(
    State(state): State<AppState>,
    Extension(claims): Extension<TeacherClaims>,
    Json(body): Json<CreateApplicationBody>,
) -> Result<StatusCode, ApiError> {
    // 1. 라운드 OPEN 검증
    let round_status: Option<RoundStatus> = sqlx::query_scalar(
        "SELECT status FROM rounds WHERE id = ?",
    )
    .bind(body.round_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match round_status {
        Some(RoundStatus::Open) => {}
        Some(_) => return Err((StatusCode::BAD_REQUEST, "라운드가 OPEN 상태가 아닙니다".into())),
        None => return Err((StatusCode::NOT_FOUND, "라운드를 찾을 수 없습니다".into())),
    }

    // 2. 학생 소속 검증
    let ok: bool = if is_grad_teacher(&claims) {
        sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM students WHERE id = ? AND is_enrolled = 0)",
        )
        .bind(body.student_id)
        .fetch_one(&state.db)
        .await
    } else {
        sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM students WHERE id = ? AND grade = ? AND class_no = ?)",
        )
        .bind(body.student_id)
        .bind(claims.grade)
        .bind(claims.class_no)
        .fetch_one(&state.db)
        .await
    }
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if !ok {
        return Err((StatusCode::FORBIDDEN, "해당 학생은 담당 학급이 아닙니다".into()));
    }

    if body.department_name.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "학과명은 필수입니다".into()));
    }

    // 3. 전형요소 정보 일괄 로드 및 검증 (트랜잭션 진입 전)
    struct AreaInfo {
        id: i64,
        calc_type: CalcType,
        teacher_editable: bool,
        lookup_scope: LookupScope,
        multi_value: bool,
        max_score: i64,
    }

    let all_areas: Vec<AreaInfo> = {
        #[derive(sqlx::FromRow)]
        struct Row {
            id: i64,
            calc_type: CalcType,
            teacher_editable: bool,
            lookup_scope: LookupScope,
            category_agg: Option<CategoryAgg>,
            max_score: i64,
        }
        sqlx::query_as::<_, Row>(
            "SELECT id, calc_type, teacher_editable, lookup_scope, category_agg, max_score FROM areas",
        )
        .fetch_all(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .into_iter()
        .map(|r| AreaInfo {
            id: r.id,
            calc_type: r.calc_type,
            teacher_editable: r.teacher_editable,
            lookup_scope: r.lookup_scope,
            multi_value: r.category_agg == Some(CategoryAgg::Sum),
            max_score: r.max_score,
        })
        .collect()
    };

    // teacher_editable 검증
    for entry in &body.base_data_entries {
        if entry.values.is_empty() {
            continue;
        }
        let info = all_areas
            .iter()
            .find(|a| a.id == entry.area_id)
            .ok_or_else(|| (StatusCode::NOT_FOUND, format!("전형요소 id={} 없음", entry.area_id)))?;
        if !info.teacher_editable {
            return Err((
                StatusCode::FORBIDDEN,
                format!("전형요소 id={}는 담임 입력이 허용되지 않습니다", entry.area_id),
            ));
        }
    }

    // teacher_editable=true인 전형요소는 모두 값이 있어야 함
    let submitted_area_ids: std::collections::HashSet<i64> = body
        .base_data_entries
        .iter()
        .filter(|e| !e.values.is_empty())
        .map(|e| e.area_id)
        .collect();
    for area in &all_areas {
        if area.teacher_editable && !submitted_area_ids.contains(&area.id) {
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("전형요소 id={}의 값이 누락되었습니다. 모든 담임 입력 전형요소에 값을 입력해야 합니다", area.id),
            ));
        }
    }

    // 4. 값 인코딩 (×100000 변환) — 트랜잭션 진입 전
    struct EncodedEntry {
        area_id: i64,
        lookup_track: Option<i64>,
        db_values: Vec<String>,
        multi_value: bool,
    }

    let mut encoded: Vec<EncodedEntry> = Vec::new();
    for entry in &body.base_data_entries {
        if entry.values.is_empty() {
            continue;
        }
        let info = all_areas
            .iter()
            .find(|a| a.id == entry.area_id)
            .ok_or_else(|| (StatusCode::NOT_FOUND, format!("전형요소 id={} 없음", entry.area_id)))?;

        let lookup_track: Option<i64> = if info.lookup_scope == LookupScope::Composite {
            Some(body.track_id)
        } else {
            None
        };

        let db_values: Vec<String> = match info.calc_type {
            CalcType::Numeric | CalcType::Manual => {
                if entry.values.len() != 1 {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        format!(
                            "전형요소 id={}: NUMERIC/MANUAL은 정확히 1개의 값이 필요합니다",
                            entry.area_id
                        ),
                    ));
                }
                let v = parse_display_value(&entry.values[0]).map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        format!("전형요소 id={}: {}", entry.area_id, e),
                    )
                })?;
                // MANUAL: 입력값이 곧 점수 — 만점 초과 금지
                if info.calc_type == CalcType::Manual && v > info.max_score {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        format!(
                            "전형요소 id={}: 값({})이 만점({})을 초과합니다",
                            entry.area_id,
                            crate::handlers::area_data::fmt_score(v),
                            crate::handlers::area_data::fmt_score(info.max_score),
                        ),
                    ));
                }
                vec![v.to_string()]
            }
            CalcType::Category => {
                if !info.multi_value && entry.values.len() > 1 {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        format!(
                            "전형요소 id={}: 단일값 전형요소에 복수값 입력 불가",
                            entry.area_id
                        ),
                    ));
                }
                let trimmed: Vec<String> = entry.values.iter()
                    .map(|v| v.trim().to_string())
                    .collect();
                if trimmed.iter().any(|v| v.is_empty()) {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        format!("전형요소 id={}: 빈 범주 값은 허용되지 않습니다", entry.area_id),
                    ));
                }
                trimmed
            }
        };

        encoded.push(EncodedEntry {
            area_id: entry.area_id,
            lookup_track,
            db_values,
            multi_value: info.multi_value,
        });
    }

    // 5. 트랜잭션: 기초데이터 저장 → 지원 upsert → 점수 계산 → results 저장
    //    base_data를 먼저 저장해야 calc_area_score가 새로 입력한 값을 읽을 수 있다.
    //    BEGIN IMMEDIATE: 시작 시점에 쓰기 잠금을 획득해 아래 상태 재확인이 확정적이 된다.
    //    (DEFERRED면 재확인 후 close_round가 커밋해 첫 쓰기가 BUSY_SNAPSHOT 500으로 실패)
    let mut tx = state
        .db
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // tx 안에서 라운드 상태 재확인 — tx 밖 확인 후 CLOSED로 전환되는 TOCTOU 방지
    let round_status_in_tx: Option<RoundStatus> = sqlx::query_scalar(
        "SELECT status FROM rounds WHERE id = ?",
    )
    .bind(body.round_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if round_status_in_tx != Some(RoundStatus::Open) {
        return Err((StatusCode::BAD_REQUEST, "라운드가 OPEN 상태가 아닙니다".into()));
    }

    // 모집단위 변경: prev_track_id가 있고 현재 track_id와 다를 때
    if let Some(prev_tid) = body.prev_track_id {
        if prev_tid != body.track_id {
            // 대상 트랙에 이미 지원이 존재하면 409
            let target_exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM applications \
                 WHERE student_id = ? AND track_id = ? AND round_id = ?)",
            )
            .bind(body.student_id)
            .bind(body.track_id)
            .bind(body.round_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

            if target_exists {
                return Err((
                    StatusCode::CONFLICT,
                    "이미 해당 모집단위에 지원되어 있습니다. 기존 지원을 먼저 취소하세요".into(),
                ));
            }

            // 이전 지원이 없으면 404
            let prev_exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM applications \
                 WHERE student_id = ? AND track_id = ? AND round_id = ?)",
            )
            .bind(body.student_id)
            .bind(prev_tid)
            .bind(body.round_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

            if !prev_exists {
                return Err((
                    StatusCode::NOT_FOUND,
                    "수정할 지원 내역을 찾을 수 없습니다".into(),
                ));
            }

            // 삭제 전 스냅샷
            let prev_detail =
                crate::audit::application_detail(&mut *tx, body.student_id, prev_tid).await?;

            // results → applications 순서 삭제 (FK 제약)
            sqlx::query(
                "DELETE FROM results WHERE student_id = ? AND track_id = ? AND round_id = ?",
            )
            .bind(body.student_id)
            .bind(prev_tid)
            .bind(body.round_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

            sqlx::query(
                "DELETE FROM applications WHERE student_id = ? AND track_id = ? AND round_id = ?",
            )
            .bind(body.student_id)
            .bind(prev_tid)
            .bind(body.round_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

            // 감사 로그: ApplicationDeleted (이전 트랙)
            crate::audit::log(
                &mut *tx,
                AuditEntry {
                    actor: Actor::Teacher { grade: claims.grade, class_no: claims.class_no },
                    action: AuditAction::ApplicationDeleted,
                    round_id: Some(body.round_id),
                    student_id: Some(body.student_id),
                    detail: prev_detail,
                },
            )
            .await?;
        }
    }

    for entry in &encoded {
        if entry.multi_value {
            // 복수값: 기존 행 전체 삭제 후 새 값 삽입
            sqlx::query(
                "DELETE FROM base_data
                 WHERE student_id = ? AND area_id = ?
                   AND (track_id = ? OR (? IS NULL AND track_id IS NULL))
                   AND multi_value = 1",
            )
            .bind(body.student_id)
            .bind(entry.area_id)
            .bind(entry.lookup_track)
            .bind(entry.lookup_track)
            .execute(&mut *tx)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

            for val in &entry.db_values {
                sqlx::query(
                    "INSERT INTO base_data (student_id, area_id, track_id, value, multi_value)
                     VALUES (?, ?, ?, ?, 1)",
                )
                .bind(body.student_id)
                .bind(entry.area_id)
                .bind(entry.lookup_track)
                .bind(val)
                .execute(&mut *tx)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            }
        } else {
            // 단일값: INSERT OR REPLACE (기존 행 대체)
            sqlx::query(
                "INSERT OR REPLACE INTO base_data (student_id, area_id, track_id, value, multi_value)
                 VALUES (?, ?, ?, ?, 0)",
            )
            .bind(body.student_id)
            .bind(entry.area_id)
            .bind(entry.lookup_track)
            .bind(&entry.db_values[0])
            .execute(&mut *tx)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        }
    }

    sqlx::query(
        "INSERT INTO applications (student_id, track_id, round_id, abandoned, department_name)
         VALUES (?, ?, ?, 0, ?)
         ON CONFLICT(student_id, track_id, round_id)
         DO UPDATE SET department_name = excluded.department_name",
    )
    .bind(body.student_id)
    .bind(body.track_id)
    .bind(body.round_id)
    .bind(&body.department_name)
    .execute(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 점수 계산을 위해 DB 데이터 로드 (트랜잭션 내에서)
    let areas: Vec<AreaRow> = sqlx::query_as::<_, AreaRow>(
        "SELECT id, name, calc_type, max_score, match_mode, category_agg, lookup_scope FROM areas ORDER BY id",
    )
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    #[derive(sqlx::FromRow)]
    struct StudentTrackInfo {
        student_code: String,
        name: String,
        univ_name: String,
        track_name: String,
    }
    let info: StudentTrackInfo = sqlx::query_as::<_, StudentTrackInfo>(
        "SELECT s.student_code, s.name, u.univ_name, ut.track_name
         FROM students s, univ_tracks ut, universities u
         WHERE s.id = ? AND ut.id = ? AND u.id = ut.univ_id",
    )
    .bind(body.student_id)
    .bind(body.track_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let ctx = StudentTrackCtx {
        student_code: info.student_code,
        student_name: info.name,
        univ_name: info.univ_name,
        track_name: info.track_name,
    };

    // 점수 계산
    let mut detail: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    let mut total: i64 = 0;
    for area in &areas {
        let sc = calc_area_score(&mut *tx, body.student_id, area, body.track_id, &ctx)
            .await
            .map_err(|e| (StatusCode::UNPROCESSABLE_ENTITY, e))?;
        detail.insert(area.id.to_string(), sc);
        total = total.checked_add(sc).ok_or_else(|| {
            (StatusCode::INTERNAL_SERVER_ERROR, "점수 합산 오버플로우".to_string())
        })?;
    }

    let detail_json = serde_json::to_string(&detail)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let now = chrono::Utc::now().to_rfc3339();

    // results 저장
    sqlx::query(
        "INSERT INTO results
           (student_id, track_id, round_id, score_detail, total_score, ranking, recommended, calculated_at)
         VALUES (?, ?, ?, ?, ?, NULL, 0, ?)
         ON CONFLICT (student_id, track_id, round_id)
         DO UPDATE SET score_detail   = excluded.score_detail,
                       total_score    = excluded.total_score,
                       ranking        = NULL,
                       calculated_at  = excluded.calculated_at",
    )
    .bind(body.student_id)
    .bind(body.track_id)
    .bind(body.round_id)
    .bind(&detail_json)
    .bind(total)
    .bind(&now)
    .execute(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 지원 변경 시 확정 자동 해제 — 확정 후 지원이 바뀌면 관리자가 보는 "확정됨"이 거짓이 됨
    let revoked_count = sqlx::query(
        "DELETE FROM round_confirmations WHERE round_id = ? AND grade = ? AND class_no = ?",
    )
    .bind(body.round_id)
    .bind(claims.grade)
    .bind(claims.class_no)
    .execute(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .rows_affected();

    let mut app_detail =
        crate::audit::application_detail(&mut *tx, body.student_id, body.track_id).await?;
    if let serde_json::Value::Object(ref mut map) = app_detail {
        map.insert(
            "department_name".to_string(),
            serde_json::Value::String(body.department_name.clone()),
        );
    }
    crate::audit::log(
        &mut *tx,
        AuditEntry {
            actor: Actor::Teacher { grade: claims.grade, class_no: claims.class_no },
            action: AuditAction::ApplicationSaved,
            round_id: Some(body.round_id),
            student_id: Some(body.student_id),
            detail: app_detail,
        },
    )
    .await?;

    if revoked_count > 0 {
        crate::audit::log(
            &mut *tx,
            AuditEntry {
                actor: Actor::Teacher { grade: claims.grade, class_no: claims.class_no },
                action: AuditAction::RoundConfirmationRevoked,
                round_id: Some(body.round_id),
                student_id: None,
                detail: serde_json::json!({ "auto": true }),
            },
        )
        .await?;
    }

    tx.commit()
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
    let round_status: Option<RoundStatus> = sqlx::query_scalar(
        "SELECT status FROM rounds WHERE id = ?",
    )
    .bind(rid)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match round_status {
        Some(RoundStatus::Open) => {}
        Some(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                "OPEN 라운드의 지원만 취소할 수 있습니다".into(),
            ))
        }
        None => return Err((StatusCode::NOT_FOUND, "라운드를 찾을 수 없습니다".into())),
    }

    let ok: bool = if is_grad_teacher(&claims) {
        sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM students WHERE id = ? AND is_enrolled = 0)",
        )
        .bind(sid)
        .fetch_one(&state.db)
        .await
    } else {
        sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM students WHERE id = ? AND grade = ? AND class_no = ?)",
        )
        .bind(sid)
        .bind(claims.grade)
        .bind(claims.class_no)
        .fetch_one(&state.db)
        .await
    }
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if !ok {
        return Err((StatusCode::FORBIDDEN, "해당 학생은 담당 학급이 아닙니다".into()));
    }

    // 트랜잭션: applications와 results 함께 삭제
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 삭제 전 스냅샷 조회 (students·univ_tracks는 삭제되지 않지만 일관성을 위해 먼저 조회)
    let detail = crate::audit::application_detail(&mut *tx, sid, tid).await?;

    // results를 먼저 삭제해야 FK 제약 위반 없음 (results → applications 참조)
    sqlx::query(
        "DELETE FROM results WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(sid)
    .bind(tid)
    .bind(rid)
    .execute(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let deleted = sqlx::query(
        "DELETE FROM applications WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(sid)
    .bind(tid)
    .bind(rid)
    .execute(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .rows_affected();

    // silent no-op 방지: 없는 지원 삭제가 204로 성공하면 확정 자동 해제·감사 로그까지
    // 유령으로 발생한다 (rollback으로 전부 취소됨)
    if deleted == 0 {
        return Err((StatusCode::NOT_FOUND, "지원 내역을 찾을 수 없습니다".into()));
    }

    // 지원 삭제 시 확정 자동 해제
    let revoked_count = sqlx::query(
        "DELETE FROM round_confirmations WHERE round_id = ? AND grade = ? AND class_no = ?",
    )
    .bind(rid)
    .bind(claims.grade)
    .bind(claims.class_no)
    .execute(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .rows_affected();

    crate::audit::log(
        &mut *tx,
        AuditEntry {
            actor: Actor::Teacher { grade: claims.grade, class_no: claims.class_no },
            action: AuditAction::ApplicationDeleted,
            round_id: Some(rid),
            student_id: Some(sid),
            detail,
        },
    )
    .await?;

    if revoked_count > 0 {
        crate::audit::log(
            &mut *tx,
            AuditEntry {
                actor: Actor::Teacher { grade: claims.grade, class_no: claims.class_no },
                action: AuditAction::RoundConfirmationRevoked,
                round_id: Some(rid),
                student_id: None,
                detail: serde_json::json!({ "auto": true }),
            },
        )
        .await?;
    }

    tx.commit()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}
