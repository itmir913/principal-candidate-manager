use axum::{
    body::Body,
    extract::{Multipart, Query, State},
    http::{header, StatusCode},
    response::Response,
    Json,
};
use calamine::{DataType, Reader, Xlsx};
use rust_xlsxwriter::Workbook;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::io::Cursor;

use crate::state::AppState;

type ApiError = (StatusCode, String);

// ── 응답 구조체 ──────────────────────────────────────────────────

#[derive(Serialize, FromRow)]
pub struct StudentRow {
    pub id: i64,
    pub student_code: String,
    pub name: String,
    pub grade: Option<i64>,
    pub class_no: Option<i64>,
    pub seq_no: Option<i64>,
    pub is_enrolled: i64,
    pub grad_year: Option<i64>,
}

#[derive(Serialize)]
pub struct ImportResult {
    pub inserted: usize,
    pub updated: usize,
    pub errors: Vec<String>,
}

#[derive(Deserialize)]
pub struct ListQuery {
    pub grade: Option<i64>,
    pub class_no: Option<i64>,
}

// ── 파싱용 중간 구조체 ───────────────────────────────────────────

struct StudentRecord {
    student_code: String,
    name: String,
    is_enrolled: i64,
    grade: Option<i64>,
    class_no: Option<i64>,
    seq_no: Option<i64>,
    grad_year: Option<i64>,
}

// ── 핸들러 ───────────────────────────────────────────────────────

/// GET /api/students
pub async fn list_students(
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Vec<StudentRow>>, ApiError> {
    let rows = match (q.grade, q.class_no) {
        (Some(g), Some(c)) => sqlx::query_as::<_, StudentRow>(
            "SELECT id, student_code, name, grade, class_no, seq_no, is_enrolled, grad_year
             FROM students WHERE grade = ? AND class_no = ?
             ORDER BY seq_no",
        )
        .bind(g)
        .bind(c)
        .fetch_all(&state.db)
        .await,

        _ => sqlx::query_as::<_, StudentRow>(
            "SELECT id, student_code, name, grade, class_no, seq_no, is_enrolled, grad_year
             FROM students ORDER BY grade, class_no, seq_no",
        )
        .fetch_all(&state.db)
        .await,
    }
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(rows))
}

/// GET /api/students/template  — 샘플 양식 xlsx 다운로드
pub async fn download_template() -> Result<Response, ApiError> {
    let buf = build_template_xlsx().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(xlsx_response(buf, "students_template.xlsx"))
}

