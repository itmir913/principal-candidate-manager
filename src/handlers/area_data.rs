/// 영역별 데이터 Excel 업로드/다운로드 핸들러
/// - 점수 기준: range_table (RANGE), category_map (CATEGORY)
/// - 기초 데이터: base_data (모든 calc_type)
use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    response::Response,
    Json,
};
use rust_xlsxwriter::Workbook;
use serde::Serialize;
use sqlx::Row;

use crate::{excel, state::AppState};

type ApiError = (StatusCode, String);
type Db = sqlx::SqlitePool;

// ── 공통 구조체 ──────────────────────────────────────────────────

#[derive(Serialize)]
pub struct ImportResult {
    pub rows: usize,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(sqlx::FromRow)]
struct AreaInfo {
    calc_type: String,
    lookup_scope: String,
}

// ── 공통 헬퍼 ────────────────────────────────────────────────────

fn db_to_display(v: i64) -> f64 {
    v as f64 / 10000.0
}

fn display_to_db(s: &str) -> Option<i64> {
    s.trim()
        .parse::<f64>()
        .ok()
        .map(|f| (f * 10000.0).round() as i64)
}

fn simple_template(headers: &[&str]) -> anyhow::Result<Vec<u8>> {
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    for (i, h) in headers.iter().enumerate() {
        ws.write_string(0, i as u16, *h)?;
    }
    Ok(wb.save_to_buffer()?)
}

async fn get_area(db: &Db, id: i64) -> Result<AreaInfo, ApiError> {
    sqlx::query_as::<_, AreaInfo>(
        "SELECT calc_type, lookup_scope FROM areas WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or_else(|| (StatusCode::NOT_FOUND, format!("영역 id={} 없음", id)))
}

/// 대학이 없으면 자동 생성 후 (id, 생성여부) 반환
async fn find_or_create_univ(
    db: &Db,
    univ_name: &str,
    track_name: &str,
) -> Result<(i64, bool), ApiError> {
    if let Some(id) = sqlx::query_scalar::<_, i64>(
        "SELECT id FROM universities WHERE univ_name = ? AND track_name = ?",
    )
    .bind(univ_name)
    .bind(track_name)
    .fetch_optional(db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    {
        return Ok((id, false));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name, track_name, capacity, prioritize_enrolled)
         VALUES (?, ?, 0, 0) RETURNING id",
    )
    .bind(univ_name)
    .bind(track_name)
    .fetch_one(db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok((id, true))
}

async fn read_file(mut multipart: Multipart) -> Result<Vec<u8>, ApiError> {
    match multipart
        .next_field()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?
    {
        Some(f) => f
            .bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string())),
        None => Err((StatusCode::BAD_REQUEST, "파일이 없습니다".into())),
    }
}

