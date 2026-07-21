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
    paths::{CONFIG_FILENAME, DATA_DIR_NAME, DB_FILENAME},
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
    let filename = format!("pcm_backup_{}.zip", timestamp);

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
    let db_bytes = read_result.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("백업 파일 읽기 실패: {}", e))
    })?;

    // config.json은 포트 설정뿐이라 데이터 복원에 필수는 아니다. 그래서 "없음"은
    // 정상으로 보고 넘어가되, **있는데 읽지 못하는** 상황은 폴더 상태가 이상하다는
    // 신호이므로 조용히 빼먹지 않고 실패시킨다.
    let config_path = parent.join(CONFIG_FILENAME);
    let config_bytes = match tokio::fs::read(&config_path).await {
        Ok(b) => Some(b),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("{} 읽기 실패: {}", CONFIG_FILENAME, e),
            ))
        }
    };

    // 복원 절차를 "폴더 통째 교체" 하나로 통일하기 위해 pcm 폴더 모양의 zip으로
    // 감싼다. 파일을 골라 넣지 않으므로 사용자가 data.db-wal 같은 파일을
    // 알아볼 필요가 없다.
    //
    // logs\는 넣지 않는다 — 진단용이지 복원 대상이 아니고, 복원 시 옛 로그가
    // 되살아나면 사고 시점 판단을 흐린다.
    //
    // 압축은 CPU 작업이라 blocking 스레드로 넘긴다. 인라인으로 돌리면 백업하는
    // 동안 담임들의 요청까지 함께 멈춘다.
    let zip_bytes = tokio::task::spawn_blocking(move || build_backup_zip(&db_bytes, config_bytes.as_deref()))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("백업 압축 작업 실패: {}", e)))?
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("백업 압축 실패: {}", e)))?;

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
                "size_bytes": zip_bytes.len(),
            }),
        },
        Some(client.ip().to_string()),
    )
    .await?;

    let response = Response::builder()
        .header("Content-Type", "application/zip")
        .header("Content-Disposition", format!("attachment; filename=\"{}\"", filename))
        .body(Body::from(zip_bytes))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(response)
}

/// 백업 산출물을 `pcm/` 폴더 모양의 zip으로 만든다.
///
/// 압축을 푼 모습이 실제 데이터 폴더와 같아야 복원이 "폴더 통째 교체" 한 가지
/// 절차로 끝난다. 파일명을 바꿔 넣게 하면(예: `data_backup_*.db` → `data.db`)
/// 확장자 숨김 상태의 탐색기에서 `data.db.db`가 되는 실수가 나온다.
fn build_backup_zip(db_bytes: &[u8], config_bytes: Option<&[u8]>) -> Result<Vec<u8>, String> {
    use std::io::{Cursor, Write};
    use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

    let mut buf = Vec::new();
    {
        let mut zip = ZipWriter::new(Cursor::new(&mut buf));
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

        zip.start_file(format!("{}/{}", DATA_DIR_NAME, DB_FILENAME), opts)
            .map_err(|e| e.to_string())?;
        zip.write_all(db_bytes).map_err(|e| e.to_string())?;

        if let Some(cfg) = config_bytes {
            zip.start_file(format!("{}/{}", DATA_DIR_NAME, CONFIG_FILENAME), opts)
                .map_err(|e| e.to_string())?;
            zip.write_all(cfg).map_err(|e| e.to_string())?;
        }

        zip.finish().map_err(|e| e.to_string())?;
    }
    Ok(buf)
}
