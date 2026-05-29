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
struct AreaRow {
    id: i64,
    calc_type: String,
    range_direction: Option<String>,
    category_agg: Option<String>,
    lookup_scope: String,
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

fn lookup_range_score(value: i64, rows: &[(i64, i64)], direction: &str) -> i64 {
    match direction {
        "UPPER" => rows
            .iter()
            .filter(|(th, _)| value >= *th)
            .max_by_key(|(th, _)| *th)
            .map(|(_, sc)| *sc)
            .unwrap_or(0),
        "LOWER" => rows
            .iter()
            .filter(|(th, _)| value <= *th)
            .min_by_key(|(th, _)| *th)
            .map(|(_, sc)| *sc)
            .unwrap_or(0),
        _ => 0,
    }
}

async fn calc_area_score(
    db: &Db,
    student_id: i64,
    area: &AreaRow,
    univ_id: i64,
) -> Result<i64, sqlx::Error> {
    // COMPOSITE 영역은 지원 대학별 데이터 사용, SIMPLE은 전역 데이터
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
                _ => scores.iter().sum(),
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
        "SELECT id, calc_type, range_direction, category_agg, lookup_scope FROM areas ORDER BY id",
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
    range_direction: Option<String>,
    category_agg: Option<String>,
    lookup_scope: String,
}

pub async fn score_preview(
    State(state): State<AppState>,
    Query(q): Query<ScorePreviewQuery>,
) -> Result<Json<ScorePreviewResponse>, ApiError> {
    let area_rows: Vec<AreaWithName> = sqlx::query_as::<_, AreaWithName>(
        "SELECT id, name, calc_type, range_direction, category_agg, lookup_scope
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── lookup_range_score 순수 함수 테스트 ────────────────────────

    fn sample_rows() -> Vec<(i64, i64)> {
        // 임계값(×10000) → 점수(×10000)
        // 100000(=1.0), 200000(=2.0), 300000(=3.0)
        vec![(100_000, 50_000), (200_000, 30_000), (300_000, 10_000)]
    }

    #[test]
    fn upper_exact_match() {
        assert_eq!(lookup_range_score(200_000, &sample_rows(), "UPPER"), 30_000);
    }

    #[test]
    fn upper_between_thresholds() {
        // 150000 >= 100000 ✓, >= 200000 ✗ → max 충족 threshold = 100000 → 50000
        assert_eq!(lookup_range_score(150_000, &sample_rows(), "UPPER"), 50_000);
    }

    #[test]
    fn upper_above_all_thresholds() {
        // 모든 threshold 충족 → max threshold = 300000 → 10000
        assert_eq!(lookup_range_score(350_000, &sample_rows(), "UPPER"), 10_000);
    }

    #[test]
    fn upper_below_all_thresholds() {
        // 아무 threshold도 충족 못함 → 0
        assert_eq!(lookup_range_score(50_000, &sample_rows(), "UPPER"), 0);
    }

    #[test]
    fn lower_exact_match() {
        assert_eq!(lookup_range_score(200_000, &sample_rows(), "LOWER"), 30_000);
    }

    #[test]
    fn lower_between_thresholds() {
        // 150000 <= 200000 ✓, <= 300000 ✓, <= 100000 ✗ → min 충족 threshold = 200000 → 30000
        assert_eq!(lookup_range_score(150_000, &sample_rows(), "LOWER"), 30_000);
    }

    #[test]
    fn lower_above_all_thresholds() {
        // 아무 threshold도 충족 못함(value > 모두) → 0
        assert_eq!(lookup_range_score(400_000, &sample_rows(), "LOWER"), 0);
    }

    #[test]
    fn lower_below_all_thresholds() {
        // 모든 threshold 충족 → min threshold = 100000 → 50000
        assert_eq!(lookup_range_score(50_000, &sample_rows(), "LOWER"), 50_000);
    }

    #[test]
    fn empty_rows_return_zero() {
        assert_eq!(lookup_range_score(100_000, &[], "UPPER"), 0);
        assert_eq!(lookup_range_score(100_000, &[], "LOWER"), 0);
    }

    #[test]
    fn unknown_direction_returns_zero() {
        assert_eq!(lookup_range_score(100_000, &sample_rows(), "UNKNOWN"), 0);
    }

    // ── calc_area_score 통합 테스트 (인메모리 SQLite) ──────────────

    async fn insert_student(pool: &Db) -> i64 {
        sqlx::query(
            "INSERT INTO students (student_code, name, is_enrolled, grad_year) VALUES ('S001', '홍길동', 0, 2024)",
        )
        .execute(pool)
        .await
        .unwrap()
        .last_insert_rowid()
    }

    async fn insert_area(
        pool: &Db,
        calc_type: &str,
        direction: Option<&str>,
        agg: Option<&str>,
        scope: &str,
    ) -> i64 {
        sqlx::query(
            "INSERT INTO areas (name, max_score, calc_type, range_direction, category_agg, lookup_scope) \
             VALUES ('TestArea', 100000, ?, ?, ?, ?)",
        )
        .bind(calc_type)
        .bind(direction)
        .bind(agg)
        .bind(scope)
        .execute(pool)
        .await
        .unwrap()
        .last_insert_rowid()
    }

    async fn insert_university(pool: &Db) -> i64 {
        sqlx::query(
            "INSERT INTO universities (univ_name, track_name, capacity) VALUES ('서울대', '컴퓨터공학', 5)",
        )
        .execute(pool)
        .await
        .unwrap()
        .last_insert_rowid()
    }

    #[tokio::test]
    async fn calc_range_simple_upper() {
        let pool = crate::db::create_test_pool().await;
        let sid = insert_student(&pool).await;
        let aid = insert_area(&pool, "RANGE", Some("UPPER"), None, "SIMPLE").await;

        // threshold 1.0(100000)→50000, 2.0(200000)→30000, 3.0(300000)→10000
        for (th, sc) in [(100_000i64, 50_000i64), (200_000, 30_000), (300_000, 10_000)] {
            sqlx::query("INSERT INTO range_table (area_id, univ_id, threshold, score) VALUES (?, NULL, ?, ?)")
                .bind(aid).bind(th).bind(sc)
                .execute(&pool).await.unwrap();
        }
        // base_data: 1.25등급 = 12500(×10000=125000)
        sqlx::query("INSERT INTO base_data (student_id, area_id, univ_id, value) VALUES (?, ?, NULL, '125000')")
            .bind(sid).bind(aid).execute(&pool).await.unwrap();

        let area = AreaRow {
            id: aid,
            calc_type: "RANGE".into(),
            range_direction: Some("UPPER".into()),
            category_agg: None,
            lookup_scope: "SIMPLE".into(),
        };
        assert_eq!(calc_area_score(&pool, sid, &area, 0).await.unwrap(), 50_000);
    }

    #[tokio::test]
    async fn calc_range_simple_lower() {
        let pool = crate::db::create_test_pool().await;
        let sid = insert_student(&pool).await;
        let aid = insert_area(&pool, "RANGE", Some("LOWER"), None, "SIMPLE").await;

        for (th, sc) in [(100_000i64, 50_000i64), (200_000, 30_000), (300_000, 10_000)] {
            sqlx::query("INSERT INTO range_table (area_id, univ_id, threshold, score) VALUES (?, NULL, ?, ?)")
                .bind(aid).bind(th).bind(sc)
                .execute(&pool).await.unwrap();
        }
        // base_data: 150000 → LOWER: 150000 <= 200000 (min) → 30000
        sqlx::query("INSERT INTO base_data (student_id, area_id, univ_id, value) VALUES (?, ?, NULL, '150000')")
            .bind(sid).bind(aid).execute(&pool).await.unwrap();

        let area = AreaRow {
            id: aid,
            calc_type: "RANGE".into(),
            range_direction: Some("LOWER".into()),
            category_agg: None,
            lookup_scope: "SIMPLE".into(),
        };
        assert_eq!(calc_area_score(&pool, sid, &area, 0).await.unwrap(), 30_000);
    }

    #[tokio::test]
    async fn calc_range_composite() {
        let pool = crate::db::create_test_pool().await;
        let sid = insert_student(&pool).await;
        let uid = insert_university(&pool).await;
        let aid = insert_area(&pool, "RANGE", Some("UPPER"), None, "COMPOSITE").await;

        sqlx::query("INSERT INTO range_table (area_id, univ_id, threshold, score) VALUES (?, ?, 100000, 80000)")
            .bind(aid).bind(uid).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO base_data (student_id, area_id, univ_id, value) VALUES (?, ?, ?, '150000')")
            .bind(sid).bind(aid).bind(uid).execute(&pool).await.unwrap();

        let area = AreaRow {
            id: aid,
            calc_type: "RANGE".into(),
            range_direction: Some("UPPER".into()),
            category_agg: None,
            lookup_scope: "COMPOSITE".into(),
        };
        assert_eq!(calc_area_score(&pool, sid, &area, uid).await.unwrap(), 80_000);
    }

    #[tokio::test]
    async fn calc_category_sum() {
        let pool = crate::db::create_test_pool().await;
        let sid = insert_student(&pool).await;
        let aid = insert_area(&pool, "CATEGORY", None, Some("SUM"), "SIMPLE").await;

        for (cat, sc) in [("회장", 30_000i64), ("봉사", 20_000)] {
            sqlx::query("INSERT INTO category_map (area_id, univ_id, category, score) VALUES (?, NULL, ?, ?)")
                .bind(aid).bind(cat).bind(sc).execute(&pool).await.unwrap();
            sqlx::query("INSERT INTO base_data (student_id, area_id, univ_id, value) VALUES (?, ?, NULL, ?)")
                .bind(sid).bind(aid).bind(cat).execute(&pool).await.unwrap();
        }

        let area = AreaRow {
            id: aid,
            calc_type: "CATEGORY".into(),
            range_direction: None,
            category_agg: Some("SUM".into()),
            lookup_scope: "SIMPLE".into(),
        };
        // 30000 + 20000 = 50000
        assert_eq!(calc_area_score(&pool, sid, &area, 0).await.unwrap(), 50_000);
    }

    #[tokio::test]
    async fn calc_category_max() {
        let pool = crate::db::create_test_pool().await;
        let sid = insert_student(&pool).await;
        let aid = insert_area(&pool, "CATEGORY", None, Some("MAX"), "SIMPLE").await;

        for (cat, sc) in [("회장", 30_000i64), ("부회장", 20_000)] {
            sqlx::query("INSERT INTO category_map (area_id, univ_id, category, score) VALUES (?, NULL, ?, ?)")
                .bind(aid).bind(cat).bind(sc).execute(&pool).await.unwrap();
            sqlx::query("INSERT INTO base_data (student_id, area_id, univ_id, value) VALUES (?, ?, NULL, ?)")
                .bind(sid).bind(aid).bind(cat).execute(&pool).await.unwrap();
        }

        let area = AreaRow {
            id: aid,
            calc_type: "CATEGORY".into(),
            range_direction: None,
            category_agg: Some("MAX".into()),
            lookup_scope: "SIMPLE".into(),
        };
        // max(30000, 20000) = 30000
        assert_eq!(calc_area_score(&pool, sid, &area, 0).await.unwrap(), 30_000);
    }

    #[tokio::test]
    async fn calc_manual() {
        let pool = crate::db::create_test_pool().await;
        let sid = insert_student(&pool).await;
        let aid = insert_area(&pool, "MANUAL", None, None, "SIMPLE").await;

        sqlx::query("INSERT INTO base_data (student_id, area_id, univ_id, value) VALUES (?, ?, NULL, '75000')")
            .bind(sid).bind(aid).execute(&pool).await.unwrap();

        let area = AreaRow {
            id: aid,
            calc_type: "MANUAL".into(),
            range_direction: None,
            category_agg: None,
            lookup_scope: "SIMPLE".into(),
        };
        assert_eq!(calc_area_score(&pool, sid, &area, 0).await.unwrap(), 75_000);
    }

    #[tokio::test]
    async fn calc_no_base_data_returns_zero() {
        let pool = crate::db::create_test_pool().await;
        let sid = insert_student(&pool).await;
        let aid = insert_area(&pool, "RANGE", Some("UPPER"), None, "SIMPLE").await;

        sqlx::query("INSERT INTO range_table (area_id, univ_id, threshold, score) VALUES (?, NULL, 100000, 50000)")
            .bind(aid).execute(&pool).await.unwrap();
        // base_data 없음 → 0 반환

        let area = AreaRow {
            id: aid,
            calc_type: "RANGE".into(),
            range_direction: Some("UPPER".into()),
            category_agg: None,
            lookup_scope: "SIMPLE".into(),
        };
        assert_eq!(calc_area_score(&pool, sid, &area, 0).await.unwrap(), 0);
    }
}

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

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::{db::create_test_pool, state::AppState};
    use axum::{extract::{Path, State}, http::StatusCode};

    fn make_state(pool: sqlx::SqlitePool) -> AppState {
        AppState { db: pool, jwt_secret: "test".into() }
    }

    /// 최소 픽스처: 학급·학생·대학·라운드 한 세트 반환
    async fn setup_full(pool: &sqlx::SqlitePool) -> (i64, i64, i64) {
        let hash = bcrypt::hash("pass", 4u32).unwrap();
        sqlx::query("INSERT INTO classes (grade, class_no, password_hash) VALUES (1, 1, ?)")
            .bind(&hash).execute(pool).await.unwrap();
        let sid: i64 = sqlx::query_scalar(
            "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
             VALUES ('S001', '홍길동', 1, 1, 1, 1) RETURNING id",
        ).fetch_one(pool).await.unwrap();
        let uid: i64 = sqlx::query_scalar(
            "INSERT INTO universities (univ_name, track_name, capacity) \
             VALUES ('서울대', '컴공', 5) RETURNING id",
        ).fetch_one(pool).await.unwrap();
        let rid: i64 = sqlx::query_scalar(
            "INSERT INTO rounds (status, opened_at) VALUES ('OPEN', '2025-01-01T00:00:00Z') RETURNING id",
        ).fetch_one(pool).await.unwrap();
        (sid, uid, rid)
    }

    // ── calculate_scores ──────────────────────────────────────────────

    #[tokio::test]
    async fn calculate_scores_nonexistent_round_returns_not_found() {
        let pool = create_test_pool().await;
        let res = calculate_scores(State(make_state(pool)), Path(9999i64)).await;
        assert_eq!(res.unwrap_err().0, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn calculate_scores_no_applications_returns_zero_count() {
        let pool = create_test_pool().await;
        let (_, _, rid) = setup_full(&pool).await;
        let axum::Json(result) =
            calculate_scores(State(make_state(pool)), Path(rid)).await.unwrap();
        assert_eq!(result["calculated"], 0);
    }

    #[tokio::test]
    async fn calculate_scores_creates_result_rows_and_ranking() {
        let pool = create_test_pool().await;
        let (sid, uid, rid) = setup_full(&pool).await;

        // 지원 등록
        sqlx::query(
            "INSERT INTO applications (student_id, univ_id, round_id, confirmed, abandoned) \
             VALUES (?, ?, ?, 1, 0)",
        )
        .bind(sid).bind(uid).bind(rid)
        .execute(&pool).await.unwrap();

        let axum::Json(result) =
            calculate_scores(State(make_state(pool.clone())), Path(rid)).await.unwrap();
        assert_eq!(result["calculated"], 1);

        // results 행 확인
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM results WHERE round_id = ?")
            .bind(rid).fetch_one(&pool).await.unwrap();
        assert_eq!(count, 1);

        // 순위 설정 확인
        let ranking: Option<i64> =
            sqlx::query_scalar("SELECT ranking FROM results WHERE round_id = ?")
                .bind(rid).fetch_one(&pool).await.unwrap();
        assert_eq!(ranking, Some(1));
    }

    #[tokio::test]
    async fn calculate_scores_ranks_higher_score_first() {
        let pool = create_test_pool().await;
        let hash = bcrypt::hash("pass", 4u32).unwrap();
        sqlx::query("INSERT INTO classes (grade, class_no, password_hash) VALUES (1, 1, ?)")
            .bind(&hash).execute(&pool).await.unwrap();

        // 학생 2명
        let sid1: i64 = sqlx::query_scalar(
            "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
             VALUES ('S001', '홍길동', 1, 1, 1, 1) RETURNING id",
        ).fetch_one(&pool).await.unwrap();
        let sid2: i64 = sqlx::query_scalar(
            "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
             VALUES ('S002', '이순신', 1, 1, 2, 1) RETURNING id",
        ).fetch_one(&pool).await.unwrap();

        let uid: i64 = sqlx::query_scalar(
            "INSERT INTO universities (univ_name, track_name, capacity) \
             VALUES ('서울대', '컴공', 5) RETURNING id",
        ).fetch_one(&pool).await.unwrap();
        let rid: i64 = sqlx::query_scalar(
            "INSERT INTO rounds (status, opened_at) VALUES ('OPEN', '2025-01-01T00:00:00Z') RETURNING id",
        ).fetch_one(&pool).await.unwrap();

        // MANUAL 영역 생성
        let aid: i64 = sqlx::query_scalar(
            "INSERT INTO areas (name, max_score, calc_type, lookup_scope) \
             VALUES ('수동점수', 1000000, 'MANUAL', 'SIMPLE') RETURNING id",
        ).fetch_one(&pool).await.unwrap();

        // sid1: 점수 높음 (800점), sid2: 낮음 (600점)
        sqlx::query(
            "INSERT INTO base_data (student_id, area_id, univ_id, value) VALUES (?, ?, NULL, '8000000')",
        ).bind(sid1).bind(aid).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO base_data (student_id, area_id, univ_id, value) VALUES (?, ?, NULL, '6000000')",
        ).bind(sid2).bind(aid).execute(&pool).await.unwrap();

        // 지원 2건
        for sid in [sid1, sid2] {
            sqlx::query(
                "INSERT INTO applications (student_id, univ_id, round_id, confirmed, abandoned) \
                 VALUES (?, ?, ?, 1, 0)",
            ).bind(sid).bind(uid).bind(rid).execute(&pool).await.unwrap();
        }

        calculate_scores(State(make_state(pool.clone())), Path(rid)).await.unwrap();

        let rank1: Option<i64> =
            sqlx::query_scalar("SELECT ranking FROM results WHERE student_id = ? AND round_id = ?")
                .bind(sid1).bind(rid).fetch_one(&pool).await.unwrap();
        let rank2: Option<i64> =
            sqlx::query_scalar("SELECT ranking FROM results WHERE student_id = ? AND round_id = ?")
                .bind(sid2).bind(rid).fetch_one(&pool).await.unwrap();
        assert_eq!(rank1, Some(1)); // 높은 점수 → 1위
        assert_eq!(rank2, Some(2));
    }

    // ── recommend_result ──────────────────────────────────────────────

    #[tokio::test]
    async fn recommend_on_open_round_returns_bad_request() {
        let pool = create_test_pool().await;
        let (sid, uid, rid) = setup_full(&pool).await;
        // OPEN 라운드 → 추천 불가
        let res = recommend_result(State(make_state(pool)), Path((sid, uid, rid))).await;
        assert_eq!(res.unwrap_err().0, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn recommend_on_closed_round_sets_flag() {
        let pool = create_test_pool().await;
        let (sid, uid, rid) = setup_full(&pool).await;

        // 지원 + 점수계산 + 라운드 마감
        sqlx::query(
            "INSERT INTO applications (student_id, univ_id, round_id, confirmed, abandoned) \
             VALUES (?, ?, ?, 1, 0)",
        )
        .bind(sid).bind(uid).bind(rid)
        .execute(&pool).await.unwrap();

        calculate_scores(State(make_state(pool.clone())), Path(rid)).await.unwrap();

        sqlx::query("UPDATE rounds SET status = 'CLOSED', closed_at = '2025-01-02T00:00:00Z' WHERE id = ?")
            .bind(rid).execute(&pool).await.unwrap();

        recommend_result(State(make_state(pool.clone())), Path((sid, uid, rid)))
            .await
            .unwrap();

        let recommended: i64 =
            sqlx::query_scalar(
                "SELECT recommended FROM results WHERE student_id = ? AND univ_id = ? AND round_id = ?",
            )
            .bind(sid).bind(uid).bind(rid)
            .fetch_one(&pool).await.unwrap();
        assert_eq!(recommended, 1);
    }
}
