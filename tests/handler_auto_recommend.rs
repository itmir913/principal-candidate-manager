mod common;

use axum::{
    extract::{Path, State},
    Json,
};
use principal_candidate_manager::handlers::scoring::{auto_recommend_results, AutoRecommendResponse};
use sqlx::SqlitePool;

// ── 헬퍼 ─────────────────────────────────────────────────────────

async fn new_univ(pool: &SqlitePool, name: &str, total_quota: Option<i64>) -> i64 {
    sqlx::query_scalar("INSERT INTO universities (univ_name, total_quota) VALUES (?, ?) RETURNING id")
        .bind(name)
        .bind(total_quota)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn new_track(pool: &SqlitePool, univ_id: i64, name: &str, unit_quota: Option<i64>) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name, unit_quota) VALUES (?, ?, ?) RETURNING id",
    )
    .bind(univ_id)
    .bind(name)
    .bind(unit_quota)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn new_student(pool: &SqlitePool, code: &str, grade: i64, class_no: i64, seq_no: i64) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES (?, ?, ?, ?, ?, 1) RETURNING id",
    )
    .bind(code)
    .bind(code)
    .bind(grade)
    .bind(class_no)
    .bind(seq_no)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn new_closed_round(pool: &SqlitePool) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at, closed_at) \
         VALUES ('CLOSED', '2025-01-01T00:00:00Z', '2025-01-02T00:00:00Z') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn new_finalized_round(pool: &SqlitePool) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at, closed_at, finalized_at) \
         VALUES ('FINALIZED', '2024-01-01T00:00:00Z', '2024-01-02T00:00:00Z', \
                 '2024-01-03T00:00:00Z') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn new_application(pool: &SqlitePool, sid: i64, tid: i64, rid: i64, abandoned: bool) {
    sqlx::query(
        "INSERT INTO applications (student_id, track_id, round_id, abandoned, department_name) \
         VALUES (?, ?, ?, ?, '학과')",
    )
    .bind(sid)
    .bind(tid)
    .bind(rid)
    .bind(abandoned as i64)
    .execute(pool)
    .await
    .unwrap();
}

async fn new_result(
    pool: &SqlitePool,
    sid: i64,
    tid: i64,
    rid: i64,
    ranking: Option<i64>,
    total_score: i64,
    recommended: bool,
) {
    sqlx::query(
        "INSERT INTO results \
         (student_id, track_id, round_id, score_detail, total_score, ranking, recommended, calculated_at) \
         VALUES (?, ?, ?, '{}', ?, ?, ?, '2025-01-02T00:00:00Z')",
    )
    .bind(sid)
    .bind(tid)
    .bind(rid)
    .bind(total_score)
    .bind(ranking)
    .bind(recommended as i64)
    .execute(pool)
    .await
    .unwrap();
}

async fn call_auto(pool: &SqlitePool, rid: i64) -> AutoRecommendResponse {
    let st = common::make_state(pool.clone());
    match auto_recommend_results(State(st), Path(rid)).await {
        Ok(Json(v)) => v,
        Err((s, msg)) => panic!("auto_recommend_results 실패: {} — {}", s, msg),
    }
}

async fn get_recommended(pool: &SqlitePool, sid: i64, tid: i64, rid: i64) -> bool {
    sqlx::query_scalar::<_, i64>(
        "SELECT recommended FROM results WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(sid)
    .bind(tid)
    .bind(rid)
    .fetch_one(pool)
    .await
    .unwrap()
        == 1
}

// ── 테스트 ───────────────────────────────────────────────────────

/// 1. 정원 2, 후보 3(순위 1,2,3) → 순위 1·2만 confirmed, 3위는 0 유지
#[tokio::test]
async fn auto_recommend_basic_quota() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let univ = new_univ(&pool, "A대", None).await;
    let tid = new_track(&pool, univ, "컴공", Some(2)).await;
    let s1 = new_student(&pool, "S1", 1, 1, 1).await;
    let s2 = new_student(&pool, "S2", 1, 1, 2).await;
    let s3 = new_student(&pool, "S3", 1, 1, 3).await;
    let rid = new_closed_round(&pool).await;
    new_application(&pool, s1, tid, rid, false).await;
    new_application(&pool, s2, tid, rid, false).await;
    new_application(&pool, s3, tid, rid, false).await;
    new_result(&pool, s1, tid, rid, Some(1), 300_000, false).await;
    new_result(&pool, s2, tid, rid, Some(2), 200_000, false).await;
    new_result(&pool, s3, tid, rid, Some(3), 100_000, false).await;

    let res = call_auto(&pool, rid).await;

    assert_eq!(res.confirmed.len(), 1);
    assert_eq!(res.confirmed[0].count, 2);
    assert_eq!(res.manual.len(), 0);
    assert!(get_recommended(&pool, s1, tid, rid).await);
    assert!(get_recommended(&pool, s2, tid, rid).await);
    assert!(!get_recommended(&pool, s3, tid, rid).await);
}

