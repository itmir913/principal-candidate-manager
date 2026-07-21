use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    response::Response,
    Json,
};
use rust_xlsxwriter::Workbook;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::{
    audit::{self, Actor, AuditEntry},
    enums::AuditAction,
    excel,
    middleware::multipart_err,
    state::AppState,
};

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
    let mut rows = sqlx::query_as::<_, ClassRow>(
        "SELECT grade, class_no, teacher_name FROM classes ORDER BY grade, class_no",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 졸업생이 한 명이라도 있으면 "졸업생" 항목을 sentinel(grade=0, class_no=0)로 추가
    let has_graduates: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM students WHERE is_enrolled = 0)",
    )
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if has_graduates {
        rows.push(ClassRow { grade: 0, class_no: 0, teacher_name: Some("졸업생".into()) });
    }

    Ok(Json(rows))
}

pub async fn classes_template() -> Result<Response, ApiError> {
    let mut wb = Workbook::new();
    let ws = wb
        .add_worksheet()
        .set_name("학급목록")
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    for (i, h) in ["학년", "반", "담임명", "비밀번호"].iter().enumerate() {
        ws.write_string(0, i as u16, *h).map_err(excel::xlsx_err)?;
    }
    ws.write_number(1, 0, 3.0).map_err(excel::xlsx_err)?;
    ws.write_number(1, 1, 1.0).map_err(excel::xlsx_err)?;
    ws.write_string(1, 2, "홍길동").map_err(excel::xlsx_err)?;
    ws.write_string(1, 3, "1234").map_err(excel::xlsx_err)?;

    let buf = wb
        .save_to_buffer()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(excel::xlsx_response(buf, "classes_template.xlsx"))
}

