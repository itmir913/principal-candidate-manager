mod common;

use axum::{
    extract::{Path, State},
    http::StatusCode,
};
use principal_candidate_manager::handlers::scoring::{
    calc_area_score, calculate_scores, lookup_range_score, recommend_result, AreaRow,
};

// ── lookup_range_score 순수 함수 ──────────────────────────────────

fn sample_rows() -> Vec<(i64, i64)> {
    vec![(100_000, 50_000), (200_000, 30_000), (300_000, 10_000)]
}

#[test]
fn upper_exact_match() {
    assert_eq!(lookup_range_score(200_000, &sample_rows(), "UPPER"), 30_000);
}

#[test]
fn upper_between_thresholds() {
    assert_eq!(lookup_range_score(150_000, &sample_rows(), "UPPER"), 50_000);
}

#[test]
fn upper_above_all_thresholds() {
    assert_eq!(lookup_range_score(350_000, &sample_rows(), "UPPER"), 10_000);
}

#[test]
fn upper_below_all_thresholds() {
    assert_eq!(lookup_range_score(50_000, &sample_rows(), "UPPER"), 0);
}

#[test]
fn lower_exact_match() {
    assert_eq!(lookup_range_score(200_000, &sample_rows(), "LOWER"), 30_000);
}

#[test]
fn lower_between_thresholds() {
    assert_eq!(lookup_range_score(150_000, &sample_rows(), "LOWER"), 30_000);
}

#[test]
fn lower_above_all_thresholds() {
    // value가 최대 threshold를 초과하면 최대 threshold 행의 점수 반환 ("5일 이상 → 5점" 케이스)
    assert_eq!(lookup_range_score(400_000, &sample_rows(), "LOWER"), 10_000);
}

#[test]
fn lower_below_all_thresholds() {
    assert_eq!(lookup_range_score(50_000, &sample_rows(), "LOWER"), 50_000);
}

#[test]
fn empty_rows_return_zero() {
    assert_eq!(lookup_range_score(100_000, &[], "UPPER"), 0);
    assert_eq!(lookup_range_score(100_000, &[], "LOWER"), 0);
}

#[test]
fn unknown_direction_returns_zero() {
    assert_eq!(lookup_range_score(100_000, &sample_rows(), "UNKNOWN"), 0);
}

// ── calc_area_score ───────────────────────────────────────────────

async fn insert_student(pool: &sqlx::SqlitePool) -> i64 {
    sqlx::query(
        "INSERT INTO students (student_code, name, is_enrolled, grad_year) \
         VALUES ('S001', '홍길동', 0, 2024)",
    )
    .execute(pool)
    .await
    .unwrap()
    .last_insert_rowid()
}

async fn insert_area(
    pool: &sqlx::SqlitePool,
    calc_type: &str,
    direction: Option<&str>,
    agg: Option<&str>,
    scope: &str,
) -> i64 {
    sqlx::query(
        "INSERT INTO areas (name, max_score, calc_type, range_direction, category_agg, lookup_scope) \
         VALUES ('TestArea', 100000, ?, ?, ?, ?)",
    )
    .bind(calc_type)
    .bind(direction)
    .bind(agg)
    .bind(scope)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_rowid()
}

async fn insert_university(pool: &sqlx::SqlitePool) -> i64 {
    sqlx::query(
        "INSERT INTO universities (univ_name, track_name, capacity) VALUES ('서울대', '컴퓨터공학', 5)",
    )
    .execute(pool)
    .await
    .unwrap()
    .last_insert_rowid()
}

