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
async fn area_context_includes_unit() {
    // unit "시간"으로 등록된 전형요소가 teacher_area_context 응답에 unit 포함
    let pool = common::create_test_pool_shared().await;
    let (sid, tid, _, _, _, _, _) = setup_base(&pool).await;

    // unit이 있는 NUMERIC 전형요소 추가
    sqlx::query(
        "INSERT INTO areas (name, max_score, calc_type, teacher_editable, lookup_scope, match_mode, unit)
         VALUES ('봉사시간_단위', 500000, 'NUMERIC', 1, 'SIMPLE', 'UPPER', '시간') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let Json(items) = teacher_area_context(
        State(common::make_state(pool)),
        Extension(common::teacher_claims(1, 1)),
        Query(AreaContextQuery { student_id: sid, track_id: tid }),
    )
    .await
    .unwrap();

    let area = items.iter().find(|i| i.area_name == "봉사시간_단위").unwrap();
    assert_eq!(area.unit.as_deref(), Some("시간"));
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

/// COMPOSITE + CATEGORY: track별 표에 일부 범주만 있고 나머지 범주는 공통 표에만 있는 경우.
/// 미리보기는 확정 계산(scoring.rs:232-249)과 동일하게 **범주 단위**로 공통 표에 폴백해야 한다.
/// 수정 전에는 "표 전체가 비었을 때만" 폴백해서, 이 시나리오에서 Y가 missing으로 잡히고
/// "점수표에 없는 범주" 오류가 났다. 저장을 강행하면 확정 계산은 성공하므로 미리보기와
/// 확정이 갈라졌다.
#[tokio::test]
async fn score_preview_category_falls_back_per_category_matching_confirm_calc() {
    let pool = common::create_test_pool_shared().await;
    setup_base(&pool).await;
    let tid: i64 = sqlx::query_scalar("SELECT id FROM univ_tracks LIMIT 1")
        .fetch_one(&pool).await.unwrap();

    // 새 COMPOSITE CATEGORY area 생성 (setup_base 것은 SIMPLE이라 폴백 경로 자체가 없음)
    let comp_aid: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, teacher_editable, lookup_scope, category_agg)
         VALUES ('COMP', 1000000, 'CATEGORY', 1, 'COMPOSITE', 'SUM') RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();

    // track별 표: X만
    sqlx::query("INSERT INTO category_map (area_id, track_id, category, score) VALUES (?, ?, 'X', 100000)")
        .bind(comp_aid).bind(tid).execute(&pool).await.unwrap();
    // 공통 표: Y만
    sqlx::query("INSERT INTO category_map (area_id, track_id, category, score) VALUES (?, NULL, 'Y', 200000)")
        .bind(comp_aid).execute(&pool).await.unwrap();

    let resp = teacher_area_score_preview(
        State(common::make_state(pool)),
        Json(AreaScorePreviewBody {
            area_id: comp_aid,
            track_id: tid,
            values: vec!["X".into(), "Y".into()],
        }),
    )
    .await.unwrap().0;

    assert!(resp.error.is_none(), "per-category 폴백이 없어 오류가 났다: {:?}", resp.error);
    assert_eq!(resp.score.unwrap().raw(), 300000, "X(track 100000) + Y(공통 폴백 200000) = 300000");
}

/// 등가성 계약: 미리보기 API와 확정 저장(teacher_create_application이 트리거하는
/// calc_area_score)이 **동일한 입력에 대해 정확히 같은 점수**를 산출해야 한다.
///
/// 이 테스트는 두 경로가 공용 헬퍼 `compute_area_score`를 통과한다는 구조적 계약을
/// 결과값 수준에서 강제한다. 누군가 어느 한쪽에서만 로직을 재구현하면 이 테스트가 깨진다.
/// 시나리오는 폴백이 실제로 발생하도록 track별·공통 표를 섞어 배치한다.
#[tokio::test]
async fn preview_and_confirmed_produce_identical_score_category_composite() {
    let pool = common::create_test_pool_shared().await;
    let (sid, _setup_tid, rid, num_aid, cat_aid, man_aid, _) = setup_base(&pool).await;

    // MANUAL(teacher_editable=0)은 관리자 사전 업로드가 필요하다.
    sqlx::query("INSERT INTO base_data (student_id, area_id, track_id, value, multi_value) VALUES (?, ?, NULL, '500000', 0)")
        .bind(sid).bind(man_aid).execute(&pool).await.unwrap();

    // 별도 트랙 (setup_base의 tid는 재사용하지 않아 category_map이 깨끗)
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name) VALUES ('등가대') RETURNING id",
    ).fetch_one(&pool).await.unwrap();
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name) VALUES (?, '자연') RETURNING id",
    ).bind(uid).fetch_one(&pool).await.unwrap();

    // COMPOSITE + CATEGORY + SUM area
    let aid: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, teacher_editable, lookup_scope, category_agg)
         VALUES ('상장', 1000000, 'CATEGORY', 1, 'COMPOSITE', 'SUM') RETURNING id",
    ).fetch_one(&pool).await.unwrap();

    // track별 표: 'A'만 (기존 setup_base tid도 배제해야 폴백 발생)
    sqlx::query("INSERT INTO category_map (area_id, track_id, category, score) VALUES (?, ?, 'A', 100000)")
        .bind(aid).bind(tid).execute(&pool).await.unwrap();
    // 공통 표: 'B'만
    sqlx::query("INSERT INTO category_map (area_id, track_id, category, score) VALUES (?, NULL, 'B', 200000)")
        .bind(aid).execute(&pool).await.unwrap();

    let values = vec!["A".to_string(), "B".to_string()];

    // (1) 미리보기 점수
    let preview_resp = teacher_area_score_preview(
        State(common::make_state(pool.clone())),
        Json(AreaScorePreviewBody {
            area_id: aid, track_id: tid, values: values.clone(),
        }),
    ).await.unwrap().0;
    assert!(preview_resp.error.is_none(), "미리보기 오류: {:?}", preview_resp.error);
    let preview_score = preview_resp.score.unwrap().raw();

    // (2) 확정 저장 (teacher_create_application 안에서 calc_area_score 호출됨)
    teacher_create_application(
        State(common::make_state(pool.clone())),
        Extension(common::teacher_claims(1, 1)),
        Json(CreateApplicationBody {
            student_id: sid, track_id: tid, round_id: rid,
            department_name: "학과".into(),
            base_data_entries: vec![
                BaseDataEntry { area_id: num_aid, values: vec!["30".into()] },
                BaseDataEntry { area_id: cat_aid, values: vec!["회장".into()] },
                BaseDataEntry { area_id: aid, values },
            ],
            ..Default::default()
        }),
    ).await.unwrap();

    let score_detail: String = sqlx::query_scalar(
        "SELECT score_detail FROM results WHERE student_id = ? AND track_id = ? AND round_id = ?",
    ).bind(sid).bind(tid).bind(rid).fetch_one(&pool).await.unwrap();

    let detail: serde_json::Value = serde_json::from_str(&score_detail).unwrap();
    // applications.rs:994의 detail은 HashMap<String, i64>로 raw i64 저장 (Score newtype 아님).
    let confirmed_score = detail[aid.to_string()]
        .as_i64()
        .expect("score_detail에서 이 area의 i64 점수를 읽지 못함");

    assert_eq!(
        preview_score, confirmed_score,
        "미리보기({})와 확정 저장({})이 갈라졌다 — compute_area_score 공용 헬퍼 계약 위반",
        preview_score, confirmed_score,
    );
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

