/// 외부 프로그램(대교협·유니브) 석차연명부 가져오기 핸들러
use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    Json,
};
use serde::Serialize;
use crate::{
    audit::{self, Actor, AuditEntry},
    enums::AuditAction,
    excel,
    middleware::multipart_err,
    state::AppState,
};
use super::area_data::{find_or_create_track, get_area, parse_display_value, ImportResult};

type ApiError = (StatusCode, String);
type Db = sqlx::SqlitePool;

// ── 공통 구조체 ──────────────────────────────────────────────────

#[derive(Serialize)]
pub struct ExternalPreview {
    pub univ_name: String,
    pub value_header: String,
    /// 파일 상단 정보 행을 한 줄로 직렬화한 문구 — 올린 파일이 맞는지 눈으로 확인용
    pub header_info: String,
    pub preview: Vec<Vec<String>>, // [학년, 반, 번호, 이름, 값] 상위 5행
    pub total: usize,
}

struct ParsedFile {
    univ_name: String,
    value_header: String,
    header_info: String,
    records: Vec<(usize, Vec<String>)>, // (엑셀 행 번호 1-based, [학년, 반, 번호, 이름, 값])
}

// ── 파싱 ─────────────────────────────────────────────────────────

fn parse_daegyo(bytes: &[u8]) -> Result<ParsedFile, String> {
    if !excel::is_xlsx(bytes) {
        return Err("대교협 양식은 .xlsx 파일이어야 합니다".into());
    }
    let rows = excel::parse_xlsx_all_rows_raw(bytes).map_err(|e| e.to_string())?;

    // 1행: "지역-대학명(캠퍼스)-전형유형-..."에서 대학명 추출
    let info = rows.first().and_then(|r| r.first()).map(|s| s.as_str()).unwrap_or("");
    let parts: Vec<&str> = info.split('-').collect();
    let univ_name = parts.get(1).map(|s| s.trim().to_string()).unwrap_or_default();

    // 2행: 헤더
    let header_row = rows.get(1).cloned().unwrap_or_default();
    let col = excel::col_map(&header_row);
    for c in &["학년", "반", "번호", "이름", "일반등급", "내점수(환산)", "내등급(환산)"] {
        if !col.contains_key(*c) {
            return Err(format!("대교협 양식에서 '{}' 열을 찾을 수 없습니다", c));
        }
    }

    // 3행~: 데이터. 행 번호는 원본 엑셀 기준(1-based) — 오류 메시지에서 사용자가 찾을 수 있어야 함
    // 내점수(환산)가 "미제공"이면 환산등급을 제공하지 않는 모집단위 → 일반등급 사용
    let records = rows
        .iter()
        .enumerate()
        .skip(2)
        .filter(|(_, row)| !row.iter().all(|c| c.is_empty()))
        .map(|(idx, row)| {
            let jum = excel::get_col(row, &col, "내점수(환산)");
            let val = if jum == "미제공" {
                excel::get_col(row, &col, "일반등급")
            } else {
                excel::get_col(row, &col, "내등급(환산)")
            };
            (idx + 1, vec![
                excel::get_col(row, &col, "학년").to_string(),
                excel::get_col(row, &col, "반").to_string(),
                excel::get_col(row, &col, "번호").to_string(),
                excel::get_col(row, &col, "이름").to_string(),
                val.to_string(),
            ])
        })
        .collect();

    Ok(ParsedFile {
        univ_name,
        value_header: "내등급(환산)".into(),
        header_info: info.trim().to_string(),
        records,
    })
}

