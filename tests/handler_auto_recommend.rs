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

/// 8. 대학 total_quota 초과 + 경계 동점: 두 모집단위(각 정원 1) 합산 2명이 대학 정원 1 초과.
///    두 후보는 대학 전체 순위가 동점(1위)이므로 2단계 대학 컷에서 원자 처리 →
///    아무도 확정되지 않고 **대학 단위** manual 1건. 타 대학 모집단위는 정상 확정.
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

    // A대 → 대학 단위 manual 1건 (대학 전체 1위 동점 2명이 잔여 1석 경합)
    assert_eq!(res.manual.len(), 1, "A대 대학 단위 manual 1건");
    assert_eq!(res.manual[0].track_id, None, "대학 단위 항목은 track_id 없음");
    assert_eq!(res.manual[0].univ_name, "A대");
    assert!(res.manual[0].reason.contains("동점"), "사유: {}", res.manual[0].reason);
    assert!(res.manual[0].reason.contains("대학 정원 1명"), "사유에 대학 정원: {}", res.manual[0].reason);
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

// ── B단계: 동점 그룹 원자적 채움 + 대학 전체 순위 컷 ──────────────

use principal_candidate_manager::handlers::scoring::{
    auto_recommend_results_univ, fill_by_rank_groups, TieBoundary,
};

/// 졸업생(is_enrolled=0). 스키마 CHECK: 졸업생은 grade/class_no/seq_no NULL + grad_year NOT NULL
async fn new_graduate(pool: &SqlitePool, code: &str, grad_year: i64) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO students (student_code, name, is_enrolled, grad_year) \
         VALUES (?, ?, 0, ?) RETURNING id",
    )
    .bind(code)
    .bind(code)
    .bind(grad_year)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// 대학 재학생 우선 ON. 트리거 "univ prioritize=1 requires track prioritize=1" 때문에