/// 2. 커트라인 동점: 정원 1, 순위 1,1 → manual + 사유에 "동점" 포함, results 불변
#[tokio::test]
async fn auto_recommend_cutline_tie() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let univ = new_univ(&pool, "A대", None).await;
    let tid = new_track(&pool, univ, "컴공", Some(1)).await;
    let s1 = new_student(&pool, "S1", 1, 1, 1).await;
    let s2 = new_student(&pool, "S2", 1, 1, 2).await;
    let rid = new_closed_round(&pool).await;
    new_application(&pool, s1, tid, rid, false).await;
    new_application(&pool, s2, tid, rid, false).await;
    new_result(&pool, s1, tid, rid, Some(1), 200_000, false).await;
    new_result(&pool, s2, tid, rid, Some(1), 200_000, false).await;

    let res = call_auto(&pool, rid).await;

    assert_eq!(res.confirmed.len(), 0);
    assert_eq!(res.manual.len(), 1);
    assert!(res.manual[0].reason.contains("동점"), "사유에 '동점' 포함: {}", res.manual[0].reason);
    assert!(!get_recommended(&pool, s1, tid, rid).await);
    assert!(!get_recommended(&pool, s2, tid, rid).await);
}

/// 3. 커트라인 안쪽 동점: 정원 2, 순위 1,1,3 → 1위 2명 확정, 3위 제외, manual 없음
#[tokio::test]
async fn auto_recommend_inner_tie() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let univ = new_univ(&pool, "A대", None).await;
    let tid = new_track(&pool, univ, "컴공", Some(2)).await;
    let s1 = new_student(&pool, "S1", 1, 1, 1).await;
    let s2 = new_student(&pool, "S2", 1, 1, 2).await;
    let s3 = new_student(&pool, "S3", 1, 1, 3).await;
    let rid = new_closed_round(&pool).await;
    new_application(&pool, s1, tid, rid, false).await;
    new_application(&pool, s2, tid, rid, false).await;
    new_application(&pool, s3, tid, rid, false).await;
    new_result(&pool, s1, tid, rid, Some(1), 300_000, false).await;
    new_result(&pool, s2, tid, rid, Some(1), 250_000, false).await;
    new_result(&pool, s3, tid, rid, Some(3), 100_000, false).await;

    let res = call_auto(&pool, rid).await;

    assert_eq!(res.confirmed.len(), 1);
    assert_eq!(res.confirmed[0].count, 2);
    assert_eq!(res.manual.len(), 0);
    assert!(get_recommended(&pool, s1, tid, rid).await);
    assert!(get_recommended(&pool, s2, tid, rid).await);
    assert!(!get_recommended(&pool, s3, tid, rid).await);
}

/// 4. 기존 수동 추천 존중: 정원 2, 순위 3인 학생이 이미 수동 recommended=1
///    → 잔여 1석에 순위 1위만 자동 확정, 기존 추천 유지
#[tokio::test]
async fn auto_recommend_respects_existing() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let univ = new_univ(&pool, "A대", None).await;
    let tid = new_track(&pool, univ, "컴공", Some(2)).await;
    let s1 = new_student(&pool, "S1", 1, 1, 1).await;
    let s2 = new_student(&pool, "S2", 1, 1, 2).await;
    let s3 = new_student(&pool, "S3", 1, 1, 3).await;
    let rid = new_closed_round(&pool).await;
    new_application(&pool, s1, tid, rid, false).await;
    new_application(&pool, s2, tid, rid, false).await;
    new_application(&pool, s3, tid, rid, false).await;
    // s3은 이미 수동으로 recommended=1
    new_result(&pool, s1, tid, rid, Some(1), 300_000, false).await;
    new_result(&pool, s2, tid, rid, Some(2), 200_000, false).await;
    new_result(&pool, s3, tid, rid, Some(3), 100_000, true).await;

    let res = call_auto(&pool, rid).await;

    // used=1(s3), remaining=1 → 상위 1명(s1) 자동 확정
    assert_eq!(res.confirmed.len(), 1);
    assert_eq!(res.confirmed[0].count, 1);
    assert!(get_recommended(&pool, s1, tid, rid).await);
    assert!(!get_recommended(&pool, s2, tid, rid).await);
    assert!(get_recommended(&pool, s3, tid, rid).await, "기존 추천 유지");
}

