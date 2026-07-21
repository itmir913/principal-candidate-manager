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

// ── v1 스키마 조각 ─────────────────────────────────────────────
// migrations/v1/ 의 파일들. 이 배열의 순서가 실행 순서다 — 파일명의 번호는
// 사람이 순서를 읽기 위한 표기일 뿐, 로더는 파일명을 정렬하지 않는다.
// 릴리즈 전까지는 이 조각 파일들에 직접 반영. 새 버전(v2) 추가 금지.
const V1_FRAGMENTS: &[&str] = &[
    include_str!("../migrations/v1/000-init.sql"),
    include_str!("../migrations/v1/001-classes.sql"),
    include_str!("../migrations/v1/002-students.sql"),
    include_str!("../migrations/v1/003-rounds.sql"),
    include_str!("../migrations/v1/004-areas.sql"),
    include_str!("../migrations/v1/005-universities.sql"),
    include_str!("../migrations/v1/006-score-tables.sql"),
    include_str!("../migrations/v1/007-base-data.sql"),
    include_str!("../migrations/v1/008-applications.sql"),
    include_str!("../migrations/v1/009-results.sql"),
    include_str!("../migrations/v1/010-audit-log.sql"),
    include_str!("../migrations/v1/011-round-confirmations.sql"),
];

// 버전별 마이그레이션: index i → v(i+1). 각 항목은 해당 버전을 구성하는 조각 목록.
const MIGRATION_FRAGMENTS: &[&[&str]] = &[V1_FRAGMENTS];

// SCHEMA_VERSION과 MIGRATION_FRAGMENTS 길이가 일치하지 않으면 컴파일 타임에 오류
const _: () = assert!(
    SCHEMA_VERSION as usize == MIGRATION_FRAGMENTS.len(),
    "SCHEMA_VERSION must equal MIGRATION_FRAGMENTS.len()"
);

/// 버전별 마이그레이션 SQL 전문 — 조각들을 결합해 run_migrations_with 입력 형태로 만든다.
pub fn migration_sqls() -> Vec<String> {
    MIGRATION_FRAGMENTS
        .iter()
        .map(|frags| frags.join("\n"))
        .collect()
}

/// 전체 스키마 SQL 전문 (새 DB 기준). 테스트 헬퍼가 in-memory DB를 만들 때 사용한다.
pub fn full_schema_sql() -> String {
    migration_sqls().join("\n")
}

pub async fn init_pool(db_path: &str) -> Result<SqlitePool> {
    let opts = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true)
        // sqlx 기본값 명시 (버전 업그레이드 시 조용한 변경 방지)
        .busy_timeout(std::time::Duration::from_secs(5));

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await?;

    let sqls = migration_sqls();
    let refs: Vec<&str> = sqls.iter().map(String::as_str).collect();
    run_migrations_with(&pool, &refs).await?;

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
