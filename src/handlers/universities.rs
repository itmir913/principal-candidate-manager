use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Deserializer, Serialize};
use sqlx::FromRow;

use crate::state::AppState;

type ApiError = (StatusCode, String);

// ── 조회용 구조체 ────────────────────────────────────────────────

#[derive(Serialize, FromRow)]
pub struct UnivRow {
    pub id: i64,
    pub univ_name: String,
    pub total_quota: Option<i64>,
    pub prioritize_enrolled: i64,
}

#[derive(Serialize, FromRow)]
pub struct TrackRow {
    pub id: i64,
    pub univ_id: i64,
    pub track_name: String,
    pub unit_quota: Option<i64>,
}

/// 모집단위 + 대학명 포함 (담임 지원 등록 드롭다운용)
#[derive(Serialize, FromRow)]
pub struct TrackWithUnivRow {
    pub id: i64,
    pub univ_id: i64,
    pub univ_name: String,
    pub track_name: String,
    pub unit_quota: Option<i64>,
}

// ── Option<Option<T>> 역직렬화 헬퍼 ─────────────────────────────
// JSON에서 필드 absent → None (변경 없음)
//          null       → Some(None) (NULL로 클리어)
//          값         → Some(Some(v)) (값 설정)

fn deser_opt_opt<'de, T, D>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Ok(Some(Option::deserialize(d)?))
}

// ── 요청 바디 ────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateUnivBody {
    pub univ_name: String,
    #[serde(default, deserialize_with = "deser_opt_opt")]
    pub total_quota: Option<Option<i64>>,
    pub prioritize_enrolled: bool,
}

#[derive(Deserialize)]
pub struct UpdateUnivBody {
    pub univ_name: Option<String>,
    #[serde(default, deserialize_with = "deser_opt_opt")]
    pub total_quota: Option<Option<i64>>,
    pub prioritize_enrolled: Option<bool>,
}

#[derive(Deserialize)]
pub struct CreateTrackBody {
    pub track_name: String,
    #[serde(default, deserialize_with = "deser_opt_opt")]
    pub unit_quota: Option<Option<i64>>,
}

#[derive(Deserialize)]
pub struct UpdateTrackBody {
    pub track_name: Option<String>,
    #[serde(default, deserialize_with = "deser_opt_opt")]
    pub unit_quota: Option<Option<i64>>,
}

// ── 대학 마스터 핸들러 ───────────────────────────────────────────

/// GET /api/universities
pub async fn list_universities(
    State(state): State<AppState>,
) -> Result<Json<Vec<UnivRow>>, ApiError> {
    let rows = sqlx::query_as::<_, UnivRow>(
        "SELECT id, univ_name, total_quota, prioritize_enrolled
         FROM universities ORDER BY id",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(rows))
}

/// POST /api/universities
pub async fn create_university(
    State(state): State<AppState>,
    Json(body): Json<CreateUnivBody>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let total_quota = body.total_quota.flatten();
    let enrolled = body.prioritize_enrolled as i64;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name, total_quota, prioritize_enrolled)
         VALUES (?, ?, ?) RETURNING id",
    )
    .bind(&body.univ_name)
    .bind(total_quota)
    .bind(enrolled)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

/// PUT /api/universities/:id
pub async fn update_university(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateUnivBody>,
) -> Result<StatusCode, ApiError> {
    if let Some(v) = body.univ_name {
        sqlx::query("UPDATE universities SET univ_name = ? WHERE id = ?")
            .bind(v).bind(id).execute(&state.db).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    if let Some(v) = body.total_quota {
        sqlx::query("UPDATE universities SET total_quota = ? WHERE id = ?")
            .bind(v).bind(id).execute(&state.db).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    if let Some(v) = body.prioritize_enrolled {
        sqlx::query("UPDATE universities SET prioritize_enrolled = ? WHERE id = ?")
            .bind(v as i64).bind(id).execute(&state.db).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE /api/universities/:id
pub async fn delete_university(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    sqlx::query("DELETE FROM universities WHERE id = ?")
        .bind(id).execute(&state.db).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

// ── 모집단위 핸들러 ──────────────────────────────────────────────

/// GET /api/universities/:id/tracks
pub async fn list_tracks(
    State(state): State<AppState>,
    Path(univ_id): Path<i64>,
) -> Result<Json<Vec<TrackRow>>, ApiError> {
    let rows = sqlx::query_as::<_, TrackRow>(
        "SELECT id, univ_id, track_name, unit_quota
         FROM univ_tracks WHERE univ_id = ? ORDER BY id",
    )
    .bind(univ_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(rows))
}

/// GET /api/univ-tracks  (전체 모집단위 목록, 대학명 포함)
pub async fn list_all_tracks(
    State(state): State<AppState>,
) -> Result<Json<Vec<TrackWithUnivRow>>, ApiError> {
    let rows = sqlx::query_as::<_, TrackWithUnivRow>(
        "SELECT ut.id, ut.univ_id, u.univ_name, ut.track_name, ut.unit_quota
         FROM univ_tracks ut
         JOIN universities u ON ut.univ_id = u.id
         ORDER BY u.univ_name, ut.track_name",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(rows))
}

/// POST /api/universities/:id/tracks
pub async fn create_track(
    State(state): State<AppState>,
    Path(univ_id): Path<i64>,
    Json(body): Json<CreateTrackBody>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let unit_quota = body.unit_quota.flatten();
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name, unit_quota)
         VALUES (?, ?, ?) RETURNING id",
    )
    .bind(univ_id)
    .bind(&body.track_name)
    .bind(unit_quota)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

/// PUT /api/univ-tracks/:id
pub async fn update_track(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateTrackBody>,
) -> Result<StatusCode, ApiError> {
    if let Some(v) = body.track_name {
        sqlx::query("UPDATE univ_tracks SET track_name = ? WHERE id = ?")
            .bind(v).bind(id).execute(&state.db).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    if let Some(v) = body.unit_quota {
        sqlx::query("UPDATE univ_tracks SET unit_quota = ? WHERE id = ?")
            .bind(v).bind(id).execute(&state.db).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE /api/univ-tracks/:id
pub async fn delete_track(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    sqlx::query("DELETE FROM univ_tracks WHERE id = ?")
        .bind(id).execute(&state.db).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}
