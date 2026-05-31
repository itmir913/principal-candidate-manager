mod common;

use axum::{extract::{Query, State}, http::StatusCode, Extension, Json};
use principal_candidate_manager::handlers::applications::{
    teacher_create_application, BaseDataEntry, CreateApplicationBody,
};
use principal_candidate_manager::handlers::teacher_areas::{
    teacher_area_context, teacher_area_score_preview, AreaContextQuery, AreaScorePreviewBody,
};

// ── 공통 픽스처 ────────────────────────────────────────────────────

async fn setup_base(pool: &sqlx::SqlitePool) -> (i64, i64, i64, i64, i64, i64, i64) {
    let hash = bcrypt::hash("pass", 4u32).unwrap();
    sqlx::query("INSERT INTO classes (grade, class_no, password_hash) VALUES (1, 1, ?)")
        .bind(&hash)
        .execute(pool)
        .await
        .unwrap();

    let sid: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled)
         VALUES ('S001', '홍길동', 1, 1, 1, 1) RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();

    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name) VALUES ('한국대') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();

    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name) VALUES (?, '자연계열') RETURNING id",
    )
    .bind(uid)
    .fetch_one(pool)
    .await
    .unwrap();

    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at) VALUES ('OPEN', '2025-01-01T00:00:00Z') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();

    // NUMERIC area (teacher_editable=1, SIMPLE)
    let num_aid: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, teacher_editable, lookup_scope, match_mode)
         VALUES ('봉사시간', 500000, 'NUMERIC', 1, 'SIMPLE', 'UPPER') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();

    // CATEGORY area (teacher_editable=1, SIMPLE, SUM, multi_value=0)
    let cat_aid: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, teacher_editable, lookup_scope, category_agg)
         VALUES ('봉사직책', 300000, 'CATEGORY', 1, 'SIMPLE', 'SUM') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();

    // MANUAL area (teacher_editable=0, SIMPLE)
    let man_aid: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, teacher_editable, lookup_scope)
         VALUES ('면접점수', 1000000, 'MANUAL', 0, 'SIMPLE') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();

    // numeric_table: 봉사시간 구간
    sqlx::query(
        "INSERT INTO numeric_table (area_id, track_id, threshold, score) VALUES
         (?, NULL, 1000000, 100000),
         (?, NULL, 2000000, 200000),
         (?, NULL, 3000000, 300000),
         (?, NULL, 4000000, 400000),
         (?, NULL, 5000000, 500000)",
    )
    .bind(num_aid).bind(num_aid).bind(num_aid).bind(num_aid).bind(num_aid)
    .execute(pool)
    .await
    .unwrap();

    // category_map: 봉사직책
    sqlx::query(
        "INSERT INTO category_map (area_id, track_id, category, score) VALUES
         (?, NULL, '회장', 300000),
         (?, NULL, '부회장', 200000),
         (?, NULL, '일반', 100000)",
    )
    .bind(cat_aid).bind(cat_aid).bind(cat_aid)
    .execute(pool)
    .await
    .unwrap();

    (sid, tid, rid, num_aid, cat_aid, man_aid, uid)
}

// ── teacher_area_context ──────────────────────────────────────────

#[tokio::test]
async fn area_context_returns_all_areas_with_tables() {
    let pool = common::create_test_pool_shared().await;
    let (sid, tid, _, _, _, _, _) = setup_base(&pool).await;

    let result = teacher_area_context(
        State(common::make_state(pool)),
        Extension(common::teacher_claims(1, 1)),
        Query(AreaContextQuery { student_id: sid, track_id: tid }),
    )
    .await
    .unwrap();

    let items = result.0;
    assert_eq!(items.len(), 3);

    // NUMERIC 항목 확인
    let numeric = items.iter().find(|i| i.area_name == "봉사시간").unwrap();
    assert!(numeric.teacher_editable);
    assert!(numeric.table.is_some());
    let table = numeric.table.as_ref().unwrap();
    assert_eq!(table.len(), 5);

    // MANUAL 항목: 테이블 없음, teacher_editable=false
    let manual = items.iter().find(|i| i.area_name == "면접점수").unwrap();
    assert!(!manual.teacher_editable);
    assert!(manual.table.is_none());
}

