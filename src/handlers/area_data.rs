/// 전형요소별 데이터 Excel 업로드/다운로드 핸들러
/// - 점수 기준: numeric_table (RANGE), category_map (CATEGORY)
/// - 기초 데이터: base_data (모든 calc_type)
use std::collections::HashSet;

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
pub(crate) struct AreaInfo {
    pub(crate) max_score: i64,
    pub(crate) calc_type: String,
    pub(crate) lookup_scope: String,
    pub(crate) multi_value: i64,
}

// ── 공통 헬퍼 ────────────────────────────────────────────────────

fn db_to_display(v: i64) -> f64 {
    v as f64 / 100_000.0
}

/// 표시값 문자열 → DB 저장값 (×100000). 소수점 5자리 초과 시 Err 반환.
/// 음수 허용: 감점 전형요소(특정 범주 해당 학생 감점)를 지원하기 위해 음수 점수가 가능.
pub(crate) fn parse_display_value(s: &str) -> Result<i64, String> {
    let trimmed = s.trim();
    let f: f64 = trimmed
        .parse()
        .map_err(|_| format!("'{}' 숫자 변환 실패", trimmed))?;
    // 소수점 자릿수 확인 (부호 제거 후 검사)
    let abs_str = trimmed.trim_start_matches('-');
    if let Some(dot_pos) = abs_str.find('.') {
        let decimals = abs_str[dot_pos + 1..].trim_end_matches('0');
        if decimals.len() > 5 {
            return Err(format!("'{}' 소수점 5자리 초과 (최대 5자리)", trimmed));
        }
    }
    Ok((f * 100_000.0).round() as i64)
}

fn simple_template(headers: &[&str]) -> anyhow::Result<Vec<u8>> {
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    for (i, h) in headers.iter().enumerate() {
        ws.write_string(0, i as u16, *h)?;
    }
    Ok(wb.save_to_buffer()?)
}