/// 5. 전 라운드 누적: FINALIZED 라운드1에서 추천 1명(포기 안 함), 정원 2
///    → 현재 라운드에서 1명만 자동 확정
#[tokio::test]
async fn auto_recommend_cross_round_accumulation() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let univ = new_univ(&pool, "A대", None).await;
    let tid = new_track(&pool, univ, "컴공", Some(2)).await;
    // 라운드 1 (FINALIZED): 먼저 생성해야 idx_one_active_round 안 걸림
    let rid1 = new_finalized_round(&pool).await;
    let s_prev = new_student(&pool, "SP", 1, 1, 1).await;
    new_application(&pool, s_prev, tid, rid1, false).await;
    new_result(&pool, s_prev, tid, rid1, Some(1), 300_000, true).await;
    // 라운드 2 (CLOSED)
    let rid2 = new_closed_round(&pool).await;
    let s1 = new_student(&pool, "S1", 1, 1, 2).await;
    let s2 = new_student(&pool, "S2", 1, 1, 3).await;
    new_application(&pool, s1, tid, rid2, false).await;
    new_application(&pool, s2, tid, rid2, false).await;
    new_result(&pool, s1, tid, rid2, Some(1), 200_000, false).await;
    new_result(&pool, s2, tid, rid2, Some(2), 100_000, false).await;

    let res = call_auto(&pool, rid2).await;

    // used=1(라운드1 s_prev), remaining=1 → s1만 확정
    assert_eq!(res.confirmed.len(), 1);
    assert_eq!(res.confirmed[0].count, 1);
    assert!(get_recommended(&pool, s1, tid, rid2).await);
    assert!(!get_recommended(&pool, s2, tid, rid2).await);
}

/// 6. 포기 반환: 라운드1 추천자가 abandoned=1 → 현재 라운드 2석 전부 자동 확정
#[tokio::test]
async fn auto_recommend_abandoned_return() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let univ = new_univ(&pool, "A대", None).await;
    let tid = new_track(&pool, univ, "컴공", Some(2)).await;
    let rid1 = new_finalized_round(&pool).await;
    let s_prev = new_student(&pool, "SP", 1, 1, 1).await;
    // abandoned=1 → used에 미포함
    new_application(&pool, s_prev, tid, rid1, true).await;
    new_result(&pool, s_prev, tid, rid1, Some(1), 300_000, true).await;
    let rid2 = new_closed_round(&pool).await;
    let s1 = new_student(&pool, "S1", 1, 1, 2).await;
    let s2 = new_student(&pool, "S2", 1, 1, 3).await;
    new_application(&pool, s1, tid, rid2, false).await;
    new_application(&pool, s2, tid, rid2, false).await;
    new_result(&pool, s1, tid, rid2, Some(1), 200_000, false).await;
    new_result(&pool, s2, tid, rid2, Some(2), 100_000, false).await;

    let res = call_auto(&pool, rid2).await;

    // used=0(포기는 카운트 안 함), remaining=2 → 2명 전부 확정
    assert_eq!(res.confirmed.len(), 1);
    assert_eq!(res.confirmed[0].count, 2);
    assert!(get_recommended(&pool, s1, tid, rid2).await);
    assert!(get_recommended(&pool, s2, tid, rid2).await);
}

/// 7. 무제한(unit_quota NULL): 후보 3명 전원 확정
#[tokio::test]
async fn auto_recommend_unlimited_quota() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let univ = new_univ(&pool, "A대", None).await;
    let tid = new_track(&pool, univ, "컴공", None).await; // unit_quota NULL
    let s1 = new_student(&pool, "S1", 1, 1, 1).await;
    let s2 = new_student(&pool, "S2", 1, 1, 2).await;
    let s3 = new_student(&pool, "S3", 1, 1, 3).await;
    let rid = new_closed_round(&pool).await;
    new_application(&pool, s1, tid, rid, false).await;
    new_application(&pool, s2, tid, rid, false).await;
    new_application(&pool, s3, tid, rid, false).await;
    new_result(&pool, s1, tid, rid, Some(1), 300_000, false).await;
    new_result(&pool, s2, tid, rid, Some(2), 200_000, false).await;
    new_result(&pool, s3, tid, rid, Some(3), 100_000, false).await;

    let res = call_auto(&pool, rid).await;

    assert_eq!(res.confirmed.len(), 1);
    assert_eq!(res.confirmed[0].count, 3);
    assert_eq!(res.manual.len(), 0);
    assert!(get_recommended(&pool, s1, tid, rid).await);
    assert!(get_recommended(&pool, s2, tid, rid).await);
    assert!(get_recommended(&pool, s3, tid, rid).await);
}

