//! 이슈 #50 — 졸업생 담당은 '관리자 입력 고정'(teacher_editable = 0) 전형요소도 입력한다.
//!
//! 소유자 결정: 관리자 값과 같은 `base_data` 행이라 저장하면 덮어쓴다(마지막 저장이 이긴다).
//! 필수 제출 집합은 teacher_editable 기준 그대로다.
//!
//! 판별력의 소재: 고정 항목만 둔 픽스처로 저장 관문(`auth::can_teacher_edit`)을 직접 누른다.
//! 일반 담임 대조군(B3)이 같은 요청에서 403 을 받아야 관문이 졸업생에게만 열렸음이 드러난다.
//! MANUAL 항목을 쓰므로 입력값이 곧 점수다 — 저장된 값과 `results.total_score` 를 함께 본다.
mod common;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    Extension, Json,
};
use principal_candidate_manager::handlers::applications::{
    teacher_create_application, BaseDataEntry, CreateApplicationBody,
};
use principal_candidate_manager::handlers::teacher_areas::{teacher_area_context, AreaContextQuery};

struct Fx {
    grad_sid: i64,
    enrolled_sid: i64,
    track_a: i64,
    track_b: i64,
    round: i64,
    simple: i64,
    composite: i64,
}

/// 졸업생 1명·재학생 1명(1학년 1반), 모집단위 둘, OPEN 라운드, 고정 MANUAL 항목 둘
/// (SIMPLE·COMPOSITE). 담임 입력 항목은 두지 않는다 — 필수 제출 검사가 끼어들지 않게.
async fn fixture(pool: &sqlx::SqlitePool) -> Fx {
    common::insert_class(pool, 1, 1).await;
    let id = |sql: &'static str| async move {
        sqlx::query_scalar::<_, i64>(sql).fetch_one(pool).await.unwrap()
    };
    let grad_sid = id("INSERT INTO students (student_code, name, is_enrolled, grad_year) \
                       VALUES ('G001', '졸업생', 0, 2024) RETURNING id").await;
    let enrolled_sid = id("INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
                           VALUES ('S001', '재학생', 1, 1, 1, 1) RETURNING id").await;
    let univ = id("INSERT INTO universities (univ_name) VALUES ('한국대') RETURNING id").await;
    let track_a: i64 = sqlx::query_scalar("INSERT INTO univ_tracks (univ_id, track_name, unit_quota) VALUES (?, '가', 5) RETURNING id")
        .bind(univ).fetch_one(pool).await.unwrap();
    let track_b: i64 = sqlx::query_scalar("INSERT INTO univ_tracks (univ_id, track_name, unit_quota) VALUES (?, '나', 5) RETURNING id")
        .bind(univ).fetch_one(pool).await.unwrap();
    let round = id("INSERT INTO rounds (status, opened_at) VALUES ('OPEN', '2025-01-01T00:00:00Z') RETURNING id").await;
    let simple = id("INSERT INTO areas (name, max_score, calc_type, teacher_editable, lookup_scope) \
                     VALUES ('내신', 1000000, 'MANUAL', 0, 'SIMPLE') RETURNING id").await;
    let composite = id("INSERT INTO areas (name, max_score, calc_type, teacher_editable, lookup_scope) \
                        VALUES ('면접', 1000000, 'MANUAL', 0, 'COMPOSITE') RETURNING id").await;
    Fx { grad_sid, enrolled_sid, track_a, track_b, round, simple, composite }
}

async fn admin_value(pool: &sqlx::SqlitePool, sid: i64, area: i64, track: Option<i64>, raw: &str) {
    sqlx::query("INSERT INTO base_data (student_id, area_id, track_id, value, multi_value) VALUES (?, ?, ?, ?, 0)")
        .bind(sid).bind(area).bind(track).bind(raw)
        .execute(pool).await.unwrap();
}

