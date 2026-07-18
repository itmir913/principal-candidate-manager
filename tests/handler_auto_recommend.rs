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
    auto_recommend_results_univ, fill_by_rank_groups, merge_univ_cut, MergeCand, TieBoundary,
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

// ── 헬퍼 단위 테스트: merge_univ_cut (D단계 k-way 병합) ──────────

/// 병합 후보 하나. sid 는 확인용 식별자.
fn mc(sid: i64, track_id: i64, track_rank: i64, univ_rank: i64) -> MergeCand {
    MergeCand { student_id: sid, track_id, track_rank, univ_rank }
}

fn sids(cands: &[MergeCand]) -> Vec<i64> {
    cands.iter().map(|c| c.student_id).collect()
}

/// 무제한(None) → 트랙 전원 확정, 수동 없음
#[test]
fn merge_unlimited_confirms_all() {
    let tracks = vec![vec![mc(1, 10, 1, 2), mc(2, 10, 2, 5)], vec![mc(3, 20, 1, 1)]];
    let out = merge_univ_cut(&tracks, None);
    let mut got = sids(&out.confirmed);
    got.sort_unstable();
    assert_eq!(got, vec![1, 2, 3]);
    assert_eq!(out.tie, None);
}

/// **선두만 경쟁**: 트랙 상위자에 막힌 후보는 대학 순위가 최상위여도 뽑히지 않는다.
/// 트랙A [재학(트랙1위, 대학3위), 졸업(트랙2위, 대학1위)], 트랙B [X(대학2위)], 정원 2
/// → {X, 재학}. 졸업은 자기 트랙 선두에 막힘.
#[test]
fn merge_head_only_blocked_candidate_never_jumps() {
    let tracks = vec![
        vec![mc(1, 10, 1, 3), mc(2, 10, 2, 1)], // 재학, 졸업
        vec![mc(3, 20, 1, 2)],                  // X
    ];
    let out = merge_univ_cut(&tracks, Some(2));
    let mut got = sids(&out.confirmed);
    got.sort_unstable();
    assert_eq!(got, vec![1, 3], "막힌 졸업생(2)은 제외");
    assert_eq!(out.tie, None, "막힌 후보는 동점 경계가 아니다");
}

/// 동점 판정은 **선두들끼리만**: 트랙A 선두가 대학 2위이고 그 뒤에 대학 2위가 또 있어도,
/// 그 뒤 후보는 트랙 순서상 하위(track_rank 다름)이므로 동점 그룹에 넣지 않는다.
/// 정원 1 → 트랙A 선두 1명 확정(수동 아님).
#[test]
fn merge_tie_group_excludes_lower_track_rank_same_univ_rank() {
    let tracks = vec![vec![mc(1, 10, 1, 2), mc(2, 10, 2, 2)]];
    let out = merge_univ_cut(&tracks, Some(1));
    assert_eq!(sids(&out.confirmed), vec![1], "트랙 순서가 우열을 정함 — 동점 아님");
    assert_eq!(out.tie, None);
}

/// 트랙 내부 **진짜 동점**(track_rank 도 같음)은 원자적: 정원 1 → 아무도 확정 안 되고 수동.
#[test]
fn merge_intra_track_true_tie_is_atomic() {
    let tracks = vec![vec![mc(1, 10, 1, 2), mc(2, 10, 1, 2)]];
    let out = merge_univ_cut(&tracks, Some(1));
    assert!(out.confirmed.is_empty());
    assert_eq!(out.tie, Some(TieBoundary { rank: 2, free: 1, contenders: 2 }));
}

/// 트랙 간 동점이 경계를 가름 → 상위는 확정, 동점 그룹은 수동
#[test]
fn merge_cross_track_tie_splits_boundary() {
    let tracks = vec![vec![mc(1, 10, 1, 1), mc(2, 10, 2, 2)], vec![mc(3, 20, 1, 2)]];
    let out = merge_univ_cut(&tracks, Some(2));
    assert_eq!(sids(&out.confirmed), vec![1], "동점 위까지는 자동 확정");
    assert_eq!(out.tie, Some(TieBoundary { rank: 2, free: 1, contenders: 2 }));
}

