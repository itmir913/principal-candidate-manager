//! require_admin / require_teacher 미들웨어 가드 자체를 검증한다.
//! (다른 모든 핸들러 테스트는 이 미들웨어를 우회하고 핸들러를 직접 호출하므로,
//! 이 파일 이전에는 두 가드의 401/통과 분기가 실행으로 검증된 적이 없었다.)

mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware as axum_middleware,
    routing::get,
    Router,
};
use principal_candidate_manager::{
    auth::{encode_admin_token, encode_teacher_token},
    middleware::{require_admin, require_teacher},
};
use tower::ServiceExt;

fn admin_router(state: principal_candidate_manager::state::AppState) -> Router {
    Router::new()
        .route("/protected", get(|| async { StatusCode::OK }))
        .route_layer(axum_middleware::from_fn_with_state(state.clone(), require_admin))
        .with_state(state)
}

fn teacher_router(state: principal_candidate_manager::state::AppState) -> Router {
    Router::new()
        .route("/protected", get(|| async { StatusCode::OK }))
        .route_layer(axum_middleware::from_fn_with_state(state.clone(), require_teacher))
        .with_state(state)
}

fn req(auth_header: Option<&str>) -> Request<Body> {
    let mut b = Request::builder().method("GET").uri("/protected");
    if let Some(h) = auth_header {
        b = b.header("authorization", h);
    }
    b.body(Body::empty()).unwrap()
}

#[tokio::test]
async fn require_admin_rejects_missing_token() {
    let pool = common::create_test_pool().await;
    let state = common::make_state(pool);
    let res = admin_router(state).oneshot(req(None)).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn require_admin_rejects_invalid_token_and_allows_valid() {
    let pool = common::create_test_pool().await;
    let state = common::make_state(pool);

    // 잘못된(서명 불일치) 토큰 → 401
    let bad = admin_router(state.clone())
        .oneshot(req(Some("Bearer not-a-real-token")))
        .await
        .unwrap();
    assert_eq!(bad.status(), StatusCode::UNAUTHORIZED);

    // 올바른 관리자 토큰 → 통과(200)
    let token = encode_admin_token(&state.jwt_secret).unwrap();
    let ok = admin_router(state)
        .oneshot(req(Some(&format!("Bearer {token}"))))
        .await
        .unwrap();
    assert_eq!(ok.status(), StatusCode::OK);
}

#[tokio::test]
async fn require_admin_rejects_teacher_token() {
    // 담임 토큰으로 관리자 라우트 접근 시도 → decode_admin_token이 role 불일치로 거부해야 함
    let pool = common::create_test_pool().await;
    let state = common::make_state(pool);
    let teacher_token = encode_teacher_token(1, 1, &state.jwt_secret).unwrap();
    let res = admin_router(state)
        .oneshot(req(Some(&format!("Bearer {teacher_token}"))))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED, "담임 토큰으로 관리자 라우트를 통과하면 안 됨");
}

#[tokio::test]
async fn require_teacher_rejects_missing_token() {
    let pool = common::create_test_pool().await;
    let state = common::make_state(pool);
    let res = teacher_router(state).oneshot(req(None)).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn require_teacher_rejects_invalid_token_and_allows_valid() {
    let pool = common::create_test_pool().await;
    let state = common::make_state(pool);

    let bad = teacher_router(state.clone())
        .oneshot(req(Some("Bearer garbage")))
        .await
        .unwrap();
    assert_eq!(bad.status(), StatusCode::UNAUTHORIZED);

    let token = encode_teacher_token(2, 3, &state.jwt_secret).unwrap();
    let ok = teacher_router(state)
        .oneshot(req(Some(&format!("Bearer {token}"))))
        .await
        .unwrap();
    assert_eq!(ok.status(), StatusCode::OK);
}
