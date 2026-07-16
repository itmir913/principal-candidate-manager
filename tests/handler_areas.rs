mod common;

use axum::{extract::State, http::StatusCode, Json};
use principal_candidate_manager::enums::{CalcType, LookupScope};
use principal_candidate_manager::handlers::areas::{create_area, CreateAreaBody};
use principal_candidate_manager::score::Score;

fn manual_area_body(name: &str, max_score: Score) -> CreateAreaBody {
    CreateAreaBody {
        name: name.into(),
        max_score,
        calc_type: CalcType::Manual,
        teacher_editable: true,
        lookup_scope: LookupScope::Simple,
        match_mode: None,
        category_agg: None,
        multi_value: false,
    }
}

// ── create_area 만점(max_score) 유효성 검증 ──────────────────────

#[tokio::test]
async fn create_area_zero_max_score_succeeds() {
    // 순수 감점 전형요소: max_score=0 (만점이 0점, 점수 기준에서 감점)
    let pool = common::create_test_pool().await;
    let (status, _) = create_area(
        State(common::make_state(pool)),
        Json(manual_area_body("순수감점", Score::from_raw(0))),
    )
    .await
    .unwrap();
    assert_eq!(status, StatusCode::CREATED);
}

#[tokio::test]
async fn create_area_negative_max_score_rejected() {
    let pool = common::create_test_pool().await;
    let res = create_area(
        State(common::make_state(pool)),
        Json(manual_area_body("test", Score::from_raw(-100_000))),
    )
    .await;
    assert_eq!(res.unwrap_err().0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_area_valid_max_score_succeeds() {
    let pool = common::create_test_pool().await;
    let res = create_area(
        State(common::make_state(pool)),
        Json(manual_area_body("수기입력", Score::from_raw(1_000_000))),
    )
    .await;
    let (status, _) = res.unwrap();
    assert_eq!(status, StatusCode::CREATED);
}

#[tokio::test]
async fn create_numeric_area_without_match_mode_rejected() {
    // NUMERIC 전형요소에 match_mode 없으면 400
    let pool = common::create_test_pool().await;
    let body = CreateAreaBody {
        name: "내신".into(),
        max_score: Score::from_raw(1_000_000),
        calc_type: CalcType::Numeric,
        teacher_editable: false,
        lookup_scope: LookupScope::Simple,
        match_mode: None,
        category_agg: None,
        multi_value: false,
    };
    let res = create_area(State(common::make_state(pool)), Json(body)).await;
    assert_eq!(res.unwrap_err().0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_category_area_without_category_agg_rejected() {
    // CATEGORY 전형요소에 category_agg 없으면 400
    let pool = common::create_test_pool().await;
    let body = CreateAreaBody {
        name: "활동".into(),
        max_score: Score::from_raw(1_000_000),
        calc_type: CalcType::Category,
        teacher_editable: false,
        lookup_scope: LookupScope::Simple,
        match_mode: None,
        category_agg: None,
        multi_value: false,
    };
    let res = create_area(State(common::make_state(pool)), Json(body)).await;
    assert_eq!(res.unwrap_err().0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_area_whitespace_only_name_rejected() {
    // 이름이 공백만 있는 경우 → 400 (trim 후 빈 문자열)
    let pool = common::create_test_pool().await;
    let res = create_area(
        State(common::make_state(pool)),
        Json(manual_area_body("   ", Score::from_raw(1_000_000))),
    )
    .await;
    assert_eq!(res.unwrap_err().0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_area_empty_name_rejected() {
    // 이름이 빈 문자열 → 400
    let pool = common::create_test_pool().await;
    let res = create_area(
        State(common::make_state(pool)),
        Json(manual_area_body("", Score::from_raw(1_000_000))),
    )
    .await;
    assert_eq!(res.unwrap_err().0, StatusCode::BAD_REQUEST);
}

// ── 세션 4 감사 후속: FINALIZED 결과가 참조하는 전형요소 삭제 차단 ─

#[tokio::test]
async fn delete_area_referenced_by_finalized_results_returns_conflict() {
    use axum::extract::Path;
    use principal_candidate_manager::handlers::areas::delete_area;

    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;

    let aid: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type) VALUES ('내신', 10000000, 'MANUAL') RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();
    let sid: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES ('S001', '홍길동', 1, 1, 1, 1) RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO universities (univ_name) VALUES ('한국대')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO univ_tracks (univ_id, track_name) VALUES (1, '컴공')")
        .execute(&pool).await.unwrap();
    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at, closed_at, finalized_at) \
         VALUES ('FINALIZED', 'now', 'now', 'now') RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO applications (student_id, track_id, round_id, confirmed) VALUES (?, 1, ?, 1)")
        .bind(sid).bind(rid).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO results (student_id, track_id, round_id, score_detail, total_score, calculated_at) \
         VALUES (?, 1, ?, ?, 8500000, 'now')",
    )
    .bind(sid).bind(rid).bind(format!("{{\"{}\":8500000}}", aid))
    .execute(&pool).await.unwrap();

    let res = delete_area(State(common::make_state(pool.clone())), Path(aid)).await;
    let err = res.unwrap_err();
    assert_eq!(err.0, StatusCode::CONFLICT);
    assert!(err.1.contains("FINALIZED"));

    // 전형요소·기초데이터가 남아있어야 함
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM areas WHERE id = ?")
        .bind(aid).fetch_one(&pool).await.unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn delete_area_not_referenced_by_finalized_results_succeeds() {
    use axum::extract::Path;
    use principal_candidate_manager::handlers::areas::delete_area;

    let pool = common::create_test_pool().await;
    let aid: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type) VALUES ('내신', 10000000, 'MANUAL') RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();

    let res = delete_area(State(common::make_state(pool.clone())), Path(aid)).await;
    assert_eq!(res.unwrap(), StatusCode::NO_CONTENT);
}
