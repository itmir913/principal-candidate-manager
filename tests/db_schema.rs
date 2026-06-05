/// DB 스키마 버전 감지 및 마이그레이션 로직 테스트
///
/// [init_pool 경로] 실제 파일 DB + MIGRATIONS 배열 기준
///   1. 새 DB → 마이그레이션 적용 후 SCHEMA_VERSION
///   2. 이미 최신 버전 → 재실행해도 OK (no-op)
///   3. 상위 버전 DB → SchemaTooNewError 반환
///   4. SCHEMA_VERSION+1 경계값 → 에러
///
/// [run_migrations_with 경로] 커스텀 마이그레이션 배열로 흐름 단위 검증
///   5. 여러 마이그레이션이 순서대로 적용됨
///   6. 이미 적용된 마이그레이션은 건너뜀
///   7. 부분 실패 후 재시작 시 마지막 커밋 버전부터 재개
///   8. 상위 버전 감지는 마이그레이션 시도 전에 일어남
use std::sync::atomic::{AtomicU64, Ordering};

use principal_candidate_manager::db::{self, run_migrations_with, SchemaTooNewError, SCHEMA_VERSION};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// 테스트마다 고유한 임시 DB 경로를 반환한다.
fn temp_db_path() -> std::path::PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    std::env::temp_dir().join(format!("pcm_schema_test_{}_{}.db", pid, n))
}

/// DB 파일을 직접 만들고 user_version만 설정하는 헬퍼.
/// init_pool의 마이그레이션을 거치지 않는다.
async fn create_raw_db_with_version(path: &std::path::Path, version: u32) {
    let opts = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .unwrap();
    sqlx::query(&format!("PRAGMA user_version = {version}"))
        .execute(&pool)
        .await
        .unwrap();
}

// ────────────────────────────────────────────────────────────────────────────
// 1. 새 DB — 마이그레이션 적용 후 SCHEMA_VERSION이 되어야 한다
// ────────────────────────────────────────────────────────────────────────────
#[tokio::test]
async fn fresh_db_migrates_to_current_version() {
    let path = temp_db_path();
    let _ = std::fs::remove_file(&path);

    let pool = db::init_pool(path.to_str().unwrap())
        .await
        .expect("새 DB 초기화 실패");

    let version: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(version as u32, SCHEMA_VERSION, "마이그레이션 후 user_version이 SCHEMA_VERSION이어야 함");

    let _ = std::fs::remove_file(&path);
}

// ────────────────────────────────────────────────────────────────────────────
// 2. 이미 최신 버전 — 두 번째 호출도 Ok이고 버전이 변하지 않는다
// ────────────────────────────────────────────────────────────────────────────
#[tokio::test]
async fn already_current_version_is_noop() {
    let path = temp_db_path();
    let _ = std::fs::remove_file(&path);

    // 첫 번째 호출: 마이그레이션 적용
    let _ = db::init_pool(path.to_str().unwrap())
        .await
        .expect("첫 번째 초기화 실패");

    // 두 번째 호출: no-op이어야 한다
    let pool = db::init_pool(path.to_str().unwrap())
        .await
        .expect("두 번째 초기화에서 에러 발생 (no-op이어야 함)");

    let version: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(version as u32, SCHEMA_VERSION);

    let _ = std::fs::remove_file(&path);
}

// ────────────────────────────────────────────────────────────────────────────
// 3. 상위 버전 DB — SchemaTooNewError를 반환해야 한다
// ────────────────────────────────────────────────────────────────────────────
#[tokio::test]
async fn future_db_version_returns_schema_too_new_error() {
    let path = temp_db_path();
    let _ = std::fs::remove_file(&path);

    let future_ver = SCHEMA_VERSION + 5;
    create_raw_db_with_version(&path, future_ver).await;

    let err = db::init_pool(path.to_str().unwrap())
        .await
        .expect_err("상위 버전 DB에서 Ok를 반환함 — SchemaTooNewError여야 함");

    let schema_err = err
        .downcast_ref::<SchemaTooNewError>()
        .unwrap_or_else(|| panic!("SchemaTooNewError가 아닌 다른 에러: {err}"));

    assert_eq!(schema_err.db_ver, future_ver, "db_ver 불일치");
    assert_eq!(schema_err.app_ver, SCHEMA_VERSION, "app_ver 불일치");

    let _ = std::fs::remove_file(&path);
}

// ────────────────────────────────────────────────────────────────────────────
// 4. SCHEMA_VERSION + 1 경계값 — 딱 한 버전 높아도 에러
// ────────────────────────────────────────────────────────────────────────────
#[tokio::test]
async fn one_version_ahead_also_errors() {
    let path = temp_db_path();
    let _ = std::fs::remove_file(&path);

    create_raw_db_with_version(&path, SCHEMA_VERSION + 1).await;

    let err = db::init_pool(path.to_str().unwrap())
        .await
        .expect_err("SCHEMA_VERSION+1 DB에서 Ok를 반환함");

    assert!(
        err.downcast_ref::<SchemaTooNewError>().is_some(),
        "SchemaTooNewError가 아닌 다른 에러: {err}"
    );

    let _ = std::fs::remove_file(&path);
}

// ============================================================================
// run_migrations_with 단위 테스트 — 커스텀 마이그레이션 배열로 흐름 검증
// ============================================================================

/// in-memory SQLite 풀 (마이그레이션 미적용 상태)
async fn uninitialized_pool() -> SqlitePool {
    let opts = SqliteConnectOptions::new()
        .filename(":memory:")
        .foreign_keys(true);
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .unwrap()
}

async fn user_version(pool: &SqlitePool) -> u32 {
    let v: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(pool)
        .await
        .unwrap();
    v as u32
}

