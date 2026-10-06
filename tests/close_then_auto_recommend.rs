//! 자동 추천이 `close_round` 가 **실제로 써 넣은** 순위 위에서 도는가.
//!
//! `handler_auto_recommend.rs` 는 results 행을 손으로 넣는다. 그러면 "마감이 쓰는
//! ranking·재학생 우선 순서"와 "자동 추천이 읽는 순서"의 정합은 관례로만 남는다.
//! 여기서는 OPEN 라운드에 지원과 기초데이터만 두고 실제 `close_round` →
//! `auto_recommend_results` 를 잇는다.
//!
//! 판별력의 소재: 학생 id 순서와 점수 순서를 일부러 엇갈리게 둔다(id 순 = 90·70·80).
//! 자동 추천이 점수 대신 등록순을 쓰거나, 재학생 우선 트랙에서 우선 규칙을 무시하면
//! 확정 집합이 달라진다.

mod common;

use axum::extract::{Path, State};
use principal_candidate_manager::handlers::{
    rounds::close_round,
    scoring::auto_recommend_results,
};
use sqlx::SqlitePool;

async fn student(pool: &SqlitePool, code: &str, seq: i64, enrolled: bool) -> i64 {
    if enrolled {
        sqlx::query_scalar(
            "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
             VALUES (?, ?, 1, 1, ?, 1) RETURNING id",
        )
        .bind(code).bind(code).bind(seq).fetch_one(pool).await.unwrap()
    } else {
        sqlx::query_scalar(
            "INSERT INTO students (student_code, name, grad_year, is_enrolled) \
             VALUES (?, ?, 2024, 0) RETURNING id",
        )
        .bind(code).bind(code).fetch_one(pool).await.unwrap()
    }
}

/// MANUAL 전형요소 하나(만점 100). 반환: area_id
async fn manual_area(pool: &SqlitePool) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO areas (name, calc_type, max_score, lookup_scope) \
         VALUES ('면접', 'MANUAL', 10000000, 'SIMPLE') RETURNING id",
    )
    .fetch_one(pool).await.unwrap()
}

async fn apply(pool: &SqlitePool, sid: i64, tid: i64, rid: i64, area: i64, score: i64) {
    sqlx::query(
        "INSERT INTO base_data (student_id, area_id, track_id, value, multi_value) VALUES (?, ?, NULL, ?, 0)",
    )
    .bind(sid).bind(area).bind((score * 100_000).to_string())
    .execute(pool).await.unwrap();
    sqlx::query(
        "INSERT INTO applications (student_id, track_id, round_id, department_name) VALUES (?, ?, ?, '학과')",
    )
    .bind(sid).bind(tid).bind(rid).execute(pool).await.unwrap();
}

async fn recommended(pool: &SqlitePool, rid: i64) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT s.student_code FROM results r JOIN students s ON s.id = r.student_id \
         WHERE r.round_id = ? AND r.recommended = 1 ORDER BY s.student_code",
    )
    .bind(rid).fetch_all(pool).await.unwrap()
}

async fn setup(pool: &SqlitePool, unit_quota: i64, prioritize: i64) -> (i64, i64, i64) {
    let hash = bcrypt::hash("pass", 4u32).unwrap();
    for (g, c) in [(1i64, 1i64), (0, 0)] {
        sqlx::query("INSERT INTO classes (grade, class_no, password_hash) VALUES (?, ?, ?)")
            .bind(g).bind(c).bind(&hash).execute(pool).await.unwrap();
    }
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name, prioritize_enrolled) VALUES ('한국대', ?) RETURNING id",
    )
    .bind(prioritize).fetch_one(pool).await.unwrap();
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name, unit_quota, prioritize_enrolled) \
         VALUES (?, '컴공', ?, ?) RETURNING id",
    )
    .bind(uid).bind(unit_quota).bind(prioritize).fetch_one(pool).await.unwrap();
    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at) VALUES ('OPEN', '2025-01-01T00:00:00Z') RETURNING id",
    )
    .fetch_one(pool).await.unwrap();
    let area = manual_area(pool).await;
    (tid, rid, area)
}

fn st(pool: &SqlitePool) -> State<principal_candidate_manager::state::AppState> {
    State(common::make_state(pool.clone()))
}

#[tokio::test]
async fn auto_recommend_after_real_close_takes_top_scores_not_insertion_order() {
    let pool = common::create_test_pool().await;
    let (tid, rid, area) = setup(&pool, 2, 0).await;
    // 등록순(id) = S1(90), S2(70), S3(80) — 점수순과 엇갈린다
    let s1 = student(&pool, "S1", 1, true).await;
    let s2 = student(&pool, "S2", 2, true).await;
    let s3 = student(&pool, "S3", 3, true).await;
    apply(&pool, s1, tid, rid, area, 90).await;
    apply(&pool, s2, tid, rid, area, 70).await;
    apply(&pool, s3, tid, rid, area, 80).await;

    let closed = close_round(st(&pool), Path(rid)).await.unwrap().0;
    assert_eq!(closed["calculated"], 3);
    let resp = auto_recommend_results(st(&pool), Path(rid)).await.unwrap().0;

    assert!(resp.manual.is_empty(), "깨끗한 경계라 수동 확인 항목이 없어야 한다");
    assert_eq!(recommended(&pool, rid).await, vec!["S1", "S3"], "점수 상위 2명");
}

#[tokio::test]
async fn auto_recommend_after_real_close_respects_enrolled_priority() {
    // 재학생 우선 트랙: 졸업생(95)이 점수로는 1위지만 정원 1석은 재학생(60)에게 간다
    let pool = common::create_test_pool().await;
    let (tid, rid, area) = setup(&pool, 1, 1).await;
    let grad = student(&pool, "G1", 0, false).await;
    let enr = student(&pool, "S1", 1, true).await;
    apply(&pool, grad, tid, rid, area, 95).await;
    apply(&pool, enr, tid, rid, area, 60).await;

    let closed = close_round(st(&pool), Path(rid)).await.unwrap().0;
    assert_eq!(closed["calculated"], 2);
    let resp = auto_recommend_results(st(&pool), Path(rid)).await.unwrap().0;

    assert!(resp.manual.is_empty(), "수동 확인 항목: {}", resp.manual.len());
    assert_eq!(recommended(&pool, rid).await, vec!["S1"], "재학생 우선");
}