#[tokio::test]
async fn calc_range_simple_upper() {
    let pool = common::create_test_pool().await;
    let sid = insert_student(&pool).await;
    let aid = insert_area(&pool, "RANGE", Some("UPPER"), None, "SIMPLE").await;

    for (th, sc) in [(100_000i64, 50_000i64), (200_000, 30_000), (300_000, 10_000)] {
        sqlx::query(
            "INSERT INTO range_table (area_id, univ_id, threshold, score) VALUES (?, NULL, ?, ?)",
        )
        .bind(aid)
        .bind(th)
        .bind(sc)
        .execute(&pool)
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO base_data (student_id, area_id, univ_id, value) VALUES (?, ?, NULL, '125000')",
    )
    .bind(sid)
    .bind(aid)
    .execute(&pool)
    .await
    .unwrap();

    let area = AreaRow {
        id: aid,
        calc_type: "RANGE".into(),
        max_score: 100_000,
        range_direction: Some("UPPER".into()),
        category_agg: None,
        lookup_scope: "SIMPLE".into(),
    };
    assert_eq!(calc_area_score(&pool, sid, &area, 0).await.unwrap(), 50_000);
}

#[tokio::test]
async fn calc_range_simple_lower() {
    let pool = common::create_test_pool().await;
    let sid = insert_student(&pool).await;
    let aid = insert_area(&pool, "RANGE", Some("LOWER"), None, "SIMPLE").await;

    for (th, sc) in [(100_000i64, 50_000i64), (200_000, 30_000), (300_000, 10_000)] {
        sqlx::query(
            "INSERT INTO range_table (area_id, univ_id, threshold, score) VALUES (?, NULL, ?, ?)",
        )
        .bind(aid)
        .bind(th)
        .bind(sc)
        .execute(&pool)
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO base_data (student_id, area_id, univ_id, value) VALUES (?, ?, NULL, '150000')",
    )
    .bind(sid)
    .bind(aid)
    .execute(&pool)
    .await
    .unwrap();

    let area = AreaRow {
        id: aid,
        calc_type: "RANGE".into(),
        max_score: 100_000,
        range_direction: Some("LOWER".into()),
        category_agg: None,
        lookup_scope: "SIMPLE".into(),
    };
    assert_eq!(calc_area_score(&pool, sid, &area, 0).await.unwrap(), 30_000);
}

#[tokio::test]
async fn calc_range_composite() {
    let pool = common::create_test_pool().await;
    let sid = insert_student(&pool).await;
    let uid = insert_university(&pool).await;
    let aid = insert_area(&pool, "RANGE", Some("UPPER"), None, "COMPOSITE").await;

    sqlx::query(
        "INSERT INTO range_table (area_id, univ_id, threshold, score) VALUES (?, ?, 100000, 80000)",
    )
    .bind(aid)
    .bind(uid)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO base_data (student_id, area_id, univ_id, value) VALUES (?, ?, ?, '150000')",
    )
    .bind(sid)
    .bind(aid)
    .bind(uid)
    .execute(&pool)
    .await
    .unwrap();

    let area = AreaRow {
        id: aid,
        calc_type: "RANGE".into(),
        max_score: 100_000,
        range_direction: Some("UPPER".into()),
        category_agg: None,
        lookup_scope: "COMPOSITE".into(),
    };
    assert_eq!(calc_area_score(&pool, sid, &area, uid).await.unwrap(), 80_000);
}

#[tokio::test]
async fn calc_category_sum() {
    let pool = common::create_test_pool().await;
    let sid = insert_student(&pool).await;
    let aid = insert_area(&pool, "CATEGORY", None, Some("SUM"), "SIMPLE").await;

    for (cat, sc) in [("회장", 30_000i64), ("봉사", 20_000)] {
        sqlx::query(
            "INSERT INTO category_map (area_id, univ_id, category, score) VALUES (?, NULL, ?, ?)",
        )
        .bind(aid)
        .bind(cat)
        .bind(sc)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO base_data (student_id, area_id, univ_id, value) VALUES (?, ?, NULL, ?)",
        )
        .bind(sid)
        .bind(aid)
        .bind(cat)
        .execute(&pool)
        .await
        .unwrap();
    }

    let area = AreaRow {
        id: aid,
        calc_type: "CATEGORY".into(),
        max_score: 100_000,
        range_direction: None,
        category_agg: Some("SUM".into()),
        lookup_scope: "SIMPLE".into(),
    };
    assert_eq!(calc_area_score(&pool, sid, &area, 0).await.unwrap(), 50_000);
}

