mod common;

use axum::{
    extract::{Path, State},
    http::StatusCode,
};
use principal_candidate_manager::handlers::universities::{delete_track, delete_university};

// ── 공통 픽스처 ────────────────────────────────────────────────────

async fn insert_univ(pool: &sqlx::SqlitePool, name: &str) -> i64 {
    sqlx::query_scalar("INSERT INTO universities (univ_name) VALUES (?) RETURNING id")
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn insert_track(pool: &sqlx::SqlitePool, univ_id: i64, name: &str) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name) VALUES (?, ?) RETURNING id",
    )
    .bind(univ_id)
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn insert_round(pool: &sqlx::SqlitePool) -> i64 {
    sqlx::query_scalar("INSERT INTO rounds (status, opened_at) VALUES ('OPEN', '2025-01-01') RETURNING id")
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn insert_student(pool: &sqlx::SqlitePool, code: &str) -> i64 {
    let hash = bcrypt::hash("pass", 4u32).unwrap();
    sqlx::query("INSERT INTO classes (grade, class_no, password_hash) VALUES (1, 1, ?) ON CONFLICT DO NOTHING")
        .bind(&hash)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES (?, '학생', 1, 1, 1, 1) RETURNING id",
    )
    .bind(code)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn insert_application(
    pool: &sqlx::SqlitePool,
    student_id: i64,
    track_id: i64,
    round_id: i64,
) {
    sqlx::query(
        "INSERT INTO applications (student_id, track_id, round_id, abandoned) \
         VALUES (?, ?, ?, 0)",
    )
    .bind(student_id)
    .bind(track_id)
    .bind(round_id)
    .execute(pool)
    .await
    .unwrap();
}

// ── delete_track ───────────────────────────────────────────────────

#[tokio::test]
async fn delete_track_no_applications_ok() {
    let pool = common::create_test_pool().await;
    let uid = insert_univ(&pool, "한국대").await;
    let tid = insert_track(&pool, uid, "컴공").await;

    let res = delete_track(State(common::make_state(pool.clone())), Path(tid))
        .await
        .unwrap();
    assert_eq!(res, StatusCode::NO_CONTENT);

    let cnt: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM univ_tracks")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(cnt, 0);
}

#[tokio::test]
async fn delete_track_with_applications_returns_conflict() {
    let pool = common::create_test_pool().await;
    let uid = insert_univ(&pool, "한국대").await;
    let tid = insert_track(&pool, uid, "컴공").await;
    let rid = insert_round(&pool).await;
    let sid = insert_student(&pool, "S001").await;
    insert_application(&pool, sid, tid, rid).await;

    let err = delete_track(State(common::make_state(pool)), Path(tid))
        .await
        .unwrap_err();
    assert_eq!(err.0, StatusCode::CONFLICT);
    assert!(err.1.contains("지원 기록"));
}

// ── delete_university ──────────────────────────────────────────────

#[tokio::test]
async fn delete_university_no_applications_ok() {
    let pool = common::create_test_pool().await;
    let uid = insert_univ(&pool, "한국대").await;
    // 트랙이 있어도 지원이 없으면 삭제 가능
    insert_track(&pool, uid, "컴공").await;

    let res = delete_university(State(common::make_state(pool.clone())), Path(uid))
        .await
        .unwrap();
    assert_eq!(res, StatusCode::NO_CONTENT);

    let cnt: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM universities")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(cnt, 0);
}

#[tokio::test]
async fn delete_university_with_applications_returns_conflict() {
    let pool = common::create_test_pool().await;
    let uid = insert_univ(&pool, "한국대").await;
    let tid = insert_track(&pool, uid, "컴공").await;
    let rid = insert_round(&pool).await;
    let sid = insert_student(&pool, "S001").await;
    insert_application(&pool, sid, tid, rid).await;

    let err = delete_university(State(common::make_state(pool.clone())), Path(uid))
        .await
        .unwrap_err();
    assert_eq!(err.0, StatusCode::CONFLICT);
    assert!(err.1.contains("지원 기록"));

    // 대학 데이터가 그대로 남아 있어야 함
    let cnt: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM universities")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(cnt, 1);
}

#[tokio::test]
async fn delete_university_cascades_track_numeric_category() {
    let pool = common::create_test_pool().await;
    let uid = insert_univ(&pool, "한국대").await;
    let tid = insert_track(&pool, uid, "컴공").await;
    // numeric_table 및 category_map 행 추가
    let area_id: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, lookup_scope) \
         VALUES ('내신', 100000, 'NUMERIC', 'SIMPLE') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO numeric_table (area_id, track_id, threshold, score) VALUES (?, ?, 100, 80)")
        .bind(area_id)
        .bind(tid)
        .execute(&pool)
        .await
        .unwrap();

    delete_university(State(common::make_state(pool.clone())), Path(uid))
        .await
        .unwrap();

    let t: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM univ_tracks").fetch_one(&pool).await.unwrap();
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM numeric_table").fetch_one(&pool).await.unwrap();
    assert_eq!(t, 0, "트랙이 CASCADE 삭제되어야 함");
    assert_eq!(n, 0, "numeric_table 행이 CASCADE 삭제되어야 함");
}