// ────────────────────────────────────────────────────────────────────────────
// 5. 여러 마이그레이션이 순서대로 모두 적용된다
// ────────────────────────────────────────────────────────────────────────────
#[tokio::test]
async fn multiple_migrations_applied_in_order() {
    let pool = uninitialized_pool().await;

    run_migrations_with(
        &pool,
        &[
            "CREATE TABLE m1 (id INTEGER PRIMARY KEY);",
            "CREATE TABLE m2 (id INTEGER PRIMARY KEY);",
        ],
    )
    .await
    .expect("마이그레이션 실패");

    assert_eq!(user_version(&pool).await, 2);

    // 두 테이블이 모두 존재해야 한다
    sqlx::query("INSERT INTO m1 VALUES (1)").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO m2 VALUES (1)").execute(&pool).await.unwrap();
}

// ────────────────────────────────────────────────────────────────────────────
// 6. 이미 적용된 마이그레이션은 건너뛰고 새 것만 적용한다
// ────────────────────────────────────────────────────────────────────────────
#[tokio::test]
async fn already_applied_migrations_are_skipped() {
    let pool = uninitialized_pool().await;

    // v1 적용
    run_migrations_with(&pool, &["CREATE TABLE m1 (id INTEGER PRIMARY KEY);"])
        .await
        .unwrap();
    assert_eq!(user_version(&pool).await, 1);

    // 같은 v1 SQL + 새 v2 SQL로 재호출
    // v1은 건너뛰고 (이미 user_version=1이므로 target=1에서 1<1 false)
    // v2만 적용되어야 한다
    run_migrations_with(
        &pool,
        &[
            "CREATE TABLE m1 (id INTEGER PRIMARY KEY);", // 건너뜀
            "CREATE TABLE m2 (id INTEGER PRIMARY KEY);", // 신규 적용
        ],
    )
    .await
    .expect("v2 마이그레이션 실패");

    assert_eq!(user_version(&pool).await, 2);

    // m1은 이미 있고, m2가 새로 생성되어야 한다
    sqlx::query("INSERT INTO m2 VALUES (1)").execute(&pool).await.unwrap();
}

// ────────────────────────────────────────────────────────────────────────────
// 7. 마이그레이션 도중 실패 → 마지막 커밋 버전부터 재개
//
//    흐름:
//      1차 호출: v1 커밋 성공 → v2 SQL 오류로 롤백 → Err 반환
//      재시작 시: current=1, v1 건너뜀, v2 재시도(유효 SQL) → 성공
// ────────────────────────────────────────────────────────────────────────────
#[tokio::test]
async fn partial_failure_resumes_from_last_committed_version() {
    let pool = uninitialized_pool().await;

    // 1차 호출: v1 성공, v2 실패
    let err = run_migrations_with(
        &pool,
        &[
            "CREATE TABLE m1 (id INTEGER PRIMARY KEY);",
            "THIS IS NOT VALID SQL !!!;", // 의도적 실패
        ],
    )
    .await;
    assert!(err.is_err(), "유효하지 않은 SQL에서 Ok를 반환함");

    // v1은 이미 커밋되어 user_version=1이어야 한다
    assert_eq!(user_version(&pool).await, 1, "v1 커밋 후 재시작 기준점이 1이어야 함");

    // m1 테이블은 존재한다 (v1 커밋 완료)
    sqlx::query("INSERT INTO m1 VALUES (1)").execute(&pool).await.unwrap();

    // 재시작: 동일 v1 + 수정된(유효한) v2
    run_migrations_with(
        &pool,
        &[
            "CREATE TABLE m1 (id INTEGER PRIMARY KEY);", // 건너뜀 (user_version=1)
            "CREATE TABLE m2 (id INTEGER PRIMARY KEY);", // v2 재시도
        ],
    )
    .await
    .expect("복구 후 마이그레이션 실패");

    assert_eq!(user_version(&pool).await, 2, "복구 후 user_version이 2이어야 함");
    sqlx::query("INSERT INTO m2 VALUES (1)").execute(&pool).await.unwrap();
}

// ────────────────────────────────────────────────────────────────────────────
// 8. 상위 버전 감지는 마이그레이션 시도 전에 일어난다
//    (마이그레이션 SQL이 실행되지 않았음을 user_version 불변으로 확인)
// ────────────────────────────────────────────────────────────────────────────
#[tokio::test]
async fn too_new_check_fires_before_any_migration() {
    let pool = uninitialized_pool().await;

    // user_version을 미래 버전으로 수동 설정
    sqlx::query("PRAGMA user_version = 99")
        .execute(&pool)
        .await
        .unwrap();

    let err = run_migrations_with(
        &pool,
        &["CREATE TABLE should_not_exist (id INTEGER PRIMARY KEY);"],
    )
    .await
    .expect_err("SchemaTooNewError를 반환해야 함");

    // SchemaTooNewError여야 하고 SQL 오류가 아니어야 한다
    let schema_err = err
        .downcast_ref::<SchemaTooNewError>()
        .unwrap_or_else(|| panic!("SchemaTooNewError가 아닌 다른 에러: {err}"));

    assert_eq!(schema_err.db_ver, 99);
    assert_eq!(schema_err.app_ver, 1); // migrations.len() = 1

    // user_version 불변 — 마이그레이션이 실행되지 않았다
    assert_eq!(user_version(&pool).await, 99);

    // 테이블이 생성되지 않았다
    let result = sqlx::query("SELECT 1 FROM should_not_exist LIMIT 1")
        .fetch_optional(&pool)
        .await;
    assert!(result.is_err(), "마이그레이션이 실행되지 않았어야 하므로 테이블이 없어야 함");
}
