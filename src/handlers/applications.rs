use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::{
    auth::TeacherClaims,
    enums::{CalcType, LookupScope, RoundStatus},
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
        return Err((StatusCode::UNAUTHORIZED, "현재 비밀번호가 틀렸습니다".into()));
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
    pub confirmed: bool,
    pub abandoned: bool,
    pub department_name: String,
    pub student_code: String,
    pub name: String,
    pub grade: Option<i64>,
    pub class_no: Option<i64>,
    pub seq_no: Option<i64>,
    pub is_enrolled: bool,
    pub univ_name: String,
    pub track_name: String,
    pub recommended: Option<bool>,
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
}

// ── Admin ──────────────────────────────────────────────────────────

pub async fn admin_list_applications(
    State(state): State<AppState>,
    Query(q): Query<ApplicationListQuery>,
) -> Result<Json<Vec<ApplicationRow>>, ApiError> {
    let rows = sqlx::query_as::<_, ApplicationRow>(
        "SELECT a.student_id, a.track_id, a.round_id, a.confirmed, a.abandoned, a.department_name,
                s.student_code, s.name, s.grade, s.class_no, s.seq_no, s.is_enrolled,
                u.univ_name, ut.track_name, r.recommended
         FROM applications a
         JOIN students s ON a.student_id = s.id
         JOIN univ_tracks ut ON a.track_id = ut.id
         JOIN universities u ON ut.univ_id = u.id
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
    // FINALIZED 라운드에서만 포기 입력 허용
    let status: Option<RoundStatus> = sqlx::query_scalar(
        "SELECT status FROM rounds WHERE id = ?",
    )
    .bind(rid)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if status != Some(RoundStatus::Finalized) {
        return Err((StatusCode::BAD_REQUEST, "FINALIZED 라운드에서만 포기 입력이 가능합니다".into()));
    }

    sqlx::query(
        "UPDATE applications SET abandoned = 1
         WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(sid)
    .bind(tid)
    .bind(rid)
    .execute(&state.db)
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

    if status != Some(RoundStatus::Finalized) {
        return Err((StatusCode::BAD_REQUEST, "FINALIZED 라운드에서만 포기 입력이 가능합니다".into()));
    }

    let in_class: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM students WHERE id = ? AND grade = ? AND class_no = ?",
    )
    .bind(sid)
    .bind(claims.grade)
    .bind(claims.class_no)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if in_class.is_none() {
        return Err((StatusCode::FORBIDDEN, "해당 학생이 이 반 소속이 아닙니다".into()));
    }

    sqlx::query(
        "UPDATE applications SET abandoned = 1
         WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(sid)
    .bind(tid)
    .bind(rid)
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
        "SELECT a.student_id, a.track_id, a.round_id, a.confirmed, a.abandoned, a.department_name,
                s.student_code, s.name, s.grade, s.class_no, s.seq_no, s.is_enrolled,
                u.univ_name, ut.track_name, r.recommended
         FROM applications a
         JOIN students s ON a.student_id = s.id
         JOIN univ_tracks ut ON a.track_id = ut.id
         JOIN universities u ON ut.univ_id = u.id
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
    }

    let all_areas: Vec<AreaInfo> = {
        #[derive(sqlx::FromRow)]
        struct Row {
            id: i64,
            calc_type: CalcType,
            teacher_editable: bool,
            lookup_scope: LookupScope,
            multi_value: bool,
        }
        sqlx::query_as::<_, Row>(
            "SELECT id, calc_type, teacher_editable, lookup_scope, multi_value FROM areas",
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
            multi_value: r.multi_value,
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
                entry.values.clone()
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
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

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
        "INSERT INTO applications (student_id, track_id, round_id, confirmed, abandoned, department_name)
         VALUES (?, ?, ?, 1, 0, ?)
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
        total += sc;
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

    if round_status != Some(RoundStatus::Open) {
        return Err((
            StatusCode::BAD_REQUEST,
            "OPEN 라운드의 지원만 취소할 수 있습니다".into(),
        ));
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

    // 트랜잭션: applications와 results 함께 삭제
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

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

    sqlx::query(
        "DELETE FROM applications WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(sid)
    .bind(tid)
    .bind(rid)
    .execute(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}
