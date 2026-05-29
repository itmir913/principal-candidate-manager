use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    response::Response,
    Json,
};
use rust_xlsxwriter::Workbook;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::{excel, state::AppState};

type ApiError = (StatusCode, String);

#[derive(Serialize, FromRow)]
pub struct ClassRow {
    pub grade: i64,
    pub class_no: i64,
    pub teacher_name: Option<String>,
}

#[derive(Deserialize)]
pub struct UpsertClassBody {
    pub teacher_name: Option<String>,
    pub password: Option<String>,
}

pub async fn list_classes(
    State(state): State<AppState>,
) -> Result<Json<Vec<ClassRow>>, ApiError> {
    let rows = sqlx::query_as::<_, ClassRow>(
        "SELECT grade, class_no, teacher_name FROM classes ORDER BY grade, class_no",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(rows))
}

pub async fn classes_template() -> Result<Response, ApiError> {
    let mut wb = Workbook::new();
    let ws = wb
        .add_worksheet()
        .set_name("학급목록")
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    for (i, h) in ["학년", "반", "담임명", "비밀번호"].iter().enumerate() {
        ws.write_string(0, i as u16, *h).ok();
    }
    ws.write_number(1, 0, 1.0).ok();
    ws.write_number(1, 1, 1.0).ok();
    ws.write_string(1, 2, "홍길동").ok();
    ws.write_string(1, 3, "pass1234").ok();

    let buf = wb
        .save_to_buffer()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(excel::xlsx_response(buf, "classes_template.xlsx"))
}

pub async fn import_classes(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, ApiError> {
    let field = multipart
        .next_field()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "파일이 없습니다".to_string()))?;
    let bytes = field
        .bytes()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let rows = excel::parse_file_rows(&bytes)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let mut inserted = 0usize;
    let mut updated = 0usize;
    let mut errors: Vec<String> = Vec::new();

    for (i, row) in rows.iter().enumerate() {
        let line = i + 2;

        let grade: i64 = match row.first().and_then(|s| s.trim().parse().ok()) {
            Some(g) if g > 0 => g,
            _ => { errors.push(format!("{}행: 학년 값이 올바르지 않습니다", line)); continue; }
        };
        let class_no: i64 = match row.get(1).and_then(|s| s.trim().parse().ok()) {
            Some(c) if c > 0 => c,
            _ => { errors.push(format!("{}행: 반 값이 올바르지 않습니다", line)); continue; }
        };
        let teacher_name: Option<String> = row.get(2)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let password: Option<String> = row.get(3)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM classes WHERE grade = ? AND class_no = ?)",
        )
        .bind(grade).bind(class_no)
        .fetch_one(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        if exists {
            if let Some(ref name) = teacher_name {
                sqlx::query("UPDATE classes SET teacher_name = ? WHERE grade = ? AND class_no = ?")
                    .bind(name).bind(grade).bind(class_no)
                    .execute(&state.db).await
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            }
            if let Some(ref pw) = password {
                let hash = bcrypt::hash(pw, bcrypt::DEFAULT_COST)
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
                sqlx::query("UPDATE classes SET password_hash = ? WHERE grade = ? AND class_no = ?")
                    .bind(hash).bind(grade).bind(class_no)
                    .execute(&state.db).await
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            }
            updated += 1;
        } else {
            let hash = if let Some(ref pw) = password {
                Some(bcrypt::hash(pw, bcrypt::DEFAULT_COST)
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?)
            } else {
                None
            };
            sqlx::query(
                "INSERT INTO classes (grade, class_no, teacher_name, password_hash) VALUES (?, ?, ?, ?)",
            )
            .bind(grade).bind(class_no).bind(teacher_name).bind(hash)
            .execute(&state.db).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            inserted += 1;
        }
    }

    Ok(Json(serde_json::json!({ "inserted": inserted, "updated": updated, "errors": errors })))
}

pub async fn export_classes(
    State(state): State<AppState>,
) -> Result<Response, ApiError> {
    let rows = sqlx::query_as::<_, ClassRow>(
        "SELECT grade, class_no, teacher_name FROM classes ORDER BY grade, class_no",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut wb = Workbook::new();
    let ws = wb
        .add_worksheet()
        .set_name("학급목록")
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    for (i, h) in ["학년", "반", "담임명"].iter().enumerate() {
        ws.write_string(0, i as u16, *h).ok();
    }
    for (row_i, r) in rows.iter().enumerate() {
        let ri = (row_i + 1) as u32;
        ws.write_number(ri, 0, r.grade as f64).ok();
        ws.write_number(ri, 1, r.class_no as f64).ok();
        ws.write_string(ri, 2, r.teacher_name.as_deref().unwrap_or("")).ok();
    }

    let buf = wb
        .save_to_buffer()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(excel::xlsx_response(buf, &format!("classes_{}.xlsx", excel::now_tag())))
}

pub async fn upsert_class(
    State(state): State<AppState>,
    Path((grade, class_no)): Path<(i64, i64)>,
    Json(body): Json<UpsertClassBody>,
) -> Result<StatusCode, ApiError> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM classes WHERE grade = ? AND class_no = ?",
    )
    .bind(grade)
    .bind(class_no)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if count == 0 {
        let password_hash = if let Some(ref pw) = body.password {
            Some(
                bcrypt::hash(pw, bcrypt::DEFAULT_COST)
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?,
            )
        } else {
            None
        };
        sqlx::query(
            "INSERT INTO classes (grade, class_no, teacher_name, password_hash) VALUES (?, ?, ?, ?)",
        )
        .bind(grade)
        .bind(class_no)
        .bind(body.teacher_name)
        .bind(password_hash)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    } else {
        if let Some(ref name) = body.teacher_name {
            sqlx::query("UPDATE classes SET teacher_name = ? WHERE grade = ? AND class_no = ?")
                .bind(name)
                .bind(grade)
                .bind(class_no)
                .execute(&state.db)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        }
        if let Some(ref pw) = body.password {
            let hash = bcrypt::hash(pw, bcrypt::DEFAULT_COST)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            sqlx::query("UPDATE classes SET password_hash = ? WHERE grade = ? AND class_no = ?")
                .bind(hash)
                .bind(grade)
                .bind(class_no)
                .execute(&state.db)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        }
    }

    Ok(StatusCode::NO_CONTENT)
}
