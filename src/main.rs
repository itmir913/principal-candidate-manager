#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

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

// Win32 메시지 펌프 (릴리스 빌드 전용)
#[cfg(not(feature = "dev"))]
mod win32 {
    #[repr(C)]
    pub struct MSG {
        pub hwnd: isize,
        pub message: u32,
        pub w_param: usize,
        pub l_param: isize,
        pub time: u32,
        pub pt_x: i32,
        pub pt_y: i32,
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        pub fn GetMessageW(
            lpmsg: *mut MSG,
            hwnd: isize,
            wmsgfiltermin: u32,
            wmsgfiltermax: u32,
        ) -> i32;
        pub fn TranslateMessage(lpmsg: *const MSG) -> i32;
        pub fn DispatchMessageW(lpmsg: *const MSG) -> isize;
    }
}

const DEFAULT_PORT: u16 = 8080;

/// 현재 시스템에서 LAN 라우팅에 사용될 IPv4 주소를 감지한다.
/// UdpSocket을 이용해 실제 패킷 없이 로컬 바인딩 주소를 얻는 방식으로,
/// loopback·VPN 어댑터보다 실제 LAN 인터페이스가 우선 선택된다.
fn detect_lan_ip() -> String {
    use std::net::UdpSocket;
    if let Ok(sock) = UdpSocket::bind("0.0.0.0:0") {
        if sock.connect("8.8.8.8:80").is_ok() {
            if let Ok(addr) = sock.local_addr() {
                return addr.ip().to_string();
            }
        }
    }
    "127.0.0.1".to_string()
}
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

/// exe 위치 기준으로 파일 경로를 반환한다. 경로 취득 실패 시 파일명만 반환.
fn exe_relative(filename: &str) -> std::path::PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join(filename)))
        .unwrap_or_else(|| std::path::PathBuf::from(filename))
}

/// exe 옆 pcm/ 서브폴더를 데이터 디렉토리로 사용한다. 없으면 자동 생성.
fn data_dir() -> std::path::PathBuf {
    let dir = exe_relative("pcm");
    std::fs::create_dir_all(&dir).ok();
    dir
}

/// exe 위치 기준으로 config.json을 읽는다.
/// 파일이 없으면 기본값으로 생성한다.
/// 파싱에 실패하면 경고 로그 후 기본값을 반환한다.
fn load_config() -> Config {
    let config_path = data_dir().join(CONFIG_FILENAME);

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

// ──────────────────────────────────────────────────────────────────────────────
// 자동 실행 설정 (app_configs 테이블 + Windows 레지스트리)
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(not(feature = "dev"))]
const AUTOSTART_KEY: &str = "autostart_enabled";
#[cfg(all(target_os = "windows", not(feature = "dev")))]
const AUTOSTART_REG_NAME: &str = "PCM";
#[cfg(all(target_os = "windows", not(feature = "dev")))]
const AUTOSTART_REG_PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

#[cfg(not(feature = "dev"))]
async fn get_autostart(db: &sqlx::SqlitePool) -> bool {
    sqlx::query_scalar::<_, String>(
        "SELECT value FROM app_configs WHERE key = ?",
    )
    .bind(AUTOSTART_KEY)
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .map(|v| v == "1")
    .unwrap_or(true) // 기본값: 활성화
}

#[cfg(not(feature = "dev"))]
async fn save_autostart(db: &sqlx::SqlitePool, enabled: bool) {
    let value = if enabled { "1" } else { "0" };
    let _ = sqlx::query(
        "INSERT OR REPLACE INTO app_configs (key, value) VALUES (?, ?)",
    )
    .bind(AUTOSTART_KEY)
    .bind(value)
    .execute(db)
    .await;
}

