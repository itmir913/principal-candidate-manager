mod db;
mod state;

use axum::{routing::get, Router};
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

    let state = AppState { db };

    let app = build_router(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    tracing::info!("listening on http://0.0.0.0:8080");
    axum::serve(listener, app).await?;

    Ok(())
}

fn build_router(state: AppState) -> Router {
    let api = Router::new().route("/health", get(health));

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