// ── matched_keys: LOWER / EXACT 분기 ──────────────────────────────
//
// 하이라이팅 행은 `scoring.rs::find_numeric_matched_threshold` 가 정하는데,
// 이 함수는 `lookup_range_score` 와 **대칭이지만 별개**다 — 점수는 맞는데 표에서
// 짚어주는 행만 어긋나는 회귀가 성립한다. 지금까지 UPPER 한 갈래만 단언돼 있어
// LOWER(폴백 포함)·EXACT 두 갈래는 통째로 미검증이었다.

/// NUMERIC 전형요소 하나를 점수표와 함께 추가한다. rows 는 (threshold, score) 원시 정수.
async fn add_numeric_area(
    pool: &sqlx::SqlitePool,
    name: &str,
    match_mode: &str,
    max_score: i64,
    rows: &[(i64, i64)],
) -> i64 {
    let aid: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, teacher_editable, lookup_scope, match_mode)
         VALUES (?, ?, 'NUMERIC', 1, 'SIMPLE', ?) RETURNING id",
    )
    .bind(name)
    .bind(max_score)
    .bind(match_mode)
    .fetch_one(pool)
    .await
    .unwrap();
    for (th, sc) in rows {
        sqlx::query("INSERT INTO numeric_table (area_id, track_id, threshold, score) VALUES (?, NULL, ?, ?)")
            .bind(aid)
            .bind(th)
            .bind(sc)
            .execute(pool)
            .await
            .unwrap();
    }
    aid
}

