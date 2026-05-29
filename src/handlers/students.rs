use axum::{
    extract::{Multipart, Path, Query, State},
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
    pub is_enrolled: Option<i64>,
}

#[derive(Serialize)]
pub struct GradeOptions {
    pub grades: Vec<i64>,
    pub by_grade: std::collections::HashMap<String, Vec<i64>>,
}

#[derive(FromRow)]
struct GradeClassRow {
    grade: i64,
    class_no: i64,
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
    let mut sql = "SELECT id, student_code, name, grade, class_no, seq_no, is_enrolled, grad_year \
                   FROM students WHERE 1=1".to_string();
    if q.grade.is_some()       { sql += " AND grade = ?"; }
    if q.class_no.is_some()    { sql += " AND class_no = ?"; }
    if q.is_enrolled.is_some() { sql += " AND is_enrolled = ?"; }
    sql += " ORDER BY is_enrolled DESC, grade, class_no, seq_no";

    let mut query = sqlx::query_as::<_, StudentRow>(&sql);
    if let Some(v) = q.grade       { query = query.bind(v); }
    if let Some(v) = q.class_no    { query = query.bind(v); }
    if let Some(v) = q.is_enrolled { query = query.bind(v); }

    let rows = query
        .fetch_all(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(rows))
}

/// GET /api/students/grade-options — 재학생 기준 등록된 학년·반 목록
pub async fn grade_options(State(state): State<AppState>) -> Result<Json<GradeOptions>, ApiError> {
    let rows = sqlx::query_as::<_, GradeClassRow>(
        "SELECT DISTINCT grade, class_no FROM students
         WHERE is_enrolled = 1 AND grade IS NOT NULL AND class_no IS NOT NULL
         ORDER BY grade, class_no",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut by_grade: std::collections::HashMap<String, Vec<i64>> = std::collections::HashMap::new();
    for r in &rows {
        by_grade.entry(r.grade.to_string()).or_default().push(r.class_no);
    }
    let mut grades: Vec<i64> = by_grade.keys().filter_map(|k| k.parse().ok()).collect();
    grades.sort_unstable();

    Ok(Json(GradeOptions { grades, by_grade }))
}

/// GET /api/students/template
pub async fn download_template() -> Result<Response, ApiError> {
    let buf = build_template_xlsx()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(excel::xlsx_response(buf, "students_all_template.xlsx"))
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
    Ok(excel::xlsx_response(buf, &format!("students_all_{}.xlsx", excel::now_tag())))
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

    let (headers, file_rows) = excel::parse_file_rows_with_headers(&bytes)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let col = excel::col_map(&headers);
    excel::require_cols(&col, &["학번", "이름", "재학여부"])
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let mut inserted = 0usize;
    let mut updated = 0usize;
    let mut errors: Vec<String> = Vec::new();

    let mut tx = state.db.begin().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    for (idx, cols) in file_rows.iter().enumerate() {
        let row_num = idx + 2;
        let rec = row_to_record(cols, &col);
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
    "학번", "이름", "재학여부",
    "학년", "반", "번호", "졸업연도",
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

// ── 재학생 전용 ───────────────────────────────────────────────────

// 가져오기 양식 헤더: 학번 제외 (백엔드에서 자동 생성)
const ENROLLED_IMPORT_HEADERS: &[&str] = &["이름", "학년", "반", "번호"];
// 내보내기 헤더: 참조용으로 학번 포함
const ENROLLED_EXPORT_HEADERS: &[&str] = &["학번", "이름", "학년", "반", "번호"];
const GRADUATED_HEADERS: &[&str] = &["학번", "이름", "졸업연도"];

/// GET /api/students/enrolled/template
pub async fn enrolled_template() -> Result<Response, ApiError> {
    let buf = build_enrolled_template_xlsx()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(excel::xlsx_response(buf, "students_enrolled_template.xlsx"))
}

/// GET /api/students/enrolled/export
pub async fn export_enrolled(State(state): State<AppState>) -> Result<Response, ApiError> {
    let rows = sqlx::query_as::<_, StudentRow>(
        "SELECT id, student_code, name, grade, class_no, seq_no, is_enrolled, grad_year
         FROM students WHERE is_enrolled = 1 ORDER BY grade, class_no, seq_no",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let buf = build_enrolled_export_xlsx(&rows)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(excel::xlsx_response(buf, &format!("students_enrolled_{}.xlsx", excel::now_tag())))
}

/// POST /api/students/enrolled/import
/// 양식 컬럼: name, grade, class_no, seq_no (student_code 없음 — 자동 생성)
/// 조회 기준: (grade, class_no, seq_no, is_enrolled=1) 위치 기반 upsert
pub async fn import_enrolled(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<ImportResult>, ApiError> {
    let bytes = loop {
        match multipart.next_field().await.map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))? {
            Some(f) => break f.bytes().await.map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?.to_vec(),
            None => return Err((StatusCode::BAD_REQUEST, "파일이 없습니다".into())),
        }
    };
    let (headers, file_rows) = excel::parse_file_rows_with_headers(&bytes)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let col = excel::col_map(&headers);
    excel::require_cols(&col, &["이름", "학년", "반", "번호"])
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let mut inserted = 0usize;
    let mut updated = 0usize;
    let mut errors: Vec<String> = Vec::new();
    let mut tx = state.db.begin().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    for (idx, cols) in file_rows.iter().enumerate() {
        let rec = row_to_enrolled_record(cols, &col);
        if let Err(e) = upsert_enrolled_by_position(&mut *tx, &rec, &mut inserted, &mut updated).await {
            errors.push(format!("{}행: {}", idx + 2, e));
        }
    }
    tx.commit().await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(ImportResult { inserted, updated, errors }))
}

// ── 졸업생 전용 ───────────────────────────────────────────────────

/// GET /api/students/graduated/template
pub async fn graduated_template() -> Result<Response, ApiError> {
    let buf = build_graduated_template_xlsx()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(excel::xlsx_response(buf, "students_graduated_template.xlsx"))
}

/// GET /api/students/graduated/export
pub async fn export_graduated(State(state): State<AppState>) -> Result<Response, ApiError> {
    let rows = sqlx::query_as::<_, StudentRow>(
        "SELECT id, student_code, name, grade, class_no, seq_no, is_enrolled, grad_year
         FROM students WHERE is_enrolled = 0 ORDER BY grad_year DESC, student_code",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let buf = build_graduated_export_xlsx(&rows)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(excel::xlsx_response(buf, &format!("students_graduated_{}.xlsx", excel::now_tag())))
}

/// POST /api/students/graduated/import
pub async fn import_graduated(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<ImportResult>, ApiError> {
    let bytes = loop {
        match multipart.next_field().await.map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))? {
            Some(f) => break f.bytes().await.map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?.to_vec(),
            None => return Err((StatusCode::BAD_REQUEST, "파일이 없습니다".into())),
        }
    };
    let (headers, file_rows) = excel::parse_file_rows_with_headers(&bytes)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let col = excel::col_map(&headers);
    excel::require_cols(&col, &["학번", "이름", "졸업연도"])
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let mut inserted = 0usize;
    let mut updated = 0usize;
    let mut errors: Vec<String> = Vec::new();
    let mut tx = state.db.begin().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    for (idx, cols) in file_rows.iter().enumerate() {
        let rec = row_to_graduated_record(cols, &col);
        if let Err(e) = upsert_student(&mut *tx, &rec, &mut inserted, &mut updated).await {
            errors.push(format!("{}행: {}", idx + 2, e));
        }
    }
    tx.commit().await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(ImportResult { inserted, updated, errors }))
}

// ── 재학생 위치 기반 upsert (student_code 자동 생성) ──────────────

async fn upsert_enrolled_by_position(
    conn: &mut sqlx::SqliteConnection,
    rec: &StudentRecord,
    inserted: &mut usize,
    updated: &mut usize,
) -> Result<(), String> {
    if rec.name.is_empty() {
        return Err("name 누락".into());
    }
    let (grade, class_no, seq_no) = match (rec.grade, rec.class_no, rec.seq_no) {
        (Some(g), Some(c), Some(s)) => (g, c, s),
        _ => return Err("grade, class_no, seq_no 필수".into()),
    };

    let class_ok: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM classes WHERE grade = ? AND class_no = ?",
    )
    .bind(grade).bind(class_no)
    .fetch_one(&mut *conn).await.map_err(|e| e.to_string())?;

    if class_ok == 0 {
        return Err(format!("{}학년 {}반이 학급 목록에 없습니다", grade, class_no));
    }

    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM students WHERE grade=? AND class_no=? AND seq_no=? AND is_enrolled=1)",
    )
    .bind(grade).bind(class_no).bind(seq_no)
    .fetch_one(&mut *conn).await.map_err(|e| e.to_string())?;

    if exists {
        sqlx::query(
            "UPDATE students SET name=? WHERE grade=? AND class_no=? AND seq_no=? AND is_enrolled=1",
        )
        .bind(&rec.name).bind(grade).bind(class_no).bind(seq_no)
        .execute(&mut *conn).await.map_err(|e| e.to_string())?;
        *updated += 1;
    } else {
        let year = chrono::Local::now().format("%Y").to_string();
        let base = format!("{}{:01}{:02}{:02}", year, grade, class_no, seq_no);
        let student_code = find_unique_code(&mut *conn, &base).await?;
        sqlx::query(
            "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled)
             VALUES (?, ?, ?, ?, ?, 1)",
        )
        .bind(&student_code).bind(&rec.name).bind(grade).bind(class_no).bind(seq_no)
        .execute(&mut *conn).await.map_err(|e| e.to_string())?;
        *inserted += 1;
    }
    Ok(())
}

