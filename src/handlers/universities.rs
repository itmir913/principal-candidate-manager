use axum::{
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
    response::Response,
    Json,
};
use rust_xlsxwriter::{DataValidation, Workbook};
use serde::{Deserialize, Deserializer, Serialize};
use sqlx::FromRow;
use std::collections::{HashMap, HashSet};

use crate::{
    audit::{self, Actor, AuditEntry},
    enums::AuditAction,
    excel,
    middleware::multipart_err,
    state::AppState,
};

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

// ── 재학생 우선 변경 가드 ────────────────────────────────────────
// results.ranking(대학 순위)은 close_round 시점에 저장된 값이고, 화면·자동 추천의
// 모집단위 순위(track_rank)는 실행 시점 라이브 계산이다. CLOSED 라운드가 있는 동안
// prioritize_enrolled 를 바꾸면 두 값의 기준 시점이 어긋나 라이브 기준 동점자 중
// 일부만 추천되는 결과가 나온다. 정원(total_quota/unit_quota)은 저장 순위에
// 영향이 없으므로 계속 허용한다.

async fn guard_prioritize_change_closed(
    tx: &mut sqlx::SqliteConnection,
) -> Result<(), ApiError> {
    let closed: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM rounds WHERE status = 'CLOSED' ORDER BY id",
    )
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if closed.is_empty() {
        return Ok(());
    }
    let labels = closed.iter().map(|id| format!("{}차", id)).collect::<Vec<_>>().join(", ");
    Err((
        StatusCode::CONFLICT,
        format!(
            "마감된 라운드({})가 있어 재학생 우선 설정을 바꿀 수 없습니다. \
             저장된 대학 순위는 마감 시점 기준이므로 설정만 바꾸면 순위와 어긋납니다. \
             변경하려면 해당 라운드를 다시 열고(재오픈) 설정을 바꾼 뒤 다시 마감하세요 \
             (마감 시 순위가 재계산됩니다). 정원 변경은 지금도 가능합니다.",
            labels
        ),
    ))
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
    let univ_name = body.univ_name.trim().to_string();
    let total_quota = body.total_quota.flatten();
    let enrolled = body.prioritize_enrolled as i64;
    let mut tx = state.db.begin().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name, total_quota, prioritize_enrolled)
         VALUES (?, ?, ?) RETURNING id",
    )
    .bind(&univ_name)
    .bind(total_quota)
    .bind(enrolled)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    audit::log(&mut *tx, AuditEntry {
        actor: Actor::Admin,
        action: AuditAction::UniversityCreated,
        round_id: None,
        student_id: None,
        detail: serde_json::json!({ "univ_name": univ_name }),
    }).await?;
    tx.commit().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