/// 깨끗한 경계: 잔여가 정확히 0 이 되며 끝 → 수동 불필요
#[test]
fn merge_clean_boundary_no_tie() {
    let tracks = vec![vec![mc(1, 10, 1, 1), mc(2, 10, 2, 3)], vec![mc(3, 20, 1, 2)]];
    let out = merge_univ_cut(&tracks, Some(2));
    let mut got = sids(&out.confirmed);
    got.sort_unstable();
    assert_eq!(got, vec![1, 3]);
    assert_eq!(out.tie, None, "남은자리 0 — 깨끗한 경계");
}

/// 잔여 0 이하 → 아무도 확정 안 됨, 수동 없음 (fill_by_rank_groups 와 같은 의미)
#[test]
fn merge_zero_remaining_confirms_none() {
    let tracks = vec![vec![mc(1, 10, 1, 1)]];
    assert!(merge_univ_cut(&tracks, Some(0)).confirmed.is_empty());
    assert_eq!(merge_univ_cut(&tracks, Some(0)).tie, None);
    assert!(merge_univ_cut(&tracks, Some(-3)).confirmed.is_empty());
    assert_eq!(merge_univ_cut(&tracks, Some(-3)).tie, None);
}

/// 후보 없음 → 빈 결과, 수동 없음
#[test]
fn merge_empty_pool() {
    let out = merge_univ_cut(&[], Some(3));
    assert!(out.confirmed.is_empty());
    assert_eq!(out.tie, None);
}

/// **동작 불변 고정**: 대학 플래그와 모든 트랙 플래그가 일치하는 구성에서는
/// 대학 순위 순서와 트랙 내부 순서가 같다(같은 대학 순위 ⟺ 같은 트랙 순위).
/// 그런 입력에서 `merge_univ_cut` 은 기존 `fill_by_rank_groups`(전체 정렬)와
/// **정확히 같은 확정 집합·같은 동점 경계**를 낸다.
#[test]
fn merge_equals_fill_when_flags_align() {
    // 대학 순위 목록(동점 포함) — 플래그 일치 구성이므로 track_rank 도 같은 값을 쓴다
    let shapes: Vec<Vec<i64>> = vec![
        vec![1, 2, 3, 4],
        vec![1, 2, 2, 4],
        vec![1, 1, 1],
        vec![1, 2, 2, 4, 5, 5],
        vec![1],
    ];
    for ranks in shapes {
        for num_tracks in 1..=3usize {
            for rem in -1..=(ranks.len() as i64 + 1) {
                // 대학 순위 오름차순 전체 목록 (fill 입력)
                let flat: Vec<(i64, MergeCand)> = ranks
                    .iter()
                    .enumerate()
                    .map(|(i, &r)| {
                        let tid = (i % num_tracks) as i64;
                        (r, mc(i as i64, tid, r, r))
                    })
                    .collect();
                // 같은 후보를 트랙별로 분배 (각 트랙 내부는 대학 순위 오름차순 = 트랙 순서)
                let mut tracks: Vec<Vec<MergeCand>> = vec![Vec::new(); num_tracks];
                for (_, c) in &flat {
                    tracks[c.track_id as usize].push(c.clone());
                }

                let fill = fill_by_rank_groups(&flat, Some(rem));
                let merge = merge_univ_cut(&tracks, Some(rem));

                let mut a = sids(&fill.confirmed);
                let mut b = sids(&merge.confirmed);
                a.sort_unstable();
                b.sort_unstable();
                assert_eq!(
                    a, b,
                    "확정 집합 불일치: ranks={:?} tracks={} rem={}",
                    ranks, num_tracks, rem
                );
                assert_eq!(
                    fill.tie, merge.tie,
                    "동점 경계 불일치: ranks={:?} tracks={} rem={}",
                    ranks, num_tracks, rem
                );
            }
        }
    }
}

