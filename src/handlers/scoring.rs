use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Response,
    Extension, Json,
};
use rust_xlsxwriter::Workbook;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Row};
use std::collections::HashMap;

use crate::{auth::TeacherClaims, excel, state::AppState};

type ApiError = (StatusCode, String);
type Db = sqlx::SqlitePool;

#[derive(FromRow)]
pub struct AreaRow {
    pub id: i64,
    pub calc_type: String,
    pub max_score: i64,
    pub range_direction: Option<String>,
    pub category_agg: Option<String>,
    pub lookup_scope: String,
}

#[derive(FromRow)]
struct AppRef {
    student_id: i64,
    univ_id: i64,
}

#[derive(Serialize, FromRow)]
pub struct ResultRow {
    pub student_id: i64,
    pub univ_id: i64,
    pub round_id: i64,
    pub total_score: i64,
    pub score_detail: String,
    pub ranking: Option<i64>,
    pub recommended: i64,
    pub abandoned: i64,
    pub student_code: String,
    pub name: String,
    pub grade: Option<i64>,
    pub class_no: Option<i64>,
    pub seq_no: Option<i64>,
    pub is_enrolled: i64,
    pub univ_name: String,
    pub track_name: String,
}

#[derive(Deserialize)]
pub struct ResultQuery {
    pub univ_id: Option<i64>,
}

// ── Scoring helpers ───────────────────────────────────────────────

pub fn lookup_range_score(value: i64, rows: &[(i64, i64)], direction: &str) -> i64 {
    match direction {
        "UPPER" => rows
            .iter()
            .filter(|(th, _)| value >= *th)
            .max_by_key(|(th, _)| *th)
            .map(|(_, sc)| *sc)
            .unwrap_or(0),
        // threshold가 허용 상한선 역할: value <= threshold인 행 중 최소 threshold 선택.
        // value가 최대 threshold를 초과하면("5일 이상: 5점") 최대 threshold 행의 점수 사용.
        "LOWER" => rows
            .iter()
            .filter(|(th, _)| value <= *th)
            .min_by_key(|(th, _)| *th)
            .map(|(_, sc)| *sc)
            .unwrap_or_else(|| {
                rows.iter().max_by_key(|(th, _)| *th).map(|(_, sc)| *sc).unwrap_or(0)
            }),
        _ => 0,
    }
}

