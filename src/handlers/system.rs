use axum::{
    body::Body,
    extract::{ConnectInfo, State},
    http::StatusCode,
    response::Response,
    Extension, Json,
};
use serde::Serialize;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};

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

/// 백업 임시 파일 이름의 충돌 방지용 시퀀스.
/// 파일명이 초 단위 타임스탬프뿐이면 같은 초에 두 번 요청했을 때
/// `VACUUM INTO`가 "출력 파일이 이미 존재한다"로 실패한다.
static BACKUP_SEQ: AtomicU64 = AtomicU64::new(0);

pub async fn download_db_backup(
    State(state): State<AppState>,
    Extension(_claims): Extension<auth::AdminClaims>,
    ConnectInfo(client): ConnectInfo<SocketAddr>,
) -> Result<Response<Body>, ApiError> {
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let filename = format!("data_backup_{}.db", timestamp);

    // 임시 파일은 DB와 같은 폴더에 만든다. 상위 폴더를 못 구할 때 CWD(".")로
    // 폴백하지 않고 즉시 실패한다 — 자동시작 시 CWD는 System32라서 전교생 PII가
    // 담긴 파일이 조용히 엉뚱한 위치에 생긴다 (main.rs data_dir 폴백 금지와 같은 취지).
    let parent = state.db_path.parent().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!(
                "데이터베이스 경로의 상위 폴더를 확인할 수 없습니다: {}",
                state.db_path.display()
            ),
        )
    })?;

    let tmp_path = parent.join(format!(
        "backup_tmp_{}_{}_{}.db",
        timestamp,
        std::process::id(),
        BACKUP_SEQ.fetch_add(1, Ordering::Relaxed)
    ));

    // to_string_lossy는 비-UTF-8 경로를 치환 문자로 바꿔 VACUUM INTO가 엉뚱한
    // 경로에 파일을 쓰게 만든다. 손실 변환 대신 즉시 실패한다.
    let tmp_str = tmp_path.to_str().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("백업 임시 파일 경로가 UTF-8이 아닙니다: {}", tmp_path.display()),
        )
    })?;

    // VACUUM INTO: 중첩 트랜잭션 없이 일관된 스냅샷 복사본 생성.
    // 연결을 통해 읽으므로 WAL(data.db-wal)에 있는 커밋까지 포함되며, 결과
    // 파일은 -wal 없이 그 자체로 완결된다 — 파일 복사 방식 백업과 다른 점이다.
    sqlx::query("VACUUM INTO ?")
        .bind(tmp_str)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("백업 생성 실패: {}", e)))?;

    // 읽기 성패와 무관하게 임시 파일을 지운다. 실패 경로에서 그냥 반환하면
    // 전교생 PII가 담긴 파일이 pcm 폴더에 남는다.
    let read_result = tokio::fs::read(&tmp_path).await;
    if let Err(e) = tokio::fs::remove_file(&tmp_path).await {
        tracing::warn!("백업 임시 파일 삭제 실패 ({}): {}", tmp_path.display(), e);
    }
    let bytes = read_result.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("백업 파일 읽기 실패: {}", e))
    })?;

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