fn parse_univ(bytes: &[u8]) -> Result<ParsedFile, String> {
    if !excel::is_xls(bytes) {
        return Err("유니브 양식은 .xls 파일이어야 합니다".into());
    }
    let rows = excel::parse_xls_all_rows_raw(bytes).map_err(|e| e.to_string())?;

    // 1행 B열(index 1): 대학명
    let univ_name = rows
        .first()
        .and_then(|r| r.get(1))
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    // 1~3행 A·B열: 대학/학과/전형 정보 — 한 줄로 직렬화
    let header_info = (0..3)
        .filter_map(|i| {
            let row = rows.get(i)?;
            let label = row.first().map(|s| s.trim()).unwrap_or("");
            let value = row.get(1).map(|s| s.trim()).unwrap_or("");
            match (label.is_empty(), value.is_empty()) {
                (true, true) => None,
                (true, false) => Some(value.to_string()),
                (false, true) => Some(label.to_string()),
                (false, false) => Some(format!("{}: {}", label, value)),
            }
        })
        .collect::<Vec<_>>()
        .join(" | ");

    // 6행(index 5): 헤더
    let header_row = rows.get(5).cloned().unwrap_or_default();
    let col = excel::col_map(&header_row);
    for c in &["학년", "반", "번호", "이름", "등급"] {
        if !col.contains_key(*c) {
            return Err(format!("유니브 양식에서 '{}' 열을 찾을 수 없습니다", c));
        }
    }

    // 7행(index 6)~: 데이터. 행 번호는 원본 엑셀 기준(1-based)
    let records = rows
        .iter()
        .enumerate()
        .skip(6)
        .filter(|(_, row)| !row.iter().all(|c| c.is_empty()))
        .map(|(idx, row)| {
            (idx + 1, vec![
                excel::get_col(row, &col, "학년").to_string(),
                excel::get_col(row, &col, "반").to_string(),
                excel::get_col(row, &col, "번호").to_string(),
                excel::get_col(row, &col, "이름").to_string(),
                excel::get_col(row, &col, "등급").to_string(),
            ])
        })
        .collect();

    Ok(ParsedFile { univ_name, value_header: "등급".into(), header_info, records })
}

// ── 멀티파트 읽기 ─────────────────────────────────────────────────

async fn read_file_only(mut multipart: Multipart) -> Result<Vec<u8>, ApiError> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(multipart_err)?
    {
        return field
            .bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(multipart_err);
    }
    Err((StatusCode::BAD_REQUEST, "파일이 없습니다".into()))
}

async fn read_import_multipart(
    mut multipart: Multipart,
) -> Result<(Vec<u8>, String, String), ApiError> {
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut univ_name = String::new();
    let mut track_name = String::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(multipart_err)?
    {
        match field.name() {
            Some("file") => {
                file_bytes = Some(
                    field
                        .bytes()
                        .await
                        .map(|b| b.to_vec())
                        .map_err(multipart_err)?,
                );
            }
            Some("univ_name") => {
                univ_name = field
                    .text()
                    .await
                    .map_err(multipart_err)?;
            }
            Some("track_name") => {
                track_name = field
                    .text()
                    .await
                    .map_err(multipart_err)?;
            }
            _ => {}
        }
    }

    let bytes =
        file_bytes.ok_or_else(|| (StatusCode::BAD_REQUEST, "파일이 없습니다".to_string()))?;
    if univ_name.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "대학명은 필수입니다".into()));
    }
    if track_name.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "모집단위명은 필수입니다".into()));
    }
    Ok((bytes, univ_name.trim().to_string(), track_name.trim().to_string()))
}

// ── 공통 임포트 로직 ─────────────────────────────────────────────