async fn find_unique_code(conn: &mut sqlx::SqliteConnection, base: &str) -> Result<String, String> {
    let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM students WHERE student_code = ?")
        .bind(base).fetch_one(&mut *conn).await.map_err(|e| e.to_string())?;
    if exists == 0 {
        return Ok(base.to_string());
    }
    for n in 2u32..=99 {
        let candidate = format!("{}-{}", base, n);
        let ex: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM students WHERE student_code = ?")
            .bind(&candidate).fetch_one(&mut *conn).await.map_err(|e| e.to_string())?;
        if ex == 0 {
            return Ok(candidate);
        }
    }
    Err(format!("학번 자동 생성 실패: {} 충돌이 너무 많습니다", base))
}

// ── 학생 삭제 ─────────────────────────────────────────────────────

/// DELETE /api/students/:id
pub async fn delete_student(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    let base_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM base_data WHERE student_id = ?")
            .bind(id)
            .fetch_one(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let app_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM applications WHERE student_id = ?")
            .bind(id)
            .fetch_one(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if base_count > 0 || app_count > 0 {
        return Err((
            StatusCode::CONFLICT,
            format!(
                "기초 데이터 {}건, 지원 기록 {}건이 있어 삭제할 수 없습니다.",
                base_count, app_count
            ),
        ));
    }

    sqlx::query("DELETE FROM students WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

// ── xlsx 생성 (재학생/졸업생 전용) ────────────────────────────────

fn build_enrolled_template_xlsx() -> anyhow::Result<Vec<u8>> {
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    for (i, h) in ENROLLED_IMPORT_HEADERS.iter().enumerate() {
        ws.write_string(0, i as u16, *h)?;
    }
    // 샘플 행: name, grade, class_no, seq_no
    ws.write_string(1, 0, "홍길동")?;
    ws.write_number(1, 1, 1.0)?;
    ws.write_number(1, 2, 1.0)?;
    ws.write_number(1, 3, 1.0)?;
    Ok(wb.save_to_buffer()?)
}

fn build_enrolled_export_xlsx(rows: &[StudentRow]) -> anyhow::Result<Vec<u8>> {
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    for (i, h) in ENROLLED_EXPORT_HEADERS.iter().enumerate() {
        ws.write_string(0, i as u16, *h)?;
    }
    for (r, row) in rows.iter().enumerate() {
        let r = r as u32 + 1;
        ws.write_string(r, 0, &row.student_code)?;
        ws.write_string(r, 1, &row.name)?;
        if let Some(v) = row.grade    { ws.write_number(r, 2, v as f64)?; }
        if let Some(v) = row.class_no { ws.write_number(r, 3, v as f64)?; }
        if let Some(v) = row.seq_no   { ws.write_number(r, 4, v as f64)?; }
    }
    Ok(wb.save_to_buffer()?)
}

fn build_graduated_template_xlsx() -> anyhow::Result<Vec<u8>> {
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    for (i, h) in GRADUATED_HEADERS.iter().enumerate() {
        ws.write_string(0, i as u16, *h)?;
    }
    ws.write_string(1, 0, "20240001")?;
    ws.write_string(1, 1, "김철수")?;
    ws.write_number(1, 2, 2024.0)?;
    Ok(wb.save_to_buffer()?)
}

fn build_graduated_export_xlsx(rows: &[StudentRow]) -> anyhow::Result<Vec<u8>> {
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    for (i, h) in GRADUATED_HEADERS.iter().enumerate() {
        ws.write_string(0, i as u16, *h)?;
    }
    for (r, row) in rows.iter().enumerate() {
        let r = r as u32 + 1;
        ws.write_string(r, 0, &row.student_code)?;
        ws.write_string(r, 1, &row.name)?;
        if let Some(v) = row.grad_year { ws.write_number(r, 2, v as f64)?; }
    }
    Ok(wb.save_to_buffer()?)
}

// ── 파싱 ─────────────────────────────────────────────────────────

fn row_to_record(
    cols: &[String],
    col: &std::collections::HashMap<String, usize>,
) -> StudentRecord {
    let get = |name| excel::get_col(cols, col, name);
    let parse_i64 = |name| get(name).parse::<i64>().ok();
    StudentRecord {
        student_code: get("학번").to_string(),
        name:         get("이름").to_string(),
        is_enrolled:  parse_i64("재학여부").unwrap_or(1),
        grade:        parse_i64("학년"),
        class_no:     parse_i64("반"),
        seq_no:       parse_i64("번호"),
        grad_year:    parse_i64("졸업연도"),
    }
}

fn row_to_enrolled_record(
    cols: &[String],
    col: &std::collections::HashMap<String, usize>,
) -> StudentRecord {
    let get = |name| excel::get_col(cols, col, name);
    let parse_i64 = |name| get(name).parse::<i64>().ok();
    StudentRecord {
        student_code: String::new(), // upsert_enrolled_by_position 에서 자동 생성
        name:         get("이름").to_string(),
        is_enrolled:  1,
        grade:        parse_i64("학년"),
        class_no:     parse_i64("반"),
        seq_no:       parse_i64("번호"),
        grad_year:    None,
    }
}

fn row_to_graduated_record(
    cols: &[String],
    col: &std::collections::HashMap<String, usize>,
) -> StudentRecord {
    let get = |name| excel::get_col(cols, col, name);
    let parse_i64 = |name| get(name).parse::<i64>().ok();
    StudentRecord {
        student_code: get("학번").to_string(),
        name:         get("이름").to_string(),
        is_enrolled:  0,
        grade:        None,
        class_no:     None,
        seq_no:       None,
        grad_year:    parse_i64("졸업연도"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::create_test_pool, state::AppState};
    use axum::{extract::{Path, State}, http::StatusCode};

    fn make_state(pool: sqlx::SqlitePool) -> AppState {
        AppState { db: pool, jwt_secret: "test".into() }
    }

    async fn insert_class(pool: &sqlx::SqlitePool, grade: i64, class_no: i64) {
        let hash = bcrypt::hash("pass", 4u32).unwrap();
        sqlx::query(
            "INSERT INTO classes (grade, class_no, password_hash) VALUES (?, ?, ?)",
        )
        .bind(grade).bind(class_no).bind(&hash)
        .execute(pool).await.unwrap();
    }

    fn enrolled_rec(code: &str, name: &str, g: i64, c: i64, s: i64) -> StudentRecord {
        StudentRecord {
            student_code: code.into(),
            name: name.into(),
            is_enrolled: 1,
            grade: Some(g),
            class_no: Some(c),
            seq_no: Some(s),
            grad_year: None,
        }
    }

    fn graduated_rec(code: &str, name: &str, year: i64) -> StudentRecord {
        StudentRecord {
            student_code: code.into(),
            name: name.into(),
            is_enrolled: 0,
            grade: None,
            class_no: None,
            seq_no: None,
            grad_year: Some(year),
        }
    }

    // ── upsert_student ────────────────────────────────────────────────

    #[tokio::test]
    async fn upsert_student_empty_code_returns_error() {
        let pool = create_test_pool().await;
        let mut tx = pool.begin().await.unwrap();
        let rec = StudentRecord {
            student_code: "".into(),
            name: "홍길동".into(),
            is_enrolled: 0,
            grade: None, class_no: None, seq_no: None,
            grad_year: Some(2024),
        };
        let mut ins = 0; let mut upd = 0;
        let res = upsert_student(&mut *tx, &rec, &mut ins, &mut upd).await;
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("student_code"));
    }

    #[tokio::test]
    async fn upsert_student_empty_name_returns_error() {
        let pool = create_test_pool().await;
        let mut tx = pool.begin().await.unwrap();
        let rec = StudentRecord {
            student_code: "SC001".into(),
            name: "".into(),
            is_enrolled: 0,
            grade: None, class_no: None, seq_no: None,
            grad_year: Some(2024),
        };
        let mut ins = 0; let mut upd = 0;
        let res = upsert_student(&mut *tx, &rec, &mut ins, &mut upd).await;
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("name"));
    }

    #[tokio::test]
    async fn upsert_student_enrolled_missing_position_returns_error() {
        let pool = create_test_pool().await;
        let mut tx = pool.begin().await.unwrap();
        let rec = StudentRecord {
            student_code: "SC001".into(),
            name: "홍길동".into(),
            is_enrolled: 1,
            grade: None, class_no: None, seq_no: None, // 없음
            grad_year: None,
        };
        let mut ins = 0; let mut upd = 0;
        let res = upsert_student(&mut *tx, &rec, &mut ins, &mut upd).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn upsert_student_enrolled_class_not_found_returns_error() {
        let pool = create_test_pool().await;
        // 학급 미등록
        let mut tx = pool.begin().await.unwrap();
        let rec = enrolled_rec("SC001", "홍길동", 1, 1, 1);
        let mut ins = 0; let mut upd = 0;
        let res = upsert_student(&mut *tx, &rec, &mut ins, &mut upd).await;
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("학급 목록에 없습니다"));
    }

    #[tokio::test]
    async fn upsert_student_enrolled_inserts_new_student() {
        let pool = create_test_pool().await;
        insert_class(&pool, 1, 1).await;
        let mut tx = pool.begin().await.unwrap();
        let rec = enrolled_rec("SC001", "홍길동", 1, 1, 1);
        let mut ins = 0; let mut upd = 0;
        upsert_student(&mut *tx, &rec, &mut ins, &mut upd).await.unwrap();
        tx.commit().await.unwrap();
        assert_eq!(ins, 1);
        assert_eq!(upd, 0);
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM students")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn upsert_student_enrolled_updates_existing() {
        let pool = create_test_pool().await;
        insert_class(&pool, 1, 1).await;
        // 첫 번째 삽입
        let mut tx = pool.begin().await.unwrap();
        let rec = enrolled_rec("SC001", "홍길동", 1, 1, 1);
        let mut ins = 0; let mut upd = 0;
        upsert_student(&mut *tx, &rec, &mut ins, &mut upd).await.unwrap();
        tx.commit().await.unwrap();
        // 두 번째 — 같은 코드, 이름 변경
        let mut tx2 = pool.begin().await.unwrap();
        let rec2 = enrolled_rec("SC001", "이순신", 1, 1, 1);
        upsert_student(&mut *tx2, &rec2, &mut ins, &mut upd).await.unwrap();
        tx2.commit().await.unwrap();
        assert_eq!(ins, 1);
        assert_eq!(upd, 1);
        let name: String = sqlx::query_scalar("SELECT name FROM students WHERE student_code = 'SC001'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(name, "이순신");
    }

    #[tokio::test]
    async fn upsert_student_graduated_missing_year_returns_error() {
        let pool = create_test_pool().await;
        let mut tx = pool.begin().await.unwrap();
        let rec = StudentRecord {
            student_code: "GR001".into(),
            name: "졸업생".into(),
            is_enrolled: 0,
            grade: None, class_no: None, seq_no: None,
            grad_year: None, // 필수값 누락
        };
        let mut ins = 0; let mut upd = 0;
        let res = upsert_student(&mut *tx, &rec, &mut ins, &mut upd).await;
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("grad_year"));
    }

    #[tokio::test]
    async fn upsert_student_graduated_inserts_ok() {
        let pool = create_test_pool().await;
        let mut tx = pool.begin().await.unwrap();
        let rec = graduated_rec("GR001", "졸업생", 2024);
        let mut ins = 0; let mut upd = 0;
        upsert_student(&mut *tx, &rec, &mut ins, &mut upd).await.unwrap();
        tx.commit().await.unwrap();
        assert_eq!(ins, 1);
    }

    // ── upsert_enrolled_by_position ───────────────────────────────────

    #[tokio::test]
    async fn enrolled_by_position_generates_student_code() {
        let pool = create_test_pool().await;
        insert_class(&pool, 1, 1).await;
        let mut tx = pool.begin().await.unwrap();
        let rec = StudentRecord {
            student_code: String::new(),
            name: "홍길동".into(),
            is_enrolled: 1,
            grade: Some(1), class_no: Some(1), seq_no: Some(1),
            grad_year: None,
        };
        let mut ins = 0; let mut upd = 0;
        upsert_enrolled_by_position(&mut *tx, &rec, &mut ins, &mut upd).await.unwrap();
        tx.commit().await.unwrap();
        assert_eq!(ins, 1);
        let code: String =
            sqlx::query_scalar("SELECT student_code FROM students WHERE name = '홍길동'")
                .fetch_one(&pool).await.unwrap();
        assert!(!code.is_empty());
    }

    #[tokio::test]
    async fn enrolled_by_position_updates_existing_by_position() {
        let pool = create_test_pool().await;
        insert_class(&pool, 1, 1).await;
        let mut tx = pool.begin().await.unwrap();
        let rec = StudentRecord {
            student_code: String::new(),
            name: "홍길동".into(),
            is_enrolled: 1,
            grade: Some(1), class_no: Some(1), seq_no: Some(1),
            grad_year: None,
        };
        let mut ins = 0; let mut upd = 0;
        upsert_enrolled_by_position(&mut *tx, &rec, &mut ins, &mut upd).await.unwrap();
        let rec2 = StudentRecord { name: "이순신".into(), ..rec };
        upsert_enrolled_by_position(&mut *tx, &rec2, &mut ins, &mut upd).await.unwrap();
        tx.commit().await.unwrap();
        assert_eq!(ins, 1);
        assert_eq!(upd, 1);
        let name: String =
            sqlx::query_scalar(
                "SELECT name FROM students WHERE grade = 1 AND class_no = 1 AND seq_no = 1",
            )
            .fetch_one(&pool).await.unwrap();
        assert_eq!(name, "이순신");
    }

    #[tokio::test]
    async fn enrolled_by_position_missing_class_returns_error() {
        let pool = create_test_pool().await;
        // 학급 미등록
        let mut tx = pool.begin().await.unwrap();
        let rec = StudentRecord {
            student_code: String::new(),
            name: "홍길동".into(),
            is_enrolled: 1,
            grade: Some(2), class_no: Some(3), seq_no: Some(1),
            grad_year: None,
        };
        let mut ins = 0; let mut upd = 0;
        let res = upsert_enrolled_by_position(&mut *tx, &rec, &mut ins, &mut upd).await;
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("학급 목록에 없습니다"));
    }

    // ── find_unique_code ──────────────────────────────────────────────

    #[tokio::test]
    async fn find_unique_code_returns_base_when_no_collision() {
        let pool = create_test_pool().await;
        let mut tx = pool.begin().await.unwrap();
        let code = find_unique_code(&mut *tx, "20251101").await.unwrap();
        assert_eq!(code, "20251101");
    }

    #[tokio::test]
    async fn find_unique_code_returns_suffix_on_collision() {
        let pool = create_test_pool().await;
        // base code를 미리 점유
        sqlx::query(
            "INSERT INTO students (student_code, name, is_enrolled, grad_year) \
             VALUES ('20251101', '기존학생', 0, 2024)",
        )
        .execute(&pool)
        .await
        .unwrap();
        let mut tx = pool.begin().await.unwrap();
        let code = find_unique_code(&mut *tx, "20251101").await.unwrap();
        assert_eq!(code, "20251101-2");
    }

    #[tokio::test]
    async fn find_unique_code_increments_suffix_until_free() {
        let pool = create_test_pool().await;
        // base + -2 모두 점유
        for suffix in &["20251101", "20251101-2"] {
            sqlx::query(
                "INSERT INTO students (student_code, name, is_enrolled, grad_year) VALUES (?, '기존학생', 0, 2024)",
            )
            .bind(suffix)
            .execute(&pool)
            .await
            .unwrap();
        }
        let mut tx = pool.begin().await.unwrap();
        let code = find_unique_code(&mut *tx, "20251101").await.unwrap();
        assert_eq!(code, "20251101-3");
    }

    // ── delete_student ────────────────────────────────────────────────

    #[tokio::test]
    async fn delete_student_no_refs_ok() {
        let pool = create_test_pool().await;
        let sid: i64 = sqlx::query_scalar(
            "INSERT INTO students (student_code, name, is_enrolled, grad_year) \
             VALUES ('S001', '홍길동', 0, 2024) RETURNING id",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        delete_student(State(make_state(pool.clone())), Path(sid))
            .await
            .unwrap();
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM students")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn delete_student_with_base_data_returns_conflict() {
        let pool = create_test_pool().await;
        let sid: i64 = sqlx::query_scalar(
            "INSERT INTO students (student_code, name, is_enrolled, grad_year) \
             VALUES ('S001', '홍길동', 0, 2024) RETURNING id",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let aid: i64 = sqlx::query_scalar(
            "INSERT INTO areas (name, max_score, calc_type, lookup_scope) \
             VALUES ('내신', 100000, 'RANGE', 'SIMPLE') RETURNING id",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO base_data (student_id, area_id, univ_id, value) VALUES (?, ?, NULL, '10000')",
        )
        .bind(sid).bind(aid)
        .execute(&pool)
        .await
        .unwrap();
        let res = delete_student(State(make_state(pool)), Path(sid)).await;
        assert_eq!(res.unwrap_err().0, StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn delete_student_with_application_returns_conflict() {
        let pool = create_test_pool().await;
        let hash = bcrypt::hash("pass", 4u32).unwrap();
        sqlx::query("INSERT INTO classes (grade, class_no, password_hash) VALUES (1, 1, ?)")
            .bind(&hash).execute(&pool).await.unwrap();
        let sid: i64 = sqlx::query_scalar(
            "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
             VALUES ('S001', '홍길동', 1, 1, 1, 1) RETURNING id",
        )
        .fetch_one(&pool).await.unwrap();
        let uid: i64 = sqlx::query_scalar(
            "INSERT INTO universities (univ_name, track_name, capacity) \
             VALUES ('서울대', '컴공', 5) RETURNING id",
        )
        .fetch_one(&pool).await.unwrap();
        let rid: i64 = sqlx::query_scalar(
            "INSERT INTO rounds (status, opened_at) VALUES ('OPEN', '2025-01-01') RETURNING id",
        )
        .fetch_one(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO applications (student_id, univ_id, round_id, confirmed, abandoned) \
             VALUES (?, ?, ?, 1, 0)",
        )
        .bind(sid).bind(uid).bind(rid)
        .execute(&pool).await.unwrap();
        let res = delete_student(State(make_state(pool)), Path(sid)).await;
        assert_eq!(res.unwrap_err().0, StatusCode::CONFLICT);
    }
}
