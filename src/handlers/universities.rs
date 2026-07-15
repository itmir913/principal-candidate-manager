use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Response,
    Json,
};
use rust_xlsxwriter::Workbook;
use serde::{Deserialize, Deserializer, Serialize};
use sqlx::FromRow;
use std::collections::HashMap;

use crate::{excel, state::AppState};

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
    pub prioritize_enrolled: i64,
}

/// 모집단위 + 대학명 포함 (담임 지원 등록 드롭다운용)
#[derive(Serialize, FromRow)]
pub struct TrackWithUnivRow {
    pub id: i64,
    pub univ_id: i64,
    pub univ_name: String,
    pub total_quota: Option<i64>,
    pub track_name: String,
    pub unit_quota: Option<i64>,
    pub prioritize_enrolled: i64,
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
    pub prioritize_enrolled: bool,
}

#[derive(Deserialize)]
pub struct UpdateTrackBody {
    pub track_name: Option<String>,
    #[serde(default, deserialize_with = "deser_opt_opt")]
    pub unit_quota: Option<Option<i64>>,
    pub prioritize_enrolled: Option<bool>,
}

// ── 대학 마스터 핸들러 ───────────────────────────────────────────

/// GET /api/universities
pub async fn list_universities(
    State(state): State<AppState>,
) -> Result<Json<Vec<UnivRow>>, ApiError> {
    let rows = sqlx::query_as::<_, UnivRow>(
        "SELECT id, univ_name, total_quota, prioritize_enrolled
         FROM universities ORDER BY univ_name",
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
    if body.univ_name.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "대학명은 필수입니다".into()));
    }
    let total_quota = body.total_quota.flatten();
    let enrolled = body.prioritize_enrolled as i64;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name, total_quota, prioritize_enrolled)
         VALUES (?, ?, ?) RETURNING id",
    )
    .bind(body.univ_name.trim())
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
        let v = v.trim().to_string();
        if v.is_empty() {
            return Err((StatusCode::BAD_REQUEST, "대학명은 필수입니다".into()));
        }
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
    // applications.track_id FK → univ_tracks 에 CASCADE 없음.
    // 지원 기록이 있으면 FK 위반으로 500이 나므로 친화적 409로 먼저 차단.
    let app_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM applications a
         JOIN univ_tracks ut ON ut.id = a.track_id
         WHERE ut.univ_id = ?",
    )
    .bind(id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if app_count > 0 {
        return Err((
            StatusCode::CONFLICT,
            format!("지원 기록 {}건이 있어 대학을 삭제할 수 없습니다.", app_count),
        ));
    }

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
        "SELECT id, univ_id, track_name, unit_quota, prioritize_enrolled
         FROM univ_tracks WHERE univ_id = ? ORDER BY track_name",
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
        "SELECT ut.id, ut.univ_id, u.univ_name, u.total_quota, ut.track_name, ut.unit_quota, ut.prioritize_enrolled
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
    if body.track_name.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "모집단위명은 필수입니다".into()));
    }
    let unit_quota = body.unit_quota.flatten();
    let enrolled = body.prioritize_enrolled as i64;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name, unit_quota, prioritize_enrolled)
         VALUES (?, ?, ?, ?) RETURNING id",
    )
    .bind(univ_id)
    .bind(body.track_name.trim())
    .bind(unit_quota)
    .bind(enrolled)
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
        let v = v.trim().to_string();
        if v.is_empty() {
            return Err((StatusCode::BAD_REQUEST, "모집단위명은 필수입니다".into()));
        }
        sqlx::query("UPDATE univ_tracks SET track_name = ? WHERE id = ?")
            .bind(v).bind(id).execute(&state.db).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    if let Some(v) = body.unit_quota {
        sqlx::query("UPDATE univ_tracks SET unit_quota = ? WHERE id = ?")
            .bind(v).bind(id).execute(&state.db).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    if let Some(v) = body.prioritize_enrolled {
        sqlx::query("UPDATE univ_tracks SET prioritize_enrolled = ? WHERE id = ?")
            .bind(v as i64).bind(id).execute(&state.db).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE /api/univ-tracks/:id
pub async fn delete_track(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    // applications.track_id FK → univ_tracks 에 CASCADE 없음.
    // 지원 기록이 있으면 FK 위반으로 500이 나므로 친화적 409로 먼저 차단.
    let app_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM applications WHERE track_id = ?",
    )
    .bind(id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if app_count > 0 {
        return Err((
            StatusCode::CONFLICT,
            format!("지원 기록 {}건이 있어 모집단위를 삭제할 수 없습니다.", app_count),
        ));
    }

    sqlx::query("DELETE FROM univ_tracks WHERE id = ?")
        .bind(id).execute(&state.db).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

// ── 잔여석 통계 ──────────────────────────────────────────────────

#[derive(Serialize)]
pub struct RoundCount {
    pub round_id: i64,
    pub count: i64,
}

#[derive(Serialize)]
pub struct TrackStat {
    pub track_id: i64,
    pub track_name: String,
    pub unit_quota: Option<i64>,
    pub unit_used: i64,
    pub by_round: Vec<RoundCount>,
}

#[derive(Serialize)]
pub struct UnivStat {
    pub univ_id: i64,
    pub univ_name: String,
    pub total_quota: Option<i64>,
    pub total_used: i64,
    pub tracks: Vec<TrackStat>,
}

#[derive(Serialize)]
pub struct QuotaStatsResponse {
    pub all_round_ids: Vec<i64>,
    pub univs: Vec<UnivStat>,
}

async fn fetch_quota_stats(db: &sqlx::SqlitePool) -> Result<QuotaStatsResponse, String> {
    #[derive(sqlx::FromRow)]
    struct UnivBasic { id: i64, univ_name: String, total_quota: Option<i64> }
    let univs_basic: Vec<UnivBasic> = sqlx::query_as(
        "SELECT id, univ_name, total_quota FROM universities ORDER BY univ_name",
    )
    .fetch_all(db).await.map_err(|e| e.to_string())?;

    #[derive(sqlx::FromRow)]
    struct TrackStatRow { id: i64, univ_id: i64, track_name: String, unit_quota: Option<i64>, unit_used: i64 }
    let track_rows: Vec<TrackStatRow> = sqlx::query_as(
        "SELECT ut.id, ut.univ_id, ut.track_name, ut.unit_quota,
                (SELECT COUNT(*) FROM results r
                 JOIN applications a ON a.student_id = r.student_id
                                    AND a.track_id  = r.track_id
                                    AND a.round_id  = r.round_id
                 WHERE r.track_id = ut.id AND r.recommended = 1 AND a.abandoned = 0
                ) AS unit_used
         FROM univ_tracks ut
         ORDER BY ut.univ_id, ut.track_name",
    )
    .fetch_all(db).await.map_err(|e| e.to_string())?;

    #[derive(sqlx::FromRow)]
    struct RoundRow { track_id: i64, round_id: i64, count: i64 }
    let round_rows: Vec<RoundRow> = sqlx::query_as(
        "SELECT r.track_id, r.round_id, COUNT(*) AS count
         FROM results r
         JOIN applications a ON a.student_id = r.student_id
                             AND a.track_id  = r.track_id
                             AND a.round_id  = r.round_id
         WHERE r.recommended = 1 AND a.abandoned = 0
         GROUP BY r.track_id, r.round_id
         ORDER BY r.track_id, r.round_id",
    )
    .fetch_all(db).await.map_err(|e| e.to_string())?;

    let mut all_round_ids: Vec<i64> = round_rows.iter().map(|r| r.round_id).collect();
    all_round_ids.sort_unstable();
    all_round_ids.dedup();

    // track_id → Vec<RoundCount>
    let mut by_round_map: HashMap<i64, Vec<RoundCount>> = HashMap::new();
    for row in round_rows {
        by_round_map.entry(row.track_id).or_default().push(RoundCount {
            round_id: row.round_id,
            count: row.count,
        });
    }

    // univ_id → Vec<TrackStat>
    let mut univ_track_map: HashMap<i64, Vec<TrackStat>> = HashMap::new();
    for row in track_rows {
        let by_round = by_round_map.remove(&row.id).unwrap_or_default();
        univ_track_map.entry(row.univ_id).or_default().push(TrackStat {
            track_id: row.id,
            track_name: row.track_name,
            unit_quota: row.unit_quota,
            unit_used: row.unit_used,
            by_round,
        });
    }

    let mut univs: Vec<UnivStat> = Vec::new();
    for u in univs_basic {
        let tracks = univ_track_map.remove(&u.id).unwrap_or_default();
        let total_used: i64 = tracks.iter().map(|t| t.unit_used).sum();
        univs.push(UnivStat {
            univ_id: u.id,
            univ_name: u.univ_name,
            total_quota: u.total_quota,
            total_used,
            tracks,
        });
    }

    Ok(QuotaStatsResponse { all_round_ids, univs })
}

/// GET /api/universities/quota-stats
pub async fn get_quota_stats(
    State(state): State<AppState>,
) -> Result<Json<QuotaStatsResponse>, ApiError> {
    let stats = fetch_quota_stats(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(stats))
}

/// GET /api/universities/quota-stats/export?univ_id=X
/// univ_id 지정 시 해당 대학만, 미지정 시 전체 내보내기
#[derive(Deserialize)]
pub struct ExportQuotaQuery {
    pub univ_id: Option<i64>,
}

pub async fn export_quota_stats(
    State(state): State<AppState>,
    Query(q): Query<ExportQuotaQuery>,
) -> Result<Response, ApiError> {
    let stats = fetch_quota_stats(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let filtered: Vec<&UnivStat> = if let Some(uid) = q.univ_id {
        stats.univs.iter().filter(|u| u.univ_id == uid).collect()
    } else {
        stats.univs.iter().collect()
    };

    let filename = if let Some(_uid) = q.univ_id {
        let univ_name = filtered.first()
            .map(|u| {
                u.univ_name.chars()
                    .map(|c| if "/\\:*?\"<>|".contains(c) { '_' } else { c })
                    .collect::<String>()
            })
            .unwrap_or_else(|| "대학".to_string());
        format!("{}_추천현황_{}.xlsx", univ_name, excel::now_tag())
    } else {
        format!("전체_추천현황_{}.xlsx", excel::now_tag())
    };

    let round_labels: Vec<String> = stats.all_round_ids.iter().enumerate()
        .map(|(i, _)| format!("{}차 추천", i + 1))
        .collect();

    let mut wb = Workbook::new();
    let ws = wb.add_worksheet()
        .set_name("추천현황")
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let fixed = ["대학명", "모집단위", "모집단위 정원", "추천인원", "잔여인원",
                 "대학 전체 정원", "대학 추천인원", "대학 잔여인원"];
    let mut col = 0u16;
    for h in &fixed { ws.write_string(0, col, *h).map_err(excel::xlsx_err)?; col += 1; }
    for label in &round_labels { ws.write_string(0, col, label).map_err(excel::xlsx_err)?; col += 1; }

    let mut row = 1u32;
    for u in &filtered {
        for t in &u.tracks {
            let mut col = 0u16;
            ws.write_string(row, col, &u.univ_name).map_err(excel::xlsx_err)?; col += 1;
            ws.write_string(row, col, &t.track_name).map_err(excel::xlsx_err)?; col += 1;

            match t.unit_quota {
                Some(q) => { ws.write_number(row, col, q as f64).map_err(excel::xlsx_err)?; }
                None    => { ws.write_string(row, col, "무제한").map_err(excel::xlsx_err)?; }
            }
            col += 1;
            ws.write_number(row, col, t.unit_used as f64).map_err(excel::xlsx_err)?; col += 1;
            match t.unit_quota {
                Some(q) => { ws.write_number(row, col, (q - t.unit_used).max(0) as f64).map_err(excel::xlsx_err)?; }
                None    => { ws.write_string(row, col, "무제한").map_err(excel::xlsx_err)?; }
            }
            col += 1;

            match u.total_quota {
                Some(q) => { ws.write_number(row, col, q as f64).map_err(excel::xlsx_err)?; }
                None    => { ws.write_string(row, col, "무제한").map_err(excel::xlsx_err)?; }
            }
            col += 1;
            ws.write_number(row, col, u.total_used as f64).map_err(excel::xlsx_err)?; col += 1;
            match u.total_quota {
                Some(q) => { ws.write_number(row, col, (q - u.total_used).max(0) as f64).map_err(excel::xlsx_err)?; }
                None    => { ws.write_string(row, col, "무제한").map_err(excel::xlsx_err)?; }
            }
            col += 1;

            let by_round_lookup: HashMap<i64, i64> = t.by_round.iter()
                .map(|r| (r.round_id, r.count)).collect();
            for rid in &stats.all_round_ids {
                let cnt = by_round_lookup.get(rid).copied().unwrap_or(0);
                ws.write_number(row, col, cnt as f64).map_err(excel::xlsx_err)?;
                col += 1;
            }

            row += 1;
        }
    }

    let buf = wb.save_to_buffer()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(excel::xlsx_response(buf, &filename))
}

// ── 모집단위 추천 확정 학생 목록 ─────────────────────────────────

#[derive(Serialize, FromRow)]
pub struct RecommendedEntry {
    pub round_id: i64,
    pub student_id: i64,
    pub student_code: String,
    pub name: String,
    pub grade: Option<i64>,
    pub class_no: Option<i64>,
    pub seq_no: Option<i64>,
    pub is_enrolled: bool,
    pub abandoned: bool,
    pub ranking: Option<i64>,
}

/// GET /api/univ-tracks/:id/recommended-list
pub async fn get_track_recommended_list(
    State(state): State<AppState>,
    Path(track_id): Path<i64>,
) -> Result<Json<Vec<RecommendedEntry>>, ApiError> {
    let rows = sqlx::query_as::<_, RecommendedEntry>(
        "SELECT r.round_id, r.student_id, s.student_code, s.name,
                s.grade, s.class_no, s.seq_no, s.is_enrolled,
                a.abandoned, r.ranking
         FROM results r
         JOIN students s ON s.id = r.student_id
         JOIN applications a ON a.student_id = r.student_id
                             AND a.track_id  = r.track_id
                             AND a.round_id  = r.round_id
         WHERE r.track_id = ? AND r.recommended = 1
         ORDER BY r.round_id, r.ranking NULLS LAST, s.name",
    )
    .bind(track_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(rows))
}