/// 소속 모집단위를 먼저 ON 으로 바꾼 뒤 대학을 켠다.
async fn set_univ_prioritize(pool: &SqlitePool, univ_id: i64) {
    sqlx::query("UPDATE univ_tracks SET prioritize_enrolled = 1 WHERE univ_id = ?")
        .bind(univ_id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("UPDATE universities SET prioritize_enrolled = 1 WHERE id = ?")
        .bind(univ_id)
        .execute(pool)
        .await
        .unwrap();
}

async fn call_auto_univ(pool: &SqlitePool, rid: i64, uid: i64) -> AutoRecommendResponse {
    let st = common::make_state(pool.clone());
    match auto_recommend_results_univ(State(st), Path((rid, uid))).await {
        Ok(Json(v)) => v,
        Err((s, msg)) => panic!("auto_recommend_results_univ 실패: {} — {}", s, msg),
    }
}

/// 지원자 1명 등록 + 결과 생성 (ranking = 대학 전체 순위, total_score = 모집단위 순위 결정)
async fn app_result(
    pool: &SqlitePool,
    sid: i64,
    tid: i64,
    rid: i64,
    ranking: i64,
    total_score: i64,
) {
    new_application(pool, sid, tid, rid, false).await;
    new_result(pool, sid, tid, rid, Some(ranking), total_score, false).await;
}

// ── 헬퍼 단위 테스트: fill_by_rank_groups ─────────────────────────

/// 무제한(None) → 전원 확정, 수동 없음
#[test]
fn fill_unlimited_confirms_all() {
    let items = vec![(1, 'a'), (2, 'b'), (2, 'c')];
    let out = fill_by_rank_groups(&items, None);
    assert_eq!(out.confirmed, vec!['a', 'b', 'c']);
    assert_eq!(out.tie, None);
}

/// 잔여 0 이하 → 아무도 확정 안 됨, 수동 없음 (정원 소진은 오류 아님)
#[test]
fn fill_zero_remaining_confirms_none() {
    let items = vec![(1, 'a')];
    assert!(fill_by_rank_groups(&items, Some(0)).confirmed.is_empty());
    assert_eq!(fill_by_rank_groups(&items, Some(0)).tie, None);
    assert!(fill_by_rank_groups(&items, Some(-3)).confirmed.is_empty());
    assert_eq!(fill_by_rank_groups(&items, Some(-3)).tie, None);
}

/// 정원이 후보보다 많음 → 전원 확정
#[test]
fn fill_quota_exceeds_candidates() {
    let items = vec![(1, 'a'), (2, 'b')];
    let out = fill_by_rank_groups(&items, Some(5));
    assert_eq!(out.confirmed, vec!['a', 'b']);
    assert_eq!(out.tie, None);
}

/// 깨끗한 경계(동점 아님): [1,2,3] 정원 2 → 상위 2명 확정, 수동 불필요
#[test]
fn fill_clean_boundary_no_tie() {
    let items = vec![(1, 'a'), (2, 'b'), (3, 'c')];
    let out = fill_by_rank_groups(&items, Some(2));
    assert_eq!(out.confirmed, vec!['a', 'b']);
    assert_eq!(out.tie, None, "남은자리 0 — 깨끗한 경계");
}

/// 깨끗한 경계(동점 그룹이 정확히 정원까지): [1,2,2,4] 정원 3 → 3명 확정, 수동 불필요
#[test]
fn fill_clean_boundary_after_tie_group() {
    let items = vec![(1, 'a'), (2, 'b'), (2, 'c'), (4, 'd')];
    let out = fill_by_rank_groups(&items, Some(3));
    assert_eq!(out.confirmed, vec!['a', 'b', 'c']);
    assert_eq!(out.tie, None, "동점 그룹이 정원에 정확히 맞음 — 수동 불필요");
}

/// 동점이 경계를 가름: [1,2,2,4] 정원 2 → 1위만 확정, 2위 동점 그룹은 수동
#[test]
fn fill_tie_splits_boundary_confirms_above() {
    let items = vec![(1, 'a'), (2, 'b'), (2, 'c'), (4, 'd')];
    let out = fill_by_rank_groups(&items, Some(2));
    assert_eq!(out.confirmed, vec!['a'], "동점 위까지는 자동 확정 (전체 차단 아님)");
    assert_eq!(out.tie, Some(TieBoundary { rank: 2, free: 1, contenders: 2 }));
}

/// 최상위가 동점: [1,1,1] 정원 2 → 아무도 확정 안 됨 + 수동
#[test]
fn fill_top_tie_confirms_none() {
    let items = vec![(1, 'a'), (1, 'b'), (1, 'c')];
    let out = fill_by_rank_groups(&items, Some(2));
    assert!(out.confirmed.is_empty());
    assert_eq!(out.tie, Some(TieBoundary { rank: 1, free: 2, contenders: 3 }));
}

// ── 1단계(모집단위) 동점 그룹 원자 처리 ──────────────────────────

/// 정원 2, 모집단위 순위 [1,2,2,4] → 1위만 확정, 2위 동점 2명은 수동.
/// (구 동작: 트랙 전체 Manual — 0명 확정. B단계에서 상위 확정으로 바뀜.)
#[tokio::test]
async fn auto_recommend_track_tie_confirms_above_group() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let univ = new_univ(&pool, "A대", None).await;
    let tid = new_track(&pool, univ, "컴공", Some(2)).await;
    let s1 = new_student(&pool, "S1", 1, 1, 1).await;
    let s2 = new_student(&pool, "S2", 1, 1, 2).await;
    let s3 = new_student(&pool, "S3", 1, 1, 3).await;
    let s4 = new_student(&pool, "S4", 1, 1, 4).await;
    let rid = new_closed_round(&pool).await;
    app_result(&pool, s1, tid, rid, 1, 400_000).await;
    app_result(&pool, s2, tid, rid, 2, 300_000).await;
    app_result(&pool, s3, tid, rid, 2, 300_000).await;
    app_result(&pool, s4, tid, rid, 4, 100_000).await;

    let res = call_auto(&pool, rid).await;

    assert_eq!(res.confirmed.len(), 1);
    assert_eq!(res.confirmed[0].count, 1, "1위 1명만 확정");
    assert_eq!(res.manual.len(), 1);
    assert_eq!(res.manual[0].track_id, Some(tid));
    let reason = &res.manual[0].reason;
    assert!(reason.contains("2위"), "사유에 순위: {}", reason);
    assert!(reason.contains("1석"), "사유에 잔여석: {}", reason);
    assert!(reason.contains("2명"), "사유에 경합 인원: {}", reason);

    assert!(get_recommended(&pool, s1, tid, rid).await);
    assert!(!get_recommended(&pool, s2, tid, rid).await);
    assert!(!get_recommended(&pool, s3, tid, rid).await);
    assert!(!get_recommended(&pool, s4, tid, rid).await);
}

