use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub jwt_secret: String,
}
