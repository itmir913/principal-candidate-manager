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
    // 커넥션 하나를 점유해 BEGIN IMMEDIATE — 다른 writers 차단
    let mut conn = state
        .db
        .acquire()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *conn)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("트랜잭션 시작 실패: {}", e)))?;

    let bytes = tokio::fs::read("data.db").await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("DB 파일 읽기 실패: {}", e),
        )
    })?;

    // 변경 없이 잠금만 해제
    let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;

    let filename = format!(
        "data_backup_{}.db",
        chrono::Local::now().format("%Y%m%d_%H%M%S")
    );

    let response = Response::builder()
        .header("Content-Type", "application/octet-stream")
        .header(
            "Content-Disposition",
            format!("attachment; filename=\"{}\"", filename),
        )
        .body(Body::from(bytes))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(response)
}
