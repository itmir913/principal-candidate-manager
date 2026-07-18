//! F단계: 추천 제외(결격) 상태.
//!
//! 존재 이유(핵심): CLOSED 라운드에서 상위 순위자를 결격 등으로 건너뛰어야 하는데
//! abandon_application 은 FINALIZED 전용이라 CLOSED 에서는 상위자를 건너뛸 정당한 수단이
//! 없었다. 제외 처리는 E2(수동 추천 트랙 순서 가드)의 블로커 집합에서 대상자를 뺀다.
//! 정원 집계(used·univ_used)는 recommended=1 기준이라 제외자(recommended=0)는 원래도
//! 집계에 없으므로 건드리지 않는다 — 이 파일의 정원 불변 테스트가 그 계약을 확인한다.

mod common;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use principal_candidate_manager::handlers::applications::{
    clear_application_exclusion, exclude_application, ExcludeApplicationBody,
};
use principal_candidate_manager::handlers::scoring::{auto_recommend_results, recommend_result};
use sqlx::SqlitePool;

// ── 픽스처 ───────────────────────────────────────────────────────

async fn setup(pool: &SqlitePool) -> (i64, i64) {
    common::insert_class(pool, 1, 1).await;
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name) VALUES ('한국대') RETURNING id",
    )
    .fetch_one(pool).await.unwrap();
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name) VALUES (?, '컴공') RETURNING id",
    )
    .bind(uid).fetch_one(pool).await.unwrap();
    (tid, 0)
}

async fn new_round(pool: &SqlitePool, status: &str) -> i64 {
    match status {
        "OPEN" => sqlx::query_scalar(
            "INSERT INTO rounds (status, opened_at) VALUES ('OPEN', '2025-01-01T00:00:00Z') RETURNING id",
        ).fetch_one(pool).await.unwrap(),
        "CLOSED" => sqlx::query_scalar(
            "INSERT INTO rounds (status, opened_at, closed_at) \
             VALUES ('CLOSED', '2025-01-01T00:00:00Z', '2025-01-02T00:00:00Z') RETURNING id",
        ).fetch_one(pool).await.unwrap(),
        "FINALIZED" => sqlx::query_scalar(
            "INSERT INTO rounds (status, opened_at, closed_at, finalized_at) \
             VALUES ('FINALIZED', '2025-01-01T00:00:00Z', '2025-01-02T00:00:00Z', \
                     '2025-01-03T00:00:00Z') RETURNING id",
        ).fetch_one(pool).await.unwrap(),
        _ => unreachable!(),
    }
}

async fn new_student(pool: &SqlitePool, code: &str, seq: i64) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES (?, ?, 1, 1, ?, 1) RETURNING id",
    )
    .bind(code).bind(code).bind(seq)
    .fetch_one(pool).await.unwrap()
}

/// 지원 + 결과 행 생성. 정원 집계 계약 확인을 위해 recommended/score 를 지정할 수 있다.
async fn new_candidate(
    pool: &SqlitePool,
    tid: i64,
    rid: i64,
    code: &str,
    seq: i64,
    score: i64,
    ranking: i64,
    recommended: bool,
) -> i64 {
    let sid = new_student(pool, code, seq).await;
    sqlx::query(
        "INSERT INTO applications (student_id, track_id, round_id, department_name) \
         VALUES (?, ?, ?, '학과')",
    )
    .bind(sid).bind(tid).bind(rid)
    .execute(pool).await.unwrap();
    sqlx::query(
        "INSERT INTO results \
         (student_id, track_id, round_id, score_detail, total_score, ranking, recommended, calculated_at) \
         VALUES (?, ?, ?, '{}', ?, ?, ?, '2025-01-02T00:00:00Z')",
    )
    .bind(sid).bind(tid).bind(rid).bind(score).bind(ranking).bind(recommended as i64)
    .execute(pool).await.unwrap();
    sid
}

async fn exclude(pool: &SqlitePool, sid: i64, tid: i64, rid: i64, reason: &str) -> Result<StatusCode, (StatusCode, String)> {
    exclude_application(
        State(common::make_state(pool.clone())),
        Path((sid, tid, rid)),
        Json(ExcludeApplicationBody { reason: reason.to_string() }),
    ).await
}