/// 정원 2, 모집단위 순위 [1,1,1] → 최상위 3명이 2석 경합 → 아무도 확정 안 됨 + 수동
#[tokio::test]
async fn auto_recommend_track_top_tie_confirms_none() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let univ = new_univ(&pool, "A대", None).await;
    let tid = new_track(&pool, univ, "컴공", Some(2)).await;
    let s1 = new_student(&pool, "S1", 1, 1, 1).await;
    let s2 = new_student(&pool, "S2", 1, 1, 2).await;
    let s3 = new_student(&pool, "S3", 1, 1, 3).await;
    let rid = new_closed_round(&pool).await;
    app_result(&pool, s1, tid, rid, 1, 300_000).await;
    app_result(&pool, s2, tid, rid, 1, 300_000).await;
    app_result(&pool, s3, tid, rid, 1, 300_000).await;

    let res = call_auto(&pool, rid).await;

    assert_eq!(res.confirmed.len(), 0);
    assert_eq!(res.manual.len(), 1);
    assert!(res.manual[0].reason.contains("1위"), "사유: {}", res.manual[0].reason);
    for s in [s1, s2, s3] {
        assert!(!get_recommended(&pool, s, tid, rid).await);
    }
}

/// 정원 3, 모집단위 순위 [1,2,2,4] → 동점 그룹이 정원에 정확히 맞음 → 3명 확정, 수동 없음
#[tokio::test]
async fn auto_recommend_track_tie_exactly_fits_quota() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let univ = new_univ(&pool, "A대", None).await;
    let tid = new_track(&pool, univ, "컴공", Some(3)).await;
    let s1 = new_student(&pool, "S1", 1, 1, 1).await;
    let s2 = new_student(&pool, "S2", 1, 1, 2).await;
    let s3 = new_student(&pool, "S3", 1, 1, 3).await;
    let s4 = new_student(&pool, "S4", 1, 1, 4).await;
    let rid = new_closed_round(&pool).await;
    app_result(&pool, s1, tid, rid, 1, 400_000).await;
    app_result(&pool, s2, tid, rid, 2, 300_000).await;
    app_result(&pool, s3, tid, rid, 2, 300_000).await;
    app_result(&pool, s4, tid, rid, 4, 100_000).await;

    let res = call_auto(&pool, rid).await;

    assert_eq!(res.manual.len(), 0, "깨끗한 경계 — 수동 불필요");
    assert_eq!(res.confirmed[0].count, 3);
    assert!(get_recommended(&pool, s3, tid, rid).await);
    assert!(!get_recommended(&pool, s4, tid, rid).await);
}

/// 1단계 재학생 우선은 **트랙 플래그**를 쓴다: 트랙 prioritize=1, 대학 prioritize=0.
/// 졸업생이 점수가 더 높아도 재학생이 모집단위 순위 상위 → 정원 1석은 재학생.
#[tokio::test]
async fn auto_recommend_track_phase_uses_track_prioritize_flag() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let univ = new_univ(&pool, "A대", None).await; // 대학 prioritize 기본 0
    let tid = sqlx::query_scalar::<_, i64>(
        "INSERT INTO univ_tracks (univ_id, track_name, unit_quota, prioritize_enrolled) \
         VALUES (?, '컴공', 1, 1) RETURNING id",
    )
    .bind(univ)
    .fetch_one(&pool)
    .await
    .unwrap();
    let grad = new_graduate(&pool, "G1", 2024).await;
    let enr = new_student(&pool, "E1", 1, 1, 1).await;
    let rid = new_closed_round(&pool).await;
    // 대학 순위(ranking)는 대학 플래그 기준이라 점수순: 졸업생 1위, 재학생 2위
    app_result(&pool, grad, tid, rid, 1, 300_000).await;
    app_result(&pool, enr, tid, rid, 2, 100_000).await;

    let res = call_auto(&pool, rid).await;

    assert_eq!(res.manual.len(), 0);
    assert_eq!(res.confirmed[0].count, 1);
    assert!(get_recommended(&pool, enr, tid, rid).await, "트랙 플래그로 재학생이 상위");
    assert!(!get_recommended(&pool, grad, tid, rid).await);
}