#[tokio::test]
async fn calc_category_max() {
    let pool = common::create_test_pool().await;
    let sid = insert_student(&pool).await;
    let aid = insert_area(&pool, "CATEGORY", None, Some("MAX"), "SIMPLE").await;

    for (cat, sc) in [("회장", 30_000i64), ("부회장", 20_000)] {
        sqlx::query(
            "INSERT INTO category_map (area_id, univ_id, category, score) VALUES (?, NULL, ?, ?)",
        )
        .bind(aid)
        .bind(cat)
        .bind(sc)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO base_data (student_id, area_id, univ_id, value) VALUES (?, ?, NULL, ?)",
        )
        .bind(sid)
        .bind(aid)
        .bind(cat)
        .execute(&pool)
        .await
        .unwrap();
    }

    let area = AreaRow {
        id: aid,
        calc_type: "CATEGORY".into(),
        max_score: 100_000,
        range_direction: None,
        category_agg: Some("MAX".into()),
        lookup_scope: "SIMPLE".into(),
    };
    assert_eq!(calc_area_score(&pool, sid, &area, 0).await.unwrap(), 30_000);
}

#[tokio::test]
async fn calc_manual() {
    let pool = common::create_test_pool().await;
    let sid = insert_student(&pool).await;
    let aid = insert_area(&pool, "MANUAL", None, None, "SIMPLE").await;

    sqlx::query(
        "INSERT INTO base_data (student_id, area_id, univ_id, value) VALUES (?, ?, NULL, '75000')",
    )
    .bind(sid)
    .bind(aid)
    .execute(&pool)
    .await
    .unwrap();

    let area = AreaRow {
        id: aid,
        calc_type: "MANUAL".into(),
        max_score: 100_000,
        range_direction: None,
        category_agg: None,
        lookup_scope: "SIMPLE".into(),
    };
    assert_eq!(calc_area_score(&pool, sid, &area, 0).await.unwrap(), 75_000);
}

#[tokio::test]
async fn calc_no_base_data_returns_zero() {
    let pool = common::create_test_pool().await;
    let sid = insert_student(&pool).await;
    let aid = insert_area(&pool, "RANGE", Some("UPPER"), None, "SIMPLE").await;

    sqlx::query(
        "INSERT INTO range_table (area_id, univ_id, threshold, score) VALUES (?, NULL, 100000, 50000)",
    )
    .bind(aid)
    .execute(&pool)
    .await
    .unwrap();

    let area = AreaRow {
        id: aid,
        calc_type: "RANGE".into(),
        max_score: 100_000,
        range_direction: Some("UPPER".into()),
        category_agg: None,
        lookup_scope: "SIMPLE".into(),
    };
    assert_eq!(calc_area_score(&pool, sid, &area, 0).await.unwrap(), 0);
}

#[tokio::test]
async fn calc_category_sum_capped_at_max_score() {
    // 합산이 max_score를 초과할 때 max_score로 상한 처리되는지 검증
    let pool = common::create_test_pool().await;
    let sid = insert_student(&pool).await;
    let aid = insert_area(&pool, "CATEGORY", None, Some("SUM"), "SIMPLE").await;
    // insert_area는 max_score=100_000으로 삽입. 세 항목 합산=110_000 > 100_000

    for (cat, sc) in [("회장", 50_000i64), ("부회장", 40_000), ("임원", 20_000)] {
        sqlx::query(
            "INSERT INTO category_map (area_id, univ_id, category, score) VALUES (?, NULL, ?, ?)",
        )
        .bind(aid).bind(cat).bind(sc).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO base_data (student_id, area_id, univ_id, value) VALUES (?, ?, NULL, ?)",
        )
        .bind(sid).bind(aid).bind(cat).execute(&pool).await.unwrap();
    }

    let area = AreaRow {
        id: aid,
        calc_type: "CATEGORY".into(),
        max_score: 100_000,
        range_direction: None,
        category_agg: Some("SUM".into()),
        lookup_scope: "SIMPLE".into(),
    };
    assert_eq!(calc_area_score(&pool, sid, &area, 0).await.unwrap(), 100_000);
}