/// **감사 보강**: 위 등가성 테스트는 `track_rank == univ_rank` 로 두 값을 같게 만들기 때문에
/// 병합의 프리픽스 조건(`univ_rank == r` **그리고** `track_rank == head.track_rank`)의 두 절이
/// 항상 동시에 참/거짓이 되어, 두 값의 **번호 체계가 다른** 실제 데이터를 검증하지 못한다.
///
/// 실제로는 대학 순위(대학 파티션)와 모집단위 순위(트랙 파티션)의 **숫자가 다르다**
/// (예: 한 트랙의 대학 순위가 1·3·5 여도 그 트랙 내부 순위는 1·2·3). 플래그가 일치하면
/// **순서와 동점 관계만** 같다. 그 조건에서도 병합 = 기존 정렬임을 고정한다.
#[test]
fn merge_equals_fill_when_track_rank_numbering_differs() {
    let shapes: Vec<Vec<i64>> = vec![
        vec![1, 2, 3, 4],
        vec![1, 2, 2, 4],
        vec![1, 1, 1],
        vec![1, 2, 2, 4, 5, 5],
        vec![1, 1, 3, 3, 5],
        vec![1],
    ];
    for ranks in shapes {
        for num_tracks in 1..=3usize {
            for rem in -1..=(ranks.len() as i64 + 1) {
                let flat: Vec<(i64, MergeCand)> = ranks
                    .iter()
                    .enumerate()
                    .map(|(i, &r)| (r, mc(i as i64, (i % num_tracks) as i64, r, r)))
                    .collect();

                let mut tracks: Vec<Vec<MergeCand>> = vec![Vec::new(); num_tracks];
                for (_, c) in &flat {
                    tracks[c.track_id as usize].push(c.clone());
                }
                // 각 트랙 내부 순위를 **그 트랙 안에서 다시 매긴다** (표준 경쟁 순위).
                // 플래그가 일치하므로 트랙 내부의 동점 관계는 대학 순위의 동점 관계와 같고,
                // 순서도 같다 — 숫자만 달라진다.
                for list in tracks.iter_mut() {
                    let mut tr = 0i64;
                    for i in 0..list.len() {
                        if i == 0 || list[i].univ_rank != list[i - 1].univ_rank {
                            tr = (i + 1) as i64;
                        }
                        list[i].track_rank = tr;
                    }
                }

                let fill = fill_by_rank_groups(&flat, Some(rem));
                let merge = merge_univ_cut(&tracks, Some(rem));

                let mut a = sids(&fill.confirmed);
                let mut b = sids(&merge.confirmed);
                a.sort_unstable();
                b.sort_unstable();
                assert_eq!(
                    a, b,
                    "확정 집합 불일치: ranks={:?} tracks={} rem={}",
                    ranks, num_tracks, rem
                );
                assert_eq!(
                    fill.tie, merge.tie,
                    "동점 경계 불일치: ranks={:?} tracks={} rem={}",
                    ranks, num_tracks, rem
                );
            }
        }
    }
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

/// **D단계 규칙 고정 (기대값 반전)**: 대학 컷은 같은 트랙 안에서 track_rank 상위자를
/// 건너뛰고 하위자를 선택할 수 없다.
///
/// 이 테스트는 B단계 감사 시점에 **잘못된 당시 동작**(대학 컷이 전체를 대학 플래그로
/// 재정렬 → 같은 트랙 내부 순서가 뒤집힘)을 고정하고 있었다. D단계에서 규칙이 바뀌었으므로
/// 같은 시나리오의 기대값을 뒤집는다.
///
/// 트랙이 하나뿐이라 이 컷은 사실상 **트랙 내부 비교**다. 트랙 prioritize=1 이므로
/// 재학생이 트랙 선두이고, 졸업생은 자기 트랙 상위자에 막혀 선두가 아니다 → 재학생 확정.
#[tokio::test]
async fn auto_recommend_univ_cut_preserves_track_order_when_track_flag_differs() {
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
    // 모집단위 순위(트랙 플래그=1) = 재학생 우선 → 재학생 1위, 졸업생 2위
    app_result(&pool, grad, tid, rid, 1, 300_000).await;
    app_result(&pool, enr, tid, rid, 2, 100_000).await;

    let res = call_auto(&pool, rid).await;

    assert_eq!(res.manual.len(), 0, "경계 비동점 — 수동 없음");
    let total: i64 = res.confirmed.iter().map(|c| c.count).sum();
    assert_eq!(total, 1);
    assert!(
        get_recommended(&pool, enr, tid, rid).await,
        "트랙 내부 순서 보존 — 대학 컷이 트랙 상위자(재학생)를 건너뛸 수 없다",
    );
    assert!(
        !get_recommended(&pool, grad, tid, rid).await,
        "졸업생은 자기 트랙 상위자에 막힘 — 오류가 아니라 모집단위 재학생 우선의 정상 작동",
    );
}

// ── D단계: 트랙 내부 순서 보존 k-way 병합 ────────────────────────

/// **트랙 간 새치기 없음**: 대학 정원 2, 대학 prioritize=0.
/// 의학A(트랙 prioritize=1): 재학 100점(트랙 1위, 대학 3위), 졸업 300점(트랙 2위, 대학 1위)
/// 트랙B(prioritize=0): X 200점(대학 2위)
/// → 확정 = {X, 재학100}. A의 졸업 300점은 대학 1위지만 자기 트랙 선두에 막혀 탈락한다.
///   (탈락은 manual 사유로 올리지 않는다 — 그 모집단위 정책의 정상 결과)
#[tokio::test]
async fn auto_recommend_univ_cut_no_cross_track_queue_jumping() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let univ = new_univ(&pool, "A대", Some(2)).await;
    let t_med = sqlx::query_scalar::<_, i64>(
        "INSERT INTO univ_tracks (univ_id, track_name, unit_quota, prioritize_enrolled) \
         VALUES (?, '의학', NULL, 1) RETURNING id",
    )
    .bind(univ)
    .fetch_one(&pool)
    .await
    .unwrap();
    let t_b = new_track(&pool, univ, "전기", None).await;
    let enr = new_student(&pool, "E1", 1, 1, 1).await;
    let grad = new_graduate(&pool, "G1", 2024).await;
    let x = new_student(&pool, "X1", 1, 1, 2).await;
    let rid = new_closed_round(&pool).await;
    app_result(&pool, grad, t_med, rid, 1, 300_000).await;
    app_result(&pool, x, t_b, rid, 2, 200_000).await;
    app_result(&pool, enr, t_med, rid, 3, 100_000).await;

    let res = call_auto(&pool, rid).await;

    assert_eq!(res.manual.len(), 0, "막힌 후보는 수동 사유가 아니다");
    let total: i64 = res.confirmed.iter().map(|c| c.count).sum();
    assert_eq!(total, 2);
    assert!(get_recommended(&pool, x, t_b, rid).await, "타 트랙 선두 — 대학 2위");
    assert!(get_recommended(&pool, enr, t_med, rid).await, "의학 트랙 선두(재학생 우선)");
    assert!(
        !get_recommended(&pool, grad, t_med, rid).await,
        "대학 1위여도 자기 트랙 선두에 막힘 — 이번 라운드 미추천",
    );
}