pub async fn calc_area_score(
    db: &Db,
    student_id: i64,
    area: &AreaRow,
    univ_id: i64,
) -> Result<i64, sqlx::Error> {
    // COMPOSITE 전형 요소는 지원 대학별 데이터 사용, SIMPLE은 전역 데이터
    let lookup_univ: Option<i64> = if area.lookup_scope == "COMPOSITE" {
        Some(univ_id)
    } else {
        None
    };

    match area.calc_type.as_str() {
        "RANGE" => {
            let value_str: Option<String> = sqlx::query_scalar(
                "SELECT value FROM base_data
                 WHERE student_id = ? AND area_id = ?
                   AND (univ_id = ? OR (? IS NULL AND univ_id IS NULL))
                 LIMIT 1",
            )
            .bind(student_id).bind(area.id).bind(lookup_univ).bind(lookup_univ)
            .fetch_optional(db).await?;

            let Some(vs) = value_str else { return Ok(0); };
            let value: i64 = vs.trim().parse().unwrap_or(0);
            let direction = area.range_direction.as_deref().unwrap_or("UPPER");

            let rows: Vec<(i64, i64)> = sqlx::query(
                "SELECT threshold, score FROM range_table
                 WHERE area_id = ? AND (univ_id = ? OR (? IS NULL AND univ_id IS NULL))
                 ORDER BY threshold",
            )
            .bind(area.id).bind(lookup_univ).bind(lookup_univ)
            .fetch_all(db).await?
            .into_iter()
            .map(|r| (r.get::<i64, _>("threshold"), r.get::<i64, _>("score")))
            .collect();

            Ok(lookup_range_score(value, &rows, direction))
        }

        "CATEGORY" => {
            let values: Vec<String> = sqlx::query_scalar(
                "SELECT value FROM base_data
                 WHERE student_id = ? AND area_id = ?
                   AND (univ_id = ? OR (? IS NULL AND univ_id IS NULL))",
            )
            .bind(student_id).bind(area.id).bind(lookup_univ).bind(lookup_univ)
            .fetch_all(db).await?;

            let mut scores: Vec<i64> = Vec::new();
            for cat in &values {
                let sc: Option<i64> = sqlx::query_scalar(
                    "SELECT score FROM category_map
                     WHERE area_id = ? AND category = ?
                       AND (univ_id = ? OR (? IS NULL AND univ_id IS NULL))",
                )
                .bind(area.id).bind(cat.as_str()).bind(lookup_univ).bind(lookup_univ)
                .fetch_optional(db).await?;
                if let Some(s) = sc { scores.push(s); }
            }

            if scores.is_empty() { return Ok(0); }
            Ok(match area.category_agg.as_deref().unwrap_or("SUM") {
                "MAX" => *scores.iter().max().unwrap_or(&0),
                _ => scores.iter().sum::<i64>().min(area.max_score),
            })
        }

        "MANUAL" => {
            let v: Option<String> = sqlx::query_scalar(
                "SELECT value FROM base_data
                 WHERE student_id = ? AND area_id = ?
                   AND (univ_id = ? OR (? IS NULL AND univ_id IS NULL))
                 LIMIT 1",
            )
            .bind(student_id).bind(area.id).bind(lookup_univ).bind(lookup_univ)
            .fetch_optional(db).await?;

            Ok(v.and_then(|s| s.trim().parse::<i64>().ok()).unwrap_or(0))
        }

        _ => Ok(0),
    }
}

// ── Handlers ──────────────────────────────────────────────────────