// ── 2단계(대학 전체 순위) 정원 컷 ────────────────────────────────

/// 대학 정원 유한 + 트랙 무제한 여럿: 대학 전체 순위 상위 N 확정, N+1위 미추천 (경계 비동점)
#[tokio::test]
async fn auto_recommend_univ_cut_across_unlimited_tracks() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let univ = new_univ(&pool, "A대", Some(2)).await;
    let t1 = new_track(&pool, univ, "컴공", None).await;
    let t2 = new_track(&pool, univ, "전기", None).await;
    let s1 = new_student(&pool, "S1", 1, 1, 1).await;
    let s2 = new_student(&pool, "S2", 1, 1, 2).await;
    let s3 = new_student(&pool, "S3", 1, 1, 3).await;
    let rid = new_closed_round(&pool).await;
    app_result(&pool, s1, t1, rid, 1, 300_000).await;
    app_result(&pool, s2, t2, rid, 2, 200_000).await;
    app_result(&pool, s3, t1, rid, 3, 100_000).await;

    let res = call_auto(&pool, rid).await;

    assert_eq!(res.manual.len(), 0, "경계 비동점 — 수동 없음");
    let total: i64 = res.confirmed.iter().map(|c| c.count).sum();
    assert_eq!(total, 2, "대학 정원 2명까지만");
    assert!(get_recommended(&pool, s1, t1, rid).await);
    assert!(get_recommended(&pool, s2, t2, rid).await);
    assert!(!get_recommended(&pool, s3, t1, rid).await, "대학 3위는 미추천");
}

/// 대학 경계 동점: 대학 정원 2, 대학 순위 [1,2,2] → 1위만 확정, 2위 동점은 대학 단위 수동
#[tokio::test]
async fn auto_recommend_univ_cut_tie_at_boundary() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let univ = new_univ(&pool, "A대", Some(2)).await;
    let t1 = new_track(&pool, univ, "컴공", None).await;
    let t2 = new_track(&pool, univ, "전기", None).await;
    let s1 = new_student(&pool, "S1", 1, 1, 1).await;
    let s2 = new_student(&pool, "S2", 1, 1, 2).await;
    let s3 = new_student(&pool, "S3", 1, 1, 3).await;
    let rid = new_closed_round(&pool).await;
    app_result(&pool, s1, t1, rid, 1, 300_000).await;
    app_result(&pool, s2, t1, rid, 2, 200_000).await;
    app_result(&pool, s3, t2, rid, 2, 200_000).await;

    let res = call_auto(&pool, rid).await;

    assert_eq!(res.manual.len(), 1, "대학 단위 manual 1건");
    assert_eq!(res.manual[0].track_id, None);
    assert_eq!(res.manual[0].track_name, None);
    let reason = &res.manual[0].reason;
    assert!(reason.contains("대학 전체 2위"), "사유에 순위: {}", reason);
    assert!(reason.contains("잔여 1석"), "사유에 잔여석: {}", reason);
    assert!(reason.contains("2명 경합"), "사유에 경합 인원: {}", reason);
    assert!(reason.contains("대학 정원 2명"), "사유에 대학 정원: {}", reason);

    assert!(get_recommended(&pool, s1, t1, rid).await, "동점 위는 자동 확정");
    assert!(!get_recommended(&pool, s2, t1, rid).await);
    assert!(!get_recommended(&pool, s3, t2, rid).await);
}

