use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub jwt_secret: String,
    pub db_path: std::path::PathBuf,
    pub server_addr: String,
}