/// **동점 원자 처리는 선두들끼리만**: 서로 다른 트랙의 선두 2명이 같은 대학 순위인데
/// 잔여 1석 → 그 대학 manual, 둘 다 미확정.
#[tokio::test]
async fn auto_recommend_univ_merge_tie_among_heads_is_manual() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let univ = new_univ(&pool, "A대", Some(1)).await;
    let t1 = new_track(&pool, univ, "컴공", None).await;
    let t2 = new_track(&pool, univ, "전기", None).await;
    let s1 = new_student(&pool, "S1", 1, 1, 1).await;
    let s2 = new_student(&pool, "S2", 1, 1, 2).await;
    let rid = new_closed_round(&pool).await;
    // 서로 다른 트랙의 선두 둘이 대학 1위 동점
    app_result(&pool, s1, t1, rid, 1, 300_000).await;
    app_result(&pool, s2, t2, rid, 1, 300_000).await;

    let res = call_auto(&pool, rid).await;

    assert_eq!(res.manual.len(), 1, "대학 단위 manual 1건");
    assert_eq!(res.manual[0].track_id, None, "대학 단위 — 트랙 없음");
    let reason = &res.manual[0].reason;
    assert!(reason.contains("대학 전체 1위"), "사유에 순위: {}", reason);
    assert!(reason.contains("잔여 1석"), "사유에 잔여석: {}", reason);
    assert!(reason.contains("2명 경합"), "사유에 경합 인원: {}", reason);
    assert!(!get_recommended(&pool, s1, t1, rid).await, "동점은 아무도 자동 확정 안 됨");
    assert!(!get_recommended(&pool, s2, t2, rid).await);
}