/// 대학 무제한 → 2단계 컷 미발동, 트랙 결과가 곧 최종
#[tokio::test]
async fn auto_recommend_unlimited_univ_no_cut() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let univ = new_univ(&pool, "A대", None).await;
    let t1 = new_track(&pool, univ, "컴공", Some(1)).await;
    let t2 = new_track(&pool, univ, "전기", Some(1)).await;
    let s1 = new_student(&pool, "S1", 1, 1, 1).await;
    let s2 = new_student(&pool, "S2", 1, 1, 2).await;
    let rid = new_closed_round(&pool).await;
    app_result(&pool, s1, t1, rid, 1, 300_000).await;
    app_result(&pool, s2, t2, rid, 2, 200_000).await;

    let res = call_auto(&pool, rid).await;

    assert_eq!(res.manual.len(), 0);
    let total: i64 = res.confirmed.iter().map(|c| c.count).sum();
    assert_eq!(total, 2);
    assert!(get_recommended(&pool, s1, t1, rid).await);
    assert!(get_recommended(&pool, s2, t2, rid).await);
}

/// 이전 라운드 univ_used 반영: total_quota=2, 이전 라운드 1명 확정 → 잔여 1석
#[tokio::test]
async fn auto_recommend_univ_cut_counts_previous_rounds() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let univ = new_univ(&pool, "A대", Some(2)).await;
    let t1 = new_track(&pool, univ, "컴공", None).await;
    let rid1 = new_finalized_round(&pool).await;
    let sp = new_student(&pool, "SP", 1, 1, 1).await;
    new_application(&pool, sp, t1, rid1, false).await;
    new_result(&pool, sp, t1, rid1, Some(1), 500_000, true).await;

    let rid2 = new_closed_round(&pool).await;
    let s1 = new_student(&pool, "S1", 1, 1, 2).await;
    let s2 = new_student(&pool, "S2", 1, 1, 3).await;
    app_result(&pool, s1, t1, rid2, 1, 300_000).await;
    app_result(&pool, s2, t1, rid2, 2, 200_000).await;

    let res = call_auto(&pool, rid2).await;

    assert_eq!(res.manual.len(), 0, "경계 비동점");
    let total: i64 = res.confirmed.iter().map(|c| c.count).sum();
    assert_eq!(total, 1, "잔여 1석만");
    assert!(get_recommended(&pool, s1, t1, rid2).await);
    assert!(!get_recommended(&pool, s2, t1, rid2).await);
}

/// 같은 학생이 두 모집단위 지원(D3): 행 단위로 각각 정원 소비, 대학 정원도 행 수로 카운트
#[tokio::test]
async fn auto_recommend_same_student_two_tracks_counts_rows() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let univ = new_univ(&pool, "A대", Some(1)).await;
    let t1 = new_track(&pool, univ, "컴공", None).await;
    let t2 = new_track(&pool, univ, "전기", None).await;
    let s1 = new_student(&pool, "S1", 1, 1, 1).await;
    let rid = new_closed_round(&pool).await;
    app_result(&pool, s1, t1, rid, 1, 300_000).await;
    app_result(&pool, s1, t2, rid, 2, 200_000).await;

    let res = call_auto(&pool, rid).await;

    assert_eq!(res.manual.len(), 0);
    let total: i64 = res.confirmed.iter().map(|c| c.count).sum();
    assert_eq!(total, 1, "대학 정원 1 — 행 1개만 확정");
    assert!(get_recommended(&pool, s1, t1, rid).await, "대학 1위 행");
    assert!(!get_recommended(&pool, s1, t2, rid).await);
}

/// 대학 컷의 재학생 우선은 **대학 플래그**를 쓴다.
/// 대학 prioritize=1, 트랙 무제한 → 점수가 낮아도 재학생이 대학 잔여석 차지.
#[tokio::test]
async fn auto_recommend_univ_cut_uses_univ_prioritize_flag() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let univ = new_univ(&pool, "A대", Some(1)).await;
    let t1 = new_track(&pool, univ, "컴공", None).await;
    let t2 = new_track(&pool, univ, "전기", None).await;
    set_univ_prioritize(&pool, univ).await;
    let grad = new_graduate(&pool, "G1", 2024).await;
    let enr = new_student(&pool, "E1", 1, 1, 1).await;
    let rid = new_closed_round(&pool).await;
    // 대학 prioritize=1 → 재학생이 대학 1위, 졸업생 2위 (점수는 졸업생이 높음)
    app_result(&pool, enr, t1, rid, 1, 100_000).await;
    app_result(&pool, grad, t2, rid, 2, 300_000).await;

    let res = call_auto(&pool, rid).await;

    assert_eq!(res.manual.len(), 0);
    let total: i64 = res.confirmed.iter().map(|c| c.count).sum();
    assert_eq!(total, 1);
    assert!(get_recommended(&pool, enr, t1, rid).await, "대학 플래그로 재학생 우선");
    assert!(!get_recommended(&pool, grad, t2, rid).await);
}

