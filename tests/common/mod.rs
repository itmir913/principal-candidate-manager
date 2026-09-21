#![allow(dead_code)]

use std::sync::atomic::{AtomicU64, Ordering};

use axum::{body::Body, extract::{FromRequest, Multipart}, http::Request};

use principal_candidate_manager::{
    auth::{AdminClaims, TeacherClaims},
    state::AppState,
};
use sqlx::SqlitePool;

pub fn make_state(pool: SqlitePool) -> AppState {
    AppState { db: pool, jwt_secret: "test".into(), db_path: std::path::PathBuf::from(":memory:"), server_addr: String::new() }
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
    sqlx::raw_sql(&principal_candidate_manager::db::full_schema_sql()).execute(&pool).await.unwrap();
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
    sqlx::raw_sql(&principal_candidate_manager::db::full_schema_sql()).execute(&pool).await.unwrap();
    pool
}

/// CSV 문자열을 multipart/form-data 요청으로 감싸 반환
pub async fn csv_multipart(csv: &str) -> Multipart {
    let boundary = "boundary42";
    let body = format!(
        "--{boundary}\r\n\
         Content-Disposition: form-data; name=\"file\"; filename=\"data.csv\"\r\n\
         Content-Type: text/csv\r\n\r\n\
         {csv}\r\n\
         --{boundary}--\r\n"
    );
    let req = Request::builder()
        .method("POST")
        .header("content-type", format!("multipart/form-data; boundary={boundary}"))
        .body(Body::from(body))
        .unwrap();
    Multipart::from_request(req, &()).await.unwrap()
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

// ── src/enums.rs 를 소스로 읽어 변형을 열거한다 ──────────────────────────────
//
// 런타임에 enum 변형을 열거할 방법이 없다(strum 미사용). 프론트 상수는 JS 라
// Rust 에서 import 할 수도 없으므로, 양쪽을 텍스트로 읽어 대조한다.
// audit_labels_coverage.rs 와 frontend_enum_sync.rs 가 함께 쓴다 — 파싱 로직을
// 두 벌 두면 그것부터 어긋난다.

/// `pub enum <name> { ... }` 블록의 변형 이름을 SCREAMING_SNAKE_CASE 로 뽑는다.
/// 모든 대상 enum 에 `#[serde(rename_all = "SCREAMING_SNAKE_CASE")]` 가 걸려 있어
/// DB·API 표기가 이것이다.
pub fn enum_variants(enum_name: &str) -> std::collections::BTreeSet<String> {
    let src = include_str!("../../src/enums.rs");
    let decl = format!("pub enum {enum_name} {{");
    let start = src
        .find(&decl)
        .unwrap_or_else(|| panic!("{enum_name} 선언을 찾지 못했다 (src/enums.rs)"));
    let body_start = start + src[start..].find('{').unwrap() + 1;
    let body_end = body_start + src[body_start..].find('}').expect("enum 본문의 끝");

    src[body_start..body_end]
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with("//") && !l.starts_with('#'))
        .map(|l| l.trim_end_matches(',').trim())
        .filter(|l| !l.is_empty())
        .map(to_screaming_snake)
        .collect()
}

pub fn to_screaming_snake(variant: &str) -> String {
    let mut out = String::new();
    for (i, c) in variant.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push('_');
        }
        out.extend(c.to_uppercase());
    }
    out
}