/// 결석일수 구간표 — LOWER(threshold = 허용 상한): 0일 5점 / 1일 4점 / 3일 3점 / 5일 2점
const ABSENCE_ROWS: [(i64, i64); 4] = [
    (0, 500_000),
    (100_000, 400_000),
    (300_000, 300_000),
    (500_000, 200_000),
];

#[tokio::test]
async fn score_preview_numeric_lower_matched_key_is_min_threshold_at_or_above_value() {
    let pool = common::create_test_pool_shared().await;
    setup_base(&pool).await;
    let tid: i64 = sqlx::query_scalar("SELECT id FROM univ_tracks LIMIT 1")
        .fetch_one(&pool).await.unwrap();
    let area_id = add_numeric_area(&pool, "결석일수", "LOWER", 500_000, &ABSENCE_ROWS).await;

    // 2일 → 값 이상인 threshold 중 최소(3일) 행이 적용 → 3점, 하이라이팅 키도 3.0
    let resp = teacher_area_score_preview(
        State(common::make_state(pool)),
        Json(AreaScorePreviewBody { area_id, track_id: tid, values: vec!["2".into()] }),
    ).await.unwrap().0;

    assert_eq!(resp.error, None);
    assert_eq!(resp.score.unwrap().raw(), 300_000, "2일 → 3일 구간의 3점");
    assert_eq!(resp.matched_keys.len(), 1, "키는 하나: {:?}", resp.matched_keys);
    assert_eq!(
        resp.matched_keys[0].as_f64(), Some(3.0),
        "하이라이팅 행은 실제 적용된 3일 구간이어야 함: {:?}", resp.matched_keys,
    );
}

#[tokio::test]
async fn score_preview_numeric_lower_above_max_threshold_matched_key_falls_back_to_max() {
    let pool = common::create_test_pool_shared().await;
    setup_base(&pool).await;
    let tid: i64 = sqlx::query_scalar("SELECT id FROM univ_tracks LIMIT 1")
        .fetch_one(&pool).await.unwrap();
    let area_id = add_numeric_area(&pool, "결석일수", "LOWER", 500_000, &ABSENCE_ROWS).await;

    // 9일 — 어떤 threshold 도 값 이상이 아니다 → 최대 threshold(5일) 행으로 폴백.
    // 점수와 하이라이팅 키가 **같은 행**을 가리켜야 한다.
    let resp = teacher_area_score_preview(
        State(common::make_state(pool)),
        Json(AreaScorePreviewBody { area_id, track_id: tid, values: vec!["9".into()] }),
    ).await.unwrap().0;

    assert_eq!(resp.error, None);
    assert_eq!(resp.score.unwrap().raw(), 200_000, "최대 구간(5일)의 2점으로 폴백");
    assert_eq!(
        resp.matched_keys.iter().filter_map(|k| k.as_f64()).collect::<Vec<_>>(),
        vec![5.0],
        "폴백 시에도 키는 최대 threshold(5일): {:?}", resp.matched_keys,
    );
}

#[tokio::test]
async fn score_preview_numeric_exact_matched_key_equals_input_value() {
    let pool = common::create_test_pool_shared().await;
    setup_base(&pool).await;
    let tid: i64 = sqlx::query_scalar("SELECT id FROM univ_tracks LIMIT 1")
        .fetch_one(&pool).await.unwrap();
    let area_id = add_numeric_area(
        &pool, "자격증급수", "EXACT", 500_000,
        &[(100_000, 100_000), (200_000, 250_000), (300_000, 400_000)],
    ).await;

    let resp = teacher_area_score_preview(
        State(common::make_state(pool)),
        Json(AreaScorePreviewBody { area_id, track_id: tid, values: vec!["2".into()] }),
    ).await.unwrap().0;

    assert_eq!(resp.error, None);
    assert_eq!(resp.score.unwrap().raw(), 250_000, "2급 행의 점수");
    assert_eq!(
        resp.matched_keys.iter().filter_map(|k| k.as_f64()).collect::<Vec<_>>(),
        vec![2.0],
        "EXACT 는 입력값과 같은 키만 짚어야 함(이웃 행 금지): {:?}", resp.matched_keys,
    );
}

