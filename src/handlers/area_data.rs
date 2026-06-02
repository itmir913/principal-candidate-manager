/// 전형요소별 데이터 Excel 업로드/다운로드 핸들러
/// - 점수 기준: numeric_table (RANGE), category_map (CATEGORY)
/// - 기초 데이터: base_data (모든 calc_type)
use std::collections::HashSet;

use axum::{
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
    response::Response,
    Json,
};
use rust_xlsxwriter::Workbook;
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::{
    enums::{CalcType, LookupScope, MatchMode},
    excel, score::Score, state::AppState,
};

type ApiError = (StatusCode, String);
type Db = sqlx::SqlitePool;

// ── 공통 구조체 ──────────────────────────────────────────────────

#[derive(Serialize)]
pub struct ImportResult {
    pub rows: usize,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Deserialize)]
pub struct StudentTypeQuery {
    #[serde(default = "student_type_graduated")]
    pub student_type: String,
}
fn student_type_graduated() -> String { "graduated".to_string() }

#[derive(Deserialize)]
pub struct PageQuery {
    #[serde(default = "default_page")]     pub page: i64,
    #[serde(default = "default_per_page")] pub per_page: i64,
}

#[derive(Deserialize)]
pub struct BaseDataPageQuery {
    #[serde(default = "default_page")]          pub page: i64,
    #[serde(default = "default_per_page")]      pub per_page: i64,
    #[serde(default = "student_type_enrolled")] pub student_type: String,
}
fn student_type_enrolled() -> String { "enrolled".to_string() }

fn default_page() -> i64 { 1 }
fn default_per_page() -> i64 { 50 }

#[derive(sqlx::FromRow)]
pub(crate) struct AreaInfo {
    pub(crate) max_score: i64,
    pub(crate) calc_type: CalcType,
    pub(crate) lookup_scope: LookupScope,
    pub(crate) match_mode: Option<MatchMode>,
    pub(crate) multi_value: bool,
}

// ── 공통 헬퍼 ────────────────────────────────────────────────────

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