/// PUT /api/universities/:id
pub async fn update_university(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateUnivBody>,
) -> Result<StatusCode, ApiError> {
    // 변경 필드가 하나도 없는 요청은 거부 — 아무것도 바꾸지 않는 UNIVERSITY_UPDATED 감사 로그를 남기지 않는다
    if body.univ_name.is_none() && body.total_quota.is_none() && body.prioritize_enrolled.is_none() {
        return Err((StatusCode::BAD_REQUEST, "수정할 내용이 없습니다".into()));
    }
    if let Some(v) = &body.univ_name {
        if v.trim().is_empty() {
            return Err((StatusCode::BAD_REQUEST, "대학명은 필수입니다".into()));
        }
    }
    let mut tx = state.db.begin().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if let Some(v) = body.univ_name {
        sqlx::query("UPDATE universities SET univ_name = ? WHERE id = ?")
            .bind(v.trim().to_string()).bind(id).execute(&mut *tx).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    if let Some(v) = body.total_quota {
        sqlx::query("UPDATE universities SET total_quota = ? WHERE id = ?")
            .bind(v).bind(id).execute(&mut *tx).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    if let Some(v) = body.prioritize_enrolled {
        // 값이 실제로 바뀔 때만 가드 — 이름/정원만 고치는 요청(폼이 전 필드를 함께 보낸다)은 통과시킨다
        let current: bool = sqlx::query_scalar(
            "SELECT prioritize_enrolled = 1 FROM universities WHERE id = ?",
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        if current != v {
            // 대학 UPDATE 를 막으면 트리거의 트랙 cascade 도 함께 차단된다
            guard_prioritize_change_closed(&mut *tx).await?;
            sqlx::query("UPDATE universities SET prioritize_enrolled = ? WHERE id = ?")
                .bind(v as i64).bind(id).execute(&mut *tx).await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        }
    }
    let univ_name: String = sqlx::query_scalar("SELECT univ_name FROM universities WHERE id = ?")
        .bind(id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    audit::log(&mut *tx, AuditEntry {
        actor: Actor::Admin,
        action: AuditAction::UniversityUpdated,
        round_id: None,
        student_id: None,
        detail: serde_json::json!({ "univ_name": univ_name }),
    }).await?;
    tx.commit().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE /api/universities/:id
pub async fn delete_university(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    let mut tx = state.db.begin().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // applications.track_id FK → univ_tracks 에 CASCADE 없음.
    // 지원 기록이 있으면 FK 위반으로 500이 나므로 친화적 409로 먼저 차단.
    let app_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM applications a
         JOIN univ_tracks ut ON ut.id = a.track_id
         WHERE ut.univ_id = ?",
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if app_count > 0 {
        return Err((
            StatusCode::CONFLICT,
            format!("지원 기록 {}건이 있어 대학을 삭제할 수 없습니다.", app_count),
        ));
    }

    // 삭제 전 이름 스냅샷 — 대상이 없으면 404 (없는 대상의 삭제 로그를 남기지 않는다)
    let univ_name: String = sqlx::query_scalar(
        "SELECT univ_name FROM universities WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, "대학을 찾을 수 없습니다".to_string()))?;

    sqlx::query("DELETE FROM universities WHERE id = ?")
        .bind(id).execute(&mut *tx).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    audit::log(&mut *tx, AuditEntry {
        actor: Actor::Admin,
        action: AuditAction::UniversityDeleted,
        round_id: None,
        student_id: None,
        detail: serde_json::json!({ "univ_name": univ_name }),
    }).await?;

    tx.commit().await
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
    let track_name = body.track_name.trim().to_string();
    let unit_quota = body.unit_quota.flatten();
    let enrolled = body.prioritize_enrolled as i64;
    let mut tx = state.db.begin().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    // 불변식 가드: 대학=1이면 트랙도 반드시 재학생 우선
    if !body.prioritize_enrolled {
        let univ_prioritize: bool = sqlx::query_scalar(
            "SELECT prioritize_enrolled = 1 FROM universities WHERE id = ?",
        )
        .bind(univ_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        if univ_prioritize {
            return Err((StatusCode::BAD_REQUEST, "재학생 우선 대학의 모집단위는 재학생 우선이어야 합니다".into()));
        }
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name, unit_quota, prioritize_enrolled)
         VALUES (?, ?, ?, ?) RETURNING id",
    )
    .bind(univ_id)
    .bind(&track_name)
    .bind(unit_quota)
    .bind(enrolled)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let univ_name: String = sqlx::query_scalar("SELECT univ_name FROM universities WHERE id = ?")
        .bind(univ_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    audit::log(&mut *tx, AuditEntry {
        actor: Actor::Admin,
        action: AuditAction::TrackCreated,
        round_id: None,
        student_id: None,
        detail: serde_json::json!({ "univ_name": univ_name, "track_name": track_name }),
    }).await?;
    tx.commit().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

/// PUT /api/univ-tracks/:id
pub async fn update_track(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<UpdateTrackBody>,
) -> Result<StatusCode, ApiError> {
    // 변경 필드가 하나도 없는 요청은 거부 — 아무것도 바꾸지 않는 TRACK_UPDATED 감사 로그를 남기지 않는다
    if body.track_name.is_none() && body.unit_quota.is_none() && body.prioritize_enrolled.is_none() {
        return Err((StatusCode::BAD_REQUEST, "수정할 내용이 없습니다".into()));
    }
    if let Some(v) = &body.track_name {
        if v.trim().is_empty() {
            return Err((StatusCode::BAD_REQUEST, "모집단위명은 필수입니다".into()));
        }
    }
    let mut tx = state.db.begin().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if let Some(v) = body.track_name {
        sqlx::query("UPDATE univ_tracks SET track_name = ? WHERE id = ?")
            .bind(v.trim().to_string()).bind(id).execute(&mut *tx).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    if let Some(v) = body.unit_quota {
        sqlx::query("UPDATE univ_tracks SET unit_quota = ? WHERE id = ?")
            .bind(v).bind(id).execute(&mut *tx).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    if let Some(v) = body.prioritize_enrolled {
        let current: bool = sqlx::query_scalar(
            "SELECT prioritize_enrolled = 1 FROM univ_tracks WHERE id = ?",
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        // 불변식 가드: 대학=1이면 트랙 0으로 다운그레이드 금지
        if !v {
            let univ_prioritize: bool = sqlx::query_scalar(
                "SELECT u.prioritize_enrolled = 1
                 FROM univ_tracks ut JOIN universities u ON u.id = ut.univ_id
                 WHERE ut.id = ?",
            )
            .bind(id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            if univ_prioritize {
                return Err((StatusCode::BAD_REQUEST, "재학생 우선 대학의 모집단위는 재학생 우선을 해제할 수 없습니다".into()));
            }
        }
        // 값이 실제로 바뀔 때만 가드 (대학 쪽과 동일 기준)
        if current != v {
            guard_prioritize_change_closed(&mut *tx).await?;
            sqlx::query("UPDATE univ_tracks SET prioritize_enrolled = ? WHERE id = ?")
                .bind(v as i64).bind(id).execute(&mut *tx).await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        }
    }
    let (univ_name, track_name): (String, String) = sqlx::query_as(
        "SELECT u.univ_name, ut.track_name
         FROM univ_tracks ut
         JOIN universities u ON u.id = ut.univ_id
         WHERE ut.id = ?",
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    audit::log(&mut *tx, AuditEntry {
        actor: Actor::Admin,
        action: AuditAction::TrackUpdated,
        round_id: None,
        student_id: None,
        detail: serde_json::json!({ "univ_name": univ_name, "track_name": track_name }),
    }).await?;
    tx.commit().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE /api/univ-tracks/:id
pub async fn delete_track(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    let mut tx = state.db.begin().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // applications.track_id FK → univ_tracks 에 CASCADE 없음.
    // 지원 기록이 있으면 FK 위반으로 500이 나므로 친화적 409로 먼저 차단.
    let app_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM applications WHERE track_id = ?",
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if app_count > 0 {
        return Err((
            StatusCode::CONFLICT,
            format!("지원 기록 {}건이 있어 모집단위를 삭제할 수 없습니다.", app_count),
        ));
    }

    // 삭제 전 이름 스냅샷 — 대상이 없으면 404 (없는 대상의 삭제 로그를 남기지 않는다)
    let (univ_name, track_name): (String, String) = sqlx::query_as(
        "SELECT u.univ_name, ut.track_name
         FROM univ_tracks ut
         JOIN universities u ON u.id = ut.univ_id
         WHERE ut.id = ?",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, "모집단위를 찾을 수 없습니다".to_string()))?;

    sqlx::query("DELETE FROM univ_tracks WHERE id = ?")
        .bind(id).execute(&mut *tx).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    audit::log(&mut *tx, AuditEntry {
        actor: Actor::Admin,
        action: AuditAction::TrackDeleted,
        round_id: None,
        student_id: None,
        detail: serde_json::json!({ "univ_name": univ_name, "track_name": track_name }),
    }).await?;

    tx.commit().await
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

// ════════════════════════════════════════════════════════════════════
//  대학·모집단위 설정 일괄 Import·Export
//  (대학 정원·모집단위 정원·재학생 우선을 Excel로 왕복 편집)
//
//  UPSERT 전용 — 파일에 없는 대학/모집단위는 건드리지 않는다(삭제 아님, CLAUDE.md §2).
//  식별자는 이름: univ_name / (univ_id, track_name).
//  import 는 All-or-Nothing (규칙 4): 오류 하나라도 rollback + 422.
//  마감 라운드 중에는 재학생 우선 변경만 차단(guard_prioritize_change_closed),
//  정원 변경은 허용.
// ════════════════════════════════════════════════════════════════════

/// 설정 시트 헤더. 파싱은 이 이름 기반(규칙 5) — 열 순서에 의존하지 않는다.
const SETTINGS_HEADERS: &[&str] = &[
    "대학명", "대학 정원", "대학 재학생우선",
    "모집단위명", "모집단위 정원", "모집단위 재학생우선",
];

/// 재학생우선 열의 열 인덱스(0-기반). 예/아니오 드롭다운을 거는 위치.
const PRIO_COLS: [u16; 2] = [2, 5];
/// 정원 열의 열 인덱스. 숫자 또는 "무제한" 입력 안내를 거는 위치.
const QUOTA_COLS: [u16; 2] = [1, 4];
/// 데이터 검증을 적용할 최대 행(넉넉히). 헤더 다음 행부터.
const DV_LAST_ROW: u32 = 5000;

/// 정원 셀 표기: NULL(무제한) → "무제한", 값 → 숫자.
/// export 와 import 가 대칭이 되도록 이 한 곳에서만 규약을 정의한다.
const UNLIMITED_TEXT: &str = "무제한";

fn fmt_prio(v: bool) -> &'static str {
    if v { "예" } else { "아니오" }
}

/// 정원 문자열 파싱: "무제한" → None, 양의 정수 → Some, 그 외 → Err.
/// 빈 문자열 처리는 호출자 몫(대학 정원=필수, 트랙 없는 행=빈 칸 허용).
fn parse_quota(s: &str) -> Result<Option<i64>, String> {
    if s == UNLIMITED_TEXT {
        return Ok(None);
    }
    match s.parse::<i64>() {
        Ok(n) if n >= 1 => Ok(Some(n)),
        _ => Err(format!(
            "정원 '{}' 인식 불가 — 1 이상 정수 또는 '{}'만 허용됩니다",
            s, UNLIMITED_TEXT
        )),
    }
}

/// 재학생우선 셀 파싱: "예" → true, "아니오" → false, 그 외 → Err.
fn parse_prio(s: &str) -> Result<bool, String> {
    match s {
        "예" => Ok(true),
        "아니오" => Ok(false),
        _ => Err(format!(
            "재학생우선 '{}' 인식 불가 — '예' 또는 '아니오'만 허용됩니다",
            s
        )),
    }
}

/// 파일에서 파싱한 한 대학의 목표 상태.
struct UnivSpec {
    univ_name: String,
    total_quota: Option<i64>,
    prioritize: bool,
    tracks: Vec<TrackSpec>,
}

struct TrackSpec {
    track_name: String,
    unit_quota: Option<i64>,
    prioritize: bool,
}

// ── 파일 → UnivSpec 파싱 + 검증 (DB 접근 없음, 규칙 4 검증 단일 출처) ──

/// 데이터 행을 파싱해 대학별로 묶는다. 오류가 하나라도 있으면 Err(오류 목록).
/// preview 와 import 가 **같은 함수**를 통과하므로 검증이 갈라지지 않는다.
fn parse_settings(
    headers: &[String],
    rows: &[Vec<String>],
) -> Result<Vec<UnivSpec>, Vec<String>> {
    let col = excel::col_map(headers);
    if let Err(e) = excel::require_cols(&col, SETTINGS_HEADERS) {
        return Err(vec![e]);
    }
    if rows.is_empty() {
        return Err(vec!["가져올 데이터 행이 없습니다 — 헤더만 있는 빈 파일입니다".into()]);
    }

    let mut errors: Vec<String> = Vec::new();

    // 대학명 → (정원, 재학생우선, 최초 등장 행, 트랙들)
    struct Acc {
        total_quota: Option<i64>,
        prioritize: bool,
        first_row: usize,
        tracks: Vec<TrackSpec>,
        track_names: HashSet<String>,
    }
    // 등장 순서 유지를 위해 별도 Vec + 인덱스 맵
    let mut order: Vec<String> = Vec::new();
    let mut accs: HashMap<String, Acc> = HashMap::new();

    for (idx, cols) in rows.iter().enumerate() {
        let row_num = idx + 2; // 헤더가 1행
        let get = |name: &str| excel::get_col(cols, &col, name);

        let univ_name = get("대학명").to_string();
        if univ_name.is_empty() {
            errors.push(format!("{}행: 대학명이 비어 있습니다", row_num));
            continue;
        }

        // 대학 정원(필수) / 재학생우선(필수)
        let univ_quota_s = get("대학 정원");
        let univ_quota = if univ_quota_s.is_empty() {
            errors.push(format!("{}행: 대학 정원이 비어 있습니다 (숫자 또는 '{}')", row_num, UNLIMITED_TEXT));
            None
        } else {
            match parse_quota(univ_quota_s) {
                Ok(v) => Some(v),
                Err(e) => { errors.push(format!("{}행: {}", row_num, e)); None }
            }
        };
        let univ_prio_s = get("대학 재학생우선");
        let univ_prio = if univ_prio_s.is_empty() {
            errors.push(format!("{}행: 대학 재학생우선이 비어 있습니다 ('예' 또는 '아니오')", row_num));
            None
        } else {
            match parse_prio(univ_prio_s) {
                Ok(v) => Some(v),
                Err(e) => { errors.push(format!("{}행: {}", row_num, e)); None }
            }
        };

        // 모집단위(선택) — 이름 없으면 "대학만" 행. 이름 있으면 정원·재학생우선 필수.
        let track_name = get("모집단위명").to_string();
        let track_quota_s = get("모집단위 정원");
        let track_prio_s = get("모집단위 재학생우선");

        let track: Option<TrackSpec> = if track_name.is_empty() {
            // 모집단위명 빈 행 — 정원/재학생우선도 함께 비어야 한다(모순 방지, §8)
            if !track_quota_s.is_empty() || !track_prio_s.is_empty() {
                errors.push(format!(
                    "{}행: 모집단위명이 비었는데 모집단위 정원·재학생우선이 채워져 있습니다 \
                     (모집단위 없는 대학 행이면 세 칸을 모두 비우세요)",
                    row_num
                ));
            }
            None
        } else {
            let tq = if track_quota_s.is_empty() {
                errors.push(format!("{}행: 모집단위 정원이 비어 있습니다 (숫자 또는 '{}')", row_num, UNLIMITED_TEXT));
                None
            } else {
                match parse_quota(track_quota_s) {
                    Ok(v) => Some(v),
                    Err(e) => { errors.push(format!("{}행: {}", row_num, e)); None }
                }
            };
            let tp = if track_prio_s.is_empty() {
                errors.push(format!("{}행: 모집단위 재학생우선이 비어 있습니다 ('예' 또는 '아니오')", row_num));
                None
            } else {
                match parse_prio(track_prio_s) {
                    Ok(v) => Some(v),
                    Err(e) => { errors.push(format!("{}행: {}", row_num, e)); None }
                }
            };
            match (tq, tp) {
                (Some(q), Some(p)) => Some(TrackSpec { track_name: track_name.clone(), unit_quota: q, prioritize: p }),
                _ => None, // 위에서 이미 오류 기록
            }
        };

        // 대학 단위 값이 확정되지 않았으면(오류) 누적하지 않는다
        let (Some(uq), Some(up)) = (univ_quota, univ_prio) else { continue };

        match accs.get_mut(&univ_name) {
            None => {
                order.push(univ_name.clone());
                let mut track_names = HashSet::new();
                let mut tracks = Vec::new();
                if let Some(t) = track {
                    track_names.insert(t.track_name.clone());
                    tracks.push(t);
                }
                accs.insert(univ_name.clone(), Acc {
                    total_quota: uq, prioritize: up, first_row: row_num, tracks, track_names,
                });
            }
            Some(acc) => {
                // 같은 대학의 반복 대학값 불일치 → 오류(§6)
                if acc.total_quota != uq || acc.prioritize != up {
                    errors.push(format!(
                        "{}행: 대학 '{}'의 대학 정원·재학생우선이 {}행과 다릅니다 \
                         (같은 대학은 모든 행에서 같은 값이어야 합니다)",
                        row_num, univ_name, acc.first_row
                    ));
                }
                if let Some(t) = track {
                    if !acc.track_names.insert(t.track_name.clone()) {
                        errors.push(format!(
                            "{}행: 모집단위 '{}/{}' 중복 — 파일에 같은 모집단위가 두 번 이상 있습니다",
                            row_num, univ_name, t.track_name
                        ));
                    } else {
                        acc.tracks.push(t);
                    }
                }
            }
        }
    }

    // 불변식(§3): 대학=예 이면 그 대학 파일 내 모든 모집단위=예
    for name in &order {
        let acc = &accs[name];
        if acc.prioritize {
            for t in &acc.tracks {
                if !t.prioritize {
                    errors.push(format!(
                        "대학 '{}'의 재학생우선이 '예'인데 모집단위 '{}'가 '아니오'입니다 \
                         (재학생 우선 대학의 모집단위는 모두 '예'여야 합니다)",
                        name, t.track_name
                    ));
                }
            }
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    Ok(order.into_iter().map(|name| {
        let acc = accs.remove(&name).unwrap();
        UnivSpec { univ_name: name, total_quota: acc.total_quota, prioritize: acc.prioritize, tracks: acc.tracks }
    }).collect())
}

// ── diff 계산 (읽기 전용, preview 용) ─────────────────────────────

#[derive(Serialize)]
pub struct FieldChange {
    pub field: String,
    pub old: String,
    pub new: String,
}

#[derive(Serialize)]
pub struct SettingsDiffEntry {
    pub kind: String,   // "univ" | "track" | "cascade"
    pub op: String,     // "create" | "update"
    pub univ_name: String,
    pub track_name: Option<String>,
    pub fields: Vec<FieldChange>,
    pub blocked: bool,  // 마감 라운드로 차단되는 재학생우선 변경 포함
}

#[derive(Serialize)]
pub struct SettingsPreview {
    pub errors: Vec<String>,
    pub changes: Vec<SettingsDiffEntry>,
    pub unchanged_count: usize,
    pub closed_round_labels: Vec<String>,
    pub has_blocked: bool,
}

fn quota_display(v: Option<i64>) -> String {
    match v {
        Some(n) => n.to_string(),
        None => UNLIMITED_TEXT.to_string(),
    }
}

async fn closed_round_labels(db: &sqlx::SqlitePool) -> Result<Vec<String>, ApiError> {
    let ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM rounds WHERE status = 'CLOSED' ORDER BY id")
        .fetch_all(db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(ids.iter().map(|id| format!("{}차", id)).collect())
}

/// 현재 DB 상태와 대비해 diff 를 만든다. **쓰기 없음.**
async fn compute_settings_changes(
    db: &sqlx::SqlitePool,
    specs: &[UnivSpec],
) -> Result<SettingsPreview, ApiError> {
    let labels = closed_round_labels(db).await?;
    let closed = !labels.is_empty();

    let mut changes: Vec<SettingsDiffEntry> = Vec::new();
    let mut unchanged = 0usize;

    #[derive(FromRow)]
    struct CurUniv { id: i64, total_quota: Option<i64>, prioritize_enrolled: i64 }
    #[derive(FromRow)]
    struct CurTrack { track_name: String, unit_quota: Option<i64>, prioritize_enrolled: i64 }

    for u in specs {
        let cur = sqlx::query_as::<_, CurUniv>(
            "SELECT id, total_quota, prioritize_enrolled FROM universities WHERE univ_name = ?",
        )
        .bind(&u.univ_name)
        .fetch_optional(db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        match cur {
            None => {
                // 신규 대학 — 생성 (마감 가드 대상 아님: 저장된 순위가 참조할 수 없음)
                changes.push(SettingsDiffEntry {
                    kind: "univ".into(), op: "create".into(),
                    univ_name: u.univ_name.clone(), track_name: None,
                    fields: vec![
                        FieldChange { field: "정원".into(), old: "—".into(), new: quota_display(u.total_quota) },
                        FieldChange { field: "재학생우선".into(), old: "—".into(), new: fmt_prio(u.prioritize).into() },
                    ],
                    blocked: false,
                });
                for t in &u.tracks {
                    changes.push(SettingsDiffEntry {
                        kind: "track".into(), op: "create".into(),
                        univ_name: u.univ_name.clone(), track_name: Some(t.track_name.clone()),
                        fields: vec![
                            FieldChange { field: "정원".into(), old: "—".into(), new: quota_display(t.unit_quota) },
                            FieldChange { field: "재학생우선".into(), old: "—".into(), new: fmt_prio(t.prioritize).into() },
                        ],
                        blocked: false,
                    });
                }
            }
            Some(cu) => {
                let cur_prio = cu.prioritize_enrolled == 1;
                let mut fields = Vec::new();
                if cu.total_quota != u.total_quota {
                    fields.push(FieldChange { field: "정원".into(), old: quota_display(cu.total_quota), new: quota_display(u.total_quota) });
                }
                let univ_prio_changed = cur_prio != u.prioritize;
                if univ_prio_changed {
                    fields.push(FieldChange { field: "재학생우선".into(), old: fmt_prio(cur_prio).into(), new: fmt_prio(u.prioritize).into() });
                }
                let univ_blocked = closed && univ_prio_changed;
                if fields.is_empty() {
                    unchanged += 1;
                } else {
                    changes.push(SettingsDiffEntry {
                        kind: "univ".into(), op: "update".into(),
                        univ_name: u.univ_name.clone(), track_name: None,
                        fields, blocked: univ_blocked,
                    });
                }

                // 기존 트랙 스냅샷
                let cur_tracks = sqlx::query_as::<_, CurTrack>(
                    "SELECT track_name, unit_quota, prioritize_enrolled FROM univ_tracks WHERE univ_id = ?",
                )
                .bind(cu.id)
                .fetch_all(db)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
                let cur_map: HashMap<&str, &CurTrack> = cur_tracks.iter().map(|t| (t.track_name.as_str(), t)).collect();
                let file_names: HashSet<&str> = u.tracks.iter().map(|t| t.track_name.as_str()).collect();

                for t in &u.tracks {
                    match cur_map.get(t.track_name.as_str()) {
                        None => {
                            changes.push(SettingsDiffEntry {
                                kind: "track".into(), op: "create".into(),
                                univ_name: u.univ_name.clone(), track_name: Some(t.track_name.clone()),
                                fields: vec![
                                    FieldChange { field: "정원".into(), old: "—".into(), new: quota_display(t.unit_quota) },
                                    FieldChange { field: "재학생우선".into(), old: "—".into(), new: fmt_prio(t.prioritize).into() },
                                ],
                                blocked: false,
                            });
                        }
                        Some(ct) => {
                            let ct_prio = ct.prioritize_enrolled == 1;
                            let mut tf = Vec::new();
                            if ct.unit_quota != t.unit_quota {
                                tf.push(FieldChange { field: "정원".into(), old: quota_display(ct.unit_quota), new: quota_display(t.unit_quota) });
                            }
                            let tprio_changed = ct_prio != t.prioritize;
                            if tprio_changed {
                                tf.push(FieldChange { field: "재학생우선".into(), old: fmt_prio(ct_prio).into(), new: fmt_prio(t.prioritize).into() });
                            }
                            if tf.is_empty() {
                                unchanged += 1;
                            } else {
                                changes.push(SettingsDiffEntry {
                                    kind: "track".into(), op: "update".into(),
                                    univ_name: u.univ_name.clone(), track_name: Some(t.track_name.clone()),
                                    fields: tf, blocked: closed && tprio_changed,
                                });
                            }
                        }
                    }
                }

                // cascade: 대학 재학생우선이 바뀌면 파일에 없는 기존 트랙도 트리거로 함께 뒤집힌다
                if univ_prio_changed {
                    for ct in &cur_tracks {
                        let ct_prio = ct.prioritize_enrolled == 1;
                        if !file_names.contains(ct.track_name.as_str()) && ct_prio != u.prioritize {
                            changes.push(SettingsDiffEntry {
                                kind: "cascade".into(), op: "update".into(),
                                univ_name: u.univ_name.clone(), track_name: Some(ct.track_name.clone()),
                                fields: vec![FieldChange {
                                    field: "재학생우선".into(),
                                    old: fmt_prio(ct_prio).into(), new: fmt_prio(u.prioritize).into(),
                                }],
                                blocked: closed, // 대학 UPDATE 가 차단되면 cascade 도 차단
                            });
                        }
                    }
                }
            }
        }
    }

    let has_blocked = changes.iter().any(|c| c.blocked);
    Ok(SettingsPreview { errors: vec![], changes, unchanged_count: unchanged, closed_round_labels: labels, has_blocked })
}

// ── 적용 (tx 쓰기) ────────────────────────────────────────────────

/// 검증된 specs 를 DB 에 반영한다. 마감 라운드 중 재학생우선 변경은
/// `guard_prioritize_change_closed` 로 차단(정원은 허용). 오류 시 호출자 tx drop 으로 rollback.
async fn apply_settings(
    tx: &mut sqlx::SqliteConnection,
    specs: &[UnivSpec],
) -> Result<(usize, usize), ApiError> {
    let mut created = 0usize;
    let mut updated = 0usize;

    for u in specs {
        let cur = sqlx::query_as::<_, (i64, Option<i64>, i64)>(
            "SELECT id, total_quota, prioritize_enrolled FROM universities WHERE univ_name = ?",
        )
        .bind(&u.univ_name)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let (univ_id, univ_is_new) = match cur {
            None => {
                let id: i64 = sqlx::query_scalar(
                    "INSERT INTO universities (univ_name, total_quota, prioritize_enrolled) VALUES (?, ?, ?) RETURNING id",
                )
                .bind(&u.univ_name).bind(u.total_quota).bind(u.prioritize as i64)
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
                created += 1;
                (id, true)
            }
            Some((id, cur_quota, cur_prio_i)) => {
                let cur_prio = cur_prio_i == 1;
                let mut changed = false;
                if cur_quota != u.total_quota {
                    sqlx::query("UPDATE universities SET total_quota = ? WHERE id = ?")
                        .bind(u.total_quota).bind(id).execute(&mut *tx).await
                        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
                    changed = true;
                }
                if cur_prio != u.prioritize {
                    // 대학 UPDATE 를 막으면 트리거의 트랙 cascade 도 함께 차단된다
                    guard_prioritize_change_closed(&mut *tx).await?;
                    sqlx::query("UPDATE universities SET prioritize_enrolled = ? WHERE id = ?")
                        .bind(u.prioritize as i64).bind(id).execute(&mut *tx).await
                        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
                    changed = true;
                }
                if changed { updated += 1; }
                (id, false)
            }
        };

        for t in &u.tracks {
            if univ_is_new {
                sqlx::query(
                    "INSERT INTO univ_tracks (univ_id, track_name, unit_quota, prioritize_enrolled) VALUES (?, ?, ?, ?)",
                )
                .bind(univ_id).bind(&t.track_name).bind(t.unit_quota).bind(t.prioritize as i64)
                .execute(&mut *tx).await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
                created += 1;
                continue;
            }
            // 기존 대학 — 트랙 조회/생성. prioritize 는 대학 cascade 이후의 라이브 값과 비교.
            let ct = sqlx::query_as::<_, (i64, Option<i64>, i64)>(
                "SELECT id, unit_quota, prioritize_enrolled FROM univ_tracks WHERE univ_id = ? AND track_name = ?",
            )
            .bind(univ_id).bind(&t.track_name)
            .fetch_optional(&mut *tx).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

            match ct {
                None => {
                    sqlx::query(
                        "INSERT INTO univ_tracks (univ_id, track_name, unit_quota, prioritize_enrolled) VALUES (?, ?, ?, ?)",
                    )
                    .bind(univ_id).bind(&t.track_name).bind(t.unit_quota).bind(t.prioritize as i64)
                    .execute(&mut *tx).await
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
                    created += 1;
                }
                Some((tid, cq, cp_i)) => {
                    let cp_live = cp_i == 1;
                    let mut changed = false;
                    if cq != t.unit_quota {
                        sqlx::query("UPDATE univ_tracks SET unit_quota = ? WHERE id = ?")
                            .bind(t.unit_quota).bind(tid).execute(&mut *tx).await
                            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
                        changed = true;
                    }
                    if cp_live != t.prioritize {
                        guard_prioritize_change_closed(&mut *tx).await?;
                        sqlx::query("UPDATE univ_tracks SET prioritize_enrolled = ? WHERE id = ?")
                            .bind(t.prioritize as i64).bind(tid).execute(&mut *tx).await
                            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
                        changed = true;
                    }
                    if changed { updated += 1; }
                }
            }
        }
    }

    Ok((created, updated))
}

// ── xlsx 생성 (template / export) ─────────────────────────────────

/// 헤더 + 데이터 검증을 새 워크시트에 심는다. 반환된 워크시트에 데이터 행을 이어 쓴다.
fn write_settings_header(ws: &mut rust_xlsxwriter::Worksheet) -> Result<(), ApiError> {
    for (i, h) in SETTINGS_HEADERS.iter().enumerate() {
        ws.write_string(0, i as u16, *h).map_err(excel::xlsx_err)?;
    }
    // 재학생우선 열: 예/아니오 드롭다운(엄격) — 이상값 입력 차단
    let prio_dv = DataValidation::new()
        .allow_list_strings(&["예", "아니오"])
        .map_err(excel::xlsx_err)?;
    for &c in &PRIO_COLS {
        ws.add_data_validation(1, c, DV_LAST_ROW, c, &prio_dv).map_err(excel::xlsx_err)?;
    }
    // 정원 열: 숫자 OR "무제한" 텍스트를 한 셀에 강제할 수 없으므로 입력 안내만(유연).
    let quota_dv = DataValidation::new()
        .ignore_blank(true)
        .set_input_title("정원 입력")
        .map_err(excel::xlsx_err)?
        .set_input_message(format!("1 이상 정수를 입력하거나 '{}'이라고 적으세요", UNLIMITED_TEXT))
        .map_err(excel::xlsx_err)?;
    for &c in &QUOTA_COLS {
        ws.add_data_validation(1, c, DV_LAST_ROW, c, &quota_dv).map_err(excel::xlsx_err)?;
    }
    Ok(())
}

/// GET /api/universities/settings/template
pub async fn settings_template() -> Result<Response, ApiError> {
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet().set_name("대학설정")
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    write_settings_header(ws)?;

    // 샘플 1: 모집단위가 있는 대학
    ws.write_string(1, 0, "한국대학교").map_err(excel::xlsx_err)?;
    ws.write_number(1, 1, 5.0).map_err(excel::xlsx_err)?;
    ws.write_string(1, 2, "아니오").map_err(excel::xlsx_err)?;
    ws.write_string(1, 3, "인문계열").map_err(excel::xlsx_err)?;
    ws.write_number(1, 4, 3.0).map_err(excel::xlsx_err)?;
    ws.write_string(1, 5, "아니오").map_err(excel::xlsx_err)?;
    // 같은 대학의 두 번째 모집단위 — 대학 정원·재학생우선은 동일하게 반복
    ws.write_string(2, 0, "한국대학교").map_err(excel::xlsx_err)?;
    ws.write_number(2, 1, 5.0).map_err(excel::xlsx_err)?;
    ws.write_string(2, 2, "아니오").map_err(excel::xlsx_err)?;
    ws.write_string(2, 3, "자연계열").map_err(excel::xlsx_err)?;
    ws.write_string(2, 4, UNLIMITED_TEXT).map_err(excel::xlsx_err)?;
    ws.write_string(2, 5, "아니오").map_err(excel::xlsx_err)?;
    // 샘플 2: 모집단위가 없는 대학 — 모집단위 3칸은 비운다
    ws.write_string(3, 0, "서울대학교").map_err(excel::xlsx_err)?;
    ws.write_string(3, 1, UNLIMITED_TEXT).map_err(excel::xlsx_err)?;
    ws.write_string(3, 2, "예").map_err(excel::xlsx_err)?;

    let buf = wb.save_to_buffer()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(excel::xlsx_response(buf, "univ_settings_template.xlsx"))
}

/// GET /api/universities/settings/export
pub async fn settings_export(State(state): State<AppState>) -> Result<Response, ApiError> {
    let univs = sqlx::query_as::<_, UnivRow>(
        "SELECT id, univ_name, total_quota, prioritize_enrolled FROM universities ORDER BY univ_name",
    )
    .fetch_all(&state.db).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let tracks = sqlx::query_as::<_, TrackRow>(
        "SELECT id, univ_id, track_name, unit_quota, prioritize_enrolled FROM univ_tracks ORDER BY univ_id, track_name",
    )
    .fetch_all(&state.db).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut by_univ: HashMap<i64, Vec<&TrackRow>> = HashMap::new();
    for t in &tracks { by_univ.entry(t.univ_id).or_default().push(t); }

    let mut wb = Workbook::new();
    let ws = wb.add_worksheet().set_name("대학설정")
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    write_settings_header(ws)?;

    let write_quota = |ws: &mut rust_xlsxwriter::Worksheet, row: u32, coln: u16, q: Option<i64>| -> Result<(), ApiError> {
        match q {
            Some(n) => ws.write_number(row, coln, n as f64).map(|_| ()).map_err(excel::xlsx_err),
            None => ws.write_string(row, coln, UNLIMITED_TEXT).map(|_| ()).map_err(excel::xlsx_err),
        }
    };

    let mut row = 1u32;
    for u in &univs {
        let u_tracks = by_univ.remove(&u.id).unwrap_or_default();
        if u_tracks.is_empty() {
            // 모집단위 없는 대학 — 대학 3칸만
            ws.write_string(row, 0, &u.univ_name).map_err(excel::xlsx_err)?;
            write_quota(ws, row, 1, u.total_quota)?;
            ws.write_string(row, 2, fmt_prio(u.prioritize_enrolled == 1)).map_err(excel::xlsx_err)?;
            row += 1;
        } else {
            for t in u_tracks {
                ws.write_string(row, 0, &u.univ_name).map_err(excel::xlsx_err)?;
                write_quota(ws, row, 1, u.total_quota)?;
                ws.write_string(row, 2, fmt_prio(u.prioritize_enrolled == 1)).map_err(excel::xlsx_err)?;
                ws.write_string(row, 3, &t.track_name).map_err(excel::xlsx_err)?;
                write_quota(ws, row, 4, t.unit_quota)?;
                ws.write_string(row, 5, fmt_prio(t.prioritize_enrolled == 1)).map_err(excel::xlsx_err)?;
                row += 1;
            }
        }
    }

    let buf = wb.save_to_buffer()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(excel::xlsx_response(buf, &format!("univ_settings_{}.xlsx", excel::now_tag())))
}

// ── preview / import 핸들러 ───────────────────────────────────────

async fn read_upload(mut multipart: Multipart) -> Result<Vec<u8>, ApiError> {
    match multipart.next_field().await.map_err(multipart_err)? {
        Some(f) => Ok(f.bytes().await.map_err(multipart_err)?.to_vec()),
        None => Err((StatusCode::BAD_REQUEST, "파일이 없습니다".into())),
    }
}

/// POST /api/universities/settings/preview
/// 파일을 파싱·검증하고 현재 DB 와의 diff 를 계산한다. **쓰기 없음.**
/// 관리자가 모달에서 확인하기 위한 advisory — 오류가 있으면 body.errors 에 담아 200 으로 반환한다.
pub async fn settings_preview(
    State(state): State<AppState>,
    multipart: Multipart,
) -> Result<Json<SettingsPreview>, ApiError> {
    let bytes = read_upload(multipart).await?;
    let (headers, rows) = match excel::parse_file_rows_with_headers(&bytes) {
        Ok(v) => v,
        Err(e) => return Ok(Json(SettingsPreview {
            errors: vec![e.to_string()], changes: vec![], unchanged_count: 0,
            closed_round_labels: vec![], has_blocked: false,
        })),
    };
    match parse_settings(&headers, &rows) {
        Err(errors) => {
            let labels = closed_round_labels(&state.db).await?;
            Ok(Json(SettingsPreview {
                errors, changes: vec![], unchanged_count: 0,
                closed_round_labels: labels, has_blocked: false,
            }))
        }
        Ok(specs) => Ok(Json(compute_settings_changes(&state.db, &specs).await?)),
    }
}

/// POST /api/universities/settings/import
/// 재검증 후 tx 적용(All-or-Nothing). 검증 오류 422 / 마감 가드 409 / 성공 200.
pub async fn settings_import(
    State(state): State<AppState>,
    multipart: Multipart,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let bytes = read_upload(multipart).await?;
    let (headers, rows) = excel::parse_file_rows_with_headers(&bytes)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let specs = match parse_settings(&headers, &rows) {
        Ok(s) => s,
        Err(errors) => return Ok((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({ "inserted": 0, "updated": 0, "errors": errors })),
        )),
    };

    let mut tx = state.db.begin().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let (created, updated) = apply_settings(&mut tx, &specs).await?;
    audit::log(&mut *tx, AuditEntry {
        actor: Actor::Admin,
        action: AuditAction::UniversitySettingsImported,
        round_id: None,
        student_id: None,
        detail: serde_json::json!({ "created": created, "updated": updated }),
    }).await?;
    tx.commit().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok((StatusCode::OK, Json(serde_json::json!({ "inserted": created, "updated": updated, "errors": [] }))))
}