#[tokio::test]
async fn score_preview_numeric_exact_miss_returns_error_and_no_key() {
    let pool = common::create_test_pool_shared().await;
    setup_base(&pool).await;
    let tid: i64 = sqlx::query_scalar("SELECT id FROM univ_tracks LIMIT 1")
        .fetch_one(&pool).await.unwrap();
    let area_id = add_numeric_area(
        &pool, "자격증급수", "EXACT", 500_000,
        &[(100_000, 100_000), (200_000, 250_000)],
    ).await;

    // 1.5급 — 등록되지 않은 값. EXACT 는 근처 행으로 조용히 떨어지면 안 된다.
    let resp = teacher_area_score_preview(
        State(common::make_state(pool)),
        Json(AreaScorePreviewBody { area_id, track_id: tid, values: vec!["1.5".into()] }),
    ).await.unwrap().0;

    assert_eq!(resp.score, None, "매칭 실패인데 점수가 나오면 안 됨");
    assert!(resp.matched_keys.is_empty(), "매칭 실패 시 하이라이팅 없음: {:?}", resp.matched_keys);
    let err = resp.error.expect("오류 메시지");
    assert!(err.contains("EXACT"), "원인이 EXACT 매칭 실패임을 밝혀야 함: {err}");
}

/// NUMERIC 은 구간표 점수가 만점을 넘을 수 있다(표가 잘못 올라온 경우).
/// 이때 캡핑 + 경고 문구가 MANUAL 과 **다른 문장**이어야 한다 —
/// 지금까지 MANUAL 갈래만 단언돼 있어 두 문구를 뒤바꿔도 통과했다.
#[tokio::test]
async fn score_preview_numeric_over_max_score_caps_and_warns_with_calculated_wording() {
    let pool = common::create_test_pool_shared().await;
    setup_base(&pool).await;
    let tid: i64 = sqlx::query_scalar("SELECT id FROM univ_tracks LIMIT 1")
        .fetch_one(&pool).await.unwrap();
    // 만점 2점인데 구간 점수는 5점
    let area_id = add_numeric_area(&pool, "초과요소", "UPPER", 200_000, &[(0, 500_000)]).await;

    let resp = teacher_area_score_preview(
        State(common::make_state(pool)),
        Json(AreaScorePreviewBody { area_id, track_id: tid, values: vec!["1".into()] }),
    ).await.unwrap().0;

    assert_eq!(resp.error, None);
    assert_eq!(resp.score.unwrap().raw(), 200_000, "만점으로 캡핑");
    let w = resp.warning.expect("만점 초과 경고");
    assert!(w.starts_with("계산된 점수가"), "NUMERIC 은 '계산된 점수' 문구: {w}");
    assert!(!w.contains("입력값"), "MANUAL 문구가 새면 안 됨: {w}");
}

// ── 미리보기 입력 검증 ─────────────────────────────────────────────

#[tokio::test]
async fn score_preview_empty_values_returns_error_without_score() {
    let pool = common::create_test_pool_shared().await;
    setup_base(&pool).await;
    let tid: i64 = sqlx::query_scalar("SELECT id FROM univ_tracks LIMIT 1")
        .fetch_one(&pool).await.unwrap();
    let area_id = get_area_id(&pool, "봉사시간").await;

    let resp = teacher_area_score_preview(
        State(common::make_state(pool)),
        Json(AreaScorePreviewBody { area_id, track_id: tid, values: vec![] }),
    ).await.unwrap().0;

    // 빈 입력을 0점으로 처리하면 담임 화면에 "0점"이 확정처럼 표시된다 — 반드시 오류
    assert_eq!(resp.score, None, "빈 입력에 점수를 매기면 안 됨");
    assert!(resp.matched_keys.is_empty());
    assert_eq!(resp.error.as_deref(), Some("값이 입력되지 않았습니다"));
}

#[tokio::test]
async fn score_preview_unknown_area_returns_404() {
    let pool = common::create_test_pool_shared().await;
    setup_base(&pool).await;
    let tid: i64 = sqlx::query_scalar("SELECT id FROM univ_tracks LIMIT 1")
        .fetch_one(&pool).await.unwrap();

    let res = teacher_area_score_preview(
        State(common::make_state(pool)),
        Json(AreaScorePreviewBody { area_id: 9999, track_id: tid, values: vec!["1".into()] }),
    ).await;
    // 없는 전형요소를 "값 없음" 미리보기(200)로 흘리면 담임 화면에 원인이 안 뜬다
    let err = match res {
        Ok(_) => panic!("없는 전형요소인데 성공 응답이 나왔다"),
        Err(e) => e,
    };

    assert_eq!(err.0, StatusCode::NOT_FOUND, "없는 전형요소는 404");
    assert!(err.1.contains("9999"), "어느 id 가 없는지 밝혀야 함: {}", err.1);
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
        }),
    ).await;
    assert_eq!(res.unwrap(), StatusCode::CREATED);
}
