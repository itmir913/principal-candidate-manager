use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::{auth, state::AppState};

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
