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

#[derive(Deserialize)]
pub struct AddEnrolledBody {
    pub name: String,
    pub grade: i64,
    pub class_no: i64,
    pub seq_no: i64,
}

#[derive(Deserialize)]
pub struct AddGraduatedBody {
    pub student_code: String,
    pub name: String,
    pub grad_year: i64,
}

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
    pub is_enrolled: bool,
    pub grad_year: Option<i64>,
}

#[derive(Debug, Serialize)]
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
    #[serde(default = "default_page")]     pub page: i64,
    #[serde(default = "default_per_page")] pub per_page: i64,
}
fn default_page() -> i64 { 1 }
fn default_per_page() -> i64 { 100 }

#[derive(Serialize)]
pub struct StudentPage {
    pub rows: Vec<StudentRow>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
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

pub struct StudentRecord {
    pub student_code: String,
    pub name: String,
    pub is_enrolled: bool,
    pub grade: Option<i64>,
    pub class_no: Option<i64>,
    pub seq_no: Option<i64>,
    pub grad_year: Option<i64>,
}

// ── 핸들러 ───────────────────────────────────────────────────────

/// GET /api/students
pub async fn list_students(
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> Result<Json<StudentPage>, ApiError> {
    let per_page = q.per_page.max(1);
    let page = q.page.max(1);
    let offset = (page - 1) * per_page;

    // COUNT (동일 조건)
    let mut count_sql = "SELECT COUNT(*) FROM students WHERE 1=1".to_string();
    if q.grade.is_some()       { count_sql += " AND grade = ?"; }
    if q.class_no.is_some()    { count_sql += " AND class_no = ?"; }
    if q.is_enrolled.is_some() { count_sql += " AND is_enrolled = ?"; }

    let mut count_query = sqlx::query_scalar::<_, i64>(&count_sql);
    if let Some(v) = q.grade       { count_query = count_query.bind(v); }
    if let Some(v) = q.class_no    { count_query = count_query.bind(v); }
    if let Some(v) = q.is_enrolled { count_query = count_query.bind(v); }

    let total = count_query
        .fetch_one(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 데이터 조회
    let mut sql = "SELECT id, student_code, name, grade, class_no, seq_no, is_enrolled, grad_year \
                   FROM students WHERE 1=1".to_string();
    if q.grade.is_some()       { sql += " AND grade = ?"; }
    if q.class_no.is_some()    { sql += " AND class_no = ?"; }
    if q.is_enrolled.is_some() { sql += " AND is_enrolled = ?"; }
    sql += " ORDER BY is_enrolled DESC, grade, class_no, seq_no LIMIT ? OFFSET ?";

    let mut query = sqlx::query_as::<_, StudentRow>(&sql);
    if let Some(v) = q.grade       { query = query.bind(v); }
    if let Some(v) = q.class_no    { query = query.bind(v); }
    if let Some(v) = q.is_enrolled { query = query.bind(v); }
    query = query.bind(per_page).bind(offset);

    let rows = query
        .fetch_all(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(StudentPage { rows, total, page, per_page }))
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
) -> Result<(StatusCode, Json<ImportResult>), ApiError> {
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
    excel::require_cols(&col, &["학생코드", "이름", "재학여부"])
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

    if !errors.is_empty() {
        return Ok((StatusCode::UNPROCESSABLE_ENTITY, Json(ImportResult { inserted: 0, updated: 0, errors })));
    }
    tx.commit().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok((StatusCode::OK, Json(ImportResult { inserted, updated, errors: vec![] })))
}

// ── DB upsert ─────────────────────────────────────────────────────

pub async fn upsert_student(
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

    if rec.is_enrolled {
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
    "학생코드", "이름", "재학여부",
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
        ws.write_number(r, 2, if row.is_enrolled { 1.0 } else { 0.0 })?;
        if let Some(v) = row.grade     { ws.write_number(r, 3, v as f64)?; }
        if let Some(v) = row.class_no  { ws.write_number(r, 4, v as f64)?; }
        if let Some(v) = row.seq_no    { ws.write_number(r, 5, v as f64)?; }
        if let Some(v) = row.grad_year { ws.write_number(r, 6, v as f64)?; }
    }
    Ok(wb.save_to_buffer()?)
}

// ── 재학생 전용 ───────────────────────────────────────────────────

// 가져오기 양식 헤더: 학생코드 제외 (백엔드에서 자동 생성)
const ENROLLED_IMPORT_HEADERS: &[&str] = &["학년", "반", "번호", "이름"];
// 내보내기 헤더: 참조용으로 학생코드 포함
const ENROLLED_EXPORT_HEADERS: &[&str] = &["학생코드", "이름", "학년", "반", "번호"];
const GRADUATED_HEADERS: &[&str] = &["학생코드", "이름", "졸업연도"];

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
) -> Result<(StatusCode, Json<ImportResult>), ApiError> {
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
    if !errors.is_empty() {
        return Ok((StatusCode::UNPROCESSABLE_ENTITY, Json(ImportResult { inserted: 0, updated: 0, errors })));
    }
    tx.commit().await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok((StatusCode::OK, Json(ImportResult { inserted, updated, errors: vec![] })))
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
) -> Result<(StatusCode, Json<ImportResult>), ApiError> {
    let bytes = loop {
        match multipart.next_field().await.map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))? {
            Some(f) => break f.bytes().await.map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?.to_vec(),
            None => return Err((StatusCode::BAD_REQUEST, "파일이 없습니다".into())),
        }
    };
    let (headers, file_rows) = excel::parse_file_rows_with_headers(&bytes)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let col = excel::col_map(&headers);
    excel::require_cols(&col, &["학생코드", "이름", "졸업연도"])
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
    if !errors.is_empty() {
        return Ok((StatusCode::UNPROCESSABLE_ENTITY, Json(ImportResult { inserted: 0, updated: 0, errors })));
    }
    tx.commit().await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok((StatusCode::OK, Json(ImportResult { inserted, updated, errors: vec![] })))
}