pub async fn calculate_scores(
    State(state): State<AppState>,
    Path(round_id): Path<i64>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let round_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM rounds WHERE id = ?)",
    )
    .bind(round_id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if !round_exists {
        return Err((StatusCode::NOT_FOUND, "라운드를 찾을 수 없습니다".into()));
    }

    let areas: Vec<AreaRow> = sqlx::query_as::<_, AreaRow>(
        "SELECT id, calc_type, max_score, range_direction, category_agg, lookup_scope FROM areas ORDER BY id",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let applications: Vec<AppRef> = sqlx::query_as::<_, AppRef>(
        "SELECT student_id, univ_id FROM applications WHERE round_id = ? AND confirmed = 1",
    )
    .bind(round_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let now = chrono::Utc::now().to_rfc3339();
    let mut count = 0usize;

    // 점수 계산(읽기 전용)은 트랜잭션 밖에서 수행
    let mut score_rows: Vec<(i64, i64, String, i64)> = Vec::new(); // (student_id, univ_id, detail_json, total)
    for app in &applications {
        let mut detail: HashMap<String, i64> = HashMap::new();
        let mut total: i64 = 0;
        for area in &areas {
            let sc = calc_area_score(&state.db, app.student_id, area, app.univ_id)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            detail.insert(area.id.to_string(), sc);
            total += sc;
        }
        let detail_json = serde_json::to_string(&detail)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        score_rows.push((app.student_id, app.univ_id, detail_json, total));
    }

    // results 쓰기 + 순위 계산 전체를 하나의 트랜잭션으로
    let mut tx = state.db.begin().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    for (student_id, univ_id, detail_json, total) in &score_rows {
        sqlx::query(
            "INSERT INTO results
               (student_id, univ_id, round_id, score_detail, total_score, ranking, recommended, calculated_at)
             VALUES (?, ?, ?, ?, ?, NULL, 0, ?)
             ON CONFLICT (student_id, univ_id, round_id)
             DO UPDATE SET score_detail   = excluded.score_detail,
                           total_score    = excluded.total_score,
                           ranking        = NULL,
                           calculated_at  = excluded.calculated_at",
        )
        .bind(student_id).bind(univ_id).bind(round_id)
        .bind(detail_json).bind(total).bind(&now)
        .execute(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        count += 1;
    }

    // 대학별 순위 재계산 (트랜잭션 내에서 읽어야 방금 쓴 점수가 보임)
    let mut univ_ids: Vec<i64> = applications.iter().map(|a| a.univ_id).collect();
    univ_ids.sort_unstable();
    univ_ids.dedup();

    for uid in univ_ids {
        let prioritize: i64 = sqlx::query_scalar(
            "SELECT prioritize_enrolled FROM universities WHERE id = ?",
        )
        .bind(uid)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let rows = sqlx::query(
            "SELECT r.student_id, r.total_score, s.is_enrolled
             FROM results r
             JOIN students s ON r.student_id = s.id
             WHERE r.round_id = ? AND r.univ_id = ?",
        )
        .bind(round_id).bind(uid)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let mut ranked: Vec<(i64, i64, i64)> = rows
            .into_iter()
            .map(|r| (
                r.get::<i64, _>("student_id"),
                r.get::<i64, _>("total_score"),
                r.get::<i64, _>("is_enrolled"),
            ))
            .collect();

        ranked.sort_by(|a, b| {
            b.1.cmp(&a.1).then_with(|| {
                if prioritize == 1 { b.2.cmp(&a.2) } else { std::cmp::Ordering::Equal }
            })
        });

        for (rank, (sid, _, _)) in ranked.iter().enumerate() {
            sqlx::query(
                "UPDATE results SET ranking = ? WHERE student_id = ? AND univ_id = ? AND round_id = ?",
            )
            .bind((rank + 1) as i64).bind(sid).bind(uid).bind(round_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        }
    }

    tx.commit().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({ "calculated": count })))
}

pub async fn get_results(
    State(state): State<AppState>,
    Path(round_id): Path<i64>,
    Query(q): Query<ResultQuery>,
) -> Result<Json<Vec<ResultRow>>, ApiError> {
    let rows = sqlx::query_as::<_, ResultRow>(
        "SELECT r.student_id, r.univ_id, r.round_id,
                r.total_score, r.score_detail, r.ranking, r.recommended,
                COALESCE(a.abandoned, 0) AS abandoned,
                s.student_code, s.name, s.grade, s.class_no, s.seq_no, s.is_enrolled,
                u.univ_name, u.track_name
         FROM results r
         JOIN students s ON r.student_id = s.id
         JOIN universities u ON r.univ_id = u.id
         LEFT JOIN applications a ON a.student_id = r.student_id
                                  AND a.univ_id   = r.univ_id
                                  AND a.round_id  = r.round_id
         WHERE r.round_id = ?
           AND (? IS NULL OR r.univ_id = ?)
         ORDER BY r.univ_id, r.ranking NULLS LAST, r.total_score DESC",
    )
    .bind(round_id)
    .bind(q.univ_id).bind(q.univ_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(rows))
}

// ── Results Excel export ──────────────────────────────────────────

#[derive(FromRow)]
struct AreaName {
    id: i64,
    name: String,
}

#[derive(FromRow)]
struct UnivRef {
    id: i64,
    univ_name: String,
    track_name: String,
}

pub async fn export_results(
    State(state): State<AppState>,
    Path(round_id): Path<i64>,
) -> Result<Response, ApiError> {
    let areas: Vec<AreaName> = sqlx::query_as::<_, AreaName>(
        "SELECT id, name FROM areas ORDER BY id",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let univs: Vec<UnivRef> = sqlx::query_as::<_, UnivRef>(
        "SELECT DISTINCT u.id, u.univ_name, u.track_name
         FROM results r
         JOIN universities u ON r.univ_id = u.id
         WHERE r.round_id = ?
         ORDER BY u.univ_name, u.track_name",
    )
    .bind(round_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let all_results = sqlx::query_as::<_, ResultRow>(
        "SELECT r.student_id, r.univ_id, r.round_id,
                r.total_score, r.score_detail, r.ranking, r.recommended,
                COALESCE(a.abandoned, 0) AS abandoned,
                s.student_code, s.name, s.grade, s.class_no, s.seq_no, s.is_enrolled,
                u.univ_name, u.track_name
         FROM results r
         JOIN students s ON r.student_id = s.id
         JOIN universities u ON r.univ_id = u.id
         LEFT JOIN applications a ON a.student_id = r.student_id
                                  AND a.univ_id   = r.univ_id
                                  AND a.round_id  = r.round_id
         WHERE r.round_id = ?
         ORDER BY r.univ_id, r.ranking NULLS LAST, r.total_score DESC",
    )
    .bind(round_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut wb = Workbook::new();

    for univ in &univs {
        let sheet_name: String = format!("{} {}", univ.univ_name, univ.track_name)
            .chars()
            .take(31)
            .collect();
        let ws = wb
            .add_worksheet()
            .set_name(&sheet_name)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        // 헤더 행
        let fixed_headers = ["순위", "학생명", "학번", "학년", "반", "번호", "재학구분"];
        let mut col = 0u16;
        for h in &fixed_headers {
            ws.write_string(0, col, *h).ok();
            col += 1;
        }
        for area in &areas {
            ws.write_string(0, col, &area.name).ok();
            col += 1;
        }
        ws.write_string(0, col, "총점").ok(); col += 1;
        ws.write_string(0, col, "추천").ok(); col += 1;
        ws.write_string(0, col, "포기").ok();

        // 데이터 행
        let univ_results: Vec<&ResultRow> =
            all_results.iter().filter(|r| r.univ_id == univ.id).collect();

        for (i, r) in univ_results.iter().enumerate() {
            let row = (i + 1) as u32;
            let mut col = 0u16;

            if let Some(rank) = r.ranking {
                ws.write_number(row, col, rank as f64).ok();
            }
            col += 1;

            ws.write_string(row, col, &r.name).ok(); col += 1;
            ws.write_string(row, col, &r.student_code).ok(); col += 1;

            if let Some(g) = r.grade { ws.write_number(row, col, g as f64).ok(); }
            col += 1;
            if let Some(c) = r.class_no { ws.write_number(row, col, c as f64).ok(); }
            col += 1;
            if let Some(s) = r.seq_no { ws.write_number(row, col, s as f64).ok(); }
            col += 1;

            ws.write_string(row, col, if r.is_enrolled == 1 { "재학" } else { "졸업" }).ok();
            col += 1;

            let detail: HashMap<String, i64> =
                serde_json::from_str(&r.score_detail).unwrap_or_default();
            for area in &areas {
                let sc = detail.get(&area.id.to_string()).copied().unwrap_or(0);
                ws.write_number(row, col, sc as f64 / 10000.0).ok();
                col += 1;
            }

            ws.write_number(row, col, r.total_score as f64 / 10000.0).ok(); col += 1;
            ws.write_string(row, col, if r.recommended == 1 { "추천" } else { "" }).ok(); col += 1;
            ws.write_string(row, col, if r.abandoned == 1 { "포기" } else { "" }).ok();
        }
    }

    let buf = wb
        .save_to_buffer()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let filename = format!("results_round_{}_{}.xlsx", round_id, excel::now_tag());
    Ok(excel::xlsx_response(buf, &filename))
}

// ── Teacher results ───────────────────────────────────────────────

#[derive(Deserialize)]
pub struct TeacherResultQuery {
    pub round_id: Option<i64>,
}

pub async fn teacher_get_results(
    State(state): State<AppState>,
    Extension(claims): Extension<TeacherClaims>,
    Query(q): Query<TeacherResultQuery>,
) -> Result<Json<Vec<ResultRow>>, ApiError> {
    let round_id = match q.round_id {
        Some(rid) => rid,
        None => {
            let rid: Option<i64> = sqlx::query_scalar(
                "SELECT id FROM rounds ORDER BY id DESC LIMIT 1",
            )
            .fetch_optional(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            match rid {
                Some(id) => id,
                None => return Ok(Json(vec![])),
            }
        }
    };

    let rows = sqlx::query_as::<_, ResultRow>(
        "SELECT r.student_id, r.univ_id, r.round_id,
                r.total_score, r.score_detail, r.ranking, r.recommended,
                COALESCE(a.abandoned, 0) AS abandoned,
                s.student_code, s.name, s.grade, s.class_no, s.seq_no, s.is_enrolled,
                u.univ_name, u.track_name
         FROM results r
         JOIN students s ON r.student_id = s.id
         JOIN universities u ON r.univ_id = u.id
         LEFT JOIN applications a ON a.student_id = r.student_id
                                  AND a.univ_id   = r.univ_id
                                  AND a.round_id  = r.round_id
         WHERE r.round_id = ?
           AND s.grade = ?
           AND s.class_no = ?
         ORDER BY s.seq_no, r.univ_id",
    )
    .bind(round_id)
    .bind(claims.grade)
    .bind(claims.class_no)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(rows))
}

// ── Score preview ─────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ScorePreviewQuery {
    pub student_id: i64,
    pub univ_id: i64,
}

#[derive(Serialize)]
pub struct AreaPreview {
    pub area_id: i64,
    pub area_name: String,
    pub score: i64,
}

#[derive(Serialize)]
pub struct ScorePreviewResponse {
    pub total: i64,
    pub detail: Vec<AreaPreview>,
}

#[derive(FromRow)]
struct AreaWithName {
    id: i64,
    name: String,
    calc_type: String,
    max_score: i64,
    range_direction: Option<String>,
    category_agg: Option<String>,
    lookup_scope: String,
}

pub async fn score_preview(
    State(state): State<AppState>,
    Query(q): Query<ScorePreviewQuery>,
) -> Result<Json<ScorePreviewResponse>, ApiError> {
    let area_rows: Vec<AreaWithName> = sqlx::query_as::<_, AreaWithName>(
        "SELECT id, name, calc_type, max_score, range_direction, category_agg, lookup_scope
         FROM areas ORDER BY id",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut detail: Vec<AreaPreview> = Vec::new();
    let mut total: i64 = 0;

    for aw in &area_rows {
        let area = AreaRow {
            id: aw.id,
            calc_type: aw.calc_type.clone(),
            max_score: aw.max_score,
            range_direction: aw.range_direction.clone(),
            category_agg: aw.category_agg.clone(),
            lookup_scope: aw.lookup_scope.clone(),
        };
        let score = calc_area_score(&state.db, q.student_id, &area, q.univ_id)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        total += score;
        detail.push(AreaPreview { area_id: aw.id, area_name: aw.name.clone(), score });
    }

    Ok(Json(ScorePreviewResponse { total, detail }))
}

// ─────────────────────────────────────────────────────────────────


pub async fn recommend_result(
    State(state): State<AppState>,
    Path((sid, uid, rid)): Path<(i64, i64, i64)>,
) -> Result<StatusCode, ApiError> {
    let status: Option<String> = sqlx::query_scalar(
        "SELECT status FROM rounds WHERE id = ?",
    )
    .bind(rid)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if status.as_deref() != Some("CLOSED") {
        return Err((StatusCode::BAD_REQUEST, "CLOSED 라운드에서만 추천 확정이 가능합니다".into()));
    }

    sqlx::query(
        "UPDATE results SET recommended = 1 WHERE student_id = ? AND univ_id = ? AND round_id = ?",
    )
    .bind(sid).bind(uid).bind(rid)
    .execute(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