/// GET /api/students/export  — 현재 DB 데이터 xlsx 다운로드
pub async fn export_students(State(state): State<AppState>) -> Result<Response, ApiError> {
    let rows = sqlx::query_as::<_, StudentRow>(
        "SELECT id, student_code, name, grade, class_no, seq_no, is_enrolled, grad_year
         FROM students ORDER BY grade, class_no, seq_no",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let buf = build_export_xlsx(&rows).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(xlsx_response(buf, "students.xlsx"))
}

/// POST /api/students/import  — xlsx 또는 CSV 일괄 업로드
pub async fn import_students(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<ImportResult>, ApiError> {
    // 멀티파트에서 파일 바이트 추출
    let bytes = loop {
        match multipart
            .next_field()
            .await
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?
        {
            Some(field) => {
                let b = field
                    .bytes()
                    .await
                    .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
                break b.to_vec();
            }
            None => return Err((StatusCode::BAD_REQUEST, "파일이 없습니다".into())),
        }
    };

    // xlsx(PK 매직바이트) vs CSV 판별
    let records = if bytes.starts_with(b"PK") {
        parse_xlsx(&bytes).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?
    } else {
        parse_csv(&bytes).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?
    };

    // DB upsert
    let mut inserted = 0usize;
    let mut updated = 0usize;
    let mut errors: Vec<String> = Vec::new();

    for (idx, rec) in records.iter().enumerate() {
        let row_num = idx + 2; // 헤더 포함 1-based

        if let Err(e) = upsert_student(&state, rec, &mut inserted, &mut updated).await {
            errors.push(format!("{}행: {}", row_num, e));
        }
    }

    Ok(Json(ImportResult { inserted, updated, errors }))
}

// ── DB upsert 헬퍼 ───────────────────────────────────────────────

async fn upsert_student(
    state: &AppState,
    rec: &StudentRecord,
    inserted: &mut usize,
    updated: &mut usize,
) -> Result<(), String> {
    // 기본 유효성
    if rec.student_code.is_empty() {
        return Err("student_code 누락".into());
    }
    if rec.name.is_empty() {
        return Err("name 누락".into());
    }

    if rec.is_enrolled == 1 {
        // 재학생: grade, class_no, seq_no 필수
        let (grade, class_no, seq_no) = match (rec.grade, rec.class_no, rec.seq_no) {
            (Some(g), Some(c), Some(s)) => (g, c, s),
            _ => return Err("재학생은 grade, class_no, seq_no 필수".into()),
        };

        // 학급 존재 확인
        let class_ok: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM classes WHERE grade = ? AND class_no = ?",
        )
        .bind(grade)
        .bind(class_no)
        .fetch_one(&state.db)
        .await
        .map_err(|e| e.to_string())?;

        if class_ok == 0 {
            return Err(format!("{}학년 {}반이 학급 목록에 없습니다", grade, class_no));
        }

        let exists: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM students WHERE student_code = ?")
                .bind(&rec.student_code)
                .fetch_one(&state.db)
                .await
                .map_err(|e| e.to_string())?;

        if exists > 0 {
            sqlx::query(
                "UPDATE students SET name=?, grade=?, class_no=?, seq_no=?, is_enrolled=1, grad_year=NULL
                 WHERE student_code=?",
            )
            .bind(&rec.name)
            .bind(grade)
            .bind(class_no)
            .bind(seq_no)
            .bind(&rec.student_code)
            .execute(&state.db)
            .await
            .map_err(|e| e.to_string())?;
            *updated += 1;
        } else {
            sqlx::query(
                "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled)
                 VALUES (?, ?, ?, ?, ?, 1)",
            )
            .bind(&rec.student_code)
            .bind(&rec.name)
            .bind(grade)
            .bind(class_no)
            .bind(seq_no)
            .execute(&state.db)
            .await
            .map_err(|e| e.to_string())?;
            *inserted += 1;
        }
    } else {
        // 졸업생: grad_year 필수
        let grad_year = rec.grad_year.ok_or("졸업생은 grad_year 필수")?;

        let exists: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM students WHERE student_code = ?")
                .bind(&rec.student_code)
                .fetch_one(&state.db)
                .await
                .map_err(|e| e.to_string())?;

        if exists > 0 {
            sqlx::query(
                "UPDATE students SET name=?, grade=NULL, class_no=NULL, seq_no=NULL, is_enrolled=0, grad_year=?
                 WHERE student_code=?",
            )
            .bind(&rec.name)
            .bind(grad_year)
            .bind(&rec.student_code)
            .execute(&state.db)
            .await
            .map_err(|e| e.to_string())?;
            *updated += 1;
        } else {
            sqlx::query(
                "INSERT INTO students (student_code, name, is_enrolled, grad_year)
                 VALUES (?, ?, 0, ?)",
            )
            .bind(&rec.student_code)
            .bind(&rec.name)
            .bind(grad_year)
            .execute(&state.db)
            .await
            .map_err(|e| e.to_string())?;
            *inserted += 1;
        }
    }

    Ok(())
}

// ── xlsx 생성 ────────────────────────────────────────────────────

const HEADERS: &[&str] = &[
    "student_code",
    "name",
    "is_enrolled",
    "grade",
    "class_no",
    "seq_no",
    "grad_year",
];

fn build_template_xlsx() -> anyhow::Result<Vec<u8>> {
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    for (col, h) in HEADERS.iter().enumerate() {
        ws.write_string(0, col as u16, *h)?;
    }
    // 샘플 재학생 행
    ws.write_string(1, 0, "20250001")?;
    ws.write_string(1, 1, "홍길동")?;
    ws.write_number(1, 2, 1.0)?; // is_enrolled
    ws.write_number(1, 3, 1.0)?; // grade
    ws.write_number(1, 4, 1.0)?; // class_no
    ws.write_number(1, 5, 1.0)?; // seq_no
    // grad_year 비움
    // 샘플 졸업생 행
    ws.write_string(2, 0, "20240001")?;
    ws.write_string(2, 1, "김철수")?;
    ws.write_number(2, 2, 0.0)?; // is_enrolled
    // grade/class_no/seq_no 비움
    ws.write_number(2, 6, 2024.0)?; // grad_year
    Ok(wb.save_to_buffer()?)
}

fn build_export_xlsx(rows: &[StudentRow]) -> anyhow::Result<Vec<u8>> {
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    for (col, h) in HEADERS.iter().enumerate() {
        ws.write_string(0, col as u16, *h)?;
    }
    for (r, row) in rows.iter().enumerate() {
        let r = r as u32 + 1;
        ws.write_string(r, 0, &row.student_code)?;
        ws.write_string(r, 1, &row.name)?;
        ws.write_number(r, 2, row.is_enrolled as f64)?;
        if let Some(v) = row.grade    { ws.write_number(r, 3, v as f64)?; }
        if let Some(v) = row.class_no { ws.write_number(r, 4, v as f64)?; }
        if let Some(v) = row.seq_no   { ws.write_number(r, 5, v as f64)?; }
        if let Some(v) = row.grad_year { ws.write_number(r, 6, v as f64)?; }
    }
    Ok(wb.save_to_buffer()?)
}

// ── xlsx 파싱 ────────────────────────────────────────────────────

fn parse_xlsx(bytes: &[u8]) -> anyhow::Result<Vec<StudentRecord>> {
    let cursor = Cursor::new(bytes.to_vec());
    let mut wb: Xlsx<_> = Xlsx::new(cursor)?;

    let sheet = wb
        .sheet_names()
        .first()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("시트가 없습니다"))?;

    let range = wb
        .worksheet_range(&sheet)
        .ok_or_else(|| anyhow::anyhow!("시트를 열 수 없습니다"))??;

    let mut records = Vec::new();
    for (i, row) in range.rows().enumerate().skip(1) {
        // 빈 행 건너뜀
        if row.iter().all(|c| matches!(c, DataType::Empty)) {
            continue;
        }
        records.push(row_to_record(row, i + 1)?);
    }
    Ok(records)
}

