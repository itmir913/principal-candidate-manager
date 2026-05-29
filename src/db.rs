use anyhow::Result;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    SqlitePool,
};

pub const SCHEMA_VERSION: u32 = 1;

// 마이그레이션 배열: index i → v(i+1) 적용 SQL
// 릴리즈 전까지는 v1.sql에 직접 반영. 마이그레이션 파일 추가 금지.
const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/v1.sql"), // v0 → v1
];

pub async fn init_pool(db_path: &str) -> Result<SqlitePool> {
    let opts = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await?;

    run_migrations(&pool).await?;

    Ok(pool)
}

async fn run_migrations(pool: &SqlitePool) -> Result<()> {
    let current: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(pool)
        .await?;
    let current = current as u32;

    if current >= SCHEMA_VERSION {
        tracing::info!("db schema up to date (v{})", current);
        return Ok(());
    }

    for (i, sql) in MIGRATIONS.iter().enumerate() {
        let target = i as u32 + 1;
        if current < target {
            tracing::info!("applying migration v{}", target);
            // 스키마 변경 전체를 트랜잭션으로 묶어 오류 시 롤백 보장
            let mut tx = pool.begin().await?;
            sqlx::raw_sql(sql).execute(&mut *tx).await?;
            sqlx::query(&format!("PRAGMA user_version = {target}"))
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
        }
    }

    tracing::info!("db schema migrated to v{}", SCHEMA_VERSION);
    Ok(())
}
