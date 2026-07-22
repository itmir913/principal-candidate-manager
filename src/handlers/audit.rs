use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Response,
    Json,
};
use rust_xlsxwriter::Workbook;
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::{excel, state::AppState};

type ApiError = (StatusCode, String);

fn default_page() -> i64 { 1 }
fn default_per_page() -> i64 { 50 }

#[derive(Deserialize)]
pub struct AuditQuery {
    #[serde(default = "default_page")]     pub page: i64,
    #[serde(default = "default_per_page")] pub per_page: i64,
    pub round_id: Option<i64>,
    pub action: Option<String>,
    /// 학급 필터 — grade·class_no 둘 다 지정 시에만 적용 (0/0 = 졸업생 담당).
    /// 지정 시 해당 학급 담임의 행위만 반환 (관리자 행은 제외됨).
    pub grade: Option<i64>,
    pub class_no: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct AuditRow {
    pub id: i64,
    pub at: String,
    pub actor_type: String,
    pub actor_grade: Option<i64>,
    pub actor_class_no: Option<i64>,
    pub actor_name: Option<String>,
    /// 계정 보안 이벤트(백업 다운로드·비밀번호 변경)에만 채워진다. 그 외는 None.
    /// 감사 화면 '상세' 열에 함께 표시된다 — src/docs/01_auth.md 'actor_ip 필드 규약'.
    pub actor_ip: Option<String>,
    pub action: String,
    pub round_id: Option<i64>,
    pub student_id: Option<i64>,
    pub detail: String,
}

#[derive(Serialize)]
pub struct AuditPage {
    pub rows: Vec<AuditRow>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

/// GET /api/audit-logs?page=&per_page=&round_id=&action=
pub async fn list_audit_logs(
    State(state): State<AppState>,
    Query(q): Query<AuditQuery>,
) -> Result<Json<AuditPage>, ApiError> {
    let per_page = q.per_page.clamp(1, 200);
    let page = q.page.max(1);
    let offset = (page - 1) * per_page;

    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log
         WHERE (? IS NULL OR round_id = ?)
           AND (? IS NULL OR action = ?)
           AND (? IS NULL OR (actor_grade = ? AND actor_class_no = ?))",
    )
    .bind(q.round_id)
    .bind(q.round_id)
    .bind(q.action.as_deref())
    .bind(q.action.as_deref())
    .bind(q.grade)
    .bind(q.grade)
    .bind(q.class_no)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let rows = sqlx::query(
        "SELECT id, at, actor_type, actor_grade, actor_class_no, actor_name, actor_ip,
                action, round_id, student_id, detail
         FROM audit_log
         WHERE (? IS NULL OR round_id = ?)
           AND (? IS NULL OR action = ?)
           AND (? IS NULL OR (actor_grade = ? AND actor_class_no = ?))
         ORDER BY id DESC
         LIMIT ? OFFSET ?",
    )
    .bind(q.round_id)
    .bind(q.round_id)
    .bind(q.action.as_deref())
    .bind(q.action.as_deref())
    .bind(q.grade)
    .bind(q.grade)
    .bind(q.class_no)
    .bind(per_page)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let result = rows
        .iter()
        .map(|row| AuditRow {
            id: row.get("id"),
            at: row.get("at"),
            actor_type: row.get("actor_type"),
            actor_grade: row.get("actor_grade"),
            actor_class_no: row.get("actor_class_no"),
            actor_name: row.get("actor_name"),
            actor_ip: row.get("actor_ip"),
            action: row.get("action"),
            round_id: row.get("round_id"),
            student_id: row.get("student_id"),
            detail: row.get("detail"),
        })
        .collect();

    Ok(Json(AuditPage { rows: result, total, page, per_page }))
}

/// GET /api/audit-logs/export — 전체 감사 기록 Excel 내보내기
pub async fn export_audit_logs(
    State(state): State<AppState>,
    Query(q): Query<AuditQuery>,
) -> Result<Response, ApiError> {
    let rows = sqlx::query(
        "SELECT id, at, actor_type, actor_grade, actor_class_no, actor_name, actor_ip,
                action, round_id, student_id, detail
         FROM audit_log
         WHERE (? IS NULL OR round_id = ?)
           AND (? IS NULL OR action = ?)
           AND (? IS NULL OR (actor_grade = ? AND actor_class_no = ?))
         ORDER BY id DESC",
    )
    .bind(q.round_id)
    .bind(q.round_id)
    .bind(q.action.as_deref())
    .bind(q.action.as_deref())
    .bind(q.grade)
    .bind(q.grade)
    .bind(q.class_no)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    ws.set_name("감사기록").map_err(excel::xlsx_err)?;

    // IP는 기존 열 순서를 흔들지 않도록 맨 뒤에 붙인다 (대부분의 행에서는 빈 칸).
    for (i, h) in ["시각", "행위자", "행위", "라운드", "상세", "IP"].iter().enumerate() {
        ws.write_string(0, i as u16, *h).map_err(excel::xlsx_err)?;
    }

    for (r, row) in rows.iter().enumerate() {
        let r = r as u32 + 1;

        let at: String = row.get("at");
        ws.write_string(r, 0, &at).map_err(excel::xlsx_err)?;

        let actor_type: String = row.get("actor_type");
        let actor = if actor_type == "ADMIN" {
            "관리자".to_string()
        } else {
            let grade: Option<i64> = row.get("actor_grade");
            let class_no: Option<i64> = row.get("actor_class_no");
            let name: Option<String> = row.get("actor_name");
            match (grade, class_no, name) {
                // grade=0/class_no=0은 졸업생 담당 특수 계정 — "0학년 0반"으로 표기하지 않는다
                (Some(0), Some(0), _) => "졸업생 담당".to_string(),
                (Some(g), Some(c), Some(n)) => format!("{}학년 {}반 {}", g, c, n),
                (Some(g), Some(c), None) => format!("{}학년 {}반", g, c),
                _ => actor_type,
            }
        };
        ws.write_string(r, 1, &actor).map_err(excel::xlsx_err)?;

        let action: String = row.get("action");
        ws.write_string(r, 2, &action).map_err(excel::xlsx_err)?;

        let round_id: Option<i64> = row.get("round_id");
        if let Some(rid) = round_id {
            ws.write_number(r, 3, rid as f64).map_err(excel::xlsx_err)?;
        }

        let detail: String = row.get("detail");
        ws.write_string(r, 4, &detail).map_err(excel::xlsx_err)?;

        let actor_ip: Option<String> = row.get("actor_ip");
        if let Some(ip) = actor_ip {
            ws.write_string(r, 5, &ip).map_err(excel::xlsx_err)?;
        }
    }

    let buf = wb
        .save_to_buffer()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(excel::xlsx_response(buf, &format!("audit_log_{}.xlsx", excel::now_tag())))
}