#[tokio::test]
async fn area_context_prefills_existing_base_data() {
    let pool = common::create_test_pool_shared().await;
    let (sid, _, _, num_aid, _, man_aid, _) = setup_base(&pool).await;

    // 관리자가 사전에 업로드한 기초데이터
    sqlx::query(
        "INSERT INTO base_data (student_id, area_id, track_id, value, multi_value)
         VALUES (?, ?, NULL, '3000000', 0)",
    )
    .bind(sid).bind(num_aid)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO base_data (student_id, area_id, track_id, value, multi_value)
         VALUES (?, ?, NULL, '850000', 0)",
    )
    .bind(sid).bind(man_aid)
    .execute(&pool)
    .await
    .unwrap();

    let tid: i64 = sqlx::query_scalar("SELECT id FROM univ_tracks LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();

    let result = teacher_area_context(
        State(common::make_state(pool)),
        Extension(common::teacher_claims(1, 1)),
        Query(AreaContextQuery { student_id: sid, track_id: tid }),
    )
    .await
    .unwrap();

    let items = result.0;
    let numeric = items.iter().find(|i| i.area_name == "봉사시간").unwrap();
    // 3000000 → "30" (표시값)
    assert_eq!(numeric.current_values, vec!["30"]);

    let manual = items.iter().find(|i| i.area_name == "면접점수").unwrap();
    // 850000 → "8.5"
    assert_eq!(manual.current_values, vec!["8.5"]);
}

#[tokio::test]
async fn area_context_wrong_class_forbidden() {
    let pool = common::create_test_pool_shared().await;
    let (sid, tid, _, _, _, _, _) = setup_base(&pool).await;

    let res = teacher_area_context(
        State(common::make_state(pool)),
        Extension(common::teacher_claims(2, 2)), // 다른 담임
        Query(AreaContextQuery { student_id: sid, track_id: tid }),
    )
    .await;
    assert_eq!(res.unwrap_err().0, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn area_context_composite_fallback_to_global_table() {
    let pool = common::create_test_pool_shared().await;
    let hash = bcrypt::hash("pass", 4u32).unwrap();
    sqlx::query("INSERT INTO classes (grade, class_no, password_hash) VALUES (1, 1, ?)")
        .bind(&hash)
        .execute(&pool)
        .await
        .unwrap();
    let sid: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled)
         VALUES ('S002', '이몽룡', 1, 1, 2, 1) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name) VALUES ('서울대') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name) VALUES (?, '공과대학') RETURNING id",
    )
    .bind(uid)
    .fetch_one(&pool)
    .await
    .unwrap();

    // COMPOSITE 전형요소: 전역 테이블만 있음 (track_id IS NULL)
    let comp_aid: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, teacher_editable, lookup_scope, match_mode)
         VALUES ('내신등급', 500000, 'NUMERIC', 1, 'COMPOSITE', 'LOWER') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO numeric_table (area_id, track_id, threshold, score) VALUES
         (?, NULL, 100000, 500000),
         (?, NULL, 200000, 400000),
         (?, NULL, 300000, 300000)",
    )
    .bind(comp_aid).bind(comp_aid).bind(comp_aid)
    .execute(&pool)
    .await
    .unwrap();

    let result = teacher_area_context(
        State(common::make_state(pool)),
        Extension(common::teacher_claims(1, 1)),
        Query(AreaContextQuery { student_id: sid, track_id: tid }),
    )
    .await
    .unwrap();

    let item = result.0.iter().find(|i| i.area_name == "내신등급").unwrap();
    // 전역 폴백: 3행 모두 표시되어야 함
    assert_eq!(item.table.as_ref().unwrap().len(), 3);
}

// ── teacher_area_score_preview ────────────────────────────────────