/// COMPOSITE 여부에 따라 헤더 결정
fn score_headers(area: &AreaInfo, key_col: &'static str) -> Vec<&'static str> {
    if area.lookup_scope == "COMPOSITE" {
        vec![key_col, "score", "univ_name", "track_name"]
    } else {
        vec![key_col, "score"]
    }
}

/// COMPOSITE 영역: univ_id 조회/생성
async fn resolve_univ(
    db: &Db,
    area: &AreaInfo,
    cols: &[String],
    un_col: usize,
    tn_col: usize,
    row_num: usize,
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
) -> Option<Option<i64>> {
    if area.lookup_scope == "COMPOSITE" {
        let un = cols.get(un_col).map(|s| s.trim()).unwrap_or("");
        let tn = cols.get(tn_col).map(|s| s.trim()).unwrap_or("");
        if un.is_empty() || tn.is_empty() {
            errors.push(format!("{}행: COMPOSITE 영역은 univ_name, track_name 필수", row_num));
            return None;
        }
        match find_or_create_univ(db, un, tn).await {
            Ok((uid, created)) => {
                if created {
                    warnings.push(format!("'{}/{}' 대학 자동 추가됨", un, tn));
                }
                Some(Some(uid))
            }
            Err(e) => {
                errors.push(format!("{}행: 대학 처리 오류 — {}", row_num, e.1));
                None
            }
        }
    } else {
        Some(None)
    }
}

// ── RANGE TABLE ──────────────────────────────────────────────────

/// GET /api/areas/:id/range-table/template
pub async fn range_table_template(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Response, ApiError> {
    let area = get_area(&state.db, id).await?;
    if area.calc_type != "RANGE" {
        return Err((StatusCode::BAD_REQUEST, "RANGE 영역만 구간표를 사용합니다".into()));
    }
    let headers = score_headers(&area, "threshold");
    let buf = simple_template(&headers)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(excel::xlsx_response(buf, "range_table_template.xlsx"))
}

/// GET /api/areas/:id/range-table/export
pub async fn range_table_export(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Response, ApiError> {
    let area = get_area(&state.db, id).await?;
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();

    if area.lookup_scope == "COMPOSITE" {
        for (i, h) in ["threshold", "score", "univ_name", "track_name"].iter().enumerate() {
            ws.write_string(0, i as u16, *h).ok();
        }
        let rows = sqlx::query(
            "SELECT rt.threshold, rt.score,
                    COALESCE(u.univ_name, '') AS univ_name,
                    COALESCE(u.track_name, '') AS track_name
             FROM range_table rt
             LEFT JOIN universities u ON rt.univ_id = u.id
             WHERE rt.area_id = ?
             ORDER BY rt.univ_id, rt.threshold",
        )
        .bind(id)
        .fetch_all(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        for (r, row) in rows.iter().enumerate() {
            let r = r as u32 + 1;
            ws.write_number(r, 0, db_to_display(row.get("threshold"))).ok();
            ws.write_number(r, 1, db_to_display(row.get("score"))).ok();
            ws.write_string(r, 2, row.get::<&str, _>("univ_name")).ok();
            ws.write_string(r, 3, row.get::<&str, _>("track_name")).ok();
        }
    } else {
        for (i, h) in ["threshold", "score"].iter().enumerate() {
            ws.write_string(0, i as u16, *h).ok();
        }
        let rows = sqlx::query(
            "SELECT threshold, score FROM range_table
             WHERE area_id = ? AND univ_id IS NULL ORDER BY threshold",
        )
        .bind(id)
        .fetch_all(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        for (r, row) in rows.iter().enumerate() {
            let r = r as u32 + 1;
            ws.write_number(r, 0, db_to_display(row.get("threshold"))).ok();
            ws.write_number(r, 1, db_to_display(row.get("score"))).ok();
        }
    }

    let buf = wb
        .save_to_buffer()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(excel::xlsx_response(buf, &format!("range_table_{}.xlsx", excel::now_tag())))
}

/// POST /api/areas/:id/range-table/import
pub async fn range_table_import(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    multipart: Multipart,
) -> Result<Json<ImportResult>, ApiError> {
    let area = get_area(&state.db, id).await?;
    if area.calc_type != "RANGE" {
        return Err((StatusCode::BAD_REQUEST, "RANGE 영역만 구간표를 사용합니다".into()));
    }
    let bytes = read_file(multipart).await?;
    let file_rows = excel::parse_file_rows(&bytes)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let mut tx = state.db.begin().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    sqlx::query("DELETE FROM range_table WHERE area_id = ?")
        .bind(id).execute(&mut *tx).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut rows = 0usize;
    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    for (i, cols) in file_rows.iter().enumerate() {
        let row_num = i + 2;
        let g = |j: usize| cols.get(j).map(|s| s.as_str()).unwrap_or("");

        let th = match display_to_db(g(0)) {
            Some(v) => v,
            None => { errors.push(format!("{}행: threshold 파싱 실패 ('{}')", row_num, g(0))); continue; }
        };
        let sc = match display_to_db(g(1)) {
            Some(v) => v,
            None => { errors.push(format!("{}행: score 파싱 실패 ('{}')", row_num, g(1))); continue; }
        };

        let univ_id = match resolve_univ(&state.db, &area, cols, 2, 3, row_num, &mut errors, &mut warnings).await {
            Some(v) => v,
            None => continue,
        };

        match sqlx::query(
            "INSERT INTO range_table (area_id, univ_id, threshold, score) VALUES (?, ?, ?, ?)",
        )
        .bind(id).bind(univ_id).bind(th).bind(sc)
        .execute(&mut *tx).await {
            Ok(_) => rows += 1,
            Err(e) => errors.push(format!("{}행: {}", row_num, e)),
        }
    }

    tx.commit().await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(ImportResult { rows, errors, warnings }))
}

// ── CATEGORY MAP ─────────────────────────────────────────────────

/// GET /api/areas/:id/category-map/template
pub async fn category_map_template(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Response, ApiError> {
    let area = get_area(&state.db, id).await?;
    if area.calc_type != "CATEGORY" {
        return Err((StatusCode::BAD_REQUEST, "CATEGORY 영역만 범주표를 사용합니다".into()));
    }
    let headers = score_headers(&area, "category");
    let buf = simple_template(&headers)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(excel::xlsx_response(buf, "category_map_template.xlsx"))
}

/// GET /api/areas/:id/category-map/export
pub async fn category_map_export(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Response, ApiError> {
    let area = get_area(&state.db, id).await?;
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();

    if area.lookup_scope == "COMPOSITE" {
        for (i, h) in ["category", "score", "univ_name", "track_name"].iter().enumerate() {
            ws.write_string(0, i as u16, *h).ok();
        }
        let rows = sqlx::query(
            "SELECT cm.category, cm.score,
                    COALESCE(u.univ_name, '') AS univ_name,
                    COALESCE(u.track_name, '') AS track_name
             FROM category_map cm
             LEFT JOIN universities u ON cm.univ_id = u.id
             WHERE cm.area_id = ?
             ORDER BY cm.univ_id, cm.category",
        )
        .bind(id)
        .fetch_all(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        for (r, row) in rows.iter().enumerate() {
            let r = r as u32 + 1;
            ws.write_string(r, 0, row.get::<&str, _>("category")).ok();
            ws.write_number(r, 1, db_to_display(row.get("score"))).ok();
            ws.write_string(r, 2, row.get::<&str, _>("univ_name")).ok();
            ws.write_string(r, 3, row.get::<&str, _>("track_name")).ok();
        }
    } else {
        for (i, h) in ["category", "score"].iter().enumerate() {
            ws.write_string(0, i as u16, *h).ok();
        }
        let rows = sqlx::query(
            "SELECT category, score FROM category_map
             WHERE area_id = ? AND univ_id IS NULL ORDER BY category",
        )
        .bind(id)
        .fetch_all(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        for (r, row) in rows.iter().enumerate() {
            let r = r as u32 + 1;
            ws.write_string(r, 0, row.get::<&str, _>("category")).ok();
            ws.write_number(r, 1, db_to_display(row.get("score"))).ok();
        }
    }

    let buf = wb
        .save_to_buffer()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(excel::xlsx_response(buf, &format!("category_map_{}.xlsx", excel::now_tag())))
}

/// POST /api/areas/:id/category-map/import
pub async fn category_map_import(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    multipart: Multipart,
) -> Result<Json<ImportResult>, ApiError> {
    let area = get_area(&state.db, id).await?;
    if area.calc_type != "CATEGORY" {
        return Err((StatusCode::BAD_REQUEST, "CATEGORY 영역만 범주표를 사용합니다".into()));
    }
    let bytes = read_file(multipart).await?;
    let file_rows = excel::parse_file_rows(&bytes)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let mut tx = state.db.begin().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    sqlx::query("DELETE FROM category_map WHERE area_id = ?")
        .bind(id).execute(&mut *tx).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut rows = 0usize;
    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    for (i, cols) in file_rows.iter().enumerate() {
        let row_num = i + 2;
        let g = |j: usize| cols.get(j).map(|s| s.as_str()).unwrap_or("");

        let category = g(0).trim().to_string();
        if category.is_empty() {
            errors.push(format!("{}행: category 누락", row_num));
            continue;
        }
        let sc = match display_to_db(g(1)) {
            Some(v) => v,
            None => { errors.push(format!("{}행: score 파싱 실패 ('{}')", row_num, g(1))); continue; }
        };

        let univ_id = match resolve_univ(&state.db, &area, cols, 2, 3, row_num, &mut errors, &mut warnings).await {
            Some(v) => v,
            None => continue,
        };

        match sqlx::query(
            "INSERT INTO category_map (area_id, univ_id, category, score) VALUES (?, ?, ?, ?)",
        )
        .bind(id).bind(univ_id).bind(&category).bind(sc)
        .execute(&mut *tx).await {
            Ok(_) => rows += 1,
            Err(e) => errors.push(format!("{}행: {}", row_num, e)),
        }
    }

    tx.commit().await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(ImportResult { rows, errors, warnings }))
}

// ── BASE DATA ────────────────────────────────────────────────────

/// GET /api/areas/:id/base-data/template
pub async fn base_data_template(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Response, ApiError> {
    let area = get_area(&state.db, id).await?;
    let headers: Vec<&str> = if area.lookup_scope == "COMPOSITE" {
        vec!["student_code", "name", "value", "univ_name", "track_name"]
    } else {
        vec!["student_code", "name", "value"]
    };
    let buf = simple_template(&headers)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(excel::xlsx_response(buf, "base_data_template.xlsx"))
}

/// GET /api/areas/:id/base-data/export
pub async fn base_data_export(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Response, ApiError> {
    let area = get_area(&state.db, id).await?;
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();

    if area.lookup_scope == "COMPOSITE" {
        for (i, h) in ["student_code", "name", "value", "univ_name", "track_name"].iter().enumerate() {
            ws.write_string(0, i as u16, *h).ok();
        }
        let rows = sqlx::query(
            "SELECT s.student_code, s.name, bd.value, u.univ_name, u.track_name
             FROM base_data bd
             JOIN students s ON bd.student_id = s.id
             JOIN universities u ON bd.univ_id = u.id
             WHERE bd.area_id = ?
             ORDER BY u.univ_name, u.track_name, s.grade, s.class_no, s.seq_no",
        )
        .bind(id)
        .fetch_all(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        for (r, row) in rows.iter().enumerate() {
            let r = r as u32 + 1;
            ws.write_string(r, 0, row.get::<&str, _>("student_code")).ok();
            ws.write_string(r, 1, row.get::<&str, _>("name")).ok();
            write_value(ws, r, 2, row.get::<&str, _>("value"), &area.calc_type);
            ws.write_string(r, 3, row.get::<&str, _>("univ_name")).ok();
            ws.write_string(r, 4, row.get::<&str, _>("track_name")).ok();
        }
    } else {
        for (i, h) in ["student_code", "name", "value"].iter().enumerate() {
            ws.write_string(0, i as u16, *h).ok();
        }
        let rows = sqlx::query(
            "SELECT s.student_code, s.name, bd.value
             FROM base_data bd
             JOIN students s ON bd.student_id = s.id
             WHERE bd.area_id = ? AND bd.univ_id IS NULL
             ORDER BY s.grade, s.class_no, s.seq_no",
        )
        .bind(id)
        .fetch_all(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        for (r, row) in rows.iter().enumerate() {
            let r = r as u32 + 1;
            ws.write_string(r, 0, row.get::<&str, _>("student_code")).ok();
            ws.write_string(r, 1, row.get::<&str, _>("name")).ok();
            write_value(ws, r, 2, row.get::<&str, _>("value"), &area.calc_type);
        }
    }

    let buf = wb
        .save_to_buffer()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(excel::xlsx_response(buf, &format!("base_data_{}.xlsx", excel::now_tag())))
}

/// POST /api/areas/:id/base-data/import
pub async fn base_data_import(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    multipart: Multipart,
) -> Result<Json<ImportResult>, ApiError> {
    let area = get_area(&state.db, id).await?;
    let bytes = read_file(multipart).await?;
    let file_rows = excel::parse_file_rows(&bytes)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let mut tx = state.db.begin().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    sqlx::query("DELETE FROM base_data WHERE area_id = ?")
        .bind(id).execute(&mut *tx).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut rows = 0usize;
    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    for (i, cols) in file_rows.iter().enumerate() {
        let row_num = i + 2;
        let g = |j: usize| cols.get(j).map(|s| s.as_str()).unwrap_or("");

        let student_code = g(0).trim();
        if student_code.is_empty() {
            errors.push(format!("{}행: student_code 누락", row_num));
            continue;
        }

        let raw_value = g(2).trim();
        if raw_value.is_empty() {
            errors.push(format!("{}행: value 누락", row_num));
            continue;
        }

        // 학번으로 학생 조회 (없으면 오류)
        let student_id: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM students WHERE student_code = ?",
        )
        .bind(student_code)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let student_id = match student_id {
            Some(sid) => sid,
            None => {
                errors.push(format!("{}행: 학번 '{}' 없음 (학생을 먼저 등록하세요)", row_num, student_code));
                continue;
            }
        };

        // value 변환 (RANGE/MANUAL: ×10000, CATEGORY: 그대로)
        let db_value = match area.calc_type.as_str() {
            "RANGE" | "MANUAL" => match display_to_db(raw_value) {
                Some(v) => v.to_string(),
                None => {
                    errors.push(format!("{}행: value '{}' 숫자 파싱 실패", row_num, raw_value));
                    continue;
                }
            },
            _ => raw_value.to_string(),
        };

        // COMPOSITE: 대학 조회/생성
        let univ_id = match resolve_univ(&state.db, &area, cols, 3, 4, row_num, &mut errors, &mut warnings).await {
            Some(v) => v,
            None => continue,
        };

        match sqlx::query(
            "INSERT INTO base_data (student_id, area_id, univ_id, value) VALUES (?, ?, ?, ?)",
        )
        .bind(student_id).bind(id).bind(univ_id).bind(&db_value)
        .execute(&mut *tx).await {
            Ok(_) => rows += 1,
            Err(e) => errors.push(format!("{}행: {}", row_num, e)),
        }
    }

    tx.commit().await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(ImportResult { rows, errors, warnings }))
}

// ── xlsx 쓰기 헬퍼 ───────────────────────────────────────────────

/// DB value 문자열 → xlsx 셀 (RANGE/MANUAL: ÷10000 숫자, CATEGORY: 문자열)
fn write_value(ws: &mut rust_xlsxwriter::Worksheet, row: u32, col: u16, value: &str, calc_type: &str) {
    match calc_type {
        "RANGE" | "MANUAL" => {
            if let Ok(v) = value.parse::<i64>() {
                ws.write_number(row, col, db_to_display(v)).ok();
            } else {
                ws.write_string(row, col, value).ok();
            }
        }
        _ => {
            ws.write_string(row, col, value).ok();
        }
    }
}
