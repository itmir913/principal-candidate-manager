use anyhow::Result;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    SqlitePool,
};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug)]
pub struct SchemaTooNewError {
    pub db_ver: u32,
    pub app_ver: u32,
}

impl std::fmt::Display for SchemaTooNewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "데이터베이스 스키마 버전(v{})이 이 앱이 지원하는 최대 버전(v{})보다 높습니다. \
             최신 버전의 앱을 다운로드하여 사용해 주세요.",
            self.db_ver, self.app_ver
        )
    }
}

impl std::error::Error for SchemaTooNewError {}

// 마이그레이션 배열: index i → v(i+1) 적용 SQL
// 릴리즈 전까지는 v1.sql에 직접 반영. 마이그레이션 파일 추가 금지.
const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/v1.sql"), // v0 → v1
];

// SCHEMA_VERSION과 MIGRATIONS 길이가 일치하지 않으면 컴파일 타임에 오류
const _: () = assert!(
    SCHEMA_VERSION as usize == MIGRATIONS.len(),
    "SCHEMA_VERSION must equal MIGRATIONS.len()"
);

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

    run_migrations_with(&pool, MIGRATIONS).await?;

    Ok(pool)
}

/// 마이그레이션 로직의 테스트 가능한 핵심부.
/// `migrations[i]`는 v(i) → v(i+1) 을 수행하는 SQL이다.
/// schema_version = migrations.len() 으로 간주한다.
pub async fn run_migrations_with(pool: &SqlitePool, migrations: &[&str]) -> Result<()> {
    let schema_version = migrations.len() as u32;

    let current: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(pool)
        .await?;
    let current = current as u32;

    // 상위 버전 DB 감지 — 마이그레이션 시도 전에 반드시 먼저 검사
    if current > schema_version {
        return Err(anyhow::Error::new(SchemaTooNewError {
            db_ver: current,
            app_ver: schema_version,
        }));
    }
    if current == schema_version {
        tracing::info!("db schema up to date (v{})", current);
        return Ok(());
    }

    for (i, sql) in migrations.iter().enumerate() {
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

    tracing::info!("db schema migrated to v{}", schema_version);
    Ok(())
}