// ── CSV 파싱 ────────────────────────────────────────────────────

fn parse_csv(bytes: &[u8]) -> anyhow::Result<Vec<StudentRecord>> {
    let content = decode_bytes(bytes);
    let mut rdr = csv::Reader::from_reader(content.as_bytes());
    let mut records = Vec::new();
    for (i, result) in rdr.records().enumerate() {
        let sr = result?;
        let row_vec: Vec<DataType> = sr
            .iter()
            .map(|s| {
                if s.is_empty() {
                    DataType::Empty
                } else {
                    DataType::String(s.to_string())
                }
            })
            .collect();
        records.push(row_to_record(&row_vec, i + 2)?);
    }
    Ok(records)
}

/// 인코딩 감지: UTF-8 BOM → UTF-8 → EUC-KR(CP949) 순으로 시도
fn decode_bytes(bytes: &[u8]) -> String {
    if bytes.starts_with(b"\xef\xbb\xbf") {
        return String::from_utf8_lossy(&bytes[3..]).into_owned();
    }
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }
    let (cow, _, _) = encoding_rs::EUC_KR.decode(bytes);
    cow.into_owned()
}

// ── 공통 행 파싱 ─────────────────────────────────────────────────

fn row_to_record(row: &[DataType], _row_num: usize) -> anyhow::Result<StudentRecord> {
    let get = |col: usize| -> String {
        row.get(col)
            .map(cell_str)
            .unwrap_or_default()
    };
    let get_i64 = |col: usize| -> Option<i64> {
        let s = get(col);
        if s.is_empty() { None } else { s.trim().parse().ok() }
    };

    let student_code = get(0).trim().to_string();
    let name         = get(1).trim().to_string();
    let is_enrolled  = get_i64(2).unwrap_or(1); // 기본값: 재학생
    let grade        = get_i64(3);
    let class_no     = get_i64(4);
    let seq_no       = get_i64(5);
    let grad_year    = get_i64(6);

    Ok(StudentRecord { student_code, name, is_enrolled, grade, class_no, seq_no, grad_year })
}

fn cell_str(cell: &DataType) -> String {
    match cell {
        DataType::String(s) => s.trim().to_string(),
        DataType::Float(f)  => {
            if f.fract() == 0.0 { (*f as i64).to_string() }
            else { f.to_string() }
        }
        DataType::Int(i)    => i.to_string(),
        DataType::Bool(b)   => if *b { "1" } else { "0" }.to_string(),
        _                   => String::new(),
    }
}

// ── 응답 헬퍼 ────────────────────────────────────────────────────

fn xlsx_response(buf: Vec<u8>, filename: &str) -> Response {
    let disposition = format!("attachment; filename=\"{}\"", filename);
    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        )
        .header(header::CONTENT_DISPOSITION, disposition)
        .body(Body::from(buf))
        .unwrap()
}