/// **트랙 내부 동점은 여전히 원자적**: 같은 트랙에서 track_rank 가 같은(진짜 동점) 두 명이
/// 병합 선두일 때 잔여 1석 → manual. 트랙 순서가 우열을 못 정하므로 시스템이 고를 수 없다.
#[tokio::test]
async fn auto_recommend_univ_merge_intra_track_tie_is_atomic() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let univ = new_univ(&pool, "A대", Some(1)).await;
    let tid = new_track(&pool, univ, "컴공", None).await;
    let a = new_student(&pool, "A1", 1, 1, 1).await;
    let b = new_student(&pool, "B1", 1, 1, 2).await;
    let rid = new_closed_round(&pool).await;
    // 같은 트랙·같은 점수 → 트랙 순위도 대학 순위도 동점
    app_result(&pool, a, tid, rid, 1, 300_000).await;
    app_result(&pool, b, tid, rid, 1, 300_000).await;

    let res = call_auto(&pool, rid).await;

    assert_eq!(res.manual.len(), 1);
    assert!(res.manual[0].reason.contains("대학 전체 1위"), "{}", res.manual[0].reason);
    assert!(!get_recommended(&pool, a, tid, rid).await);
    assert!(!get_recommended(&pool, b, tid, rid).await);
}

/// **깨끗한 경계**: 병합이 잔여 0에서 정확히 끝나면 수동 불필요.
/// 대학 정원 2, 트랙 2개 · 대학 순위 [1,2,3] → 상위 2명 확정, 3위는 조용히 미추천.
#[tokio::test]
async fn auto_recommend_univ_merge_clean_boundary_no_manual() {
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
    app_result(&pool, s3, t2, rid, 3, 100_000).await;

    let res = call_auto(&pool, rid).await;

    assert_eq!(res.manual.len(), 0, "잔여 0 — 깨끗한 경계, 수동 불필요");
    let total: i64 = res.confirmed.iter().map(|c| c.count).sum();
    assert_eq!(total, 2);
    assert!(get_recommended(&pool, s1, t1, rid).await);
    assert!(get_recommended(&pool, s2, t2, rid).await);
    assert!(!get_recommended(&pool, s3, t2, rid).await);
}

/// **동작 불변**: 대학 플래그와 모든 트랙 플래그가 일치하면(여기선 전부 1)
/// 대학 순위와 트랙 순서가 같으므로 병합 결과 = 기존 전체 정렬 결과.
/// 대학 정원 2 · 트랙 무제한 2개, 재학생 우선 전면 ON → 재학생 2명이 졸업생 고득점자를 이긴다.
#[tokio::test]
async fn auto_recommend_univ_merge_matches_sort_when_flags_align() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let univ = new_univ(&pool, "A대", Some(2)).await;
    let t1 = new_track(&pool, univ, "컴공", None).await;
    let t2 = new_track(&pool, univ, "전기", None).await;
    set_univ_prioritize(&pool, univ).await; // 대학=1 ⇒ 모든 트랙=1
    let e1 = new_student(&pool, "E1", 1, 1, 1).await;
    let e2 = new_student(&pool, "E2", 1, 1, 2).await;
    let g1 = new_graduate(&pool, "G1", 2024).await;
    let rid = new_closed_round(&pool).await;
    // 대학 순위 = 재학생 우선 → e1(1), e2(2), g1(3) — 점수는 g1 이 최고
    app_result(&pool, e1, t1, rid, 1, 200_000).await;
    app_result(&pool, e2, t2, rid, 2, 100_000).await;
    app_result(&pool, g1, t1, rid, 3, 900_000).await;

    let res = call_auto(&pool, rid).await;

    assert_eq!(res.manual.len(), 0);
    let total: i64 = res.confirmed.iter().map(|c| c.count).sum();
    assert_eq!(total, 2);
    assert!(get_recommended(&pool, e1, t1, rid).await);
    assert!(get_recommended(&pool, e2, t2, rid).await);
    assert!(!get_recommended(&pool, g1, t1, rid).await, "플래그 일치 구성 — 기존 정렬과 동일");
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