#[cfg(all(target_os = "windows", not(feature = "dev")))]
fn autostart_registry_set(exe_path: &str) {
    use winreg::{enums::*, RegKey};
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(key) = hkcu.open_subkey_with_flags(AUTOSTART_REG_PATH, KEY_SET_VALUE) {
        let _ = key.set_value(AUTOSTART_REG_NAME, &exe_path);
    }
}

#[cfg(all(target_os = "windows", not(feature = "dev")))]
fn autostart_registry_remove() {
    use winreg::{enums::*, RegKey};
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(key) = hkcu.open_subkey_with_flags(AUTOSTART_REG_PATH, KEY_SET_VALUE) {
        let _ = key.delete_value(AUTOSTART_REG_NAME);
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// dev 빌드: 기존 방식 (tokio::main + 콘솔)
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "dev")]
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
    let db_path = data_dir().join("data.db");

    let db = db::init_pool(db_path.to_str().unwrap_or("data.db")).await?;
    tracing::info!("database ready");

    let mut secret_bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut secret_bytes);
    let jwt_secret: String = secret_bytes.iter().map(|b| format!("{:02x}", b)).collect();

    let server_addr = format!("{}:{}", detect_lan_ip(), config.port);
    let state = AppState { db, jwt_secret, db_path: db_path.clone(), server_addr };

    let app = build_router(state);

    let addr = format!("0.0.0.0:{}", config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("listening on http://{}", addr);
    axum::serve(listener, app).await?;

    Ok(())
}

