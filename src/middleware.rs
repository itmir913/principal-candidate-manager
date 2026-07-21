use axum::{
    extract::{multipart::MultipartError, DefaultBodyLimit, Request, State},
    http::{header, StatusCode},
    middleware::{self as axum_middleware, Next},
    response::{IntoResponse, Response},
    Router,
};

use crate::{auth, state::AppState};

pub const UPLOAD_LIMIT_BYTES: usize = 20 * 1024 * 1024;

fn bearer_token(req: &Request) -> Option<String> {
    req.headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(String::from)
}

pub async fn require_admin(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Response {
    let Some(token) = bearer_token(&req) else {
        return (StatusCode::UNAUTHORIZED, "토큰이 없습니다").into_response();
    };
    match auth::decode_admin_token(&token, &state.jwt_secret) {
        Ok(claims) => {
            req.extensions_mut().insert(claims);
            next.run(req).await
        }
        Err(_) => (StatusCode::UNAUTHORIZED, "유효하지 않은 토큰").into_response(),
    }
}

pub async fn require_teacher(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Response {
    let Some(token) = bearer_token(&req) else {
        return (StatusCode::UNAUTHORIZED, "토큰이 없습니다").into_response();
    };
    match auth::decode_teacher_token(&token, &state.jwt_secret) {
        Ok(claims) => {
            req.extensions_mut().insert(claims);
            next.run(req).await
        }
        Err(_) => (StatusCode::UNAUTHORIZED, "유효하지 않은 토큰").into_response(),
    }
}

/// axum의 `DefaultBodyLimit` 초과 시 기본 영문 응답 대신 한국어 안내로 교체한다.
pub async fn korean_body_limit_message(req: Request, next: Next) -> Response {
    let resp = next.run(req).await;
    if resp.status() == StatusCode::PAYLOAD_TOO_LARGE {
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            "업로드 파일 크기가 너무 큽니다 (최대 20MB)",
        )
            .into_response();
    }
    resp
}

/// multipart 파싱 오류를 (StatusCode, String) `ApiError`로 변환.
/// 업로드 상한(20MB) 초과는 413 + 한국어로, 그 외 파싱 오류는 400 + 원본 메시지로 처리한다.
pub fn multipart_err(e: MultipartError) -> (StatusCode, String) {
    if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
        (
            StatusCode::PAYLOAD_TOO_LARGE,
            "업로드 파일 크기가 너무 큽니다 (최대 20MB)".to_string(),
        )
    } else {
        (StatusCode::BAD_REQUEST, e.to_string())
    }
}

/// 업로드 상한(20MB) + 초과 시 한국어 413 응답을 라우터에 적용한다.
pub fn with_upload_guards<S>(router: Router<S>) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    router
        .layer(axum_middleware::from_fn(korean_body_limit_message))
        .layer(DefaultBodyLimit::max(UPLOAD_LIMIT_BYTES))
}