async fn do_import(
    db: &Db,
    area_id: i64,
    parsed: ParsedFile,
    univ_name: String,
    track_name: String,
    source: &str,
) -> Result<(StatusCode, Json<ImportResult>), ApiError> {
    let area = get_area(db, area_id).await?;
    if area.lookup_scope != crate::enums::LookupScope::Composite {
        return Err((
            StatusCode::BAD_REQUEST,
            "외부 가져오기는 대학별 환산점수 조회 전형요소에서만 사용할 수 있습니다".into(),
        ));
    }
    // 복수값(CATEGORY SUM) 전형요소 차단: 석차연명부는 학생당 단일 값인데
    // INSERT OR REPLACE의 유니크 인덱스가 value를 포함해 값 변경 재업로드 시
    // 기존 행이 남아 SUM이 이중 합산된다
    if area.multi_value {
        return Err((
            StatusCode::BAD_REQUEST,
            "복수값(합산) 전형요소에는 외부 가져오기를 사용할 수 없습니다. 기초 데이터 업로드를 사용해 주세요".into(),
        ));
    }
    if parsed.records.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "파일에 데이터 행이 없습니다".into()));
    }

    let mut warnings: Vec<String> = Vec::new();
    let mut info: Vec<String> = Vec::new();

    let mut tx = db
        .begin()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // find_or_create_track을 tx 안에서 실행 — import 실패 시 생성된 대학/트랙도 롤백됨
    let (track_id, created) = find_or_create_track(&mut *tx, &univ_name, &track_name).await?;
    if created {
        info.push(format!("'{}/{}' 모집단위 자동 추가됨", univ_name, track_name));
    }


    let mut rows = 0usize;
    let mut errors: Vec<String> = Vec::new();
    // 석차 값이 없어 건너뛴 행 — rollback 시에도 사유를 보여줘야 하므로 warnings와 분리
    let mut skipped: Vec<String> = Vec::new();
    // 파일 내 동일 학생 중복 행 감지 — 마지막 행이 조용히 이기면 중복=error 정책 위반
    let mut seen: std::collections::HashSet<i64> = std::collections::HashSet::new();

    for (row_num, record) in parsed.records.iter() {
        let row_num = *row_num;
        let (grade_s, class_s, seq_s, name, val_s) = match record.as_slice() {
            [g, c, s, n, v, ..] => (g.as_str(), c.as_str(), s.as_str(), n.as_str(), v.as_str()),
            _ => continue,
        };

        let grade: i64 = match grade_s.parse() {
            Ok(v) => v,
            Err(_) => {
                errors.push(format!("{}행: 학년 '{}' 숫자 변환 실패", row_num, grade_s));
                continue;
            }
        };
        let class_no: i64 = match class_s.parse() {
            Ok(v) => v,
            Err(_) => {
                errors.push(format!("{}행: 반 '{}' 숫자 변환 실패", row_num, class_s));
                continue;
            }
        };
        let seq_no: i64 = match seq_s.parse() {
            Ok(v) => v,
            Err(_) => {
                errors.push(format!("{}행: 번호 '{}' 숫자 변환 실패", row_num, seq_s));
                continue;
            }
        };

        // tx 커넥션으로 조회 — pool 조회는 tx 보유 중 별도 커넥션을 점유하고
        // 행마다 다른 스냅샷을 볼 수 있다
        let student: Option<(i64, String)> = sqlx::query_as(
            "SELECT id, name FROM students
             WHERE grade = ? AND class_no = ? AND seq_no = ? AND is_enrolled = 1",
        )
        .bind(grade)
        .bind(class_no)
        .bind(seq_no)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let (student_id, db_name) = match student {
            Some(s) => s,
            None => {
                errors.push(format!(
                    "{}행: {}학년 {}반 {}번 {} — 등록된 재학생을 찾을 수 없습니다",
                    row_num, grade, class_no, seq_no, name
                ));
                continue;
            }
        };

        if !seen.insert(student_id) {
            errors.push(format!(
                "{}행: {}학년 {}반 {}번 중복 — 파일에 같은 학생이 두 번 이상 존재합니다",
                row_num, grade, class_no, seq_no
            ));
            continue;
        }

        if db_name.trim() != name.trim() {
            warnings.push(format!(
                "{}행: {}학년 {}반 {}번 이름 불일치 — 가져오기 완료됨 (파일: '{}', DB: '{}')",
                row_num, grade, class_no, seq_no, name, db_name
            ));
        }

        // 석차 값 누락·변환 실패는 해당 학생만 건너뛰고 warning — 전출·자퇴 학생은
        // 외부 프로그램이 등급을 '-'/공백으로 내보낸다. 이를 error로 처리하면 관리자가
        // 매 업로드마다 석차연명부 원본을 손봐야 한다 (silent_fallback_allowed.md #29).
        // 건너뛴 학생은 base_data가 없어 점수 계산 단계에서 별도로 드러난다.
        if val_s.trim().is_empty() {
            skipped.push(format!(
                "{}행: {}학년 {}반 {}번 {} — 석차 값이 비어 있어 건너뜀",
                row_num, grade, class_no, seq_no, name
            ));
            continue;
        }

        let db_value = match area.calc_type {
            crate::enums::CalcType::Numeric | crate::enums::CalcType::Manual => {
                match parse_display_value(val_s) {
                    Ok(v) => v.to_string(),
                    Err(e) => {
                        skipped.push(format!(
                            "{}행: {}학년 {}반 {}번 {} — 석차 값 {} → 건너뜀",
                            row_num, grade, class_no, seq_no, name, e
                        ));
                        continue;
                    }
                }
            }
            crate::enums::CalcType::Category => val_s.to_string(),
        };

        match sqlx::query(
            "INSERT OR REPLACE INTO base_data (student_id, area_id, track_id, value, multi_value)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(student_id)
        .bind(area_id)
        .bind(track_id)
        .bind(&db_value)
        .bind(area.multi_value)
        .execute(&mut *tx)
        .await
        {
            Ok(_) => rows += 1,
            Err(e) => errors.push(format!("{}행: {}", row_num, e)),
        }
    }

    if !errors.is_empty() {
        return Ok((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ImportResult { rows: 0, errors, warnings: skipped, info: vec![] }),
        ));
    }
    // 모든 행이 건너뛰어졌으면 트랙만 생성되는 no-op — 데이터 0행과 동일하게 거부.
    // 값 열을 잘못 고른 파일이 "완료 — 0건"으로 조용히 넘어가는 것을 막는다
    if rows == 0 {
        return Ok((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ImportResult {
                rows: 0,
                errors: vec![
                    "저장된 행이 없습니다 — 모든 행의 석차 값이 비어 있거나 숫자로 변환할 수 없습니다".into(),
                ],
                warnings: skipped,
                info: vec![],
            }),
        ));
    }
    warnings.extend(skipped);
    let area_name: String = sqlx::query_scalar("SELECT name FROM areas WHERE id = ?")
        .bind(area_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    audit::log(&mut *tx, AuditEntry {
        actor: Actor::Admin,
        action: AuditAction::BaseDataImported,
        round_id: None,
        student_id: None,
        detail: serde_json::json!({
            "source": source,
            "area_name": area_name,
            "univ_name": univ_name,
            "track_name": track_name,
            "rows": rows,
        }),
    }).await?;
    tx.commit()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok((StatusCode::OK, Json(ImportResult { rows, errors: vec![], warnings, info })))
}

