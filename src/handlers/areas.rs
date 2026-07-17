use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::Response,
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::audit::{self, Actor, AuditEntry};
use crate::enums::{AuditAction, CalcType, CategoryAgg, LookupScope, MatchMode};
use crate::score::Score;
use crate::state::AppState;

type ApiError = (StatusCode, String);

#[derive(Serialize, FromRow)]
pub struct AreaRow {
    pub id: i64,
    pub name: String,
    pub max_score: Score,
    pub calc_type: CalcType,
    pub teacher_editable: bool,
    pub lookup_scope: LookupScope,
    pub match_mode: Option<MatchMode>,
    pub category_agg: Option<CategoryAgg>,
    pub multi_value: bool,
}

#[derive(Deserialize)]
pub struct CreateAreaBody {
    pub name: String,
    pub max_score: Score,
    pub calc_type: CalcType,
    pub teacher_editable: bool,
    pub lookup_scope: LookupScope,
    pub match_mode: Option<MatchMode>,
    pub category_agg: Option<CategoryAgg>,
    #[serde(default)]
    pub multi_value: bool,
}

#[derive(Deserialize)]
pub struct UpdateAreaBody {
    pub name: Option<String>,
    pub teacher_editable: Option<bool>,
}

pub async fn list_areas(State(state): State<AppState>) -> Result<Json<Vec<AreaRow>>, ApiError> {
    let rows = sqlx::query_as::<_, AreaRow>(
        "SELECT id, name, max_score, calc_type, teacher_editable, lookup_scope,
                match_mode, category_agg, multi_value
         FROM areas ORDER BY name",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(rows))
}

pub(crate) async fn guard_no_closed_round(db: &sqlx::SqlitePool) -> Result<(), ApiError> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM rounds WHERE status IN ('CLOSED', 'FINALIZED')",
    )
    .fetch_one(db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if count > 0 {
        return Err((
            StatusCode::CONFLICT,
            "종료되거나 마감된 라운드가 존재하므로 전형요소 설정을 변경할 수 없습니다".into(),
        ));
    }
    Ok(())
}

pub async fn create_area(
    State(state): State<AppState>,
    Json(body): Json<CreateAreaBody>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    guard_no_closed_round(&state.db).await?;

    if body.name.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "전형요소 이름은 필수입니다".into()));
    }
    if body.max_score.raw() < 0 {
        return Err((StatusCode::BAD_REQUEST, "만점은 0 이상이어야 합니다".into()));
    }
    if body.calc_type == CalcType::Numeric && body.match_mode.is_none() {
        return Err((StatusCode::BAD_REQUEST, "수치형 입력 전형요소는 구간 탐색 방향(UPPER/LOWER/EXACT)이 필수입니다".into()));
    }
    if body.calc_type == CalcType::Category && body.category_agg.is_none() {
        return Err((StatusCode::BAD_REQUEST, "선택형 입력 전형요소는 복수 활동 처리 방식(SUM/MAX)이 필수입니다".into()));
    }
    let multi_value = body.category_agg == Some(CategoryAgg::Sum);

    let area_name = body.name.clone();
    let mut tx = state.db.begin().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, teacher_editable, lookup_scope,
                            match_mode, category_agg, multi_value)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)
         RETURNING id",
    )
    .bind(&body.name)
    .bind(body.max_score.raw())
    .bind(body.calc_type)
    .bind(body.teacher_editable)
    .bind(body.lookup_scope)
    .bind(body.match_mode)
    .bind(body.category_agg)
    .bind(multi_value)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    audit::log(&mut *tx, AuditEntry {
        actor: Actor::Admin,
        action: AuditAction::AreaCreated,
        round_id: None,
        student_id: None,
        detail: serde_json::json!({ "name": area_name }),
    }).await?;
    tx.commit().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

pub async fn update_area(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateAreaBody>,
) -> Result<StatusCode, ApiError> {
    guard_no_closed_round(&state.db).await?;
    // 변경 필드가 하나도 없는 요청은 거부 — 아무것도 바꾸지 않는 AREA_UPDATED 감사 로그를 남기지 않는다
    if body.name.is_none() && body.teacher_editable.is_none() {
        return Err((StatusCode::BAD_REQUEST, "수정할 내용이 없습니다".into()));
    }
    let mut tx = state.db.begin().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if let Some(v) = body.name {
        let v = v.trim().to_string();
        if v.is_empty() {
            return Err((StatusCode::BAD_REQUEST, "전형요소 이름은 필수입니다".into()));
        }
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

    let (area_name, te_raw): (String, i64) = sqlx::query_as(
        "SELECT name, teacher_editable FROM areas WHERE id = ?",
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    audit::log(&mut *tx, AuditEntry {
        actor: Actor::Admin,
        action: AuditAction::AreaUpdated,
        round_id: None,
        student_id: None,
        detail: serde_json::json!({ "name": area_name, "teacher_editable": te_raw != 0 }),
    }).await?;

    tx.commit().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn delete_area(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    guard_no_closed_round(&state.db).await?;

    let mut tx = state.db.begin().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 삭제 전 이름 스냅샷 — 대상이 없으면 404 (없는 대상의 삭제 로그를 남기지 않는다)
    let area_name: String = sqlx::query_scalar("SELECT name FROM areas WHERE id = ?")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "전형요소를 찾을 수 없습니다".to_string()))?;

    sqlx::query("DELETE FROM areas WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    audit::log(&mut *tx, AuditEntry {
        actor: Actor::Admin,
        action: AuditAction::AreaDeleted,
        round_id: None,
        student_id: None,
        detail: serde_json::json!({ "name": area_name }),
    }).await?;

    tx.commit().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

const SCORE_TEMPLATE_NAMES: &[&str] = &[
    "grade", "attendance", "volunteer", "award", "extracurricular", "penalty",
];

pub async fn score_template(Path(name): Path<String>) -> Result<Response, ApiError> {
    if !SCORE_TEMPLATE_NAMES.contains(&name.as_str()) {
        return Err((StatusCode::NOT_FOUND, "존재하지 않는 템플릿입니다".into()));
    }

    let filename = format!("{}.xlsx", name);
    match crate::score_templates::Assets::get(&filename) {
        Some(file) => {
            let disposition = format!("attachment; filename=\"{}_score_sample.xlsx\"", name);
            Ok(Response::builder()
                .header(header::CONTENT_TYPE, "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet")
                .header(header::CONTENT_DISPOSITION, disposition)
                .body(Body::from(file.data.into_owned()))
                .unwrap())
        }
        None => Err((StatusCode::NOT_FOUND, "샘플 파일이 아직 준비되지 않았습니다".into())),
    }
}
