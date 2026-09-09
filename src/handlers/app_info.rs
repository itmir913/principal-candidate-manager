//! 프로그램 제목·부제 설정 (이슈 #23).
//!
//! 인원제한 O/X 전용 인스턴스를 한 대에서 두 개 돌리거나 학교 이름을 드러내야 할 때,
//! 화면과 트레이에서 두 인스턴스를 구분할 수 있어야 한다. 값은 `app_configs`에
//! 키-값으로 넣는다 — 이 테이블은 범용 key-value라 **스키마 변경이 아니다**
//! (`SCHEMA_VERSION`·`migrations/` 무변경, 출시 후 스키마 동결 규칙과 무관).

use axum::{extract::State, http::StatusCode, Extension, Json};
use serde::{Deserialize, Serialize};

use crate::{
    audit::{self, Actor, AuditEntry},
    auth,
    enums::AuditAction,
    state::AppState,
};

type ApiError = (StatusCode, String);

pub const TITLE_KEY: &str = "app_title";
pub const DESC_KEY: &str = "app_desc";

/// 설정된 값이 없을 때 쓰는 기본 문구 — 0.2.14까지 화면에 하드코딩돼 있던 그대로다.
pub const DEFAULT_TITLE: &str = "학교장 추천자";
pub const DEFAULT_DESC: &str = "선발 관리 시스템";

/// 제목·부제 길이 상한. 사이드바 한 줄과 트레이 툴팁(Windows 128자 제한)에 들어가야 한다.
pub const MAX_TITLE_LEN: usize = 40;
pub const MAX_DESC_LEN: usize = 60;

#[derive(Debug, Serialize, Deserialize)]
pub struct AppInfo {
    pub title: String,
    pub desc: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAppInfoBody {
    pub title: String,
    pub desc: String,
}

/// `app_configs`에서 제목·부제를 읽는다. 행이 없으면 기본값.
///
/// 트레이 툴팁도 이 함수를 쓴다 — 화면과 트레이가 다른 문구를 보이면 안 된다.
pub async fn read_app_info(conn: &mut sqlx::SqliteConnection) -> Result<AppInfo, sqlx::Error> {
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT key, value FROM app_configs WHERE key IN (?, ?)")
            .bind(TITLE_KEY)
            .bind(DESC_KEY)
            .fetch_all(&mut *conn)
            .await?;

    let pick = |k: &str| rows.iter().find(|(key, _)| key == k).map(|(_, v)| v.clone());

    Ok(AppInfo {
        title: pick(TITLE_KEY).unwrap_or_else(|| DEFAULT_TITLE.to_string()),
        desc: pick(DESC_KEY).unwrap_or_else(|| DEFAULT_DESC.to_string()),
    })
}

/// 공개 엔드포인트 — 로그인·시작·서버오류 화면은 인증 전에 제목을 그려야 한다.
pub async fn get_app_info(State(state): State<AppState>) -> Result<Json<AppInfo>, ApiError> {
    let mut conn = state
        .db
        .acquire()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    read_app_info(&mut conn)
        .await
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

/// 관리자 전용 — 제목·부제 저장.
///
/// 제목은 화면의 유일한 식별자라 빈 값을 허용하지 않는다(name 계열 trim+empty 거부 규칙).
/// 부제는 비워 둘 수 있다 — 학교명만 쓰고 설명은 안 쓰는 경우가 있다.
pub async fn update_app_info(
    State(state): State<AppState>,
    Extension(_claims): Extension<auth::AdminClaims>,
    Json(body): Json<UpdateAppInfoBody>,
) -> Result<Json<AppInfo>, ApiError> {
    let title = body.title.trim().to_string();
    let desc = body.desc.trim().to_string();

    if title.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "제목을 입력하세요".into()));
    }
    if title.chars().count() > MAX_TITLE_LEN {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("제목은 {}자 이하여야 합니다", MAX_TITLE_LEN),
        ));
    }
    if desc.chars().count() > MAX_DESC_LEN {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("설명은 {}자 이하여야 합니다", MAX_DESC_LEN),
        ));
    }

    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    for (key, value) in [(TITLE_KEY, &title), (DESC_KEY, &desc)] {
        sqlx::query("INSERT OR REPLACE INTO app_configs (key, value) VALUES (?, ?)")
            .bind(key)
            .bind(value)
            .execute(&mut *tx)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    audit::log(
        &mut tx,
        AuditEntry {
            actor: Actor::Admin,
            action: AuditAction::AppInfoUpdated,
            round_id: None,
            student_id: None,
            detail: serde_json::json!({ "title": title, "desc": desc }),
        },
    )
    .await?;

    tx.commit()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(AppInfo { title, desc }))
}
