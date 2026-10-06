//! 추천 확정 정원 레이스 — `recommend_result` 를 두 커넥션에서 동시에 부른다.
//!
//! 운영과 같은 조건(파일 DB + WAL + busy_timeout, `db::init_pool`)에서 돌린다.
//! 공유 캐시 인메모리 DB 는 테이블 단위 잠금이라 `BEGIN IMMEDIATE` 대기 의미가
//! 운영과 달라 여기서는 쓰지 않는다.
//!
//! 판별력의 소재: 정원 1 에 동점 후보 둘을 동시에 확정할 때 **결과 코드 쌍**이
//! 정확히 {204, 409} 이고 확정 인원이 1 이어야 한다. `recommend_result` 의
//! `begin_with("BEGIN IMMEDIATE")` 를 `begin()`(DEFERRED)으로 바꾸면 두 트랜잭션이
//! 같은 스냅샷에서 COUNT 를 읽고, 늦은 쪽의 UPDATE 가 SQLITE_BUSY(스냅샷 충돌)로
//! 500 이 된다 — 그 경우를 잡는다(2026-10-06 변이로 확인: 첫 회차에 500 "database is
//! locked"). 인터리빙이 비결정적이라 한 번에 안 걸릴 수 있어
//! 여러 회 반복한다.

use axum::{extract::{Path, State}, http::StatusCode};
use principal_candidate_manager::{db::init_pool, handlers::scoring::recommend_result};
use sqlx::SqlitePool;

mod common;

const ITERATIONS: usize = 15;

struct TempDb {
    pool: SqlitePool,
    path: std::path::PathBuf,
}

impl TempDb {
    async fn new(tag: &str, i: usize) -> Self {
        let path = std::env::temp_dir().join(format!(
            "pcm_conc_{tag}_{}_{i}.db",
            std::process::id()
        ));
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
        let pool = init_pool(path.to_str().unwrap()).await.unwrap();
        Self { pool, path }
    }

    async fn close(self) {
        self.pool.close().await;
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", self.path.display()));
        }
    }
}

/// CLOSED 라운드 + 대학 하나 + 모집단위 두 개. 반환: (track_a, track_b, round)
async fn setup(pool: &SqlitePool, unit_quota: Option<i64>, total_quota: Option<i64>) -> (i64, i64, i64) {
    let hash = bcrypt::hash("pass", 4u32).unwrap();
    for (g, c) in [(1i64, 1i64), (0, 0)] {
        sqlx::query("INSERT INTO classes (grade, class_no, password_hash) VALUES (?, ?, ?)")
            .bind(g).bind(c).bind(&hash).execute(pool).await.unwrap();
    }
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name, total_quota) VALUES ('한국대', ?) RETURNING id",
    )
    .bind(total_quota).fetch_one(pool).await.unwrap();
    let mut tids = Vec::new();
    for name in ["컴공", "전자"] {
        let tid: i64 = sqlx::query_scalar(
            "INSERT INTO univ_tracks (univ_id, track_name, unit_quota) VALUES (?, ?, ?) RETURNING id",
        )
        .bind(uid).bind(name).bind(unit_quota).fetch_one(pool).await.unwrap();
        tids.push(tid);
    }
    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at, closed_at) \
         VALUES ('CLOSED', '2025-01-01T00:00:00Z', '2025-01-02T00:00:00Z') RETURNING id",
    )
    .fetch_one(pool).await.unwrap();
    (tids[0], tids[1], rid)
}

/// 동점(같은 점수·순위 1) 후보 — 순서 가드가 서로를 막지 않게 한다.
async fn candidate(pool: &SqlitePool, tid: i64, rid: i64, code: &str, seq: i64) -> i64 {
    let sid: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES (?, ?, 1, 1, ?, 1) RETURNING id",
    )
    .bind(code).bind(code).bind(seq).fetch_one(pool).await.unwrap();
    sqlx::query(
        "INSERT INTO applications (student_id, track_id, round_id, department_name) \
         VALUES (?, ?, ?, '학과')",
    )
    .bind(sid).bind(tid).bind(rid).execute(pool).await.unwrap();
    sqlx::query(
        "INSERT INTO results \
         (student_id, track_id, round_id, score_detail, total_score, ranking, recommended, calculated_at) \
         VALUES (?, ?, ?, '{}', 500000, 1, 0, '2025-01-02T00:00:00Z')",
    )
    .bind(sid).bind(tid).bind(rid).execute(pool).await.unwrap();
    sid
}

fn code_of(r: Result<StatusCode, (StatusCode, String)>) -> (StatusCode, String) {
    match r {
        Ok(s) => (s, String::new()),
        Err(e) => e,
    }
}

async fn race(pool: &SqlitePool, a: (i64, i64), b: (i64, i64), rid: i64) -> Vec<(StatusCode, String)> {
    let (ra, rb) = tokio::join!(
        recommend_result(State(common::make_state(pool.clone())), Path((a.0, a.1, rid))),
        recommend_result(State(common::make_state(pool.clone())), Path((b.0, b.1, rid))),
    );
    let mut out = vec![code_of(ra), code_of(rb)];
    out.sort_by_key(|(s, _)| s.as_u16());
    out
}

async fn recommended_count(pool: &SqlitePool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM results WHERE recommended = 1")
        .fetch_one(pool).await.unwrap()
}

#[tokio::test]
async fn concurrent_recommend_respects_unit_quota() {
    for i in 0..ITERATIONS {
        let db = TempDb::new("unit", i).await;
        let (ta, _, rid) = setup(&db.pool, Some(1), None).await;
        let s1 = candidate(&db.pool, ta, rid, "S001", 1).await;
        let s2 = candidate(&db.pool, ta, rid, "S002", 2).await;

        let codes = race(&db.pool, (s1, ta), (s2, ta), rid).await;
        assert_eq!(codes[0].0, StatusCode::NO_CONTENT, "회차 {i}: {codes:?}");
        assert_eq!(codes[1].0, StatusCode::CONFLICT, "회차 {i}: {codes:?}");
        assert!(codes[1].1.contains("모집단위 정원"), "회차 {i}: 정원 사유여야 한다: {codes:?}");
        assert_eq!(recommended_count(&db.pool).await, 1, "회차 {i}: 정원 초과 확정");
        db.close().await;
    }
}

#[tokio::test]
async fn concurrent_recommend_respects_total_quota_across_tracks() {
    // 서로 다른 모집단위라 모집단위 정원은 무관 — 대학 전체 정원(1)만이 막는다
    for i in 0..ITERATIONS {
        let db = TempDb::new("total", i).await;
        let (ta, tb, rid) = setup(&db.pool, None, Some(1)).await;
        let s1 = candidate(&db.pool, ta, rid, "S001", 1).await;
        let s2 = candidate(&db.pool, tb, rid, "S002", 2).await;

        let codes = race(&db.pool, (s1, ta), (s2, tb), rid).await;
        assert_eq!(codes[0].0, StatusCode::NO_CONTENT, "회차 {i}: {codes:?}");
        assert_eq!(codes[1].0, StatusCode::CONFLICT, "회차 {i}: {codes:?}");
        assert_eq!(recommended_count(&db.pool).await, 1, "회차 {i}: 정원 초과 확정");
        db.close().await;
    }
}