async fn get_area_id(pool: &sqlx::SqlitePool, name: &str) -> i64 {
    sqlx::query_scalar("SELECT id FROM areas WHERE name = ?")
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn score_preview_numeric_upper_returns_correct_score_and_key() {
    let pool = common::create_test_pool_shared().await;
    setup_base(&pool).await;
    let tid: i64 = sqlx::query_scalar("SELECT id FROM univ_tracks LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let area_id = get_area_id(&pool, "봉사시간").await;

    // 35시간: 3000000 threshold(30시간)에 해당 → 300000점
    let resp = teacher_area_score_preview(
        State(common::make_state(pool)),
        Json(AreaScorePreviewBody {
            area_id,
            track_id: tid,
            values: vec!["35".into()],
        }),
    )
    .await
    .unwrap()
    .0;

    assert!(resp.error.is_none());
    assert!(resp.warning.is_none());
    // score = 3.0 (300000 / 100000)
    assert_eq!(resp.score.unwrap().raw(), 300000);
    // matched_key = 30.0 (threshold 3000000 / 100000)
    assert_eq!(resp.matched_keys.len(), 1);
    assert!((resp.matched_keys[0].as_f64().unwrap() - 30.0).abs() < 1e-9);
}

#[tokio::test]
async fn score_preview_numeric_upper_below_all_thresholds_returns_error() {
    let pool = common::create_test_pool_shared().await;
    setup_base(&pool).await;
    let tid: i64 = sqlx::query_scalar("SELECT id FROM univ_tracks LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let area_id = get_area_id(&pool, "봉사시간").await;

    // 5시간: 최소 threshold(10시간=1000000)보다 낮음 → 오류
    let resp = teacher_area_score_preview(
        State(common::make_state(pool)),
        Json(AreaScorePreviewBody {
            area_id,
            track_id: tid,
            values: vec!["5".into()],
        }),
    )
    .await
    .unwrap()
    .0;

    assert!(resp.error.is_some());
    assert!(resp.score.is_none());
}

#[tokio::test]
async fn score_preview_category_single_returns_correct() {
    let pool = common::create_test_pool_shared().await;
    setup_base(&pool).await;
    let tid: i64 = sqlx::query_scalar("SELECT id FROM univ_tracks LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let area_id = get_area_id(&pool, "봉사직책").await;

    let resp = teacher_area_score_preview(
        State(common::make_state(pool)),
        Json(AreaScorePreviewBody {
            area_id,
            track_id: tid,
            values: vec!["회장".into()],
        }),
    )
    .await
    .unwrap()
    .0;

    assert!(resp.error.is_none());
    assert_eq!(resp.score.unwrap().raw(), 300000);
    assert_eq!(resp.matched_keys[0].as_str().unwrap(), "회장");
}

#[tokio::test]
async fn score_preview_category_unknown_value_returns_error() {
    let pool = common::create_test_pool_shared().await;
    setup_base(&pool).await;
    let tid: i64 = sqlx::query_scalar("SELECT id FROM univ_tracks LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let area_id = get_area_id(&pool, "봉사직책").await;

    let resp = teacher_area_score_preview(
        State(common::make_state(pool)),
        Json(AreaScorePreviewBody {
            area_id,
            track_id: tid,
            values: vec!["총무".into()], // 존재하지 않는 범주
        }),
    )
    .await
    .unwrap()
    .0;

    assert!(resp.error.is_some());
    assert!(resp.score.is_none());
}

#[tokio::test]
async fn score_preview_manual_returns_score_directly() {
    let pool = common::create_test_pool_shared().await;
    setup_base(&pool).await;
    let tid: i64 = sqlx::query_scalar("SELECT id FROM univ_tracks LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let area_id = get_area_id(&pool, "면접점수").await;

    let resp = teacher_area_score_preview(
        State(common::make_state(pool)),
        Json(AreaScorePreviewBody {
            area_id,
            track_id: tid,
            values: vec!["8.5".into()],
        }),
    )
    .await
    .unwrap()
    .0;

    assert!(resp.error.is_none());
    assert!(resp.warning.is_none());
    assert_eq!(resp.score.unwrap().raw(), 850000);
    assert!(resp.matched_keys.is_empty());
}

#[tokio::test]
async fn score_preview_manual_exceeds_max_score_returns_warning() {
    let pool = common::create_test_pool_shared().await;
    setup_base(&pool).await;
    let tid: i64 = sqlx::query_scalar("SELECT id FROM univ_tracks LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let area_id = get_area_id(&pool, "면접점수").await;

    // 만점 10점 초과 → 만점으로 캡핑 + 경고
    let resp = teacher_area_score_preview(
        State(common::make_state(pool)),
        Json(AreaScorePreviewBody {
            area_id,
            track_id: tid,
            values: vec!["15".into()],
        }),
    )
    .await
    .unwrap()
    .0;

    assert!(resp.error.is_none());
    assert!(resp.warning.is_some());
    assert_eq!(resp.score.unwrap().raw(), 1000000); // 만점 캡핑
}

// ── base_data 저장 포함 지원 등록 ─────────────────────────────────

#[tokio::test]
async fn create_application_with_base_data_saves_correctly() {
    let pool = common::create_test_pool_shared().await;
    let (sid, tid, rid, num_aid, cat_aid, man_aid, _) = setup_base(&pool).await;

    // 관리자가 사전에 업로드한 면접점수 데이터 (teacher_editable=0)
    sqlx::query("INSERT INTO base_data (student_id, area_id, track_id, value, multi_value) VALUES (?, ?, NULL, '800000', 0)")
        .bind(sid).bind(man_aid).execute(&pool).await.unwrap();

    let res = teacher_create_application(
        State(common::make_state(pool.clone())),
        Extension(common::teacher_claims(1, 1)),
        Json(CreateApplicationBody {
            student_id: sid,
            track_id: tid,
            round_id: rid,
            department_name: "컴퓨터공학과".into(),
            base_data_entries: vec![
                BaseDataEntry { area_id: num_aid, values: vec!["35".into()] },
                BaseDataEntry { area_id: cat_aid, values: vec!["회장".into()] },
            ],
        }),
    )
    .await;
    assert_eq!(res.unwrap(), StatusCode::CREATED);

    // 지원 등록 확인
    let dept: String = sqlx::query_scalar(
        "SELECT department_name FROM applications WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(sid).bind(tid).bind(rid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(dept, "컴퓨터공학과");

    // NUMERIC 기초데이터 확인 (35 → 3500000)
    let num_val: String = sqlx::query_scalar(
        "SELECT value FROM base_data WHERE student_id = ? AND area_id = ?",
    )
    .bind(sid).bind(num_aid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(num_val, "3500000");

    // CATEGORY 기초데이터 확인
    let cat_val: String = sqlx::query_scalar(
        "SELECT value FROM base_data WHERE student_id = ? AND area_id = ?",
    )
    .bind(sid).bind(cat_aid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(cat_val, "회장");
}

#[tokio::test]
async fn create_application_non_editable_area_rejected() {
    let pool = common::create_test_pool_shared().await;
    let (sid, tid, rid, _, _, man_aid, _) = setup_base(&pool).await;

    // teacher_editable=0인 면접점수 전형요소에 값 입력 시도
    let res = teacher_create_application(
        State(common::make_state(pool)),
        Extension(common::teacher_claims(1, 1)),
        Json(CreateApplicationBody {
            student_id: sid,
            track_id: tid,
            round_id: rid,
            department_name: "전자공학과".into(),
            base_data_entries: vec![
                BaseDataEntry { area_id: man_aid, values: vec!["8.5".into()] },
            ],
        }),
    )
    .await;
    assert_eq!(res.unwrap_err().0, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn create_application_base_data_overwritten_on_resave() {
    let pool = common::create_test_pool_shared().await;
    let (sid, tid, rid, num_aid, cat_aid, man_aid, _) = setup_base(&pool).await;

    // 관리자가 사전에 업로드한 면접점수 데이터 (teacher_editable=0)
    sqlx::query("INSERT INTO base_data (student_id, area_id, track_id, value, multi_value) VALUES (?, ?, NULL, '800000', 0)")
        .bind(sid).bind(man_aid).execute(&pool).await.unwrap();

    // 첫 번째 저장
    teacher_create_application(
        State(common::make_state(pool.clone())),
        Extension(common::teacher_claims(1, 1)),
        Json(CreateApplicationBody {
            student_id: sid, track_id: tid, round_id: rid,
            department_name: "컴퓨터공학과".into(),
            base_data_entries: vec![
                BaseDataEntry { area_id: num_aid, values: vec!["30".into()] },
                BaseDataEntry { area_id: cat_aid, values: vec!["회장".into()] },
            ],
        }),
    )
    .await
    .unwrap();

    // 두 번째 저장 (값 변경)
    teacher_create_application(
        State(common::make_state(pool.clone())),
        Extension(common::teacher_claims(1, 1)),
        Json(CreateApplicationBody {
            student_id: sid, track_id: tid, round_id: rid,
            department_name: "전자공학과".into(),
            base_data_entries: vec![
                BaseDataEntry { area_id: num_aid, values: vec!["45".into()] },
                BaseDataEntry { area_id: cat_aid, values: vec!["부회장".into()] },
            ],
        }),
    )
    .await
    .unwrap();

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM base_data WHERE student_id = ? AND area_id = ?")
            .bind(sid).bind(num_aid)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1, "단일값 덮어쓰기 후 행 수는 1이어야 함");

    let val: String = sqlx::query_scalar(
        "SELECT value FROM base_data WHERE student_id = ? AND area_id = ?",
    )
    .bind(sid).bind(num_aid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(val, "4500000");
}

// ── MANUAL 만점 초과 검증 ─────────────────────────────────────────

/// MANUAL 전형요소 하나만 있는 단순 환경: (sid, tid, rid, man_editable_aid)
async fn setup_manual_only(pool: &sqlx::SqlitePool, max_score_display: &str) -> (i64, i64, i64, i64) {
    let hash = bcrypt::hash("pass", 4u32).unwrap();
    sqlx::query("INSERT INTO classes (grade, class_no, password_hash) VALUES (1, 1, ?)")
        .bind(&hash).execute(pool).await.unwrap();

    let sid: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled)
         VALUES ('M001', '학생', 1, 1, 1, 1) RETURNING id",
    ).fetch_one(pool).await.unwrap();

    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name) VALUES ('테스트대') RETURNING id",
    ).fetch_one(pool).await.unwrap();

    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name) VALUES (?, '학과') RETURNING id",
    ).bind(uid).fetch_one(pool).await.unwrap();

    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at) VALUES ('OPEN', '2025-01-01') RETURNING id",
    ).fetch_one(pool).await.unwrap();

    // max_score를 ×100000 변환해서 저장
    let max_score_raw: i64 = max_score_display.parse::<f64>().unwrap() as i64 * 100_000;
    let man_aid: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, teacher_editable, lookup_scope)
         VALUES ('교사평가', ?, 'MANUAL', 1, 'SIMPLE') RETURNING id",
    ).bind(max_score_raw).fetch_one(pool).await.unwrap();

    (sid, tid, rid, man_aid)
}

#[tokio::test]
async fn create_application_manual_exceeds_max_score_returns_bad_request() {
    let pool = common::create_test_pool_shared().await;
    // max_score = 10점, 초과값 10.01 제출 → 400
    let (sid, tid, rid, man_aid) = setup_manual_only(&pool, "10").await;

    let res = teacher_create_application(
        State(common::make_state(pool)),
        Extension(common::teacher_claims(1, 1)),
        Json(CreateApplicationBody {
            student_id: sid, track_id: tid, round_id: rid,
            department_name: "학과명".into(),
            base_data_entries: vec![
                BaseDataEntry { area_id: man_aid, values: vec!["10.01".into()] },
            ],
        }),
    ).await;
    assert_eq!(res.unwrap_err().0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_application_manual_at_max_score_is_accepted() {
    let pool = common::create_test_pool_shared().await;
    // max_score = 10점, 정확히 10 제출 → 201
    let (sid, tid, rid, man_aid) = setup_manual_only(&pool, "10").await;

    let res = teacher_create_application(
        State(common::make_state(pool)),
        Extension(common::teacher_claims(1, 1)),
        Json(CreateApplicationBody {
            student_id: sid, track_id: tid, round_id: rid,
            department_name: "학과명".into(),
            base_data_entries: vec![
                BaseDataEntry { area_id: man_aid, values: vec!["10".into()] },
            ],
        }),
    ).await;
    assert_eq!(res.unwrap(), StatusCode::CREATED);
}
