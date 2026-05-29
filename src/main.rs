mod auth;
mod db;
mod handlers;
mod middleware;
mod state;

use axum::{
    middleware as axum_middleware,
    routing::{delete, get, post, put},
    Router,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
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

const DEFAULT_PORT: u16 = 8080;
const CONFIG_FILENAME: &str = "config.json";

#[derive(Serialize, Deserialize)]
struct Config {
    port: u16,
}

impl Default for Config {
    fn default() -> Self {
        Config { port: DEFAULT_PORT }
    }
}

/// exe 위치 기준으로 config.json을 읽는다.
/// 파일이 없으면 기본값으로 생성한다.
/// 파싱에 실패하면 경고 로그 후 기본값을 반환한다.
fn load_config() -> Config {
    let config_path = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join(CONFIG_FILENAME)))
        .unwrap_or_else(|| std::path::PathBuf::from(CONFIG_FILENAME));

    match std::fs::read_to_string(&config_path) {
        Ok(contents) => match serde_json::from_str::<Config>(&contents) {
            Ok(cfg) => {
                tracing::info!("config loaded: port={}", cfg.port);
                cfg
            }
            Err(e) => {
                tracing::warn!(
                    "config.json 파싱 실패 ({}), 기본값 사용: port={}",
                    e,
                    DEFAULT_PORT
                );
                Config::default()
            }
        },
        Err(_) => {
            // 파일 없음 → 기본값으로 생성 시도
            let default_cfg = Config::default();
            if let Ok(json) = serde_json::to_string_pretty(&default_cfg) {
                if let Err(e) = std::fs::write(&config_path, json) {
                    tracing::warn!("config.json 생성 실패: {}", e);
                } else {
                    tracing::info!("config.json 생성됨: {:?}", config_path);
                }
            }
            default_cfg
        }
    }
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

    let config = load_config();

    let db = db::init_pool("data.db").await?;
    tracing::info!("database ready");

    let mut secret_bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut secret_bytes);
    let jwt_secret: String = secret_bytes.iter().map(|b| format!("{:02x}", b)).collect();

    let state = AppState { db, jwt_secret };

    let app = build_router(state);

    let addr = format!("0.0.0.0:{}", config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("listening on http://{}", addr);
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
        .route("/admin/status", get(handlers::auth::admin_status))
        .route("/admin", post(handlers::auth::admin_login))
        .route("/teacher", post(handlers::auth::teacher_login))
        .merge(protected_auth);

    let admin_routes = Router::new()
        .route("/students", get(handlers::students::list_students))
        .route("/students/template", get(handlers::students::download_template))
        .route("/students/export", get(handlers::students::export_students))
        .route("/students/import", post(handlers::students::import_students))
        .route("/classes", get(handlers::classes::list_classes))
        .route(
            "/classes/:grade/:class_no",
            put(handlers::classes::upsert_class),
        )
        .route("/areas", get(handlers::areas::list_areas))
        .route("/areas", post(handlers::areas::create_area))
        .route("/areas/:id", put(handlers::areas::update_area))
        .route("/areas/:id", delete(handlers::areas::delete_area))
        .route("/areas/:id/range-table", get(handlers::areas::get_range_table))
        .route("/areas/:id/range-table", put(handlers::areas::put_range_table))
        .route("/areas/:id/category-map", get(handlers::areas::get_category_map))
        .route("/areas/:id/category-map", put(handlers::areas::put_category_map))
        .route("/universities", get(handlers::universities::list_universities))
        .route("/universities", post(handlers::universities::create_university))
        .route("/universities/:id", put(handlers::universities::update_university))
        .route("/universities/:id", delete(handlers::universities::delete_university))
        .route_layer(axum_middleware::from_fn_with_state(
            state.clone(),
            middleware::require_admin,
        ));

    let api = Router::new()
        .route("/health", get(health))
        .nest("/auth", auth_routes)
        .merge(admin_routes);

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
