use principal_candidate_manager::{db, handlers, middleware};
use principal_candidate_manager::state::AppState;

use axum::{
    middleware as axum_middleware,
    routing::{delete, get, post, put},
    Router,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
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
        .route("/db-backup", get(handlers::system::download_db_backup))
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
        // classes GET은 로그인 폼(반 목록 조회)에서도 필요하므로 공개 라우트에 별도 등록
        .route("/students", get(handlers::students::list_students))
        .route("/students/grade-options", get(handlers::students::grade_options))
        .route("/students/template", get(handlers::students::download_template))
        .route("/students/export", get(handlers::students::export_students))
        .route("/students/import", post(handlers::students::import_students))
        .route("/students/enrolled/template", get(handlers::students::enrolled_template))
        .route("/students/enrolled/export", get(handlers::students::export_enrolled))
        .route("/students/enrolled/import", post(handlers::students::import_enrolled))
        .route("/students/graduated/template", get(handlers::students::graduated_template))
        .route("/students/graduated/export", get(handlers::students::export_graduated))
        .route("/students/graduated/import", post(handlers::students::import_graduated))
        .route("/students/:id", delete(handlers::students::delete_student))
        .route("/classes/template", get(handlers::classes::classes_template))
        .route("/classes/export", get(handlers::classes::export_classes))
        .route("/classes/import", post(handlers::classes::import_classes))
        .route(
            "/classes/:grade/:class_no",
            put(handlers::classes::upsert_class)
                .delete(handlers::classes::delete_class),
        )
        .route("/areas", get(handlers::areas::list_areas))
        .route("/areas", post(handlers::areas::create_area))
        .route("/areas/:id", put(handlers::areas::update_area))
        .route("/areas/:id", delete(handlers::areas::delete_area))
        .route("/areas/:id/numeric-table/list",     get(handlers::area_data::numeric_table_list))
        .route("/areas/:id/numeric-table/template", get(handlers::area_data::numeric_table_template))
        .route("/areas/:id/numeric-table/export",   get(handlers::area_data::numeric_table_export))
        .route("/areas/:id/numeric-table/import",  post(handlers::area_data::numeric_table_import))
        .route("/areas/:id/category-map/list",     get(handlers::area_data::category_map_list))
        .route("/areas/:id/category-map/template", get(handlers::area_data::category_map_template))
        .route("/areas/:id/category-map/export",   get(handlers::area_data::category_map_export))
        .route("/areas/:id/category-map/import",  post(handlers::area_data::category_map_import))
        .route("/areas/:id/base-data/list",     get(handlers::area_data::base_data_list))
        .route("/areas/:id/base-data/template", get(handlers::area_data::base_data_template))
        .route("/areas/:id/base-data/export",   get(handlers::area_data::base_data_export))
        .route("/areas/:id/base-data/import",  post(handlers::area_data::base_data_import))
        .route("/areas/:id/base-data/external/daegyo/preview", post(handlers::external_import::daegyo_preview))
        .route("/areas/:id/base-data/external/daegyo/import",  post(handlers::external_import::daegyo_import))
        .route("/areas/:id/base-data/external/univ/preview",   post(handlers::external_import::univ_preview))
        .route("/areas/:id/base-data/external/univ/import",    post(handlers::external_import::univ_import))
        .route("/universities", get(handlers::universities::list_universities))
        .route("/universities", post(handlers::universities::create_university))
        .route("/universities/quota-stats", get(handlers::universities::get_quota_stats))
        .route("/universities/quota-stats/export", get(handlers::universities::export_quota_stats))
        .route("/universities/:id", put(handlers::universities::update_university))
        .route("/universities/:id", delete(handlers::universities::delete_university))
        .route("/universities/:id/tracks", get(handlers::universities::list_tracks))
        .route("/universities/:id/tracks", post(handlers::universities::create_track))
        .route("/univ-tracks", get(handlers::universities::list_all_tracks))
        .route("/univ-tracks/:id", put(handlers::universities::update_track))
        .route("/univ-tracks/:id", delete(handlers::universities::delete_track))
        .route("/univ-tracks/:id/recommended-list", get(handlers::universities::get_track_recommended_list))
        // 5단계: 라운드 관리
        .route("/rounds", get(handlers::rounds::list_rounds))
        .route("/rounds/open", post(handlers::rounds::open_round))
        .route("/rounds/:id/close", put(handlers::rounds::close_round))
        .route("/rounds/:id/reopen", put(handlers::rounds::reopen_round))
        .route("/rounds/:id/finalize", put(handlers::rounds::finalize_round))
        .route("/rounds/:id/calculate", post(handlers::scoring::calculate_scores))
        .route("/rounds/:id/results", get(handlers::scoring::get_results))
        .route("/rounds/:id/results/export", get(handlers::scoring::export_results))
        .route("/rounds/:id/summary/export", get(handlers::scoring::export_round_summary))
        // 7단계: 점수 미리보기
        .route("/score-preview", get(handlers::scoring::score_preview))
        // 5단계: 지원·추천
        .route("/applications", get(handlers::applications::admin_list_applications))
        .route(
            "/applications/:sid/:tid/:rid/abandon",
            put(handlers::applications::abandon_application),
        )
        .route(
            "/results/:sid/:tid/:rid/recommend",
            put(handlers::scoring::recommend_result),
        )
        .route(
            "/results/:sid/:tid/:rid/unrecommend",
            put(handlers::scoring::unrecommend_result),
        )
        .route_layer(axum_middleware::from_fn_with_state(
            state.clone(),
            middleware::require_admin,
        ));

    let teacher_routes = Router::new()
        .route("/students", get(handlers::applications::teacher_list_students))
        .route("/universities", get(handlers::universities::list_universities))
        .route("/universities/:id/tracks", get(handlers::universities::list_tracks))
        .route("/univ-tracks", get(handlers::universities::list_all_tracks))
        .route("/applications", get(handlers::applications::teacher_list_applications))
        .route("/applications", post(handlers::applications::teacher_create_application))
        .route(
            "/applications/:sid/:tid/:rid",
            delete(handlers::applications::teacher_delete_application),
        )
        .route(
            "/applications/:sid/:tid/:rid/abandon",
            put(handlers::applications::teacher_abandon_application),
        )
        .route("/password", put(handlers::applications::teacher_change_password))
        .route("/area-context", get(handlers::teacher_areas::teacher_area_context))
        .route("/area-score-preview", post(handlers::teacher_areas::teacher_area_score_preview))
        // 7단계: 담임 결과 조회
        .route("/results", get(handlers::scoring::teacher_get_results))
        .route_layer(axum_middleware::from_fn_with_state(
            state.clone(),
            middleware::require_teacher,
        ));

    let api = Router::new()
        .route("/health", get(health))
        .route("/version", get(handlers::system::get_version))
        .route("/rounds/current", get(handlers::rounds::get_current_round))
        // 로그인 폼에서 반 목록 조회 (인증 불필요)
        .route("/classes", get(handlers::classes::list_classes))
        .nest("/auth", auth_routes)
        .merge(admin_routes)
        .nest("/teacher", teacher_routes);

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
