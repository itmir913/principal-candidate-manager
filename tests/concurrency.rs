//! 동시성 — 쓰기 잠금(`BEGIN IMMEDIATE`)에 기대는 핸들러를 두 커넥션에서 동시에 부른다.
//!
//! ① 추천 확정 정원 레이스(`recommend_result`) ② 진행 중인 마감과 담임 확정의 순서
//! (`teacher_confirm_round`).
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

use axum::{extract::{Path, State}, http::StatusCode, Extension};
use principal_candidate_manager::{
    db::init_pool,
    handlers::{round_confirmations::teacher_confirm_round, scoring::recommend_result},
};
use sqlx::SqlitePool;

mod common;

const ITERATIONS: usize = 15;

struct TempDb {
    pool: SqlitePool,
    path: std::path::PathBuf,
}

/// 임시 파일 DB. 정상 경로는 `close` 가 지운다. **단언이 실패해 패닉하면 파일이 남는다**
/// — 풀이 열린 채라 Windows 에서는 Drop 에서 지울 수도 없다. 이름에 pid·회차가 들어가고
/// `new` 가 같은 이름을 먼저 지우므로 다음 실행을 방해하지는 않는다(%TEMP% 의 `pcm_conc_*`).
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

// ── ② 담임 확정 ↔ 라운드 마감 ─────────────────────────────────────
//
// 확정은 OPEN 라운드에만 들어가야 한다. 마감이 쓰기 잠금을 쥔 채 진행 중일 때 확정이
// 들어오면, 확정은 마감 커밋을 기다렸다가 CLOSED 를 보고 400 이어야 한다.
//
// 처음에는 두 핸들러를 `tokio::join!` 으로 경합시켰으나, 확정 쪽 `BEGIN IMMEDIATE` 를
// `begin()` 으로 바꾼 변이가 살아남았다(15회 중 위험 구간에 한 번도 걸리지 않았다).
// 그래서 순서를 강제한다: 별도 커넥션이 `BEGIN IMMEDIATE` 로 잠금을 쥐고(진행 중인
// `close_round` 대역), 확정을 띄운 뒤 잠시 양보하고, 그 커넥션에서 라운드를 CLOSED 로
// 바꿔 커밋한다.
//
// 판별력의 소재: 확정이 DEFERRED 로 시작하면 잠금을 기다리지 않고 OPEN 을 읽은 뒤
// INSERT 에서 막히고, 커밋 후 스냅샷 충돌로 500 이 된다 — 400 단언이 잡는다.
// 대역은 `close_round` 자체가 아니므로 이 테스트가 지키는 것은 확정 쪽의 잠금이다.

#[tokio::test]
async fn teacher_confirm_waits_for_in_flight_close_and_sees_closed() {
    let db = TempDb::new("confirm", 0).await;
    let hash = bcrypt::hash("pass", 4u32).unwrap();
    sqlx::query("INSERT INTO classes (grade, class_no, password_hash) VALUES (1, 1, ?)")
        .bind(&hash).execute(&db.pool).await.unwrap();
    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at) VALUES ('OPEN', '2025-01-01T00:00:00Z') RETURNING id",
    )
    .fetch_one(&db.pool).await.unwrap();

    let mut closing = db.pool.begin_with("BEGIN IMMEDIATE").await.unwrap();

    let pool = db.pool.clone();
    let confirm = tokio::spawn(async move {
        teacher_confirm_round(
            State(common::make_state(pool)),
            Extension(common::teacher_claims(1, 1)),
            Path(rid),
        )
        .await
    });
    // 확정 태스크가 잠금 앞(정상) 또는 상태 조회 뒤(변이)까지 가도록 양보한다
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    sqlx::query("UPDATE rounds SET status = 'CLOSED', closed_at = '2025-01-02T00:00:00Z' WHERE id = ?")
        .bind(rid).execute(&mut *closing).await.unwrap();
    closing.commit().await.unwrap();

    let res = code_of(confirm.await.unwrap());
    assert_eq!(res.0, StatusCode::BAD_REQUEST, "마감 뒤 확정은 400 이어야 한다: {res:?}");
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM round_confirmations WHERE round_id = ?")
        .bind(rid).fetch_one(&db.pool).await.unwrap();
    assert_eq!(rows, 0, "마감된 라운드에 확정이 들어갔다");
    db.close().await;
}