fn fmt_score(v: i64) -> String {
    let s = format!("{:.5}", v as f64 / 100_000.0);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

pub(crate) async fn get_area(db: &Db, id: i64) -> Result<AreaInfo, ApiError> {
    sqlx::query_as::<_, AreaInfo>(
        "SELECT max_score, calc_type, lookup_scope, multi_value FROM areas WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or_else(|| (StatusCode::NOT_FOUND, format!("전형요소 id={} 없음", id)))
}

/// 대학+모집단위가 없으면 자동 생성 후 (track_id, 생성여부) 반환
pub(crate) async fn find_or_create_track(
    db: &Db,
    univ_name: &str,
    track_name: &str,
) -> Result<(i64, bool), ApiError> {
    // 1단계: 대학 마스터 조회 or 생성
    let univ_id: i64 = if let Some(id) = sqlx::query_scalar::<_, i64>(
        "SELECT id FROM universities WHERE univ_name = ?",
    )
    .bind(univ_name)
    .fetch_optional(db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    {
        id
    } else {
        sqlx::query_scalar(
            "INSERT INTO universities (univ_name) VALUES (?) RETURNING id",
        )
        .bind(univ_name)
        .fetch_one(db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    };

    // 2단계: 모집단위 조회 or 생성
    if let Some(id) = sqlx::query_scalar::<_, i64>(
        "SELECT id FROM univ_tracks WHERE univ_id = ? AND track_name = ?",
    )
    .bind(univ_id)
    .bind(track_name)
    .fetch_optional(db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    {
        return Ok((id, false));
    }

    let id: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name) VALUES (?, ?) RETURNING id",
    )
    .bind(univ_id)
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
        vec![key_col, "점수", "대학명", "모집단위명"]
    } else {
        vec![key_col, "점수"]
    }
}

/// COMPOSITE 전형요소: track_id 조회/생성 (열 이름 기반)
async fn resolve_track(
    db: &Db,
    area: &AreaInfo,
    cols: &[String],
    col: &std::collections::HashMap<String, usize>,
    row_num: usize,
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
) -> Option<Option<i64>> {
    if area.lookup_scope == "COMPOSITE" {
        let un = excel::get_col(cols, col, "대학명");
        let tn = excel::get_col(cols, col, "모집단위명");
        match (un.is_empty(), tn.is_empty()) {
            (true, true) => return Some(None), // 공통 테이블로 저장
            (false, true) | (true, false) => {
                errors.push(format!("{}행: 대학명과 모집단위명은 함께 입력하거나 함께 비워야 합니다", row_num));
                return None;
            }
            (false, false) => {}
        }
        match find_or_create_track(db, un, tn).await {
            Ok((track_id, created)) => {
                if created {
                    warnings.push(format!("'{}/{}' 모집단위 자동 추가됨", un, tn));
                }
                Some(Some(track_id))
            }
            Err(e) => {
                errors.push(format!("{}행: 모집단위 처리 오류 — {}", row_num, e.1));
                None
            }
        }
    } else {
        Some(None)
    }
}

// ── RANGE TABLE ──────────────────────────────────────────────────

/// GET /api/areas/:id/range-table/template
pub async fn numeric_table_template(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Response, ApiError> {
    let area = get_area(&state.db, id).await?;
    if area.calc_type != "NUMERIC" {
        return Err((StatusCode::BAD_REQUEST, "RANGE 전형요소만 구간표를 사용합니다".into()));
    }
    let headers = score_headers(&area, "기준값");
    let buf = simple_template(&headers)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(excel::xlsx_response(buf, "numeric_table_template.xlsx"))
}

/// GET /api/areas/:id/range-table/export
pub async fn numeric_table_export(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Response, ApiError> {
    let area = get_area(&state.db, id).await?;
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    if area.lookup_scope == "COMPOSITE" {
        for (i, h) in ["기준값", "점수", "대학명", "모집단위명"].iter().enumerate() {
            ws.write_string(0, i as u16, *h).ok();
        }
        let rows = sqlx::query(
            "SELECT rt.threshold, rt.score,
                    COALESCE(u.univ_name, '') AS univ_name,
                    COALESCE(ut.track_name, '') AS track_name
             FROM numeric_table rt
             LEFT JOIN univ_tracks ut ON rt.track_id = ut.id
             LEFT JOIN universities u ON ut.univ_id = u.id
             WHERE rt.area_id = ?
             ORDER BY u.univ_name, ut.track_name, rt.score DESC, rt.threshold",
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
        for (i, h) in ["기준값", "점수"].iter().enumerate() {
            ws.write_string(0, i as u16, *h).ok();
        }
        let rows = sqlx::query(
            "SELECT threshold, score FROM numeric_table
             WHERE area_id = ? AND track_id IS NULL ORDER BY score DESC, threshold",
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
    Ok(excel::xlsx_response(buf, &format!("numeric_table_{}.xlsx", excel::now_tag())))
}

/// POST /api/areas/:id/range-table/import
pub async fn numeric_table_import(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    multipart: Multipart,
) -> Result<(StatusCode, Json<ImportResult>), ApiError> {
    let area = get_area(&state.db, id).await?;
    if area.calc_type != "NUMERIC" {
        return Err((StatusCode::BAD_REQUEST, "RANGE 전형요소만 구간표를 사용합니다".into()));
    }
    let bytes = read_file(multipart).await?;
    let (headers, file_rows) = excel::parse_file_rows_with_headers(&bytes)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let col = excel::col_map(&headers);
    excel::require_cols(&col, &["기준값", "점수"])
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let mut tx = state.db.begin().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    sqlx::query("DELETE FROM numeric_table WHERE area_id = ?")
        .bind(id).execute(&mut *tx).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut rows = 0usize;
    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut seen: HashSet<(Option<i64>, i64)> = HashSet::new();

    for (i, cols) in file_rows.iter().enumerate() {
        let row_num = i + 2;

        let th = match parse_display_value(excel::get_col(cols, &col, "기준값")) {
            Ok(v) => v,
            Err(e) => { errors.push(format!("{}행: 기준값 — {}", row_num, e)); continue; }
        };
        let sc = match parse_display_value(excel::get_col(cols, &col, "점수")) {
            Ok(v) => v,
            Err(e) => { errors.push(format!("{}행: 점수 — {}", row_num, e)); continue; }
        };

        if sc > area.max_score {
            errors.push(format!(
                "{}행: 점수({})가 전형요소 만점({})을 초과합니다",
                row_num, fmt_score(sc), fmt_score(area.max_score)
            ));
            continue;
        }

        let track_id = match resolve_track(&state.db, &area, cols, &col, row_num, &mut errors, &mut warnings).await {
            Some(v) => v,
            None => continue,
        };

        if !seen.insert((track_id, th)) {
            errors.push(format!("{}행: 기준값 '{}' 중복 — 같은 기준값은 한 번만 등록할 수 있습니다",
                row_num, excel::get_col(cols, &col, "기준값")));
            continue;
        }

        match sqlx::query(
            "INSERT INTO numeric_table (area_id, track_id, threshold, score) VALUES (?, ?, ?, ?)",
        )
        .bind(id).bind(track_id).bind(th).bind(sc)
        .execute(&mut *tx).await {
            Ok(_) => rows += 1,
            Err(e) => errors.push(format!("{}행: {}", row_num, e)),
        }
    }

    if !errors.is_empty() {
        // tx이 drop되면 자동 rollback — 부분 삽입 없음
        return Ok((StatusCode::UNPROCESSABLE_ENTITY, Json(ImportResult { rows: 0, errors, warnings: vec![] })));
    }
    tx.commit().await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok((StatusCode::OK, Json(ImportResult { rows, errors: vec![], warnings })))
}

// ── CATEGORY MAP ─────────────────────────────────────────────────

/// GET /api/areas/:id/category-map/template
pub async fn category_map_template(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Response, ApiError> {
    let area = get_area(&state.db, id).await?;
    if area.calc_type != "CATEGORY" {
        return Err((StatusCode::BAD_REQUEST, "CATEGORY 전형요소만 범주표를 사용합니다".into()));
    }
    let headers = score_headers(&area, "범주");
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
        for (i, h) in ["범주", "점수", "대학명", "모집단위명"].iter().enumerate() {
            ws.write_string(0, i as u16, *h).ok();
        }
        let rows = sqlx::query(
            "SELECT cm.category, cm.score,
                    COALESCE(u.univ_name, '') AS univ_name,
                    COALESCE(ut.track_name, '') AS track_name
             FROM category_map cm
             LEFT JOIN univ_tracks ut ON cm.track_id = ut.id
             LEFT JOIN universities u ON ut.univ_id = u.id
             WHERE cm.area_id = ?
             ORDER BY u.univ_name, ut.track_name, cm.score DESC, cm.category",
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
        for (i, h) in ["범주", "점수"].iter().enumerate() {
            ws.write_string(0, i as u16, *h).ok();
        }
        let rows = sqlx::query(
            "SELECT category, score FROM category_map
             WHERE area_id = ? AND track_id IS NULL ORDER BY score DESC, category",
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
) -> Result<(StatusCode, Json<ImportResult>), ApiError> {
    let area = get_area(&state.db, id).await?;
    if area.calc_type != "CATEGORY" {
        return Err((StatusCode::BAD_REQUEST, "CATEGORY 전형요소만 범주표를 사용합니다".into()));
    }
    let bytes = read_file(multipart).await?;
    let (headers, file_rows) = excel::parse_file_rows_with_headers(&bytes)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let col = excel::col_map(&headers);
    excel::require_cols(&col, &["범주", "점수"])
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let mut tx = state.db.begin().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    sqlx::query("DELETE FROM category_map WHERE area_id = ?")
        .bind(id).execute(&mut *tx).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut rows = 0usize;
    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut seen: HashSet<(Option<i64>, String)> = HashSet::new();

    for (i, cols) in file_rows.iter().enumerate() {
        let row_num = i + 2;

        let category = excel::get_col(cols, &col, "범주").to_string();
        if category.is_empty() {
            errors.push(format!("{}행: 범주 누락", row_num));
            continue;
        }
        let sc = match parse_display_value(excel::get_col(cols, &col, "점수")) {
            Ok(v) => v,
            Err(e) => { errors.push(format!("{}행: 점수 — {}", row_num, e)); continue; }
        };

        if sc > area.max_score {
            errors.push(format!(
                "{}행: 점수({})가 전형요소 만점({})을 초과합니다",
                row_num, fmt_score(sc), fmt_score(area.max_score)
            ));
            continue;
        }

        let track_id = match resolve_track(&state.db, &area, cols, &col, row_num, &mut errors, &mut warnings).await {
            Some(v) => v,
            None => continue,
        };

        if !seen.insert((track_id, category.clone())) {
            errors.push(format!("{}행: 범주 '{}' 중복 — 같은 범주는 한 번만 등록할 수 있습니다", row_num, category));
            continue;
        }

        match sqlx::query(
            "INSERT INTO category_map (area_id, track_id, category, score) VALUES (?, ?, ?, ?)",
        )
        .bind(id).bind(track_id).bind(&category).bind(sc)
        .execute(&mut *tx).await {
            Ok(_) => rows += 1,
            Err(e) => errors.push(format!("{}행: {}", row_num, e)),
        }
    }

    if !errors.is_empty() {
        return Ok((StatusCode::UNPROCESSABLE_ENTITY, Json(ImportResult { rows: 0, errors, warnings: vec![] })));
    }
    tx.commit().await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok((StatusCode::OK, Json(ImportResult { rows, errors: vec![], warnings })))
}

// ── BASE DATA ────────────────────────────────────────────────────

/// GET /api/areas/:id/base-data/template
pub async fn base_data_template(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Response, ApiError> {
    let area = get_area(&state.db, id).await?;
    let headers: Vec<&str> = if area.lookup_scope == "COMPOSITE" {
        vec!["학생코드", "이름", "값", "대학명", "모집단위명"]
    } else {
        vec!["학생코드", "이름", "값"]
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
        for (i, h) in ["학생코드", "이름", "값", "대학명", "모집단위명"].iter().enumerate() {
            ws.write_string(0, i as u16, *h).ok();
        }
        let rows = sqlx::query(
            "SELECT s.student_code, s.name, bd.value, u.univ_name, ut.track_name
             FROM base_data bd
             JOIN students s ON bd.student_id = s.id
             JOIN univ_tracks ut ON bd.track_id = ut.id
             JOIN universities u ON ut.univ_id = u.id
             WHERE bd.area_id = ?
             ORDER BY u.univ_name, ut.track_name, s.grade, s.class_no, s.seq_no",
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
        for (i, h) in ["학생코드", "이름", "값"].iter().enumerate() {
            ws.write_string(0, i as u16, *h).ok();
        }
        let rows = sqlx::query(
            "SELECT s.student_code, s.name, bd.value
             FROM base_data bd
             JOIN students s ON bd.student_id = s.id
             WHERE bd.area_id = ? AND bd.track_id IS NULL
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
) -> Result<(StatusCode, Json<ImportResult>), ApiError> {
    let area = get_area(&state.db, id).await?;
    let bytes = read_file(multipart).await?;
    let (headers, file_rows) = excel::parse_file_rows_with_headers(&bytes)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let col = excel::col_map(&headers);
    excel::require_cols(&col, &["학생코드", "값"])
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let mut tx = state.db.begin().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    sqlx::query("DELETE FROM base_data WHERE area_id = ?")
        .bind(id).execute(&mut *tx).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut rows = 0usize;
    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    // multi_value=0 전형요소: (student_id, track_id) 중복 추적 — 첫 번째 행 우선
    let single_value = area.multi_value == 0;
    let mut seen: HashSet<(i64, Option<i64>)> = HashSet::new();

    for (i, cols) in file_rows.iter().enumerate() {
        let row_num = i + 2;

        let student_code = excel::get_col(cols, &col, "학생코드");
        if student_code.is_empty() {
            errors.push(format!("{}행: 학생코드 누락", row_num));
            continue;
        }

        let raw_value = excel::get_col(cols, &col, "값");
        if raw_value.is_empty() {
            errors.push(format!("{}행: 값 누락", row_num));
            continue;
        }

        // 학생코드로 학생 조회 (없으면 오류)
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
                errors.push(format!("{}행: 학생코드 '{}' 없음 (학생을 먼저 등록하세요)", row_num, student_code));
                continue;
            }
        };

        // value 변환 (NUMERIC/MANUAL: ×100000, CATEGORY: 그대로)
        let db_value = match area.calc_type.as_str() {
            "NUMERIC" | "MANUAL" => match parse_display_value(raw_value) {
                Ok(v) => v.to_string(),
                Err(e) => {
                    errors.push(format!("{}행: 값 — {}", row_num, e));
                    continue;
                }
            },
            _ => raw_value.to_string(),
        };

        // COMPOSITE: 모집단위 조회/생성
        let track_id = match resolve_track(&state.db, &area, cols, &col, row_num, &mut errors, &mut warnings).await {
            Some(v) => v,
            None => continue,
        };

        // 단일값 전형요소: 동일 (student, track) 중복 행은 전체 import 거부
        if single_value && !seen.insert((student_id, track_id)) {
            errors.push(format!(
                "{}행: 학생코드 '{}' 중복 — 파일에 같은 학생이 두 번 이상 존재합니다",
                row_num, student_code
            ));
            continue;
        }

        match sqlx::query(
            "INSERT INTO base_data (student_id, area_id, track_id, value, multi_value) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(student_id).bind(id).bind(track_id).bind(&db_value).bind(area.multi_value)
        .execute(&mut *tx).await {
            Ok(_) => rows += 1,
            Err(e) => errors.push(format!("{}행: {}", row_num, e)),
        }
    }

    if !errors.is_empty() {
        return Ok((StatusCode::UNPROCESSABLE_ENTITY, Json(ImportResult { rows: 0, errors, warnings: vec![] })));
    }
    tx.commit().await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok((StatusCode::OK, Json(ImportResult { rows, errors: vec![], warnings })))
}

// ── LIST (JSON 조회) ─────────────────────────────────────────────

#[derive(Serialize)]
pub struct RangeTableListRow {
    pub threshold: f64,
    pub score: f64,
    pub univ_name: Option<String>,
    pub track_name: Option<String>,
}

#[derive(Serialize)]
pub struct CategoryMapListRow {
    pub category: String,
    pub score: f64,
    pub univ_name: Option<String>,
    pub track_name: Option<String>,
}

#[derive(Serialize)]
pub struct BaseDataListRow {
    pub student_code: String,
    pub name: String,
    pub value: String,
    pub univ_name: Option<String>,
    pub track_name: Option<String>,
}

/// GET /api/areas/:id/range-table/list
pub async fn numeric_table_list(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Vec<RangeTableListRow>>, ApiError> {
    let area = get_area(&state.db, id).await?;
    let composite = area.lookup_scope == "COMPOSITE";

    let rows = sqlx::query(
        "SELECT rt.threshold, rt.score,
                COALESCE(u.univ_name, '') AS univ_name,
                COALESCE(ut.track_name, '') AS track_name
         FROM numeric_table rt
         LEFT JOIN univ_tracks ut ON rt.track_id = ut.id
         LEFT JOIN universities u ON ut.univ_id = u.id
         WHERE rt.area_id = ?
         ORDER BY u.univ_name, ut.track_name, rt.score DESC, rt.threshold",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let result = rows
        .iter()
        .map(|row| RangeTableListRow {
            threshold: db_to_display(row.get("threshold")),
            score: db_to_display(row.get("score")),
            univ_name: if composite { Some(row.get("univ_name")) } else { None },
            track_name: if composite { Some(row.get("track_name")) } else { None },
        })
        .collect();
    Ok(Json(result))
}

/// GET /api/areas/:id/category-map/list
pub async fn category_map_list(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Vec<CategoryMapListRow>>, ApiError> {
    let area = get_area(&state.db, id).await?;
    let composite = area.lookup_scope == "COMPOSITE";

    let rows = sqlx::query(
        "SELECT cm.category, cm.score,
                COALESCE(u.univ_name, '') AS univ_name,
                COALESCE(ut.track_name, '') AS track_name
         FROM category_map cm
         LEFT JOIN univ_tracks ut ON cm.track_id = ut.id
         LEFT JOIN universities u ON ut.univ_id = u.id
         WHERE cm.area_id = ?
         ORDER BY u.univ_name, ut.track_name, cm.score DESC, cm.category",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let result = rows
        .iter()
        .map(|row| CategoryMapListRow {
            category: row.get("category"),
            score: db_to_display(row.get("score")),
            univ_name: if composite { Some(row.get("univ_name")) } else { None },
            track_name: if composite { Some(row.get("track_name")) } else { None },
        })
        .collect();
    Ok(Json(result))
}

/// GET /api/areas/:id/base-data/list
pub async fn base_data_list(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Vec<BaseDataListRow>>, ApiError> {
    let area = get_area(&state.db, id).await?;
    let composite = area.lookup_scope == "COMPOSITE";

    let rows = sqlx::query(
        "SELECT s.student_code, s.name, bd.value,
                COALESCE(u.univ_name, '') AS univ_name,
                COALESCE(ut.track_name, '') AS track_name
         FROM base_data bd
         JOIN students s ON bd.student_id = s.id
         LEFT JOIN univ_tracks ut ON bd.track_id = ut.id
         LEFT JOIN universities u ON ut.univ_id = u.id
         WHERE bd.area_id = ?
         ORDER BY bd.track_id, s.grade, s.class_no, s.seq_no",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let result = rows
        .iter()
        .map(|row| {
            let raw: String = row.get("value");
            let value = match area.calc_type.as_str() {
                "NUMERIC" | "MANUAL" => raw
                    .parse::<i64>()
                    .map(|v| format!("{}", db_to_display(v)))
                    .unwrap_or(raw),
                _ => raw,
            };
            BaseDataListRow {
                student_code: row.get("student_code"),
                name: row.get("name"),
                value,
                univ_name: if composite { Some(row.get("univ_name")) } else { None },
                track_name: if composite { Some(row.get("track_name")) } else { None },
            }
        })
        .collect();
    Ok(Json(result))
}

// ── xlsx 쓰기 헬퍼 ───────────────────────────────────────────────

/// DB value 문자열 → xlsx 셀 (NUMERIC/MANUAL: ÷100000 숫자, CATEGORY: 문자열)
fn write_value(ws: &mut rust_xlsxwriter::Worksheet, row: u32, col: u16, value: &str, calc_type: &str) {
    match calc_type {
        "NUMERIC" | "MANUAL" => {
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