async fn stored(pool: &sqlx::SqlitePool, sid: i64, area: i64, track: Option<i64>) -> Option<String> {
    sqlx::query_scalar(
        "SELECT value FROM base_data WHERE student_id = ? AND area_id = ? \
         AND (track_id = ? OR (? IS NULL AND track_id IS NULL))",
    )
    .bind(sid).bind(area).bind(track).bind(track)
    .fetch_optional(pool).await.unwrap()
}

async fn total(pool: &sqlx::SqlitePool, sid: i64, track: i64, round: i64) -> i64 {
    sqlx::query_scalar("SELECT total_score FROM results WHERE student_id = ? AND track_id = ? AND round_id = ?")
        .bind(sid).bind(track).bind(round)
        .fetch_one(pool).await.unwrap()
}

async fn save(
    pool: &sqlx::SqlitePool,
    grade: i64,
    class_no: i64,
    sid: i64,
    track: i64,
    round: i64,
    entries: Vec<(i64, &str)>,
) -> Result<StatusCode, (StatusCode, String)> {
    teacher_create_application(
        State(common::make_state(pool.clone())),
        Extension(common::teacher_claims(grade, class_no)),
        Json(CreateApplicationBody {
            student_id: sid,
            track_id: track,
            round_id: round,
            department_name: "학과".into(),
            base_data_entries: entries
                .into_iter()
                .map(|(area_id, v)| BaseDataEntry { area_id, values: vec![v.into()] })
                .collect(),
            ..Default::default()
        }),
    )
    .await
}

// B1. 판별력: 저장 관문이 teacher_editable 만 보면 403 이 난다.
#[tokio::test]
async fn b1_graduate_manager_can_enter_admin_fixed_area() {
    let pool = common::create_test_pool().await;
    let f = fixture(&pool).await;
    let res = save(&pool, 0, 0, f.grad_sid, f.track_a, f.round,
                   vec![(f.simple, "3.5"), (f.composite, "2")]).await;
    assert_eq!(res.unwrap(), StatusCode::CREATED);
    assert_eq!(stored(&pool, f.grad_sid, f.simple, None).await.as_deref(), Some("350000"));
    assert_eq!(total(&pool, f.grad_sid, f.track_a, f.round).await, 550000);
}

// B2. 덮어쓰기 결정 고정: 관리자 값이 있어도 졸업생 담당 값이 이긴다.
#[tokio::test]
async fn b2_graduate_manager_overwrites_admin_value() {
    let pool = common::create_test_pool().await;
    let f = fixture(&pool).await;
    admin_value(&pool, f.grad_sid, f.simple, None, "900000").await;
    admin_value(&pool, f.grad_sid, f.composite, Some(f.track_a), "100000").await;
    let res = save(&pool, 0, 0, f.grad_sid, f.track_a, f.round, vec![(f.simple, "3.5")]).await;
    assert_eq!(res.unwrap(), StatusCode::CREATED);
    assert_eq!(stored(&pool, f.grad_sid, f.simple, None).await.as_deref(), Some("350000"));
    assert_eq!(total(&pool, f.grad_sid, f.track_a, f.round).await, 450000);
}

// B3. 대조군: 일반 담임은 같은 고정 항목을 여전히 못 넣는다. 관문을 늘 열면 여기서 걸린다.
#[tokio::test]
async fn b3_homeroom_teacher_still_forbidden_on_fixed_area() {
    let pool = common::create_test_pool().await;
    let f = fixture(&pool).await;
    let res = save(&pool, 1, 1, f.enrolled_sid, f.track_a, f.round, vec![(f.simple, "3.5")]).await;
    assert_eq!(res.unwrap_err().0, StatusCode::FORBIDDEN);
    assert_eq!(stored(&pool, f.enrolled_sid, f.simple, None).await, None);
}