async fn clear(pool: &SqlitePool, sid: i64, tid: i64, rid: i64) -> Result<StatusCode, (StatusCode, String)> {
    clear_application_exclusion(State(common::make_state(pool.clone())), Path((sid, tid, rid))).await
}

async fn recommend(pool: &SqlitePool, sid: i64, tid: i64, rid: i64) -> Result<StatusCode, (StatusCode, String)> {
    recommend_result(State(common::make_state(pool.clone())), Path((sid, tid, rid))).await
}

async fn is_excluded(pool: &SqlitePool, sid: i64, tid: i64, rid: i64) -> bool {
    sqlx::query_scalar::<_, i64>(
        "SELECT excluded FROM applications WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(sid).bind(tid).bind(rid)
    .fetch_one(pool).await.unwrap() == 1
}

// ── 1. 정상 경로 ─────────────────────────────────────────────────

#[tokio::test]
async fn exclude_then_clear_round_trip() {
    let pool = common::create_test_pool().await;
    let (tid, _) = setup(&pool).await;
    let rid = new_round(&pool, "CLOSED").await;
    let sid = new_candidate(&pool, tid, rid, "S001", 1, 500000, 1, false).await;

    assert_eq!(exclude(&pool, sid, tid, rid, "서류 미비").await.unwrap(), StatusCode::NO_CONTENT);
    assert!(is_excluded(&pool, sid, tid, rid).await);
    let reason: Option<String> = sqlx::query_scalar(
        "SELECT excluded_reason FROM applications WHERE student_id = ? AND track_id = ? AND round_id = ?",
    ).bind(sid).bind(tid).bind(rid).fetch_one(&pool).await.unwrap();
    assert_eq!(reason.as_deref(), Some("서류 미비"));

    assert_eq!(clear(&pool, sid, tid, rid).await.unwrap(), StatusCode::NO_CONTENT);
    assert!(!is_excluded(&pool, sid, tid, rid).await);
    let reason: Option<String> = sqlx::query_scalar(
        "SELECT excluded_reason FROM applications WHERE student_id = ? AND track_id = ? AND round_id = ?",
    ).bind(sid).bind(tid).bind(rid).fetch_one(&pool).await.unwrap();
    assert_eq!(reason, None);
}

#[tokio::test]
async fn exclude_requires_nonblank_reason() {
    let pool = common::create_test_pool().await;
    let (tid, _) = setup(&pool).await;
    let rid = new_round(&pool, "CLOSED").await;
    let sid = new_candidate(&pool, tid, rid, "S001", 1, 500000, 1, false).await;

    let err = exclude(&pool, sid, tid, rid, "   ").await.unwrap_err();
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(!is_excluded(&pool, sid, tid, rid).await);
}

#[tokio::test]
async fn exclude_twice_is_conflict() {
    let pool = common::create_test_pool().await;
    let (tid, _) = setup(&pool).await;
    let rid = new_round(&pool, "CLOSED").await;
    let sid = new_candidate(&pool, tid, rid, "S001", 1, 500000, 1, false).await;

    assert_eq!(exclude(&pool, sid, tid, rid, "사유1").await.unwrap(), StatusCode::NO_CONTENT);
    let err = exclude(&pool, sid, tid, rid, "사유2").await.unwrap_err();
    assert_eq!(err.0, StatusCode::CONFLICT);
}

#[tokio::test]
async fn clear_when_not_excluded_is_conflict() {
    let pool = common::create_test_pool().await;
    let (tid, _) = setup(&pool).await;
    let rid = new_round(&pool, "CLOSED").await;
    let sid = new_candidate(&pool, tid, rid, "S001", 1, 500000, 1, false).await;

    let err = clear(&pool, sid, tid, rid).await.unwrap_err();
    assert_eq!(err.0, StatusCode::CONFLICT);
}

#[tokio::test]
async fn exclude_missing_application_is_not_found() {
    let pool = common::create_test_pool().await;
    let (tid, _) = setup(&pool).await;
    let rid = new_round(&pool, "CLOSED").await;

    let err = exclude(&pool, 9999, tid, rid, "사유").await.unwrap_err();
    assert_eq!(err.0, StatusCode::NOT_FOUND);
}

// ── 2. 라운드 상태 가드 ──────────────────────────────────────────

#[tokio::test]
async fn exclude_rejected_in_open_round() {
    let pool = common::create_test_pool().await;
    let (tid, _) = setup(&pool).await;
    let rid = new_round(&pool, "OPEN").await;
    // OPEN 라운드는 applications UPDATE 트리거가 막지 않는 정상 지원 흐름이라 직접 INSERT
    let sid = new_student(&pool, "S001", 1).await;
    sqlx::query(
        "INSERT INTO applications (student_id, track_id, round_id, department_name) VALUES (?, ?, ?, '학과')",
    ).bind(sid).bind(tid).bind(rid).execute(&pool).await.unwrap();

    let err = exclude(&pool, sid, tid, rid, "사유").await.unwrap_err();
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn exclude_rejected_in_finalized_round() {
    let pool = common::create_test_pool().await;
    let (tid, _) = setup(&pool).await;
    let rid = new_round(&pool, "FINALIZED").await;
    let sid = new_candidate(&pool, tid, rid, "S001", 1, 500000, 1, true).await;

    let err = exclude(&pool, sid, tid, rid, "사유").await.unwrap_err();
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
}

// ── 3. DB CHECK 제약 ─────────────────────────────────────────────

#[tokio::test]
async fn db_check_rejects_excluded_without_reason() {
    let pool = common::create_test_pool().await;
    let (tid, _) = setup(&pool).await;
    let rid = new_round(&pool, "CLOSED").await;
    let sid = new_student(&pool, "S001", 1).await;

    let result = sqlx::query(
        "INSERT INTO applications (student_id, track_id, round_id, department_name, excluded, excluded_reason) \
         VALUES (?, ?, ?, '학과', 1, NULL)",
    )
    .bind(sid).bind(tid).bind(rid)
    .execute(&pool)
    .await;
    assert!(result.is_err(), "excluded=1 인데 reason NULL 은 CHECK 위반으로 거부되어야 한다");

    let result = sqlx::query(
        "INSERT INTO applications (student_id, track_id, round_id, department_name, excluded, excluded_reason) \
         VALUES (?, ?, ?, '학과', 1, '   ')",
    )
    .bind(sid).bind(tid).bind(rid)
    .execute(&pool)
    .await;
    assert!(result.is_err(), "excluded=1 인데 reason 공백 문자열도 CHECK 위반으로 거부되어야 한다");
}

// ── 4. E2 가드 해소 (핵심) ───────────────────────────────────────

#[tokio::test]
async fn excluding_top_candidate_unblocks_manual_recommend_of_second() {
    let pool = common::create_test_pool().await;
    let (tid, _) = setup(&pool).await;
    let rid = new_round(&pool, "CLOSED").await;
    let top = new_candidate(&pool, tid, rid, "S001", 1, 900000, 1, false).await;
    let low = new_candidate(&pool, tid, rid, "S002", 2, 500000, 2, false).await;

    // 제외 전: 1위가 미추천 상태라 2위 추천은 409로 막힌다 (E2 가드)
    let err = recommend(&pool, low, tid, rid).await.unwrap_err();
    assert_eq!(err.0, StatusCode::CONFLICT);

    // 1위를 결격 처리로 제외
    assert_eq!(exclude(&pool, top, tid, rid, "결격").await.unwrap(), StatusCode::NO_CONTENT);

    // 이제 2위를 추천할 수 있다 — 제외자는 블로커 집합에서 빠진다
    assert_eq!(recommend(&pool, low, tid, rid).await.unwrap(), StatusCode::NO_CONTENT);

    // 제외 해제하면 다시 막힌다 (2위 추천 취소 후 재확인)
    sqlx::query("UPDATE results SET recommended = 0 WHERE student_id = ?")
        .bind(low).execute(&pool).await.unwrap();
    assert_eq!(clear(&pool, top, tid, rid).await.unwrap(), StatusCode::NO_CONTENT);
    let err = recommend(&pool, low, tid, rid).await.unwrap_err();
    assert_eq!(err.0, StatusCode::CONFLICT);
}

// ── 5. 제외 상태 추천 불가 ───────────────────────────────────────

#[tokio::test]
async fn recommending_excluded_application_is_conflict() {
    let pool = common::create_test_pool().await;
    let (tid, _) = setup(&pool).await;
    let rid = new_round(&pool, "CLOSED").await;
    let sid = new_candidate(&pool, tid, rid, "S001", 1, 900000, 1, false).await;

    assert_eq!(exclude(&pool, sid, tid, rid, "결격").await.unwrap(), StatusCode::NO_CONTENT);
    let err = recommend(&pool, sid, tid, rid).await.unwrap_err();
    assert_eq!(err.0, StatusCode::CONFLICT);

    let rec: i64 = sqlx::query_scalar("SELECT recommended FROM results WHERE student_id = ?")
        .bind(sid).fetch_one(&pool).await.unwrap();
    assert_eq!(rec, 0);
}

// ── 6. 자동 추천이 제외자를 건너뛴다 ─────────────────────────────

#[tokio::test]
async fn auto_recommend_skips_excluded_candidate() {
    let pool = common::create_test_pool().await;
    let (tid, _) = setup(&pool).await;
    let rid = new_round(&pool, "CLOSED").await;
    // 정원 1, 두 후보 중 1위를 제외 → 2위가 자동 확정되어야 한다
    sqlx::query("UPDATE univ_tracks SET unit_quota = 1 WHERE id = ?").bind(tid).execute(&pool).await.unwrap();
    let top = new_candidate(&pool, tid, rid, "S001", 1, 900000, 1, false).await;
    let low = new_candidate(&pool, tid, rid, "S002", 2, 500000, 2, false).await;

    assert_eq!(exclude(&pool, top, tid, rid, "결격").await.unwrap(), StatusCode::NO_CONTENT);

    let st = common::make_state(pool.clone());
    let res = auto_recommend_results(State(st), Path(rid)).await.unwrap().0;
    assert_eq!(res.confirmed.iter().map(|c| c.count).sum::<i64>(), 1);

    let top_rec: i64 = sqlx::query_scalar("SELECT recommended FROM results WHERE student_id = ?")
        .bind(top).fetch_one(&pool).await.unwrap();
    let low_rec: i64 = sqlx::query_scalar("SELECT recommended FROM results WHERE student_id = ?")
        .bind(low).fetch_one(&pool).await.unwrap();
    assert_eq!(top_rec, 0, "제외된 1위는 자동 확정되면 안 된다");
    assert_eq!(low_rec, 1, "제외로 빈 자리는 2위가 채워야 한다");
}

// ── 7. 정원 집계 불변 ────────────────────────────────────────────

#[tokio::test]
async fn quota_count_unaffected_by_exclusion() {
    let pool = common::create_test_pool().await;
    let (tid, _) = setup(&pool).await;
    let rid = new_round(&pool, "CLOSED").await;
    let recommended_student = new_candidate(&pool, tid, rid, "S001", 1, 900000, 1, true).await;
    let excluded_student = new_candidate(&pool, tid, rid, "S002", 2, 500000, 2, false).await;

    let used_before: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM results r \
         JOIN applications a ON a.student_id = r.student_id AND a.track_id = r.track_id AND a.round_id = r.round_id \
         WHERE r.track_id = ? AND r.recommended = 1 AND a.abandoned = 0",
    ).bind(tid).fetch_one(&pool).await.unwrap();

    assert_eq!(exclude(&pool, excluded_student, tid, rid, "결격").await.unwrap(), StatusCode::NO_CONTENT);

    let used_after: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM results r \
         JOIN applications a ON a.student_id = r.student_id AND a.track_id = r.track_id AND a.round_id = r.round_id \
         WHERE r.track_id = ? AND r.recommended = 1 AND a.abandoned = 0",
    ).bind(tid).fetch_one(&pool).await.unwrap();

    assert_eq!(used_before, used_after, "제외 처리로 정원 집계가 바뀌면 안 된다(제외자는 이미 recommended=0)");
    assert_eq!(used_before, 1);
    let _ = recommended_student;
}