pub async fn import_classes(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let field = multipart
        .next_field()
        .await
        .map_err(multipart_err)?
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "파일이 없습니다".to_string()))?;
    let bytes = field
        .bytes()
        .await
        .map_err(multipart_err)?;

    let (headers, rows) = excel::parse_file_rows_with_headers(&bytes)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    let col = excel::col_map(&headers);
    excel::require_cols(&col, &["학년", "반", "담임명"])
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    let mut inserted = 0usize;
    let mut updated = 0usize;
    let mut errors: Vec<String> = Vec::new();
    // 파일 내 동일 (학년, 반) 중복 감지 — 마지막 행이 조용히 이기면 중복=error 정책 위반
    let mut seen: std::collections::HashSet<(i64, i64)> = std::collections::HashSet::new();

    let mut tx = state.db.begin().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    for (i, row) in rows.iter().enumerate() {
        let line = i + 2;

        let grade: i64 = match excel::get_col(row, &col, "학년").parse::<i64>().ok() {
            Some(g) if g > 0 => g,
            _ => { errors.push(format!("{}행: 학년 값이 올바르지 않습니다", line)); continue; }
        };
        let class_no: i64 = match excel::get_col(row, &col, "반").parse::<i64>().ok() {
            Some(c) if c > 0 => c,
            _ => { errors.push(format!("{}행: 반 값이 올바르지 않습니다", line)); continue; }
        };
        if !seen.insert((grade, class_no)) {
            errors.push(format!(
                "{}행: {}학년 {}반 중복 — 파일에 같은 반이 두 번 이상 존재합니다",
                line, grade, class_no
            ));
            continue;
        }
        let teacher_name_str = excel::get_col(row, &col, "담임명").to_string();
        if teacher_name_str.is_empty() {
            errors.push(format!("{}행: 담임명 누락", line));
            continue;
        }
        let teacher_name = Some(teacher_name_str);
        let password: Option<String> = {
            let v = excel::get_col(row, &col, "비밀번호").to_string();
            if v.is_empty() { None } else { Some(v) }
        };

        // 비밀번호 최소 길이 검증 (4자 미만이면 해당 행 오류 처리)
        if let Some(ref pw) = password {
            if pw.len() < 4 {
                errors.push(format!("{}행: 비밀번호는 4자 이상이어야 합니다", line));
                continue;
            }
        }

        // bcrypt는 CPU 작업이므로 트랜잭션 진입 전에 미리 계산
        let password_hash: Option<String> = if let Some(ref pw) = password {
            Some(bcrypt::hash(pw, bcrypt::DEFAULT_COST)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?)
        } else {
            None
        };

        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM classes WHERE grade = ? AND class_no = ?)",
        )
        .bind(grade).bind(class_no)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        if exists {
            if let Some(ref name) = teacher_name {
                sqlx::query("UPDATE classes SET teacher_name = ? WHERE grade = ? AND class_no = ?")
                    .bind(name).bind(grade).bind(class_no)
                    .execute(&mut *tx).await
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            }
            if let Some(ref hash) = password_hash {
                sqlx::query("UPDATE classes SET password_hash = ? WHERE grade = ? AND class_no = ?")
                    .bind(hash).bind(grade).bind(class_no)
                    .execute(&mut *tx).await
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            }
            updated += 1;
        } else {
            // 신규 학급은 password_hash NOT NULL — 누락 시 SQL 제약 500이 아니라 행 오류로 처리
            let Some(ref hash) = password_hash else {
                errors.push(format!("{}행: 신규 학급은 비밀번호가 필요합니다", line));
                continue;
            };
            sqlx::query(
                "INSERT INTO classes (grade, class_no, teacher_name, password_hash) VALUES (?, ?, ?, ?)",
            )
            .bind(grade).bind(class_no).bind(teacher_name).bind(hash)
            .execute(&mut *tx).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            inserted += 1;
        }
    }

    if !errors.is_empty() {
        return Ok((StatusCode::UNPROCESSABLE_ENTITY, Json(serde_json::json!({ "inserted": 0, "updated": 0, "errors": errors }))));
    }
    audit::log(&mut *tx, AuditEntry {
        actor: Actor::Admin,
        action: AuditAction::ClassesImported,
        round_id: None,
        student_id: None,
        detail: serde_json::json!({ "inserted": inserted, "updated": updated }),
    }).await?;
    tx.commit().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok((StatusCode::OK, Json(serde_json::json!({ "inserted": inserted, "updated": updated, "errors": [] }))))
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
        ws.write_string(0, i as u16, *h).map_err(excel::xlsx_err)?;
    }
    for (row_i, r) in rows.iter().enumerate() {
        let ri = (row_i + 1) as u32;
        ws.write_number(ri, 0, r.grade as f64).map_err(excel::xlsx_err)?;
        ws.write_number(ri, 1, r.class_no as f64).map_err(excel::xlsx_err)?;
        ws.write_string(ri, 2, r.teacher_name.as_deref().unwrap_or("")).map_err(excel::xlsx_err)?;
    }

    let buf = wb
        .save_to_buffer()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(excel::xlsx_response(buf, &format!("classes_{}.xlsx", excel::now_tag())))
}