/// 한 대학에 트랙 동점 manual 과 대학 동점 manual 이 동시에 날 수 있다 — 둘 다 보고
#[tokio::test]
async fn auto_recommend_reports_both_track_and_univ_manual() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let univ = new_univ(&pool, "A대", Some(2)).await;
    // t1: 정원 1, 후보 2명 동점 → 트랙 동점 manual (확정 0)
    let t1 = new_track(&pool, univ, "컴공", Some(1)).await;
    // t2: 무제한, 후보 3명 (대학 순위 1 / 2 동점 / 2 동점)
    let t2 = new_track(&pool, univ, "전기", None).await;
    let a1 = new_student(&pool, "A1", 1, 1, 1).await;
    let a2 = new_student(&pool, "A2", 1, 1, 2).await;
    let b1 = new_student(&pool, "B1", 1, 1, 3).await;
    let b2 = new_student(&pool, "B2", 1, 1, 4).await;
    let b3 = new_student(&pool, "B3", 1, 1, 5).await;
    let rid = new_closed_round(&pool).await;
    app_result(&pool, a1, t1, rid, 4, 200_000).await;
    app_result(&pool, a2, t1, rid, 4, 200_000).await;
    app_result(&pool, b1, t2, rid, 1, 500_000).await;
    app_result(&pool, b2, t2, rid, 2, 300_000).await;
    app_result(&pool, b3, t2, rid, 2, 300_000).await;

    let res = call_auto(&pool, rid).await;

    assert_eq!(res.manual.len(), 2, "트랙 manual + 대학 manual");
    assert!(res.manual.iter().any(|m| m.track_id == Some(t1)), "트랙 동점 manual");
    assert!(res.manual.iter().any(|m| m.track_id.is_none()), "대학 동점 manual");
    // 대학 정원 2, 대학 순위 1위(b1) 확정 → 잔여 1석에 2위 동점 2명 경합 → 수동
    assert!(get_recommended(&pool, b1, t2, rid).await);
    assert!(!get_recommended(&pool, b2, t2, rid).await);
    assert!(!get_recommended(&pool, b3, t2, rid).await);
    assert!(!get_recommended(&pool, a1, t1, rid).await);
    assert!(!get_recommended(&pool, a2, t1, rid).await);
}

// ── 대학별 개별 자동 추천 버튼 ───────────────────────────────────

/// 대학별 버튼: 지정 대학만 처리, 다른 대학 results 무변경
#[tokio::test]
async fn auto_recommend_univ_scoped_only_touches_that_univ() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let univ_a = new_univ(&pool, "A대", None).await;
    let ta = new_track(&pool, univ_a, "컴공", Some(5)).await;
    let univ_b = new_univ(&pool, "B대", None).await;
    let tb = new_track(&pool, univ_b, "수학", Some(5)).await;
    let sa = new_student(&pool, "A1", 1, 1, 1).await;
    let sb = new_student(&pool, "B1", 1, 1, 2).await;
    let rid = new_closed_round(&pool).await;
    app_result(&pool, sa, ta, rid, 1, 300_000).await;
    app_result(&pool, sb, tb, rid, 1, 300_000).await;

    let res = call_auto_univ(&pool, rid, univ_a).await;

    assert_eq!(res.confirmed.len(), 1);
    assert_eq!(res.confirmed[0].univ_name, "A대");
    assert!(get_recommended(&pool, sa, ta, rid).await);
    assert!(!get_recommended(&pool, sb, tb, rid).await, "B대 무변경");
}

/// 대학별 버튼: 없는 대학 → 404 (Fail-Fast, 빈 결과로 조용히 성공하지 않음)
#[tokio::test]
async fn auto_recommend_univ_scoped_unknown_univ_404() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let rid = new_closed_round(&pool).await;
    let st = common::make_state(pool.clone());
    let err = auto_recommend_results_univ(State(st), Path((rid, 9999)))
        .await
        .err()
        .expect("없는 대학은 404");
    assert_eq!(err.0, axum::http::StatusCode::NOT_FOUND);
}

