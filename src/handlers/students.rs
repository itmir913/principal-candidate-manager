use axum::{
    extract::{Multipart, Query, State},
    http::StatusCode,
    response::Response,
    Json,
};
use rust_xlsxwriter::Workbook;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::{excel, state::AppState};

type ApiError = (StatusCode, String);

// ── 구조체 ───────────────────────────────────────────────────────

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
             FROM students WHERE grade = ? AND class_no = ? ORDER BY seq_no",
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

/// GET /api/students/template
pub async fn download_template() -> Result<Response, ApiError> {
    let buf = build_template_xlsx()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(excel::xlsx_response(buf, "students_template.xlsx"))
}

/// GET /api/students/export
pub async fn export_students(State(state): State<AppState>) -> Result<Response, ApiError> {
    let rows = sqlx::query_as::<_, StudentRow>(
        "SELECT id, student_code, name, grade, class_no, seq_no, is_enrolled, grad_year
         FROM students ORDER BY grade, class_no, seq_no",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let buf = build_export_xlsx(&rows)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(excel::xlsx_response(buf, &format!("students_{}.xlsx", excel::now_tag())))
}

/// POST /api/students/import
pub async fn import_students(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<ImportResult>, ApiError> {
    let bytes = loop {
        match multipart
            .next_field()
            .await
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?
        {
            Some(f) => break f.bytes().await
                .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?
                .to_vec(),
            None => return Err((StatusCode::BAD_REQUEST, "파일이 없습니다".into())),
        }
    };

    let file_rows = excel::parse_file_rows(&bytes)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let mut inserted = 0usize;
    let mut updated = 0usize;
    let mut errors: Vec<String> = Vec::new();

    let mut tx = state.db.begin().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    for (idx, cols) in file_rows.iter().enumerate() {
        let row_num = idx + 2;
        let rec = row_to_record(cols);
        if let Err(e) = upsert_student(&mut *tx, &rec, &mut inserted, &mut updated).await {
            errors.push(format!("{}행: {}", row_num, e));
        }
    }

    tx.commit().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(ImportResult { inserted, updated, errors }))
}

// ── DB upsert ─────────────────────────────────────────────────────

async fn upsert_student(
    conn: &mut sqlx::SqliteConnection,
    rec: &StudentRecord,
    inserted: &mut usize,
    updated: &mut usize,
) -> Result<(), String> {
    if rec.student_code.is_empty() {
        return Err("student_code 누락".into());
    }
    if rec.name.is_empty() {
        return Err("name 누락".into());
    }

    if rec.is_enrolled == 1 {
        let (grade, class_no, seq_no) = match (rec.grade, rec.class_no, rec.seq_no) {
            (Some(g), Some(c), Some(s)) => (g, c, s),
            _ => return Err("재학생은 grade, class_no, seq_no 필수".into()),
        };

        let class_ok: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM classes WHERE grade = ? AND class_no = ?",
        )
        .bind(grade)
        .bind(class_no)
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;

        if class_ok == 0 {
            return Err(format!("{}학년 {}반이 학급 목록에 없습니다", grade, class_no));
        }

        let exists: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM students WHERE student_code = ?")
                .bind(&rec.student_code)
                .fetch_one(&mut *conn)
                .await
                .map_err(|e| e.to_string())?;

        if exists > 0 {
            sqlx::query(
                "UPDATE students SET name=?, grade=?, class_no=?, seq_no=?,
                 is_enrolled=1, grad_year=NULL WHERE student_code=?",
            )
            .bind(&rec.name).bind(grade).bind(class_no).bind(seq_no).bind(&rec.student_code)
            .execute(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
            *updated += 1;
        } else {
            sqlx::query(
                "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled)
                 VALUES (?, ?, ?, ?, ?, 1)",
            )
            .bind(&rec.student_code).bind(&rec.name).bind(grade).bind(class_no).bind(seq_no)
            .execute(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
            *inserted += 1;
        }
    } else {
        let grad_year = rec.grad_year.ok_or("졸업생은 grad_year 필수")?;

        let exists: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM students WHERE student_code = ?")
                .bind(&rec.student_code)
                .fetch_one(&mut *conn)
                .await
                .map_err(|e| e.to_string())?;

        if exists > 0 {
            sqlx::query(
                "UPDATE students SET name=?, grade=NULL, class_no=NULL, seq_no=NULL,
                 is_enrolled=0, grad_year=? WHERE student_code=?",
            )
            .bind(&rec.name).bind(grad_year).bind(&rec.student_code)
            .execute(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
            *updated += 1;
        } else {
            sqlx::query(
                "INSERT INTO students (student_code, name, is_enrolled, grad_year)
                 VALUES (?, ?, 0, ?)",
            )
            .bind(&rec.student_code).bind(&rec.name).bind(grad_year)
            .execute(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
            *inserted += 1;
        }
    }
    Ok(())
}

// ── xlsx 생성 ────────────────────────────────────────────────────

const HEADERS: &[&str] = &[
    "student_code", "name", "is_enrolled",
    "grade", "class_no", "seq_no", "grad_year",
];

fn build_template_xlsx() -> anyhow::Result<Vec<u8>> {
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    for (i, h) in HEADERS.iter().enumerate() {
        ws.write_string(0, i as u16, *h)?;
    }
    // 샘플 재학생
    ws.write_string(1, 0, "20250001")?;
    ws.write_string(1, 1, "홍길동")?;
    ws.write_number(1, 2, 1.0)?;
    ws.write_number(1, 3, 1.0)?;
    ws.write_number(1, 4, 1.0)?;
    ws.write_number(1, 5, 1.0)?;
    // 샘플 졸업생
    ws.write_string(2, 0, "20240001")?;
    ws.write_string(2, 1, "김철수")?;
    ws.write_number(2, 2, 0.0)?;
    ws.write_number(2, 6, 2024.0)?;
    Ok(wb.save_to_buffer()?)
}

fn build_export_xlsx(rows: &[StudentRow]) -> anyhow::Result<Vec<u8>> {
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    for (i, h) in HEADERS.iter().enumerate() {
        ws.write_string(0, i as u16, *h)?;
    }
    for (r, row) in rows.iter().enumerate() {
        let r = r as u32 + 1;
        ws.write_string(r, 0, &row.student_code)?;
        ws.write_string(r, 1, &row.name)?;
        ws.write_number(r, 2, row.is_enrolled as f64)?;
        if let Some(v) = row.grade     { ws.write_number(r, 3, v as f64)?; }
        if let Some(v) = row.class_no  { ws.write_number(r, 4, v as f64)?; }
        if let Some(v) = row.seq_no    { ws.write_number(r, 5, v as f64)?; }
        if let Some(v) = row.grad_year { ws.write_number(r, 6, v as f64)?; }
    }
    Ok(wb.save_to_buffer()?)
}

// ── 파싱 ─────────────────────────────────────────────────────────

fn row_to_record(cols: &[String]) -> StudentRecord {
    let get = |i: usize| cols.get(i).cloned().unwrap_or_default();
    let parse_i64 = |i: usize| get(i).trim().parse::<i64>().ok();
    StudentRecord {
        student_code: get(0).trim().to_string(),
        name:         get(1).trim().to_string(),
        is_enrolled:  parse_i64(2).unwrap_or(1),
        grade:        parse_i64(3),
        class_no:     parse_i64(4),
        seq_no:       parse_i64(5),
        grad_year:    parse_i64(6),
    }
}