pub async fn delete_class(
    State(state): State<AppState>,
    Path((grade, class_no)): Path<(i64, i64)>,
) -> Result<StatusCode, ApiError> {
    let mut tx = state.db.begin().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let student_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM students WHERE grade = ? AND class_no = ?",
    )
    .bind(grade)
    .bind(class_no)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if student_count > 0 {
        return Err((
            StatusCode::CONFLICT,
            format!(
                "{}학년 {}반에 학생 {}명이 등록되어 있어 삭제할 수 없습니다.",
                grade, class_no, student_count
            ),
        ));
    }

    let teacher_name: Option<String> = sqlx::query_scalar::<_, Option<String>>(
        "SELECT teacher_name FROM classes WHERE grade = ? AND class_no = ?",
    )
    .bind(grade)
    .bind(class_no)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    // 행 없음(404)과 담임명 미설정(NULL)을 구분 — 없는 학급의 삭제 로그를 남기지 않는다
    .ok_or((StatusCode::NOT_FOUND, "학급을 찾을 수 없습니다".to_string()))?;

    sqlx::query("DELETE FROM classes WHERE grade = ? AND class_no = ?")
        .bind(grade)
        .bind(class_no)
        .execute(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    audit::log(&mut *tx, AuditEntry {
        actor: Actor::Admin,
        action: AuditAction::ClassDeleted,
        round_id: None,
        student_id: None,
        detail: serde_json::json!({ "grade": grade, "class_no": class_no, "teacher_name": teacher_name }),
    }).await?;

    tx.commit().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn upsert_class(
    State(state): State<AppState>,
    Path((grade, class_no)): Path<(i64, i64)>,
    Json(body): Json<UpsertClassBody>,
) -> Result<StatusCode, ApiError> {
    if grade == 0 && class_no == 0 {
        return Err((StatusCode::BAD_REQUEST, "학년=0, 반=0은 졸업생 전용 예약값으로 사용할 수 없습니다".into()));
    }
    // import 경로(g > 0 검사)와 동일 기준 — 0/음수 학년·반 생성 차단
    if grade <= 0 || class_no <= 0 {
        return Err((StatusCode::BAD_REQUEST, "학년과 반은 1 이상이어야 합니다".into()));
    }

    // 비밀번호 최소 길이 검증
    if let Some(ref pw) = body.password {
        if pw.len() < 4 {
            return Err((StatusCode::BAD_REQUEST, "비밀번호는 4자 이상이어야 합니다".into()));
        }
    }

    let password_changed = body.password.is_some();

    // bcrypt는 CPU 작업이므로 트랜잭션 밖에서 미리 계산
    let password_hash = if let Some(ref pw) = body.password {
        Some(bcrypt::hash(pw, bcrypt::DEFAULT_COST)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?)
    } else {
        None
    };

    let mut tx = state.db.begin().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM classes WHERE grade = ? AND class_no = ?",
    )
    .bind(grade)
    .bind(class_no)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if count == 0 {
        let hash = password_hash.ok_or((
            StatusCode::BAD_REQUEST,
            "신규 학급은 비밀번호를 설정해야 합니다".to_string(),
        ))?;
        sqlx::query(
            "INSERT INTO classes (grade, class_no, teacher_name, password_hash) VALUES (?, ?, ?, ?)",
        )
        .bind(grade)
        .bind(class_no)
        .bind(body.teacher_name)
        .bind(hash)
        .execute(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    } else {
        // 무변경 저장 — 프론트가 변경 사항 없이 저장하면 빈 body를 보낸다 (ClassesTab saveEdit).
        // 쓰기가 없으므로 감사 로그도 남기지 않고 성공으로 조기 종료한다 (400을 주면 정상 UI 조작이 오류가 됨).
        if body.teacher_name.is_none() && password_hash.is_none() {
            return Ok(StatusCode::NO_CONTENT);
        }
        if let Some(ref name) = body.teacher_name {
            sqlx::query("UPDATE classes SET teacher_name = ? WHERE grade = ? AND class_no = ?")
                .bind(name)
                .bind(grade)
                .bind(class_no)
                .execute(&mut *tx)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        }
        if let Some(ref hash) = password_hash {
            sqlx::query("UPDATE classes SET password_hash = ? WHERE grade = ? AND class_no = ?")
                .bind(hash)
                .bind(grade)
                .bind(class_no)
                .execute(&mut *tx)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        }
    }

    let teacher_name: Option<String> = sqlx::query_scalar::<_, Option<String>>(
        "SELECT teacher_name FROM classes WHERE grade = ? AND class_no = ?",
    )
    .bind(grade)
    .bind(class_no)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .flatten();

    audit::log(&mut *tx, AuditEntry {
        actor: Actor::Admin,
        action: AuditAction::ClassSaved,
        round_id: None,
        student_id: None,
        detail: serde_json::json!({
            "grade": grade,
            "class_no": class_no,
            "teacher_name": teacher_name,
            "password_changed": password_changed,
        }),
    }).await?;

    tx.commit().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}
