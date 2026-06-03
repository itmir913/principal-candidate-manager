use axum::{extract::State, http::StatusCode, Extension, Json};
use serde::Serialize;

use crate::{auth, state::AppState};

type ApiError = (StatusCode, String);

// ── Response types ─────────────────────────────────────────────

#[derive(Serialize)]
pub struct OverviewResponse {
    pub server_addr: String,
    pub version: &'static str,
    pub round: Option<OverviewRound>,
    pub classes: Vec<OverviewClass>,
    pub universities: Vec<OverviewUniversity>,
    pub all_time: OverviewAllTime,
}

#[derive(Serialize)]
pub struct OverviewRound {
    pub id: i64,
    pub status: String,
    pub opened_at: String,
}

#[derive(Serialize)]
pub struct OverviewClass {
    pub grade: i64,
    pub class_no: i64,
    pub teacher_name: Option<String>,
    pub submitted: i64,
}

#[derive(Serialize)]
pub struct OverviewUniversity {
    pub univ_id: i64,
    pub univ_name: String,
    pub total_quota: Option<i64>,
    pub tracks: Vec<OverviewTrack>,
}

#[derive(Serialize)]
pub struct OverviewTrack {
    pub track_id: i64,
    pub track_name: String,
    pub unit_quota: Option<i64>,
    pub applicants: i64,
}

#[derive(Serialize)]
pub struct OverviewAllTime {
    pub total_rounds: i64,
    pub total_applicants: i64,
    pub confirmed: i64,
    pub abandoned: i64,
}

// ── DB row types ───────────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct RoundRow {
    id: i64,
    status: String,
    opened_at: String,
}

#[derive(sqlx::FromRow)]
struct ClassRow {
    grade: i64,
    class_no: i64,
    teacher_name: Option<String>,
    submitted: i64,
}

#[derive(sqlx::FromRow)]
struct UnivTrackRow {
    univ_id: i64,
    univ_name: String,
    total_quota: Option<i64>,
    track_id: Option<i64>,
    track_name: Option<String>,
    unit_quota: Option<i64>,
    applicants: i64,
}

#[derive(sqlx::FromRow)]
struct AllTimeRow {
    total_rounds: i64,
    total_applicants: i64,
    confirmed: i64,
    abandoned: i64,
}

// ── Handler ────────────────────────────────────────────────────

pub async fn get_overview(
    State(state): State<AppState>,
    Extension(_claims): Extension<auth::AdminClaims>,
) -> Result<Json<OverviewResponse>, ApiError> {
    let db_err = |e: sqlx::Error| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string());

    // 1. Current round: OPEN 또는 CLOSED 중 최신 1건
    let round_row = sqlx::query_as::<_, RoundRow>(
        "SELECT id, status, opened_at FROM rounds \
         WHERE status IN ('OPEN', 'CLOSED') ORDER BY id DESC LIMIT 1",
    )
    .fetch_optional(&state.db)
    .await
    .map_err(db_err)?;

    let round_id: Option<i64> = round_row.as_ref().map(|r| r.id);
    let round = round_row.map(|r| OverviewRound {
        id: r.id,
        status: r.status,
        opened_at: r.opened_at,
    });

    // 2. 학급별 지원자 수 (이번 라운드, grade=0 class_no=0 제외)
    let class_rows = sqlx::query_as::<_, ClassRow>(
        "SELECT c.grade, c.class_no, c.teacher_name,
                COUNT(DISTINCT a.student_id) AS submitted
         FROM classes c
         LEFT JOIN students s
               ON s.grade = c.grade AND s.class_no = c.class_no AND s.is_enrolled = 1
         LEFT JOIN applications a
               ON a.student_id = s.id AND a.round_id = ?
         WHERE NOT (c.grade = 0 AND c.class_no = 0)
         GROUP BY c.grade, c.class_no, c.teacher_name
         ORDER BY c.grade, c.class_no",
    )
    .bind(round_id)
    .fetch_all(&state.db)
    .await
    .map_err(db_err)?;

    let classes = class_rows
        .into_iter()
        .map(|r| OverviewClass {
            grade: r.grade,
            class_no: r.class_no,
            teacher_name: r.teacher_name,
            submitted: r.submitted,
        })
        .collect();

    // 3. 대학/모집단위별 지원자 수 (이번 라운드)
    let univ_rows = sqlx::query_as::<_, UnivTrackRow>(
        "SELECT u.id AS univ_id, u.univ_name, u.total_quota,
                t.id AS track_id, t.track_name, t.unit_quota,
                COUNT(a.student_id) AS applicants
         FROM universities u
         LEFT JOIN univ_tracks t ON t.univ_id = u.id
         LEFT JOIN applications a ON a.track_id = t.id AND a.round_id = ?
         GROUP BY u.id, t.id
         ORDER BY u.id, t.id",
    )
    .bind(round_id)
    .fetch_all(&state.db)
    .await
    .map_err(db_err)?;

    // 대학별로 묶기 (SQL이 u.id 기준으로 정렬되어 있으므로 순차 처리)
    let mut universities: Vec<OverviewUniversity> = Vec::new();
    for row in univ_rows {
        if universities.last().map(|u: &OverviewUniversity| u.univ_id) != Some(row.univ_id) {
            universities.push(OverviewUniversity {
                univ_id: row.univ_id,
                univ_name: row.univ_name,
                total_quota: row.total_quota,
                tracks: vec![],
            });
        }
        if let (Some(track_id), Some(track_name)) = (row.track_id, row.track_name) {
            if let Some(last) = universities.last_mut() {
                last.tracks.push(OverviewTrack {
                    track_id,
                    track_name,
                    unit_quota: row.unit_quota,
                    applicants: row.applicants,
                });
            }
        }
    }

    // 4. 전체 누적 통계
    let stats = sqlx::query_as::<_, AllTimeRow>(
        "SELECT
             (SELECT COUNT(*)                    FROM rounds)                          AS total_rounds,
             (SELECT COUNT(*)                    FROM applications)                    AS total_applicants,
             (SELECT COUNT(*)                    FROM results       WHERE recommended = 1) AS confirmed,
             (SELECT COUNT(*)                    FROM applications  WHERE abandoned   = 1) AS abandoned",
    )
    .fetch_one(&state.db)
    .await
    .map_err(db_err)?;

    Ok(Json(OverviewResponse {
        server_addr: state.server_addr.clone(),
        version: env!("CARGO_PKG_VERSION"),
        round,
        classes,
        universities,
        all_time: OverviewAllTime {
            total_rounds: stats.total_rounds,
            total_applicants: stats.total_applicants,
            confirmed: stats.confirmed,
            abandoned: stats.abandoned,
        },
    }))
}
