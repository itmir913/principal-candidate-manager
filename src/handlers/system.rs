use axum::{
    body::Body,
    extract::{ConnectInfo, State},
    http::StatusCode,
    response::Response,
    Extension, Json,
};
use serde::Serialize;
use std::net::SocketAddr;

use crate::{
    audit::{self, Actor, AuditEntry},
    auth,
    enums::AuditAction,
    state::AppState,
};

type ApiError = (StatusCode, String);

#[derive(Serialize)]
pub struct VersionResponse {
    pub version: &'static str,
}

pub async fn get_version() -> Json<VersionResponse> {
    Json(VersionResponse {
        version: env!("CARGO_PKG_VERSION"),
    })
}

pub async fn download_db_backup(
    State(state): State<AppState>,
    Extension(_claims): Extension<auth::AdminClaims>,
    ConnectInfo(client): ConnectInfo<SocketAddr>,
) -> Result<Response<Body>, ApiError> {
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let filename = format!("data_backup_{}.db", timestamp);

    // VACUUM INTO: 중첩 트랜잭션 없이 일관된 스냅샷 복사본 생성
    let tmp_path = state
        .db_path
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .join(format!("backup_tmp_{}.db", timestamp));

    let tmp_str = tmp_path.to_string_lossy().to_string();
    sqlx::query("VACUUM INTO ?")
        .bind(&tmp_str)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("백업 생성 실패: {}", e)))?;

    let bytes = tokio::fs::read(&tmp_path).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("백업 파일 읽기 실패: {}", e))
    })?;

    tokio::fs::remove_file(&tmp_path).await.ok();

    // 감사 로그 — 전교생 PII 전량 반출이므로 IP까지 함께 기록한다.
    // 응답 전송 직전에 커밋해 다운로드 실패(브라우저 중단 등)와 로그를 분리한다:
    // 파일 생성 자체는 이미 성공했으므로 다운로드 시도 사실을 남긴다.
    let mut conn = state.db.acquire().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    audit::log_with_ip(
        &mut conn,
        AuditEntry {
            actor: Actor::Admin,
            action: AuditAction::DbBackupDownloaded,
            round_id: None,
            student_id: None,
            detail: serde_json::json!({
                "filename": filename,
                "size_bytes": bytes.len(),
            }),
        },
        Some(client.ip().to_string()),
    )
    .await?;

    let response = Response::builder()
        .header("Content-Type", "application/octet-stream")
        .header("Content-Disposition", format!("attachment; filename=\"{}\"", filename))
        .body(Body::from(bytes))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(response)
}