// ── 핸들러 ───────────────────────────────────────────────────────

pub async fn daegyo_preview(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    multipart: Multipart,
) -> Result<Json<ExternalPreview>, ApiError> {
    get_area(&state.db, id).await?;
    let bytes = read_file_only(multipart).await?;
    let parsed = parse_daegyo(&bytes).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let total = parsed.records.len();
    let preview = parsed.records.into_iter().take(5).map(|(_, r)| r).collect();
    Ok(Json(ExternalPreview {
        univ_name: parsed.univ_name,
        value_header: parsed.value_header,
        header_info: parsed.header_info,
        preview,
        total,
    }))
}

pub async fn daegyo_import(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    multipart: Multipart,
) -> Result<(StatusCode, Json<ImportResult>), ApiError> {
    let (bytes, univ_name, track_name) = read_import_multipart(multipart).await?;
    let parsed = parse_daegyo(&bytes).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    do_import(&state.db, id, parsed, univ_name, track_name, "daegyo").await
}

pub async fn univ_preview(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    multipart: Multipart,
) -> Result<Json<ExternalPreview>, ApiError> {
    get_area(&state.db, id).await?;
    let bytes = read_file_only(multipart).await?;
    let parsed = parse_univ(&bytes).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let total = parsed.records.len();
    let preview = parsed.records.into_iter().take(5).map(|(_, r)| r).collect();
    Ok(Json(ExternalPreview {
        univ_name: parsed.univ_name,
        value_header: parsed.value_header,
        header_info: parsed.header_info,
        preview,
        total,
    }))
}

pub async fn univ_import(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    multipart: Multipart,
) -> Result<(StatusCode, Json<ImportResult>), ApiError> {
    let (bytes, univ_name, track_name) = read_import_multipart(multipart).await?;
    let parsed = parse_univ(&bytes).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    do_import(&state.db, id, parsed, univ_name, track_name, "univ").await
}
