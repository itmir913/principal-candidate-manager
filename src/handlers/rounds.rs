use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Serialize;
use sqlx::FromRow;

use crate::enums::RoundStatus;
use crate::handlers::scoring::run_calculate_scores;
use crate::state::AppState;

type ApiError = (StatusCode, String);

#[derive(Serialize, FromRow)]
pub struct RoundRow {
    pub id: i64,
    pub status: RoundStatus,
    pub opened_at: String,
    pub closed_at: Option<String>,
    pub finalized_at: Option<String>,
}

pub async fn list_rounds(
    State(state): State<AppState>,
) -> Result<Json<Vec<RoundRow>>, ApiError> {
    let rows = sqlx::query_as::<_, RoundRow>(
        "SELECT id, status, opened_at, closed_at, finalized_at FROM rounds ORDER BY id DESC",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(rows))
}

pub async fn get_current_round(
    State(state): State<AppState>,
) -> Result<Json<Option<RoundRow>>, ApiError> {
    let row = sqlx::query_as::<_, RoundRow>(
        "SELECT id, status, opened_at, closed_at, finalized_at FROM rounds WHERE status = 'OPEN' LIMIT 1",
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(row))
}

pub async fn open_round(
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let existing: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM rounds WHERE status IN ('OPEN', 'CLOSED') LIMIT 1",
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if existing.is_some() {
        return Err((StatusCode::CONFLICT, "진행 중인 라운드가 있습니다. 모든 라운드가 마감된 후에만 새 라운드를 열 수 있습니다".into()));
    }

    let now = chrono::Utc::now().to_rfc3339();
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at) VALUES ('OPEN', ?) RETURNING id",
    )
    .bind(&now)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

pub async fn close_round(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // 기초데이터 누락 사전 검증 — status 변경 전에 확인하여 CLOSED 상태로 진입 후 계산 실패를 방지
    let missing: Vec<(String, String, String, String, String)> = sqlx::query_as(
        "SELECT a.name, s.name, s.student_code, u.univ_name, ut.track_name
         FROM applications ap
         JOIN students s ON s.id = ap.student_id
         JOIN univ_tracks ut ON ut.id = ap.track_id
         JOIN universities u ON u.id = ut.univ_id
         CROSS JOIN areas a
         WHERE ap.round_id = ? AND ap.confirmed = 1
           AND NOT EXISTS (
             SELECT 1 FROM base_data bd
             WHERE bd.student_id = ap.student_id AND bd.area_id = a.id
               AND CASE WHEN a.lookup_scope = 'COMPOSITE'
                        THEN bd.track_id = ap.track_id
                        ELSE bd.track_id IS NULL END
           )
         LIMIT 5",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if !missing.is_empty() {
        let details: Vec<String> = missing
            .iter()
            .map(|(area, student, code, univ, track)| {
                format!("전형요소 '{}': {} {} 지원자 {} ({})", area, univ, track, student, code)
            })
            .collect();
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("기초데이터 누락으로 라운드를 종료할 수 없습니다:\n{}", details.join("\n")),
        ));
    }

    let now = chrono::Utc::now().to_rfc3339();
    let affected = sqlx::query(
        "UPDATE rounds SET status = 'CLOSED', closed_at = ? WHERE id = ? AND status = 'OPEN'",
    )
    .bind(&now)
    .bind(id)
    .execute(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .rows_affected();

    if affected == 0 {
        return Err((StatusCode::NOT_FOUND, format!("라운드 id={} 없거나 이미 CLOSED", id)));
    }

    let count = run_calculate_scores(&state.db, id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(serde_json::json!({ "calculated": count })))
}

pub async fn reopen_round(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let affected = sqlx::query(
        "UPDATE rounds SET status = 'OPEN', closed_at = NULL WHERE id = ? AND status = 'CLOSED'",
    )
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .rows_affected();

    if affected == 0 {
        return Err((StatusCode::NOT_FOUND, "라운드를 찾을 수 없거나 CLOSED 상태가 아닙니다".into()));
    }

    // 추천 플래그 및 순위 초기화 — 재계산 전 stale 데이터 노출 방지
    sqlx::query(
        "UPDATE results SET recommended = 0, ranking = NULL WHERE round_id = ?",
    )
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn finalize_round(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    let now = chrono::Utc::now().to_rfc3339();
    let affected = sqlx::query(
        "UPDATE rounds SET status = 'FINALIZED', finalized_at = ? WHERE id = ? AND status = 'CLOSED'",
    )
    .bind(&now)
    .bind(id)
    .execute(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .rows_affected();

    if affected == 0 {
        return Err((StatusCode::NOT_FOUND, "라운드를 찾을 수 없거나 CLOSED 상태가 아닙니다".into()));
    }

    Ok(StatusCode::NO_CONTENT)
}