// ── 재학생 위치 기반 upsert (student_code 자동 생성) ──────────────

pub async fn upsert_enrolled_by_position(
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

pub async fn find_unique_code(conn: &mut sqlx::SqliteConnection, base: &str) -> Result<String, String> {
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
    Err(format!("학생코드 자동 생성 실패: {} 충돌이 너무 많습니다", base))
}

// ── 학생 개별 추가 ────────────────────────────────────────────────

/// POST /api/students/enrolled/add
pub async fn add_enrolled(
    State(state): State<AppState>,
    Json(body): Json<AddEnrolledBody>,
) -> Result<(StatusCode, Json<ImportResult>), ApiError> {
    let name = body.name.trim().to_string();
    if name.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "이름이 비어있습니다".into()));
    }
    let rec = StudentRecord {
        student_code: String::new(),
        name,
        is_enrolled: true,
        grade: Some(body.grade),
        class_no: Some(body.class_no),
        seq_no: Some(body.seq_no),
        grad_year: None,
    };
    let mut tx = state.db.begin().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mut inserted = 0usize;
    let mut updated = 0usize;
    upsert_enrolled_by_position(&mut *tx, &rec, &mut inserted, &mut updated)
        .await
        .map_err(|e| (StatusCode::UNPROCESSABLE_ENTITY, e))?;
    tx.commit().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok((StatusCode::OK, Json(ImportResult { inserted, updated, errors: vec![] })))
}

/// POST /api/students/graduated/add
pub async fn add_graduated(
    State(state): State<AppState>,
    Json(body): Json<AddGraduatedBody>,
) -> Result<(StatusCode, Json<ImportResult>), ApiError> {
    let name = body.name.trim().to_string();
    let student_code = body.student_code.trim().to_string();
    if name.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "이름이 비어있습니다".into()));
    }
    if student_code.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "학생코드가 비어있습니다".into()));
    }
    let rec = StudentRecord {
        student_code,
        name,
        is_enrolled: false,
        grade: None,
        class_no: None,
        seq_no: None,
        grad_year: Some(body.grad_year),
    };
    let mut tx = state.db.begin().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mut inserted = 0usize;
    let mut updated = 0usize;
    upsert_student(&mut *tx, &rec, &mut inserted, &mut updated)
        .await
        .map_err(|e| (StatusCode::UNPROCESSABLE_ENTITY, e))?;
    tx.commit().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok((StatusCode::OK, Json(ImportResult { inserted, updated, errors: vec![] })))
}

// ── 학생 삭제 ─────────────────────────────────────────────────────

/// DELETE /api/students/:id
pub async fn delete_student(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    // COUNT와 DELETE를 같은 tx 안에서 실행 — TOCTOU 방지
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let base_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM base_data WHERE student_id = ?")
            .bind(id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let app_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM applications WHERE student_id = ?")
            .bind(id)
            .fetch_one(&mut *tx)
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
        .execute(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    tx.commit()
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
    // 샘플 행: grade, class_no, seq_no, name
    ws.write_number(1, 0, 3.0)?;
    ws.write_number(1, 1, 1.0)?;
    ws.write_number(1, 2, 1.0)?;
    ws.write_string(1, 3, "홍길동")?;
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
        student_code: get("학생코드").to_string(),
        name:         get("이름").to_string(),
        is_enrolled:  parse_i64("재학여부").map_or(true, |v| v != 0),
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
        is_enrolled:  true,
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
        student_code: get("학생코드").to_string(),
        name:         get("이름").to_string(),
        is_enrolled:  false,
        grade:        None,
        class_no:     None,
        seq_no:       None,
        grad_year:    parse_i64("졸업연도"),
    }
}
