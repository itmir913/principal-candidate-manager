#![allow(dead_code)]

use std::sync::atomic::{AtomicU64, Ordering};

use principal_candidate_manager::{
    auth::{AdminClaims, TeacherClaims},
    state::AppState,
};
use sqlx::SqlitePool;

pub fn make_state(pool: SqlitePool) -> AppState {
    AppState { db: pool, jwt_secret: "test".into() }
}

pub fn teacher_claims(grade: i64, class_no: i64) -> TeacherClaims {
    TeacherClaims { role: "teacher".into(), grade, class_no, exp: 9_999_999_999 }
}

pub fn admin_claims() -> AdminClaims {
    AdminClaims { role: "admin".into(), exp: 9_999_999_999 }
}

pub async fn create_test_pool() -> SqlitePool {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    let opts = SqliteConnectOptions::new()
        .filename(":memory:")
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!("../../migrations/v1.sql")).execute(&pool).await.unwrap();
    pool
}

/// 트랜잭션과 병렬 쿼리가 공존하는 핸들러 테스트용 멀티-커넥션 풀.
/// SQLite shared in-memory DB (고유 이름)로 연결 간 데이터 공유 보장.
pub async fn create_test_pool_shared() -> SqlitePool {
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let url = format!("sqlite:file:testdb_{n}?mode=memory&cache=shared");
    let opts: SqliteConnectOptions = url.parse().unwrap();
    let opts = opts.foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!("../../migrations/v1.sql")).execute(&pool).await.unwrap();
    pool
}

pub async fn insert_class(pool: &SqlitePool, grade: i64, class_no: i64) {
    let hash = bcrypt::hash("pass", 4u32).unwrap();
    sqlx::query("INSERT INTO classes (grade, class_no, password_hash) VALUES (?, ?, ?)")
        .bind(grade)
        .bind(class_no)
        .bind(hash)
        .execute(pool)
        .await
        .unwrap();
}