// B4. 결정 기록(방어선 아님): 필수 제출 집합은 넓히지 않았다 — 고정 항목을 빼고 내면
//     관리자 값으로 계산된다.
#[tokio::test]
async fn b4_omitted_fixed_area_uses_admin_value() {
    let pool = common::create_test_pool().await;
    let f = fixture(&pool).await;
    admin_value(&pool, f.grad_sid, f.simple, None, "900000").await;
    admin_value(&pool, f.grad_sid, f.composite, Some(f.track_a), "100000").await;
    let res = save(&pool, 0, 0, f.grad_sid, f.track_a, f.round, vec![]).await;
    assert_eq!(res.unwrap(), StatusCode::CREATED);
    assert_eq!(total(&pool, f.grad_sid, f.track_a, f.round).await, 1000000);
}

async fn context(pool: &sqlx::SqlitePool, grade: i64, class_no: i64, sid: i64, track: i64) -> Vec<serde_json::Value> {
    let axum::Json(items) = teacher_area_context(
        State(common::make_state(pool.clone())),
        Extension(common::teacher_claims(grade, class_no)),
        Query(AreaContextQuery { student_id: sid, track_id: track }),
    )
    .await
    .unwrap();
    items.iter().map(|i| serde_json::to_value(i).unwrap()).collect()
}

// B5. 판별력: `editable` 이 teacher_editable 를 그대로 옮기면 졸업생 담당 쪽이 false 가 된다.
//     `teacher_editable` 은 설정값 그대로여야 한다(의미를 바꾸지 않는다).
#[tokio::test]
async fn b5_context_editable_follows_account_teacher_editable_stays_setting() {
    let pool = common::create_test_pool().await;
    let f = fixture(&pool).await;
    for v in context(&pool, 0, 0, f.grad_sid, f.track_a).await {
        assert_eq!(v["editable"], serde_json::json!(true), "{v}");
        assert_eq!(v["teacher_editable"], serde_json::json!(false), "{v}");
    }
    for v in context(&pool, 1, 1, f.enrolled_sid, f.track_a).await {
        assert_eq!(v["editable"], serde_json::json!(false), "{v}");
        assert_eq!(v["teacher_editable"], serde_json::json!(false), "{v}");
    }
}

// B7. COMPOSITE 고정 항목은 모집단위별 행이다 — A 저장이 B 의 관리자 값을 바꾸지 않는다.
#[tokio::test]
async fn b7_composite_overwrite_is_scoped_to_track() {
    let pool = common::create_test_pool().await;
    let f = fixture(&pool).await;
    admin_value(&pool, f.grad_sid, f.composite, Some(f.track_b), "700000").await;
    let res = save(&pool, 0, 0, f.grad_sid, f.track_a, f.round,
                   vec![(f.simple, "1"), (f.composite, "2")]).await;
    assert_eq!(res.unwrap(), StatusCode::CREATED);
    assert_eq!(stored(&pool, f.grad_sid, f.composite, Some(f.track_a)).await.as_deref(), Some("200000"));
    assert_eq!(stored(&pool, f.grad_sid, f.composite, Some(f.track_b)).await.as_deref(), Some("700000"));
}

// B8. 기록(방어선 아님): SIMPLE 항목은 학생당 한 행이라 모집단위 간에 공유된다 —
//     A 에서 저장한 값이 B 컨텍스트의 current_values 에도 보인다.
#[tokio::test]
async fn b8_simple_value_is_shared_across_tracks() {
    let pool = common::create_test_pool().await;
    let f = fixture(&pool).await;
    let res = save(&pool, 0, 0, f.grad_sid, f.track_a, f.round,
                   vec![(f.simple, "3.5"), (f.composite, "2")]).await;
    assert_eq!(res.unwrap(), StatusCode::CREATED);
    let ctx_b = context(&pool, 0, 0, f.grad_sid, f.track_b).await;
    let simple_b = ctx_b.iter().find(|v| v["area_id"] == serde_json::json!(f.simple)).unwrap();
    assert_eq!(simple_b["current_values"], serde_json::json!(["3.5"]), "{simple_b}");
}
