mod auth;
mod db;
mod handlers;
mod middleware;
mod state;

use axum::{middleware as axum_middleware, routing::get, routing::post, routing::put, Router};
use rand::RngCore;
use state::AppState;
use tower_http::cors::CorsLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// 릴리스 빌드: frontend/dist/ 를 바이너리에 내장
#[cfg(not(feature = "dev"))]
mod frontend {
    use rust_embed::Embed;

    #[derive(Embed)]
    #[folder = "frontend/dist/"]
    pub struct Assets;
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let db = db::init_pool("data.db").await?;
    tracing::info!("database ready");

    let mut secret_bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut secret_bytes);
    let jwt_secret: String = secret_bytes.iter().map(|b| format!("{:02x}", b)).collect();

    let state = AppState { db, jwt_secret };

    let app = build_router(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    tracing::info!("listening on http://0.0.0.0:8080");
    axum::serve(listener, app).await?;

    Ok(())
}

fn build_router(state: AppState) -> Router {
    let protected_auth = Router::new()
        .route("/admin/password", put(handlers::auth::change_admin_password))
        .route_layer(axum_middleware::from_fn_with_state(
            state.clone(),
            middleware::require_admin,
        ));

    let auth_routes = Router::new()
        .route("/admin", post(handlers::auth::admin_login))
        .route("/teacher", post(handlers::auth::teacher_login))
        .merge(protected_auth);

    let api = Router::new()
        .route("/health", get(health))
        .nest("/auth", auth_routes);

    let app = Router::new()
        .nest("/api", api)
        .layer(CorsLayer::permissive())
        .with_state(state);

    // 개발 모드: Vite dev server(5173)가 정적파일 담당, API만 제공
    // 릴리스 모드: 내장 dist/ 를 SPA fallback으로 제공
    #[cfg(not(feature = "dev"))]
    let app = app.fallback(static_handler);

    app
}

async fn health() -> &'static str {
    "ok"
}

#[cfg(not(feature = "dev"))]
async fn static_handler(uri: axum::http::Uri) -> axum::response::Response {
    use axum::{
        body::Body,
        http::{header, StatusCode},
        response::Response,
    };

    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    match frontend::Assets::get(path) {
        Some(file) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            Response::builder()
                .header(header::CONTENT_TYPE, mime.as_ref())
                .body(Body::from(file.data.into_owned()))
                .unwrap()
        }
        // SPA fallback: 클라이언트 라우트는 index.html 반환
        None => match frontend::Assets::get("index.html") {
            Some(file) => Response::builder()
                .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
                .body(Body::from(file.data.into_owned()))
                .unwrap(),
            None => Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::empty())
                .unwrap(),
        },
    }
}
