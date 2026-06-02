use axum::{
    body::Body,
    extract::State,
    http::StatusCode,
    response::Response,
    Extension, Json,
};
use serde::Serialize;

use crate::{auth, state::AppState};

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

    let response = Response::builder()
        .header("Content-Type", "application/octet-stream")
        .header("Content-Disposition", format!("attachment; filename=\"{}\"", filename))
        .body(Body::from(bytes))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(response)
}