#[tokio::test]
async fn calc_range_lower_above_max_threshold_uses_last_score() {
    // "결석 5일 이상: 5점" 케이스 — value가 최대 threshold를 초과해도 최대 threshold 점수 반환
    let pool = common::create_test_pool().await;
    let sid = insert_student(&pool).await;
    let aid = insert_area(&pool, "RANGE", Some("LOWER"), None, "SIMPLE").await;

    for (th, sc) in [(0i64, 100_000i64), (10_000, 80_000), (50_000, 50_000)] {
        sqlx::query(
            "INSERT INTO range_table (area_id, univ_id, threshold, score) VALUES (?, NULL, ?, ?)",
        )
        .bind(aid).bind(th).bind(sc).execute(&pool).await.unwrap();
    }
    // value=70_000 은 최대 threshold(50_000)를 초과 → 50_000점 반환 기대
    sqlx::query(
        "INSERT INTO base_data (student_id, area_id, univ_id, value) VALUES (?, ?, NULL, '70000')",
    )
    .bind(sid).bind(aid).execute(&pool).await.unwrap();

    let area = AreaRow {
        id: aid,
        calc_type: "RANGE".into(),
        max_score: 100_000,
        range_direction: Some("LOWER".into()),
        category_agg: None,
        lookup_scope: "SIMPLE".into(),
    };
    assert_eq!(calc_area_score(&pool, sid, &area, 0).await.unwrap(), 50_000);
}

// ── calculate_scores 통합 ─────────────────────────────────────────

async fn setup_full(pool: &sqlx::SqlitePool) -> (i64, i64, i64) {
    let hash = bcrypt::hash("pass", 4u32).unwrap();
    sqlx::query("INSERT INTO classes (grade, class_no, password_hash) VALUES (1, 1, ?)")
        .bind(&hash)
        .execute(pool)
        .await
        .unwrap();
    let sid: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES ('S001', '홍길동', 1, 1, 1, 1) RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name, track_name, capacity) \
         VALUES ('서울대', '컴공', 5) RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at) VALUES ('OPEN', '2025-01-01T00:00:00Z') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    (sid, uid, rid)
}