pub(crate) fn fmt_score(v: i64) -> String {
    let s = format!("{:.5}", v as f64 / 100_000.0);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

pub(crate) async fn get_area(db: &Db, id: i64) -> Result<AreaInfo, ApiError> {
    sqlx::query_as::<_, AreaInfo>(
        "SELECT max_score, calc_type, lookup_scope, match_mode, multi_value FROM areas WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or_else(|| (StatusCode::NOT_FOUND, format!("전형요소 id={} 없음", id)))
}

/// 대학+모집단위가 없으면 자동 생성 후 (track_id, 생성여부) 반환.
/// 호출자의 트랜잭션 연결을 받아 같은 tx 안에서 실행한다.
pub(crate) async fn find_or_create_track(
    conn: &mut sqlx::SqliteConnection,
    univ_name: &str,
    track_name: &str,
) -> Result<(i64, bool), ApiError> {
    // 1단계: 대학 마스터 조회 or 생성
    let univ_id: i64 = if let Some(id) = sqlx::query_scalar::<_, i64>(
        "SELECT id FROM universities WHERE univ_name = ?",
    )
    .bind(univ_name)
    .fetch_optional(&mut *conn)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    {
        id
    } else {
        sqlx::query_scalar(
            "INSERT INTO universities (univ_name) VALUES (?) RETURNING id",
        )
        .bind(univ_name)
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    };

    // 2단계: 모집단위 조회 or 생성
    if let Some(id) = sqlx::query_scalar::<_, i64>(
        "SELECT id FROM univ_tracks WHERE univ_id = ? AND track_name = ?",
    )
    .bind(univ_id)
    .bind(track_name)
    .fetch_optional(&mut *conn)
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
    .fetch_one(&mut *conn)
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
    if area.lookup_scope == LookupScope::Composite {
        vec![key_col, "점수", "대학명", "모집단위명"]
    } else {
        vec![key_col, "점수"]
    }
}

/// COMPOSITE 전형요소: track_id 조회/생성 (열 이름 기반).
/// 호출자 tx 커넥션을 받아 같은 tx 안에서 실행 — import 실패 시 대학/트랙 생성도 롤백됨.
async fn resolve_track(
    conn: &mut sqlx::SqliteConnection,
    area: &AreaInfo,
    cols: &[String],
    col: &std::collections::HashMap<String, usize>,
    row_num: usize,
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
) -> Option<Option<i64>> {
    if area.lookup_scope == LookupScope::Composite {
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
        match find_or_create_track(conn, un, tn).await {
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
    if area.calc_type != CalcType::Numeric {
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
    if area.lookup_scope == LookupScope::Composite {
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
            ws.write_number(r, 0, row.get::<i64, _>("threshold") as f64 / 100_000.0).ok();
            ws.write_number(r, 1, row.get::<i64, _>("score") as f64 / 100_000.0).ok();
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
            ws.write_number(r, 0, row.get::<i64, _>("threshold") as f64 / 100_000.0).ok();
            ws.write_number(r, 1, row.get::<i64, _>("score") as f64 / 100_000.0).ok();
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
    if area.calc_type != CalcType::Numeric {
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

        let track_id = match resolve_track(&mut *tx, &area, cols, &col, row_num, &mut errors, &mut warnings).await {
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

    // ▲ 이상(Upper) 방향: 기준값 0 행이 없으면 최저값 미만 학생이 점수 산출 실패 → 경고
    if area.match_mode == Some(MatchMode::Upper) {
        let track_ids: HashSet<Option<i64>> = seen.iter().map(|(tid, _)| *tid).collect();
        for tid in &track_ids {
            if !seen.contains(&(*tid, 0)) {
                let min_th = seen.iter()
                    .filter(|(t, _)| t == tid)
                    .map(|(_, th)| *th)
                    .min()
                    .unwrap_or(0);
                let label = match tid {
                    Some(t) => format!(" (모집단위 id={})", t),
                    None => String::new(),
                };
                warnings.push(format!(
                    "기준값 0 항목이 없습니다{}: 최저 기준값 {} 미만 학생은 점수 산출이 되지 않습니다",
                    label, fmt_score(min_th)
                ));
            }
        }
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
    if area.calc_type != CalcType::Category {
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

    if area.lookup_scope == LookupScope::Composite {
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
            ws.write_number(r, 1, row.get::<i64, _>("score") as f64 / 100_000.0).ok();
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
            ws.write_number(r, 1, row.get::<i64, _>("score") as f64 / 100_000.0).ok();
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
    if area.calc_type != CalcType::Category {
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

        let track_id = match resolve_track(&mut *tx, &area, cols, &col, row_num, &mut errors, &mut warnings).await {
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

    // 0점 항목 검증: (area_id, track_id) 그룹별로 score=0 행이 최소 1개 이상 필요
    let groups: Vec<(i64,)> = sqlx::query_as::<_, (i64,)>(
        "SELECT DISTINCT COALESCE(track_id, 0) FROM category_map WHERE area_id = ?",
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    for (track_id_or_zero,) in &groups {
        let track_id = if *track_id_or_zero == 0 { None } else { Some(*track_id_or_zero) };

        // 양수 점수가 하나도 없으면(감점 전용 그룹) 0점 행 없이도 허용 — 미해당 학생의 0점이 암묵적 기본값
        let has_positive: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM category_map WHERE area_id = ? AND COALESCE(track_id, 0) = ? AND score > 0)",
        )
        .bind(id)
        .bind(track_id_or_zero)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        if !has_positive {
            continue;
        }

        let has_zero: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM category_map WHERE area_id = ? AND COALESCE(track_id, 0) = ? AND score = 0)",
        )
        .bind(id)
        .bind(track_id_or_zero)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        if !has_zero {
            let track_label = if let Some(tid) = track_id {
                format!(" (모집단위 id={})", tid)
            } else {
                " (공통)".to_string()
            };
            errors.push(format!(
                "전형요소 점수 0점 기준(해당하지 않음)이 필수입니다{}: 가장 낮은 점수를 0점으로 설정해 주세요",
                track_label
            ));
        }
    }

    if !errors.is_empty() {
        return Ok((StatusCode::UNPROCESSABLE_ENTITY, Json(ImportResult { rows: 0, errors, warnings: vec![] })));
    }

    tx.commit().await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok((StatusCode::OK, Json(ImportResult { rows, errors: vec![], warnings })))
}

// ── BASE DATA ────────────────────────────────────────────────────

/// GET /api/areas/:id/base-data/template?student_type=enrolled|graduated
pub async fn base_data_template(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(q): Query<StudentTypeQuery>,
) -> Result<Response, ApiError> {
    let area = get_area(&state.db, id).await?;
    let composite = area.lookup_scope == LookupScope::Composite;

    // 재학생: 빈 양식만 반환
    if q.student_type == "enrolled" {
        let headers: Vec<&str> = if composite {
            vec!["학년", "반", "번호", "이름", "값", "대학명", "모집단위명"]
        } else {
            vec!["학년", "반", "번호", "이름", "값"]
        };
        let buf = simple_template(&headers)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        return Ok(excel::xlsx_response(buf, "base_data_template.xlsx"));
    }

    // 졸업생: 학생 명단 + (COMPOSITE이면 모집단위) 미리 채워서 반환
    let graduates = sqlx::query(
        "SELECT student_code, name FROM students WHERE is_enrolled = 0 ORDER BY student_code",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();

    if composite {
        for (i, h) in ["학생코드", "이름", "값", "대학명", "모집단위명"].iter().enumerate() {
            ws.write_string(0, i as u16, *h).ok();
        }

        let tracks = sqlx::query(
            "SELECT u.univ_name, ut.track_name
             FROM univ_tracks ut
             JOIN universities u ON ut.univ_id = u.id
             ORDER BY u.univ_name, ut.track_name",
        )
        .fetch_all(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let mut row_i: u32 = 1;
        for g in &graduates {
            let code: &str = g.get("student_code");
            let name: &str = g.get("name");
            for t in &tracks {
                let univ: &str = t.get("univ_name");
                let track: &str = t.get("track_name");
                ws.write_string(row_i, 0, code).ok();
                ws.write_string(row_i, 1, name).ok();
                // 값 열(2)은 공백
                ws.write_string(row_i, 3, univ).ok();
                ws.write_string(row_i, 4, track).ok();
                row_i += 1;
            }
        }
    } else {
        for (i, h) in ["학생코드", "이름", "값"].iter().enumerate() {
            ws.write_string(0, i as u16, *h).ok();
        }
        for (i, g) in graduates.iter().enumerate() {
            let r = i as u32 + 1;
            ws.write_string(r, 0, g.get::<&str, _>("student_code")).ok();
            ws.write_string(r, 1, g.get::<&str, _>("name")).ok();
            // 값 열(2)은 공백
        }
    }

    let buf = wb.save_to_buffer()
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

    if area.lookup_scope == LookupScope::Composite {
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
            write_value(ws, r, 2, row.get::<&str, _>("value"), area.calc_type);
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
            write_value(ws, r, 2, row.get::<&str, _>("value"), area.calc_type);
        }
    }

    let buf = wb
        .save_to_buffer()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(excel::xlsx_response(buf, &format!("base_data_{}.xlsx", excel::now_tag())))
}

/// POST /api/areas/:id/base-data/import?student_type=enrolled|graduated
pub async fn base_data_import(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(q): Query<StudentTypeQuery>,
    multipart: Multipart,
) -> Result<(StatusCode, Json<ImportResult>), ApiError> {
    let area = get_area(&state.db, id).await?;
    let enrolled = q.student_type == "enrolled";
    let bytes = read_file(multipart).await?;
    let (headers, file_rows) = excel::parse_file_rows_with_headers(&bytes)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let col = excel::col_map(&headers);
    if enrolled {
        excel::require_cols(&col, &["학년", "반", "번호", "값"])
            .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    } else {
        excel::require_cols(&col, &["학생코드", "값"])
            .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    }

    let mut tx = state.db.begin().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    // 재학생/졸업생 데이터를 각각 독립적으로 교체 — 반대 student_type 데이터는 보존
    let is_enrolled_val = if enrolled { 1i64 } else { 0i64 };
    sqlx::query(
        "DELETE FROM base_data WHERE area_id = ? AND student_id IN (SELECT id FROM students WHERE is_enrolled = ?)"
    )
    .bind(id).bind(is_enrolled_val).execute(&mut *tx).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut rows = 0usize;
    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    // multi_value=0 전형요소: (student_id, track_id) 중복 추적 — 첫 번째 행 우선
    let single_value = !area.multi_value;
    let mut seen: HashSet<(i64, Option<i64>)> = HashSet::new();

    for (i, cols) in file_rows.iter().enumerate() {
        let row_num = i + 2;

        // ── 학생 조회 ──────────────────────────────────────────────
        let student_id: i64;
        let student_label: String;
        if enrolled {
            let grade_s   = excel::get_col(cols, &col, "학년");
            let class_s   = excel::get_col(cols, &col, "반");
            let seq_s     = excel::get_col(cols, &col, "번호");
            if grade_s.is_empty() || class_s.is_empty() || seq_s.is_empty() {
                errors.push(format!("{}행: 학년/반/번호 누락", row_num));
                continue;
            }
            let grade: i64 = match grade_s.parse() {
                Ok(v) => v,
                Err(_) => { errors.push(format!("{}행: 학년 '{}' 숫자 변환 실패", row_num, grade_s)); continue; }
            };
            let class_no: i64 = match class_s.parse() {
                Ok(v) => v,
                Err(_) => { errors.push(format!("{}행: 반 '{}' 숫자 변환 실패", row_num, class_s)); continue; }
            };
            let seq_no: i64 = match seq_s.parse() {
                Ok(v) => v,
                Err(_) => { errors.push(format!("{}행: 번호 '{}' 숫자 변환 실패", row_num, seq_s)); continue; }
            };
            let sid: Option<i64> = sqlx::query_scalar(
                "SELECT id FROM students WHERE grade = ? AND class_no = ? AND seq_no = ? AND is_enrolled = 1",
            )
            .bind(grade).bind(class_no).bind(seq_no)
            .fetch_optional(&state.db).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            match sid {
                Some(v) => {
                    student_id = v;
                    student_label = format!("{}학년 {}반 {}번", grade, class_no, seq_no);
                }
                None => {
                    errors.push(format!("{}행: {}학년 {}반 {}번 — 등록된 재학생을 찾을 수 없습니다", row_num, grade, class_no, seq_no));
                    continue;
                }
            }
        } else {
            let student_code = excel::get_col(cols, &col, "학생코드");
            if student_code.is_empty() {
                errors.push(format!("{}행: 학생코드 누락", row_num));
                continue;
            }
            let sid: Option<i64> = sqlx::query_scalar(
                "SELECT id FROM students WHERE student_code = ?",
            )
            .bind(student_code)
            .fetch_optional(&state.db).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            match sid {
                Some(v) => {
                    student_id = v;
                    student_label = format!("학생코드 '{}'", student_code);
                }
                None => {
                    errors.push(format!("{}행: 학생코드 '{}' 없음 (학생을 먼저 등록하세요)", row_num, student_code));
                    continue;
                }
            }
        }

        let raw_value = excel::get_col(cols, &col, "값");
        if raw_value.is_empty() {
            errors.push(format!("{}행: 값 누락", row_num));
            continue;
        }

        // value 변환 (NUMERIC/MANUAL: ×100000, CATEGORY: 그대로)
        let db_value = match area.calc_type {
            CalcType::Numeric | CalcType::Manual => {
                let v = match parse_display_value(raw_value) {
                    Ok(v) => v,
                    Err(e) => {
                        errors.push(format!("{}행: 값 — {}", row_num, e));
                        continue;
                    }
                };
                // MANUAL: 입력값이 곧 점수 — 만점 초과 금지
                if area.calc_type == CalcType::Manual && v > area.max_score {
                    errors.push(format!(
                        "{}행: 값({})이 전형요소 만점({})을 초과합니다",
                        row_num, fmt_score(v), fmt_score(area.max_score)
                    ));
                    continue;
                }
                v.to_string()
            }
            CalcType::Category => raw_value.to_string(),
        };

        // COMPOSITE: 모집단위 조회/생성
        let track_id = match resolve_track(&mut *tx, &area, cols, &col, row_num, &mut errors, &mut warnings).await {
            Some(v) => v,
            None => continue,
        };

        // 단일값 전형요소: 동일 (student, track) 중복 행은 전체 import 거부
        if single_value && !seen.insert((student_id, track_id)) {
            errors.push(format!(
                "{}행: {} 중복 — 파일에 같은 학생이 두 번 이상 존재합니다",
                row_num, student_label
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
    pub threshold: Score,
    pub score: Score,
    pub univ_name: Option<String>,
    pub track_name: Option<String>,
}

#[derive(Serialize)]
pub struct NumericTablePage {
    pub rows: Vec<RangeTableListRow>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

#[derive(Serialize)]
pub struct CategoryMapListRow {
    pub category: String,
    pub score: Score,
    pub univ_name: Option<String>,
    pub track_name: Option<String>,
}

#[derive(Serialize)]
pub struct CategoryMapPage {
    pub rows: Vec<CategoryMapListRow>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

#[derive(Debug, Serialize)]
pub struct BaseDataListRow {
    pub student_code: String,
    pub name: String,
    pub value: String,
    pub univ_name: Option<String>,
    pub track_name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BaseDataPage {
    pub rows: Vec<BaseDataListRow>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

/// GET /api/areas/:id/range-table/list
pub async fn numeric_table_list(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(q): Query<PageQuery>,
) -> Result<Json<NumericTablePage>, ApiError> {
    let area = get_area(&state.db, id).await?;
    let composite = area.lookup_scope == LookupScope::Composite;

    let per_page = q.per_page.max(1);
    let page = q.page.max(1);
    let offset = (page - 1) * per_page;

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM numeric_table WHERE area_id = ?")
        .bind(id)
        .fetch_one(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let rows = sqlx::query(
        "SELECT rt.threshold, rt.score,
                COALESCE(u.univ_name, '') AS univ_name,
                COALESCE(ut.track_name, '') AS track_name
         FROM numeric_table rt
         LEFT JOIN univ_tracks ut ON rt.track_id = ut.id
         LEFT JOIN universities u ON ut.univ_id = u.id
         WHERE rt.area_id = ?
         ORDER BY u.univ_name, ut.track_name, rt.score DESC, rt.threshold
         LIMIT ? OFFSET ?",
    )
    .bind(id)
    .bind(per_page)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let result = rows
        .iter()
        .map(|row| RangeTableListRow {
            threshold: Score::from_raw(row.get("threshold")),
            score: Score::from_raw(row.get("score")),
            univ_name: if composite { Some(row.get("univ_name")) } else { None },
            track_name: if composite { Some(row.get("track_name")) } else { None },
        })
        .collect();
    Ok(Json(NumericTablePage { rows: result, total, page, per_page }))
}

/// GET /api/areas/:id/category-map/list
pub async fn category_map_list(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(q): Query<PageQuery>,
) -> Result<Json<CategoryMapPage>, ApiError> {
    let area = get_area(&state.db, id).await?;
    let composite = area.lookup_scope == LookupScope::Composite;

    let per_page = q.per_page.max(1);
    let page = q.page.max(1);
    let offset = (page - 1) * per_page;

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM category_map WHERE area_id = ?")
        .bind(id)
        .fetch_one(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let rows = sqlx::query(
        "SELECT cm.category, cm.score,
                COALESCE(u.univ_name, '') AS univ_name,
                COALESCE(ut.track_name, '') AS track_name
         FROM category_map cm
         LEFT JOIN univ_tracks ut ON cm.track_id = ut.id
         LEFT JOIN universities u ON ut.univ_id = u.id
         WHERE cm.area_id = ?
         ORDER BY u.univ_name, ut.track_name, cm.score DESC, cm.category
         LIMIT ? OFFSET ?",
    )
    .bind(id)
    .bind(per_page)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let result = rows
        .iter()
        .map(|row| CategoryMapListRow {
            category: row.get("category"),
            score: Score::from_raw(row.get("score")),
            univ_name: if composite { Some(row.get("univ_name")) } else { None },
            track_name: if composite { Some(row.get("track_name")) } else { None },
        })
        .collect();
    Ok(Json(CategoryMapPage { rows: result, total, page, per_page }))
}

/// GET /api/areas/:id/base-data/list
pub async fn base_data_list(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(q): Query<BaseDataPageQuery>,
) -> Result<Json<BaseDataPage>, ApiError> {
    let area = get_area(&state.db, id).await?;
    let composite = area.lookup_scope == LookupScope::Composite;
    let is_enrolled_val = if q.student_type != "graduated" { 1i64 } else { 0i64 };

    let per_page = q.per_page.max(1);
    let page = q.page.max(1);
    let offset = (page - 1) * per_page;

    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM base_data bd
         JOIN students s ON bd.student_id = s.id
         WHERE bd.area_id = ? AND s.is_enrolled = ?",
    )
    .bind(id)
    .bind(is_enrolled_val)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let rows = sqlx::query(
        "SELECT s.student_code, s.name, bd.value,
                COALESCE(u.univ_name, '') AS univ_name,
                COALESCE(ut.track_name, '') AS track_name
         FROM base_data bd
         JOIN students s ON bd.student_id = s.id
         LEFT JOIN univ_tracks ut ON bd.track_id = ut.id
         LEFT JOIN universities u ON ut.univ_id = u.id
         WHERE bd.area_id = ? AND s.is_enrolled = ?
         ORDER BY bd.track_id, s.grade, s.class_no, s.seq_no
         LIMIT ? OFFSET ?",
    )
    .bind(id)
    .bind(is_enrolled_val)
    .bind(per_page)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let result: Vec<BaseDataListRow> = rows
        .iter()
        .map(|row| {
            let raw: String = row.get("value");
            let value = match area.calc_type {
                CalcType::Numeric | CalcType::Manual => {
                    let v = raw.parse::<i64>().map_err(|_| (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("base_data 값 '{}' 을 정수로 파싱할 수 없습니다 (area_id={})", raw, id),
                    ))?;
                    format!("{}", v as f64 / 100_000.0)
                }
                CalcType::Category => raw,
            };
            Ok(BaseDataListRow {
                student_code: row.get("student_code"),
                name: row.get("name"),
                value,
                univ_name: if composite { Some(row.get("univ_name")) } else { None },
                track_name: if composite { Some(row.get("track_name")) } else { None },
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;

    Ok(Json(BaseDataPage { rows: result, total, page, per_page }))
}

// ── xlsx 쓰기 헬퍼 ───────────────────────────────────────────────

/// DB value 문자열 → xlsx 셀 (NUMERIC/MANUAL: ÷100000 숫자, CATEGORY: 문자열)
fn write_value(ws: &mut rust_xlsxwriter::Worksheet, row: u32, col: u16, value: &str, calc_type: CalcType) {
    match calc_type {
        CalcType::Numeric | CalcType::Manual => {
            if let Ok(v) = value.parse::<i64>() {
                ws.write_number(row, col, v as f64 / 100_000.0).ok();
            } else {
                ws.write_string(row, col, value).ok();
            }
        }
        CalcType::Category => {
            ws.write_string(row, col, value).ok();
        }
    }
}