/// 대학별 버튼도 라운드 상태를 검증한다 (FINALIZED → 400)
#[tokio::test]
async fn auto_recommend_univ_scoped_rejects_non_closed_round() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let univ = new_univ(&pool, "A대", None).await;
    let rid = new_finalized_round(&pool).await;
    let st = common::make_state(pool.clone());
    let err = auto_recommend_results_univ(State(st), Path((rid, univ)))
        .await
        .err()
        .expect("CLOSED 아닌 라운드는 400");
    assert_eq!(err.0, axum::http::StatusCode::BAD_REQUEST);
}

// ── 감사 추가 회귀 테스트 (B단계 리뷰) ───────────────────────────

/// **설계 귀결 고정**: 대학 total_quota 가 있으면 2단계 컷이 **대학 플래그**로 재정렬하므로,
/// 대학 prioritize=0 · 트랙 prioritize=1 인 트랙의 재학생 우선은 대학 컷에서 유지되지 않는다.
/// (1단계 트랙 정원 컷에서는 유지된다 — auto_recommend_track_phase_uses_track_prioritize_flag)
/// D2 "각 범위 자기 플래그만" 의 직접적 귀결이며, 의도적 동작임을 이 테스트로 고정한다.
/// 변경하려면 설계 결정(D2) 자체를 다시 정해야 한다.
#[tokio::test]
async fn auto_recommend_univ_cut_uses_univ_flag_even_when_track_flag_differs() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let univ = new_univ(&pool, "A대", Some(1)).await; // 대학 정원 1, 대학 prioritize 0
    let tid = sqlx::query_scalar::<_, i64>(
        "INSERT INTO univ_tracks (univ_id, track_name, unit_quota, prioritize_enrolled) \
         VALUES (?, '컴공', NULL, 1) RETURNING id",
    )
    .bind(univ)
    .fetch_one(&pool)
    .await
    .unwrap();
    let grad = new_graduate(&pool, "G1", 2024).await;
    let enr = new_student(&pool, "E1", 1, 1, 1).await;
    let rid = new_closed_round(&pool).await;
    // 대학 순위(대학 플래그=0) = 점수순 → 졸업생 1위, 재학생 2위
    app_result(&pool, grad, tid, rid, 1, 300_000).await;
    app_result(&pool, enr, tid, rid, 2, 100_000).await;

    let res = call_auto(&pool, rid).await;

    // 1단계(트랙 무제한)는 둘 다 통과 → 2단계 대학 컷 1석이 대학 순위 1위를 택함
    assert_eq!(res.manual.len(), 0, "경계 비동점 — 수동 없음");
    let total: i64 = res.confirmed.iter().map(|c| c.count).sum();
    assert_eq!(total, 1);
    assert!(
        get_recommended(&pool, grad, tid, rid).await,
        "대학 컷은 대학 플래그(prioritize=0) 기준 — 트랙 재학생 우선이 여기선 적용되지 않음",
    );
    assert!(!get_recommended(&pool, enr, tid, rid).await);
}