#[tokio::test]
async fn calculate_scores_nonexistent_round_returns_not_found() {
    let pool = common::create_test_pool().await;
    let res = calculate_scores(State(common::make_state(pool)), Path(9999i64)).await;
    assert_eq!(res.unwrap_err().0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn calculate_scores_no_applications_returns_zero_count() {
    let pool = common::create_test_pool().await;
    let (_, _, rid) = setup_full(&pool).await;
    let axum::Json(result) =
        calculate_scores(State(common::make_state(pool)), Path(rid)).await.unwrap();
    assert_eq!(result["calculated"], 0);
}

#[tokio::test]
async fn calculate_scores_creates_result_rows_and_ranking() {
    let pool = common::create_test_pool().await;
    let (sid, uid, rid) = setup_full(&pool).await;

    sqlx::query(
        "INSERT INTO applications (student_id, univ_id, round_id, confirmed, abandoned) \
         VALUES (?, ?, ?, 1, 0)",
    )
    .bind(sid)
    .bind(uid)
    .bind(rid)
    .execute(&pool)
    .await
    .unwrap();

    let axum::Json(result) =
        calculate_scores(State(common::make_state(pool.clone())), Path(rid)).await.unwrap();
    assert_eq!(result["calculated"], 1);

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM results WHERE round_id = ?")
        .bind(rid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);

    let ranking: Option<i64> =
        sqlx::query_scalar("SELECT ranking FROM results WHERE round_id = ?")
            .bind(rid)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(ranking, Some(1));
}

#[tokio::test]
async fn calculate_scores_ranks_higher_score_first() {
    let pool = common::create_test_pool().await;
    let hash = bcrypt::hash("pass", 4u32).unwrap();
    sqlx::query("INSERT INTO classes (grade, class_no, password_hash) VALUES (1, 1, ?)")
        .bind(&hash)
        .execute(&pool)
        .await
        .unwrap();

    let sid1: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES ('S001', '홍길동', 1, 1, 1, 1) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let sid2: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES ('S002', '이순신', 1, 1, 2, 1) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name, track_name, capacity) \
         VALUES ('서울대', '컴공', 5) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at) VALUES ('OPEN', '2025-01-01T00:00:00Z') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let aid: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, lookup_scope) \
         VALUES ('수동점수', 1000000, 'MANUAL', 'SIMPLE') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO base_data (student_id, area_id, univ_id, value) VALUES (?, ?, NULL, '8000000')",
    )
    .bind(sid1)
    .bind(aid)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO base_data (student_id, area_id, univ_id, value) VALUES (?, ?, NULL, '6000000')",
    )
    .bind(sid2)
    .bind(aid)
    .execute(&pool)
    .await
    .unwrap();

    for sid in [sid1, sid2] {
        sqlx::query(
            "INSERT INTO applications (student_id, univ_id, round_id, confirmed, abandoned) \
             VALUES (?, ?, ?, 1, 0)",
        )
        .bind(sid)
        .bind(uid)
        .bind(rid)
        .execute(&pool)
        .await
        .unwrap();
    }

    let _ = calculate_scores(State(common::make_state(pool.clone())), Path(rid)).await.unwrap();

    let rank1: Option<i64> =
        sqlx::query_scalar("SELECT ranking FROM results WHERE student_id = ? AND round_id = ?")
            .bind(sid1)
            .bind(rid)
            .fetch_one(&pool)
            .await
            .unwrap();
    let rank2: Option<i64> =
        sqlx::query_scalar("SELECT ranking FROM results WHERE student_id = ? AND round_id = ?")
            .bind(sid2)
            .bind(rid)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(rank1, Some(1));
    assert_eq!(rank2, Some(2));
}

// ── recommend_result ──────────────────────────────────────────────

#[tokio::test]
async fn recommend_on_open_round_returns_bad_request() {
    let pool = common::create_test_pool().await;
    let (sid, uid, rid) = setup_full(&pool).await;
    let res = recommend_result(State(common::make_state(pool)), Path((sid, uid, rid))).await;
    assert_eq!(res.unwrap_err().0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn recommend_on_closed_round_sets_flag() {
    let pool = common::create_test_pool().await;
    let (sid, uid, rid) = setup_full(&pool).await;

    sqlx::query(
        "INSERT INTO applications (student_id, univ_id, round_id, confirmed, abandoned) \
         VALUES (?, ?, ?, 1, 0)",
    )
    .bind(sid)
    .bind(uid)
    .bind(rid)
    .execute(&pool)
    .await
    .unwrap();

    let _ = calculate_scores(State(common::make_state(pool.clone())), Path(rid)).await.unwrap();

    sqlx::query(
        "UPDATE rounds SET status = 'CLOSED', closed_at = '2025-01-02T00:00:00Z' WHERE id = ?",
    )
    .bind(rid)
    .execute(&pool)
    .await
    .unwrap();

    recommend_result(State(common::make_state(pool.clone())), Path((sid, uid, rid)))
        .await
        .unwrap();

    let recommended: i64 = sqlx::query_scalar(
        "SELECT recommended FROM results WHERE student_id = ? AND univ_id = ? AND round_id = ?",
    )
    .bind(sid)
    .bind(uid)
    .bind(rid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(recommended, 1);
}
