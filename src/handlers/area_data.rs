/// 전형요소별 데이터 Excel 업로드/다운로드 핸들러
/// - 점수 기준: numeric_table (RANGE), category_map (CATEGORY)
/// - 기초 데이터: base_data (모든 calc_type)
use std::collections::{HashMap, HashSet};

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
    audit::{self, Actor, AuditEntry},
    enums::{AuditAction, CalcType, LookupScope, MatchMode},
    excel, middleware::multipart_err, score::Score, state::AppState,
    handlers::areas::guard_no_closed_round,
};

type ApiError = (StatusCode, String);
type Db = sqlx::SqlitePool;

// ── 공통 구조체 ──────────────────────────────────────────────────

#[derive(Serialize)]
pub struct ImportResult {
    pub rows: usize,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub info: Vec<String>,
}

#[derive(Deserialize)]
pub struct StudentTypeQuery {
    // 기본값은 BaseDataPageQuery와 동일하게 enrolled — 파라미터 누락 시
    // 목록은 재학생인데 import는 졸업생으로 동작하는 비대칭을 막는다
    #[serde(default = "student_type_enrolled")]
    pub student_type: String,
}

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

/// student_type 검증 — enrolled=true / graduated=false, 그 외 값은 silent fallback 없이 400
pub(crate) fn parse_student_type(s: &str) -> Result<bool, ApiError> {
    match s {
        "enrolled" => Ok(true),
        "graduated" => Ok(false),
        other => Err((
            StatusCode::BAD_REQUEST,
            format!("student_type은 'enrolled' 또는 'graduated'만 허용됩니다 (입력값: '{}')", other),
        )),
    }
}

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
/// pub: tests/invariants.rs의 왕복 불변식 테스트에서 사용 (pub(crate)는 tests/에서 접근 불가)
pub fn parse_display_value(s: &str) -> Result<i64, String> {
    let trimmed = s.trim();
    // 지수 표기(1e-6, 2E5 등) 거부.
    // Rust f64 파서는 지수 표기를 수용하지만, 아래 소수 자릿수 검사가 원본 문자열의
    // '.' 위치에 의존하므로 지수 표기 입력은 자릿수 검사를 조용히 우회한다.
    // 예: "1e-6"은 0.1로 계산되어 round()로 0이 저장되지만, 동일 값 "0.000001"은
    // "소수점 5자리 초과"로 거부된다 — 표기에 따라 다른 결과가 나오는 것 자체가
    // Fail-Fast 위반. 학교 성적·점수 도메인에 지수 표기가 필요한 정당한 이유도 없다.
    if trimmed.contains('e') || trimmed.contains('E') {
        return Err(format!(
            "'{}' 지수 표기는 지원되지 않습니다. 소수 표기를 사용하세요 (예: 0.000001)",
            trimmed
        ));
    }
    let f: f64 = trimmed
        .parse()
        .map_err(|_| format!("'{}' 숫자 변환 실패", trimmed))?;
    // Rust f64 파서는 "nan"/"inf" 문자열을 허용하고, `as i64` 캐스트는
    // NaN→0, ±∞→i64::MIN/MAX로 포화시켜 잘못된 값이 조용히 저장된다 → 즉시 거부
    if !f.is_finite() {
        return Err(format!("'{}' 유한한 숫자가 아닙니다", trimmed));
    }
    // |값| > 10억이면 ×100000 결과가 f64 정밀도 한계(ULP > 0.5)에 걸려
    // round()가 정확한 정수를 보장하지 못한다. 도메인상 초과 값은 입력 오류.
    if f.abs() > 1_000_000_000.0 {
        return Err(format!("'{}' 허용 범위(±10억)를 초과합니다", trimmed));
    }
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

/// pub: tests/invariants.rs의 왕복 불변식 테스트에서 사용
pub fn fmt_score(v: i64) -> String {
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
        .map_err(multipart_err)?
    {
        Some(f) => f
            .bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(multipart_err),
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
    _warnings: &mut Vec<String>,
    info: &mut Vec<String>,
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
                    info.push(format!("'{}/{}' 모집단위 자동 추가됨", un, tn));
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
            ws.write_string(0, i as u16, *h).map_err(excel::xlsx_err)?;
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
            ws.write_number(r, 0, row.get::<i64, _>("threshold") as f64 / 100_000.0).map_err(excel::xlsx_err)?;
            ws.write_number(r, 1, row.get::<i64, _>("score") as f64 / 100_000.0).map_err(excel::xlsx_err)?;
            ws.write_string(r, 2, row.get::<&str, _>("univ_name")).map_err(excel::xlsx_err)?;
            ws.write_string(r, 3, row.get::<&str, _>("track_name")).map_err(excel::xlsx_err)?;
        }
    } else {
        for (i, h) in ["기준값", "점수"].iter().enumerate() {
            ws.write_string(0, i as u16, *h).map_err(excel::xlsx_err)?;
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
            ws.write_number(r, 0, row.get::<i64, _>("threshold") as f64 / 100_000.0).map_err(excel::xlsx_err)?;
            ws.write_number(r, 1, row.get::<i64, _>("score") as f64 / 100_000.0).map_err(excel::xlsx_err)?;
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
    guard_no_closed_round(&state.db).await?;
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
    // 헤더만 있는 파일이 기준표 전체를 조용히 비우는 것을 차단
    if file_rows.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "파일에 데이터 행이 없습니다".into()));
    }

    let mut tx = state.db.begin().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    sqlx::query("DELETE FROM numeric_table WHERE area_id = ?")
        .bind(id).execute(&mut *tx).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut rows = 0usize;
    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut info: Vec<String> = Vec::new();
    let mut seen: HashSet<(Option<i64>, i64)> = HashSet::new();
    let mut track_rows: HashMap<Option<i64>, Vec<(i64, i64)>> = HashMap::new();

    for (i, cols) in file_rows.iter().enumerate() {
        let row_num = i + 2;

        let raw_th = excel::get_col(cols, &col, "기준값");
        if raw_th.is_empty() { errors.push(format!("{}행: 기준값 누락", row_num)); continue; }
        let th = match parse_display_value(raw_th) {
            Ok(v) => v,
            Err(e) => { errors.push(format!("{}행: 기준값 — {}", row_num, e)); continue; }
        };
        let raw_sc = excel::get_col(cols, &col, "점수");
        if raw_sc.is_empty() { errors.push(format!("{}행: 점수 누락", row_num)); continue; }
        let sc = match parse_display_value(raw_sc) {
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

        let track_id = match resolve_track(&mut *tx, &area, cols, &col, row_num, &mut errors, &mut warnings, &mut info).await {
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
            Ok(_) => {
                rows += 1;
                track_rows.entry(track_id).or_default().push((th, sc));
            }
            Err(e) => errors.push(format!("{}행: {}", row_num, e)),
        }
    }

    if !errors.is_empty() {
        // tx이 drop되면 자동 rollback — 부분 삽입 없음
        return Ok((StatusCode::UNPROCESSABLE_ENTITY, Json(ImportResult { rows: 0, errors, warnings: vec![], info: vec![] })));
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

    // 단조성 검사: UPPER → threshold↑ 시 score 비감소, LOWER → threshold↑ 시 score 비증가
    // 역전 입력(예: 높은 기준값에 낮은 점수를 UPPER 모드로 등록)은 silent wrong 점수를 생성하므로 오류 처리
    if let Some(mode) = area.match_mode {
        if mode == MatchMode::Upper || mode == MatchMode::Lower {
            for (tid, pairs) in &mut track_rows {
                pairs.sort_by_key(|&(th, _)| th);
                let label = match tid {
                    Some(t) => format!(" (모집단위 id={})", t),
                    None => String::new(),
                };
                for w in pairs.windows(2) {
                    let (th1, sc1) = w[0];
                    let (th2, sc2) = w[1];
                    let violated = match mode {
                        MatchMode::Upper => sc2 < sc1,
                        MatchMode::Lower => sc2 > sc1,
                        MatchMode::Exact => false,
                    };
                    if violated {
                        let hint = match mode {
                            MatchMode::Upper => "UPPER 모드에서는 기준값이 높을수록 점수도 높거나 같아야 합니다",
                            MatchMode::Lower => "LOWER 모드에서는 기준값이 높을수록 점수도 낮거나 같아야 합니다",
                            MatchMode::Exact => "",
                        };
                        errors.push(format!(
                            "점수 순서 오류{}: 기준값 {}→{} 구간에서 점수가 {}→{} — {}",
                            label,
                            fmt_score(th1), fmt_score(th2),
                            fmt_score(sc1), fmt_score(sc2),
                            hint
                        ));
                        break;
                    }
                }
            }
        }
    }

    if !errors.is_empty() {
        return Ok((StatusCode::UNPROCESSABLE_ENTITY, Json(ImportResult { rows: 0, errors, warnings: vec![], info: vec![] })));
    }

    let area_name: String = sqlx::query_scalar("SELECT name FROM areas WHERE id = ?")
        .bind(id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    audit::log(&mut *tx, AuditEntry {
        actor: Actor::Admin,
        action: AuditAction::ScoreTableImported,
        round_id: None,
        student_id: None,
        detail: serde_json::json!({ "area_name": area_name, "rows": rows }),
    }).await?;
    tx.commit().await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok((StatusCode::OK, Json(ImportResult { rows, errors: vec![], warnings, info })))
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
            ws.write_string(0, i as u16, *h).map_err(excel::xlsx_err)?;
        }
        let rows = sqlx::query(
            "SELECT cm.category, cm.score,
                    COALESCE(u.univ_name, '') AS univ_name,
                    COALESCE(ut.track_name, '') AS track_name
             FROM category_map cm
             LEFT JOIN univ_tracks ut ON cm.track_id = ut.id
             LEFT JOIN universities u ON ut.univ_id = u.id
             WHERE cm.area_id = ?
             ORDER BY u.univ_name, ut.track_name, cm.category, cm.score",
        )
        .bind(id)
        .fetch_all(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        for (r, row) in rows.iter().enumerate() {
            let r = r as u32 + 1;
            ws.write_string(r, 0, row.get::<&str, _>("category")).map_err(excel::xlsx_err)?;
            ws.write_number(r, 1, row.get::<i64, _>("score") as f64 / 100_000.0).map_err(excel::xlsx_err)?;
            ws.write_string(r, 2, row.get::<&str, _>("univ_name")).map_err(excel::xlsx_err)?;
            ws.write_string(r, 3, row.get::<&str, _>("track_name")).map_err(excel::xlsx_err)?;
        }
    } else {
        for (i, h) in ["범주", "점수"].iter().enumerate() {
            ws.write_string(0, i as u16, *h).map_err(excel::xlsx_err)?;
        }
        let rows = sqlx::query(
            "SELECT category, score FROM category_map
             WHERE area_id = ? AND track_id IS NULL ORDER BY category, score",
        )
        .bind(id)
        .fetch_all(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        for (r, row) in rows.iter().enumerate() {
            let r = r as u32 + 1;
            ws.write_string(r, 0, row.get::<&str, _>("category")).map_err(excel::xlsx_err)?;
            ws.write_number(r, 1, row.get::<i64, _>("score") as f64 / 100_000.0).map_err(excel::xlsx_err)?;
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
    guard_no_closed_round(&state.db).await?;
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
    // 헤더만 있는 파일이 범주표 전체를 조용히 비우는 것을 차단
    if file_rows.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "파일에 데이터 행이 없습니다".into()));
    }

    let mut tx = state.db.begin().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    sqlx::query("DELETE FROM category_map WHERE area_id = ?")
        .bind(id).execute(&mut *tx).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut rows = 0usize;
    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut info: Vec<String> = Vec::new();
    let mut seen: HashSet<(Option<i64>, String)> = HashSet::new();

    for (i, cols) in file_rows.iter().enumerate() {
        let row_num = i + 2;

        let category = excel::get_col(cols, &col, "범주").to_string();
        if category.is_empty() {
            errors.push(format!("{}행: 범주 누락", row_num));
            continue;
        }
        let raw_sc = excel::get_col(cols, &col, "점수");
        if raw_sc.is_empty() { errors.push(format!("{}행: 점수 누락", row_num)); continue; }
        let sc = match parse_display_value(raw_sc) {
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

        let track_id = match resolve_track(&mut *tx, &area, cols, &col, row_num, &mut errors, &mut warnings, &mut info).await {
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
        return Ok((StatusCode::UNPROCESSABLE_ENTITY, Json(ImportResult { rows: 0, errors, warnings: vec![], info: vec![] })));
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
        return Ok((StatusCode::UNPROCESSABLE_ENTITY, Json(ImportResult { rows: 0, errors, warnings: vec![], info: vec![] })));
    }

    let area_name: String = sqlx::query_scalar("SELECT name FROM areas WHERE id = ?")
        .bind(id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    audit::log(&mut *tx, AuditEntry {
        actor: Actor::Admin,
        action: AuditAction::ScoreTableImported,
        round_id: None,
        student_id: None,
        detail: serde_json::json!({ "area_name": area_name, "rows": rows }),
    }).await?;
    tx.commit().await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok((StatusCode::OK, Json(ImportResult { rows, errors: vec![], warnings, info })))
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
    let enrolled = parse_student_type(&q.student_type)?;

    // 재학생: 빈 양식만 반환
    if enrolled {
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
            ws.write_string(0, i as u16, *h).map_err(excel::xlsx_err)?;
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
                ws.write_string(row_i, 0, code).map_err(excel::xlsx_err)?;
                ws.write_string(row_i, 1, name).map_err(excel::xlsx_err)?;
                // 값 열(2)은 공백
                ws.write_string(row_i, 3, univ).map_err(excel::xlsx_err)?;
                ws.write_string(row_i, 4, track).map_err(excel::xlsx_err)?;
                row_i += 1;
            }
        }
    } else {
        for (i, h) in ["학생코드", "이름", "값"].iter().enumerate() {
            ws.write_string(0, i as u16, *h).map_err(excel::xlsx_err)?;
        }
        for (i, g) in graduates.iter().enumerate() {
            let r = i as u32 + 1;
            ws.write_string(r, 0, g.get::<&str, _>("student_code")).map_err(excel::xlsx_err)?;
            ws.write_string(r, 1, g.get::<&str, _>("name")).map_err(excel::xlsx_err)?;
            // 값 열(2)은 공백
        }
    }

    let buf = wb.save_to_buffer()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(excel::xlsx_response(buf, "base_data_template.xlsx"))
}

/// GET /api/areas/:id/base-data/export?student_type=enrolled|graduated
///
/// import와 대칭인 student_type별 헤더로 내보낸다 — 산출물을 그대로 같은
/// student_type으로 재import(내려받아 수정 후 재업로드)할 수 있어야 한다:
/// - 재학생: 학년/반/번호/이름/값 (import require_cols와 동일)
/// - 졸업생: 학생코드/이름/값
/// COMPOSITE 전형요소는 대학명/모집단위명 열 추가. 공통 테이블 행(track_id NULL)도
/// 빈 대학명/모집단위명으로 포함한다 (INNER JOIN으로 누락시키면 왕복 시 데이터 유실).
pub async fn base_data_export(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(q): Query<StudentTypeQuery>,
) -> Result<Response, ApiError> {
    let area = get_area(&state.db, id).await?;
    let enrolled = parse_student_type(&q.student_type)?;
    let composite = area.lookup_scope == LookupScope::Composite;

    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();

    let mut headers: Vec<&str> = if enrolled {
        vec!["학년", "반", "번호", "이름", "값"]
    } else {
        vec!["학생코드", "이름", "값"]
    };
    if composite {
        headers.extend_from_slice(&["대학명", "모집단위명"]);
    }
    for (i, h) in headers.iter().enumerate() {
        ws.write_string(0, i as u16, *h).map_err(excel::xlsx_err)?;
    }

    let rows = sqlx::query(
        "SELECT s.student_code, s.name, s.grade, s.class_no, s.seq_no, bd.value,
                COALESCE(u.univ_name, '') AS univ_name,
                COALESCE(ut.track_name, '') AS track_name
         FROM base_data bd
         JOIN students s ON bd.student_id = s.id
         LEFT JOIN univ_tracks ut ON bd.track_id = ut.id
         LEFT JOIN universities u ON ut.univ_id = u.id
         WHERE bd.area_id = ? AND s.is_enrolled = ?
           AND (? OR bd.track_id IS NULL)
         ORDER BY u.univ_name, ut.track_name, s.grade, s.class_no, s.seq_no, s.student_code",
    )
    .bind(id)
    .bind(enrolled as i64)
    // SIMPLE 전형요소는 공통 테이블(track_id NULL)만 내보낸다 — import도 track 열이 없음
    .bind(composite)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    for (r, row) in rows.iter().enumerate() {
        let r = r as u32 + 1;
        let mut col: u16 = 0;
        if enrolled {
            // 재학생 CHECK 제약상 학년/반/번호는 NOT NULL — 없으면 데이터 손상이므로 500
            for key in ["grade", "class_no", "seq_no"] {
                let v: Option<i64> = row.get(key);
                let v = v.ok_or_else(|| (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("재학생 위치({}) 누락: 학생코드 '{}'", key, row.get::<&str, _>("student_code")),
                ))?;
                ws.write_number(r, col, v as f64).map_err(excel::xlsx_err)?;
                col += 1;
            }
        } else {
            ws.write_string(r, col, row.get::<&str, _>("student_code")).map_err(excel::xlsx_err)?;
            col += 1;
        }
        ws.write_string(r, col, row.get::<&str, _>("name")).map_err(excel::xlsx_err)?;
        col += 1;
        write_value(ws, r, col, row.get::<&str, _>("value"), area.calc_type)?;
        col += 1;
        if composite {
            ws.write_string(r, col, row.get::<&str, _>("univ_name")).map_err(excel::xlsx_err)?;
            col += 1;
            ws.write_string(r, col, row.get::<&str, _>("track_name")).map_err(excel::xlsx_err)?;
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
    let enrolled = parse_student_type(&q.student_type)?;
    let bytes = read_file(multipart).await?;
    let (headers, file_rows) = excel::parse_file_rows_with_headers(&bytes)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let col = excel::col_map(&headers);
    if enrolled {
        excel::require_cols(&col, &["학년", "반", "번호", "이름", "값"])
            .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    } else {
        excel::require_cols(&col, &["학생코드", "이름", "값"])
            .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    }

    let mut tx = state.db.begin().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut rows = 0usize;
    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut info: Vec<String> = Vec::new();
    // multi_value=0 전형요소: (student_id, track_id) 중복 추적 — 첫 번째 행 우선
    let single_value = !area.multi_value;
    let mut seen: HashSet<(i64, Option<i64>)> = HashSet::new();
    let mut multi_records: Vec<(i64, Option<i64>, String)> = Vec::new();

    for (i, cols) in file_rows.iter().enumerate() {
        let row_num = i + 2;

        // ── 학생 조회 ──────────────────────────────────────────────
        let student_id: i64;
        let student_label: String;
        if enrolled {
            let name_val = excel::get_col(cols, &col, "이름");
            if name_val.is_empty() {
                errors.push(format!("{}행: 이름 누락", row_num));
                continue;
            }
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
            // tx 커넥션으로 조회 — pool 조회는 tx 보유 중 별도 커넥션을 점유하고
            // 행마다 다른 스냅샷을 볼 수 있다
            let sid: Option<i64> = sqlx::query_scalar(
                "SELECT id FROM students WHERE grade = ? AND class_no = ? AND seq_no = ? AND is_enrolled = 1",
            )
            .bind(grade).bind(class_no).bind(seq_no)
            .fetch_optional(&mut *tx).await
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
            let name_val = excel::get_col(cols, &col, "이름");
            if name_val.is_empty() {
                errors.push(format!("{}행: 이름 누락", row_num));
                continue;
            }
            // is_enrolled=0 필터 필수 — 재학생 student_code가 섞인 파일이
            // 재학생 base_data를 조용히 덮어쓰는 것을 차단 (student_type 정책)
            let sid: Option<i64> = sqlx::query_scalar(
                "SELECT id FROM students WHERE student_code = ? AND is_enrolled = 0",
            )
            .bind(student_code)
            .fetch_optional(&mut *tx).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            match sid {
                Some(v) => {
                    student_id = v;
                    student_label = format!("학생코드 '{}'", student_code);
                }
                None => {
                    errors.push(format!("{}행: 학생코드 '{}'에 해당하는 졸업생이 없습니다 (졸업생을 먼저 등록하세요)", row_num, student_code));
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
        let track_id = match resolve_track(&mut *tx, &area, cols, &col, row_num, &mut errors, &mut warnings, &mut info).await {
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

        if single_value {
            match sqlx::query(
                "INSERT OR REPLACE INTO base_data (student_id, area_id, track_id, value, multi_value) VALUES (?, ?, ?, ?, ?)",
            )
            .bind(student_id).bind(id).bind(track_id).bind(&db_value).bind(area.multi_value)
            .execute(&mut *tx).await {
                Ok(_) => rows += 1,
                Err(e) => errors.push(format!("{}행: {}", row_num, e)),
            }
        } else {
            multi_records.push((student_id, track_id, db_value));
        }
    }

    if !errors.is_empty() {
        return Ok((StatusCode::UNPROCESSABLE_ENTITY, Json(ImportResult { rows: 0, errors, warnings: vec![], info: vec![] })));
    }

    if !single_value {
        // 파일에 등장한 (student, track) 조합만 삭제 후 재삽입 — 파일에 없는 학생 데이터는 보존
        let affected: HashSet<(i64, Option<i64>)> = multi_records.iter()
            .map(|(s, t, _)| (*s, *t))
            .collect();

        for (sid, tid) in &affected {
            let res = sqlx::query(
                "DELETE FROM base_data WHERE area_id = ? AND student_id = ? AND COALESCE(track_id, 0) = COALESCE(?, 0)",
            )
            .bind(id).bind(sid).bind(tid)
            .execute(&mut *tx).await;
            if let Err(e) = res {
                let msg = e.to_string();
                // CLOSED 라운드 지원자 보호 트리거(trg_prevent_base_data_delete_for_applied)의
                // ABORT를 500 대신 422 + 한국어 안내로 번역한다. 보호 로직 자체는 트리거가
                // 단일 진실 원천 — 여기서는 오류 매핑만 담당 (02_round_lifecycle.md 정책)
                if msg.contains("Cannot delete base_data") {
                    let code: String = sqlx::query_scalar(
                        "SELECT student_code FROM students WHERE id = ?",
                    )
                    .bind(sid)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
                    return Ok((StatusCode::UNPROCESSABLE_ENTITY, Json(ImportResult {
                        rows: 0,
                        errors: vec![format!(
                            "종료(CLOSED)된 라운드 지원자의 기초데이터는 교체할 수 없습니다 (학생코드: {}). 라운드를 다시 열거나 해당 학생을 파일에서 제외하세요",
                            code
                        )],
                        warnings: vec![],
                        info: vec![],
                    })));
                }
                return Err((StatusCode::INTERNAL_SERVER_ERROR, msg));
            }
        }
        for (sid, tid, val) in &multi_records {
            sqlx::query(
                "INSERT INTO base_data (student_id, area_id, track_id, value, multi_value) VALUES (?, ?, ?, ?, ?)",
            )
            .bind(sid).bind(id).bind(tid).bind(val).bind(area.multi_value)
            .execute(&mut *tx).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            rows += 1;
        }
    }

    let area_name: String = sqlx::query_scalar("SELECT name FROM areas WHERE id = ?")
        .bind(id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    audit::log(&mut *tx, AuditEntry {
        actor: Actor::Admin,
        action: AuditAction::BaseDataImported,
        round_id: None,
        student_id: None,
        detail: serde_json::json!({ "area_name": area_name, "student_type": q.student_type, "rows": rows }),
    }).await?;
    tx.commit().await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok((StatusCode::OK, Json(ImportResult { rows, errors: vec![], warnings, info })))
}

// ── LIST (JSON 조회) ─────────────────────────────────────────────

#[derive(Serialize)]
pub struct RangeTableListRow {
    pub threshold: Score,
    pub score: Score,
    pub univ_name: Option<String>,
    pub track_name: Option<String>,
    pub track_id: Option<i64>,
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
    pub track_id: Option<i64>,
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
        "SELECT rt.threshold, rt.score, rt.track_id,
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
            track_id: if composite { row.get("track_id") } else { None },
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
        "SELECT cm.category, cm.score, cm.track_id,
                COALESCE(u.univ_name, '') AS univ_name,
                COALESCE(ut.track_name, '') AS track_name
         FROM category_map cm
         LEFT JOIN univ_tracks ut ON cm.track_id = ut.id
         LEFT JOIN universities u ON ut.univ_id = u.id
         WHERE cm.area_id = ?
         ORDER BY u.univ_name, ut.track_name, cm.category, cm.score
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
            track_id: if composite { row.get("track_id") } else { None },
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
    let is_enrolled_val = if parse_student_type(&q.student_type)? { 1i64 } else { 0i64 };

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
                    fmt_score(v)
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
fn write_value(
    ws: &mut rust_xlsxwriter::Worksheet,
    row: u32,
    col: u16,
    value: &str,
    calc_type: CalcType,
) -> Result<(), ApiError> {
    match calc_type {
        CalcType::Numeric | CalcType::Manual => {
            if let Ok(v) = value.parse::<i64>() {
                ws.write_number(row, col, v as f64 / 100_000.0).map_err(excel::xlsx_err)?;
            } else {
                ws.write_string(row, col, value).map_err(excel::xlsx_err)?;
            }
        }
        CalcType::Category => {
            ws.write_string(row, col, value).map_err(excel::xlsx_err)?;
        }
    }
    Ok(())
}