/// 8. 대학 total_quota 초과: 두 모집단위(각 정원 1) 합산이 대학 정원 1 초과
///    → 그 대학 두 모집단위 전부 manual + results 불변, 타 대학 모집단위는 정상 확정
#[tokio::test]
async fn auto_recommend_univ_total_quota_exceeded() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    // 대학 A: total_quota=1, 두 모집단위
    let univ_a = new_univ(&pool, "A대", Some(1)).await;
    let tid_a1 = new_track(&pool, univ_a, "컴공", Some(1)).await;
    let tid_a2 = new_track(&pool, univ_a, "전기", Some(1)).await;
    // 대학 B: total_quota 없음, 한 모집단위
    let univ_b = new_univ(&pool, "B대", None).await;
    let tid_b = new_track(&pool, univ_b, "수학", Some(1)).await;

    let sa1 = new_student(&pool, "A1", 1, 1, 1).await;
    let sa2 = new_student(&pool, "A2", 1, 1, 2).await;
    let sb  = new_student(&pool, "B1", 1, 1, 3).await;
    let rid = new_closed_round(&pool).await;

    new_application(&pool, sa1, tid_a1, rid, false).await;
    new_application(&pool, sa2, tid_a2, rid, false).await;
    new_application(&pool, sb,  tid_b,  rid, false).await;
    new_result(&pool, sa1, tid_a1, rid, Some(1), 200_000, false).await;
    new_result(&pool, sa2, tid_a2, rid, Some(1), 200_000, false).await;
    new_result(&pool, sb,  tid_b,  rid, Some(1), 200_000, false).await;

    let res = call_auto(&pool, rid).await;

    // A대 두 모집단위 → manual (auto_count=2 > total_quota=1)
    assert_eq!(res.manual.len(), 2, "A대 모집단위 2개 manual");
    // B대 모집단위 → confirmed
    assert_eq!(res.confirmed.len(), 1, "B대 모집단위 confirmed");
    assert_eq!(res.confirmed[0].count, 1);

    // A대 results 불변 (recommended=0 유지)
    assert!(!get_recommended(&pool, sa1, tid_a1, rid).await);
    assert!(!get_recommended(&pool, sa2, tid_a2, rid).await);
    // B대 confirmed
    assert!(get_recommended(&pool, sb, tid_b, rid).await);
}

/// 9. 멱등성: 같은 호출 2회 → 2회째 confirmed 비어 있고 DB 불변
#[tokio::test]
async fn auto_recommend_idempotent() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let univ = new_univ(&pool, "A대", None).await;
    let tid = new_track(&pool, univ, "컴공", Some(1)).await;
    let s1 = new_student(&pool, "S1", 1, 1, 1).await;
    let rid = new_closed_round(&pool).await;
    new_application(&pool, s1, tid, rid, false).await;
    new_result(&pool, s1, tid, rid, Some(1), 200_000, false).await;

    let res1 = call_auto(&pool, rid).await;
    assert_eq!(res1.confirmed.len(), 1);
    assert!(get_recommended(&pool, s1, tid, rid).await);

    let res2 = call_auto(&pool, rid).await;
    assert_eq!(res2.confirmed.len(), 0, "2회째 confirmed 없음");
    assert_eq!(res2.manual.len(), 0);
    assert!(get_recommended(&pool, s1, tid, rid).await, "기존 추천 유지");
}

/// 10. 후보 없음/정원 소진 모집단위는 confirmed·manual 어느 쪽에도 나타나지 않음
#[tokio::test]
async fn auto_recommend_skip_no_candidates_or_exhausted() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let univ = new_univ(&pool, "A대", None).await;
    // 트랙 A: 정원 1, 이미 1명 recommended → remaining=0 → skip (정원 소진)
    let tid_a = new_track(&pool, univ, "컴공", Some(1)).await;
    // 트랙 B: 정원 5, 후보 모두 recommended=1 → candidates empty → skip (후보 없음)
    let tid_b = new_track(&pool, univ, "전기", Some(5)).await;

    let sa = new_student(&pool, "A1", 1, 1, 1).await;
    let sb = new_student(&pool, "B1", 1, 1, 2).await;
    let rid = new_closed_round(&pool).await;

    new_application(&pool, sa, tid_a, rid, false).await;
    new_application(&pool, sb, tid_b, rid, false).await;
    // 두 결과 모두 recommended=1 (이미 수동 처리된 상태)
    new_result(&pool, sa, tid_a, rid, Some(1), 200_000, true).await;
    new_result(&pool, sb, tid_b, rid, Some(1), 200_000, true).await;

    let res = call_auto(&pool, rid).await;

    assert_eq!(res.confirmed.len(), 0, "아무 모집단위도 confirmed 없음");
    assert_eq!(res.manual.len(), 0, "아무 모집단위도 manual 없음");
}