// ──────────────────────────────────────────────────────────────────────────────
// 릴리스 빌드: 트레이 아이콘 + 브라우저 자동 열기
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(not(feature = "dev"))]
fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = load_config();
    let port = config.port;
    let db_path = data_dir().join("data.db");

    // tokio 런타임을 수동 생성 (메인 스레드를 tokio에 넘기지 않기 위해)
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio 런타임 생성 실패");

    let db = match rt.block_on(db::init_pool(db_path.to_str().unwrap_or("data.db"))) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("데이터베이스 초기화 오류: {}", e);
            std::process::exit(1);
        }
    };
    tracing::info!("database ready");

    let db_for_tray = db.clone();
    let rt_handle = rt.handle().clone();

    let mut secret_bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut secret_bytes);
    let jwt_secret: String = secret_bytes.iter().map(|b| format!("{:02x}", b)).collect();
    let server_addr = format!("{}:{}", detect_lan_ip(), port);
    let state = AppState { db, jwt_secret, db_path: db_path.clone(), server_addr };

    let app = build_router(state);
    let addr = format!("0.0.0.0:{}", port);

    // 서버가 바인딩되면 Ok(()), 실패하면 Err(메시지) 전송
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(), String>>();

    std::thread::spawn(move || {
        rt.block_on(async move {
            match tokio::net::TcpListener::bind(&addr).await {
                Ok(listener) => {
                    tracing::info!("listening on http://{}", addr);
                    let _ = ready_tx.send(Ok(()));
                    if let Err(e) = axum::serve(listener, app).await {
                        eprintln!("서버 오류: {}", e);
                    }
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(e.to_string()));
                }
            }
        });
    });

    match ready_rx.recv() {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            eprintln!("서버 시작 실패 (포트 {}): {}", port, e);
            std::process::exit(1);
        }
        Err(_) => {
            eprintln!("서버 스레드 비정상 종료");
            std::process::exit(1);
        }
    }

    let url = format!("http://localhost:{}", port);
    let _ = webbrowser::open(&url);

    // 자동 실행 초기 상태 읽기 및 레지스트리 반영
    let autostart_on = rt_handle.block_on(get_autostart(&db_for_tray));
    let exe_path = std::env::current_exe()
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    if autostart_on {
        autostart_registry_set(&exe_path);
    } else {
        autostart_registry_remove();
    }

    // 트레이 아이콘 설정
    use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{Icon, TrayIconBuilder};

    let icon = {
        const ICON_BYTES: &[u8] = include_bytes!("../assets/icon.ico");
        let img = image::load_from_memory(ICON_BYTES)
            .expect("assets/icon.ico 로드 실패")
            .into_rgba8();
        let (w, h) = img.dimensions();
        Icon::from_rgba(img.into_raw(), w, h).expect("트레이 아이콘 생성 실패")
    };

    let menu = Menu::new();
    let autostart_item = CheckMenuItem::new("시작 시 자동 실행", true, autostart_on, None);
    let open_item = MenuItem::new("열기", true, None);
    let quit_item = MenuItem::new("종료", true, None);
    menu.append(&autostart_item).expect("메뉴 항목 추가 실패");
    menu.append(&PredefinedMenuItem::separator()).expect("메뉴 항목 추가 실패");
    menu.append(&open_item).expect("메뉴 항목 추가 실패");
    menu.append(&quit_item).expect("메뉴 항목 추가 실패");
    let autostart_id = autostart_item.id().clone();
    let open_id = open_item.id().clone();
    let quit_id = quit_item.id().clone();

    let _tray = TrayIconBuilder::new()
        .with_icon(icon)
        .with_menu(Box::new(menu))
        .with_tooltip("학교장추천 관리 시스템")
        .build()
        .expect("트레이 생성 실패");

    let menu_channel = MenuEvent::receiver();

    // Win32 메시지 루프 (메인 스레드에서 실행 필수)
    unsafe {
        let mut msg: win32::MSG = std::mem::zeroed();
        loop {
            let ret = win32::GetMessageW(&mut msg, 0, 0, 0);
            if ret == 0 || ret == -1 {
                break;
            }
            win32::TranslateMessage(&msg);
            win32::DispatchMessageW(&msg);

            while let Ok(event) = menu_channel.try_recv() {
                if event.id == autostart_id {
                    // muda가 클릭 시 checked 상태를 자동 토글하므로
                    // is_checked()는 이미 새 상태를 반환함
                    let new_state = autostart_item.is_checked();
                    if new_state {
                        autostart_registry_set(&exe_path);
                    } else {
                        autostart_registry_remove();
                    }
                    rt_handle.block_on(save_autostart(&db_for_tray, new_state));
                } else if event.id == open_id {
                    let _ = webbrowser::open(&url);
                } else if event.id == quit_id {
                    std::process::exit(0);
                }
            }
        }
    }
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
        .route("/overview", get(handlers::overview::get_overview))
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
        .route("/areas/score-template/:name", get(handlers::areas::score_template))
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
        .route("/rounds", get(handlers::rounds::list_rounds))
        .route("/rounds/open", post(handlers::rounds::open_round))
        .route("/rounds/:id/close", put(handlers::rounds::close_round))
        .route("/rounds/:id/reopen", put(handlers::rounds::reopen_round))
        .route("/rounds/:id/finalize", put(handlers::rounds::finalize_round))
        .route("/rounds/:id/calculate", post(handlers::scoring::calculate_scores))
        .route("/rounds/:id/results", get(handlers::scoring::get_results))
        .route("/rounds/:id/results/export", get(handlers::scoring::export_results))
        .route("/rounds/:id/summary/export", get(handlers::scoring::export_round_summary))
        .route("/score-preview", get(handlers::scoring::score_preview))
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
        .route("/results", get(handlers::scoring::teacher_get_results))
        .route_layer(axum_middleware::from_fn_with_state(
            state.clone(),
            middleware::require_teacher,
        ));

    let api = Router::new()
        .route("/health", get(health))
        .route("/version", get(handlers::system::get_version))
        .route("/rounds/current", get(handlers::rounds::get_current_round))
        .route("/classes", get(handlers::classes::list_classes))
        .nest("/auth", auth_routes)
        .merge(admin_routes)
        .nest("/teacher", teacher_routes);

    let app = Router::new()
        .nest("/api", api)
        .layer(CorsLayer::permissive())
        .with_state(state);

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