/// 1단계 모집단위 순위는 **이미 추천 확정된 행까지 포함해** RANK() 로 계산하고,
/// 잔여 정원은 used 를 뺀 값이다. 둘이 맞물려 동점 경계가 정확히 계산되는지 고정.
/// 정원 3, 확정 1명(1위) → 잔여 2, 후보 순위 [2,2,4] → 동점 2명이 잔여 2석에 정확히 맞음 → 전원 확정.
#[tokio::test]
async fn auto_recommend_track_tie_boundary_accounts_for_existing_recommend() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let univ = new_univ(&pool, "A대", None).await;
    let tid = new_track(&pool, univ, "컴공", Some(3)).await;
    let x = new_student(&pool, "X1", 1, 1, 1).await;
    let a = new_student(&pool, "A1", 1, 1, 2).await;
    let b = new_student(&pool, "B1", 1, 1, 3).await;
    let c = new_student(&pool, "C1", 1, 1, 4).await;
    let rid = new_closed_round(&pool).await;
    new_application(&pool, x, tid, rid, false).await;
    new_result(&pool, x, tid, rid, Some(1), 500_000, true).await; // 이미 확정
    app_result(&pool, a, tid, rid, 2, 300_000).await;
    app_result(&pool, b, tid, rid, 2, 300_000).await;
    app_result(&pool, c, tid, rid, 4, 200_000).await;

    let res = call_auto(&pool, rid).await;

    assert_eq!(res.manual.len(), 0, "동점 그룹이 잔여 2석에 정확히 맞음 — 수동 불필요");
    assert_eq!(res.confirmed[0].count, 2);
    assert!(get_recommended(&pool, a, tid, rid).await);
    assert!(get_recommended(&pool, b, tid, rid).await);
    assert!(!get_recommended(&pool, c, tid, rid).await);
    assert!(get_recommended(&pool, x, tid, rid).await, "기존 확정 유지");
}

/// 같은 배치에서 정원만 2로 줄이면 잔여 1석 — 동점 2명이 경합 → 아무도 확정 안 됨 + 수동.
/// 사유의 순위는 화면(모집단위 순위)과 같은 2위여야 한다(확정 행 포함 RANK).
#[tokio::test]
async fn auto_recommend_track_tie_reason_rank_matches_screen_rank() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let univ = new_univ(&pool, "A대", None).await;
    let tid = new_track(&pool, univ, "컴공", Some(2)).await;
    let x = new_student(&pool, "X1", 1, 1, 1).await;
    let a = new_student(&pool, "A1", 1, 1, 2).await;
    let b = new_student(&pool, "B1", 1, 1, 3).await;
    let rid = new_closed_round(&pool).await;
    new_application(&pool, x, tid, rid, false).await;
    new_result(&pool, x, tid, rid, Some(1), 500_000, true).await;
    app_result(&pool, a, tid, rid, 2, 300_000).await;
    app_result(&pool, b, tid, rid, 2, 300_000).await;

    let res = call_auto(&pool, rid).await;

    assert_eq!(res.confirmed.len(), 0);
    assert_eq!(res.manual.len(), 1);
    let reason = &res.manual[0].reason;
    assert!(reason.contains("모집단위 2위"), "화면 순위와 같은 2위여야 함: {}", reason);
    assert!(reason.contains("1석"), "잔여 1석: {}", reason);
    assert!(!get_recommended(&pool, a, tid, rid).await);
    assert!(!get_recommended(&pool, b, tid, rid).await);
}

/// 대학별 버튼도 **그 대학의 대학 정원 컷**을 적용한다(전체 버튼과 동일 로직).
/// 다른 대학의 결과·정원은 건드리지 않는다.
#[tokio::test]
async fn auto_recommend_univ_scoped_applies_univ_cut() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let univ_a = new_univ(&pool, "A대", Some(1)).await;
    let ta1 = new_track(&pool, univ_a, "컴공", None).await;
    let ta2 = new_track(&pool, univ_a, "전기", None).await;
    let univ_b = new_univ(&pool, "B대", Some(1)).await;
    let tb = new_track(&pool, univ_b, "수학", None).await;
    let s1 = new_student(&pool, "S1", 1, 1, 1).await;
    let s2 = new_student(&pool, "S2", 1, 1, 2).await;
    let s3 = new_student(&pool, "S3", 1, 1, 3).await;
    let rid = new_closed_round(&pool).await;
    app_result(&pool, s1, ta1, rid, 1, 300_000).await;
    app_result(&pool, s2, ta2, rid, 2, 200_000).await;
    app_result(&pool, s3, tb, rid, 1, 100_000).await;

    let res = call_auto_univ(&pool, rid, univ_a).await;

    let total: i64 = res.confirmed.iter().map(|c| c.count).sum();
    assert_eq!(total, 1, "A대 정원 1 — 대학 순위 1위만");
    assert!(get_recommended(&pool, s1, ta1, rid).await);
    assert!(!get_recommended(&pool, s2, ta2, rid).await, "대학 컷 탈락");
    assert!(!get_recommended(&pool, s3, tb, rid).await, "B대 무변경");
}
