mod common;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
};
use principal_candidate_manager::handlers::universities::{
    create_track, delete_track, delete_university, export_quota_stats, update_track,
    CreateTrackBody, ExportQuotaQuery, UpdateTrackBody,
};

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

// ── export_quota_stats ─────────────────────────────────────────────

async fn export_bytes(pool: &sqlx::SqlitePool, univ_id: Option<i64>) -> (Vec<u8>, String) {
    let q = Query(ExportQuotaQuery { univ_id });
    let resp = export_quota_stats(State(common::make_state(pool.clone())), q)
        .await
        .unwrap();
    let cd = String::from_utf8(
        resp.headers()
            .get("content-disposition")
            .unwrap()
            .as_bytes()
            .to_vec(),
    )
    .unwrap();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec();
    (bytes, cd)
}

fn parse_rows(bytes: &[u8]) -> Vec<Vec<String>> {
    principal_candidate_manager::excel::parse_xlsx_all_rows_raw(bytes).unwrap()
}

#[tokio::test]
async fn export_quota_stats_all_returns_all_tracks() {
    let pool = common::create_test_pool().await;
    let uid1 = insert_univ(&pool, "한국대").await;
    let uid2 = insert_univ(&pool, "서울대").await;
    insert_track(&pool, uid1, "컴공").await;
    insert_track(&pool, uid1, "전자").await;
    insert_track(&pool, uid2, "경제").await;

    let (bytes, cd) = export_bytes(&pool, None).await;
    let rows = parse_rows(&bytes);
    // 헤더 1행 + 모집단위 3행
    assert_eq!(rows.len(), 4, "헤더+3개 모집단위");
    let body: Vec<&Vec<String>> = rows.iter().skip(1).collect();
    let univ_names: Vec<&str> = body.iter().map(|r| r[0].as_str()).collect();
    assert!(univ_names.contains(&"한국대"), "한국대 포함");
    assert!(univ_names.contains(&"서울대"), "서울대 포함");
    assert!(cd.contains("전체_추천현황"), "파일명에 '전체_추천현황' 포함: {cd}");
}

#[tokio::test]
async fn export_quota_stats_filtered_returns_one_univ() {
    let pool = common::create_test_pool().await;
    let uid1 = insert_univ(&pool, "한국대").await;
    let uid2 = insert_univ(&pool, "서울대").await;
    insert_track(&pool, uid1, "컴공").await;
    insert_track(&pool, uid2, "경제").await;
    insert_track(&pool, uid2, "법학").await;

    let (bytes, cd) = export_bytes(&pool, Some(uid2)).await;
    let rows = parse_rows(&bytes);
    // 헤더 1행 + 서울대 모집단위 2행
    assert_eq!(rows.len(), 3, "헤더+서울대 2개 모집단위");
    let univ_names: Vec<&str> = rows.iter().skip(1).map(|r| r[0].as_str()).collect();
    assert!(univ_names.iter().all(|&n| n == "서울대"), "서울대 행만 존재");
    let flat = rows.iter().skip(1).flat_map(|r| r.iter().map(String::as_str)).collect::<Vec<_>>();
    assert!(!flat.contains(&"한국대"), "한국대 미포함");
    assert!(cd.contains("서울대"), "파일명에 대학명 포함: {cd}");
    assert!(cd.contains("_추천현황_"), "파일명 패턴: {cd}");
}

#[tokio::test]
async fn export_quota_stats_content_disposition_all_vs_filtered() {
    let pool = common::create_test_pool().await;
    let uid = insert_univ(&pool, "고려대").await;
    insert_track(&pool, uid, "의대").await;

    let (_, cd_all) = export_bytes(&pool, None).await;
    let (_, cd_filtered) = export_bytes(&pool, Some(uid)).await;

    assert!(cd_all.contains("전체_추천현황"), "전체 경로 파일명: {cd_all}");
    assert!(cd_filtered.contains("고려대"), "필터 경로 파일명에 대학명: {cd_filtered}");
    assert!(cd_filtered.contains("_추천현황_"), "필터 경로 파일명 패턴: {cd_filtered}");
}

#[tokio::test]
async fn export_quota_stats_empty_db_returns_header_only() {
    let pool = common::create_test_pool().await;

    let (bytes, cd) = export_bytes(&pool, None).await;
    let rows = parse_rows(&bytes);
    // 헤더 행만 존재
    assert_eq!(rows.len(), 1, "빈 DB → 헤더 행만");
    assert_eq!(rows[0][0], "대학명", "첫 번째 헤더 열");
    assert!(cd.contains("전체_추천현황"), "빈 DB 파일명: {cd}");
}

// ── prioritize_enrolled 불변식 트리거 (DB 레벨) ────────────────────

#[tokio::test]
async fn trigger_cascade_univ_0_to_1_updates_all_tracks() {
    // 대학 prioritize 0→1 UPDATE 시 그 대학의 모든 트랙이 1로 cascade되어야 함
    let pool = common::create_test_pool().await;
    let uid = insert_univ(&pool, "한국대").await;
    // 트랙 2개를 prioritize=0으로 생성
    let tid1 = insert_track(&pool, uid, "컴공").await;
    let tid2 = insert_track(&pool, uid, "전자").await;

    sqlx::query("UPDATE universities SET prioritize_enrolled = 1 WHERE id = ?")
        .bind(uid).execute(&pool).await.unwrap();

    let pe1: i64 = sqlx::query_scalar("SELECT prioritize_enrolled FROM univ_tracks WHERE id = ?")
        .bind(tid1).fetch_one(&pool).await.unwrap();
    let pe2: i64 = sqlx::query_scalar("SELECT prioritize_enrolled FROM univ_tracks WHERE id = ?")
        .bind(tid2).fetch_one(&pool).await.unwrap();
    assert_eq!(pe1, 1, "트랙1이 cascade되어야 함");
    assert_eq!(pe2, 1, "트랙2이 cascade되어야 함");
}

#[tokio::test]
async fn trigger_insert_guard_blocks_track_prioritize_0_when_univ_1() {
    // 대학=1인 상태에서 트랙 prioritize=0 INSERT → 에러 발생
    let pool = common::create_test_pool().await;
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name, prioritize_enrolled) VALUES ('한국대', 1) RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();

    let res = sqlx::query(
        "INSERT INTO univ_tracks (univ_id, track_name, prioritize_enrolled) VALUES (?, '컴공', 0)",
    )
    .bind(uid).execute(&pool).await;
    assert!(res.is_err(), "대학=1에서 트랙 prioritize=0 INSERT는 실패해야 함");
}

#[tokio::test]
async fn trigger_update_guard_blocks_track_prioritize_downgrade_when_univ_1() {
    // 대학=1에서 트랙 prioritize=0으로 UPDATE → 에러 발생
    let pool = common::create_test_pool().await;
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name, prioritize_enrolled) VALUES ('한국대', 1) RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();
    // 대학=1이므로 트랙도 prioritize=1로 직접 삽입
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name, prioritize_enrolled) VALUES (?, '컴공', 1) RETURNING id",
    )
    .bind(uid).fetch_one(&pool).await.unwrap();

    let res = sqlx::query("UPDATE univ_tracks SET prioritize_enrolled = 0 WHERE id = ?")
        .bind(tid).execute(&pool).await;
    assert!(res.is_err(), "대학=1에서 트랙 prioritize=0 UPDATE는 실패해야 함");
}

#[tokio::test]
async fn trigger_insert_ok_when_univ_1_and_track_1() {
    // 대학=1이어도 트랙 prioritize=1 INSERT는 정상 통과
    let pool = common::create_test_pool().await;
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name, prioritize_enrolled) VALUES ('한국대', 1) RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();

    let res = sqlx::query(
        "INSERT INTO univ_tracks (univ_id, track_name, prioritize_enrolled) VALUES (?, '컴공', 1)",
    )
    .bind(uid).execute(&pool).await;
    assert!(res.is_ok(), "트랙 prioritize=1 INSERT는 통과해야 함");
}

#[tokio::test]
async fn trigger_univ_1_to_0_allows_track_edit() {
    // 대학 1→0으로 변경 후 트랙 prioritize=0으로 UPDATE 가능
    let pool = common::create_test_pool().await;
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name, prioritize_enrolled) VALUES ('한국대', 1) RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();
    // 대학=1이므로 트랙도 prioritize=1로 직접 삽입
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name, prioritize_enrolled) VALUES (?, '컴공', 1) RETURNING id",
    )
    .bind(uid).fetch_one(&pool).await.unwrap();

    // 대학을 1→0으로
    sqlx::query("UPDATE universities SET prioritize_enrolled = 0 WHERE id = ?")
        .bind(uid).execute(&pool).await.unwrap();

    // 이제 트랙 0으로 변경 가능해야 함
    let res = sqlx::query("UPDATE univ_tracks SET prioritize_enrolled = 0 WHERE id = ?")
        .bind(tid).execute(&pool).await;
    assert!(res.is_ok(), "대학=0이면 트랙 0 UPDATE 가능해야 함");
}

/// **양방향 cascade (D단계)**: 대학 1→0 이면 그 대학 모든 트랙도 0 으로 되돌린다.
/// 그 트랙들의 1 은 관리자가 고른 값이 아니라 0→1 cascade 가 강제한 값이므로,
/// 되돌리지 않으면 "대학 재학생 우선을 껐는데 전 모집단위가 여전히 우선"이 된다.
#[tokio::test]
async fn trigger_cascade_univ_1_to_0_clears_all_tracks() {
    let pool = common::create_test_pool().await;
    let uid = insert_univ(&pool, "한국대").await;
    let tid1 = insert_track(&pool, uid, "컴공").await;
    let tid2 = insert_track(&pool, uid, "전자").await;

    // 0→1 (트랙 전부 1 로 cascade)
    sqlx::query("UPDATE universities SET prioritize_enrolled = 1 WHERE id = ?")
        .bind(uid).execute(&pool).await.unwrap();
    // 1→0 (트랙 전부 0 으로 되돌아와야 함)
    sqlx::query("UPDATE universities SET prioritize_enrolled = 0 WHERE id = ?")
        .bind(uid).execute(&pool).await.unwrap();

    let pe1: i64 = sqlx::query_scalar("SELECT prioritize_enrolled FROM univ_tracks WHERE id = ?")
        .bind(tid1).fetch_one(&pool).await.unwrap();
    let pe2: i64 = sqlx::query_scalar("SELECT prioritize_enrolled FROM univ_tracks WHERE id = ?")
        .bind(tid2).fetch_one(&pool).await.unwrap();
    assert_eq!(pe1, 0, "대학 1→0 이면 트랙1도 0 으로 cascade");
    assert_eq!(pe2, 0, "대학 1→0 이면 트랙2도 0 으로 cascade");
}

/// 양방향 cascade 는 대학=0 상태의 **트랙별 개별 설정을 막지 않는다**.
/// (대학=0 · 트랙=1 = "이 모집단위만 재학생 우선" — D2 에서 허용된 정상 구성)
#[tokio::test]
async fn track_prioritize_1_allowed_while_univ_0() {
    let pool = common::create_test_pool().await;
    let uid = insert_univ(&pool, "한국대").await; // prioritize 0
    let tid1 = insert_track(&pool, uid, "의학").await;
    let tid2 = insert_track(&pool, uid, "전자").await;

    let res = sqlx::query("UPDATE univ_tracks SET prioritize_enrolled = 1 WHERE id = ?")
        .bind(tid1).execute(&pool).await;
    assert!(res.is_ok(), "대학=0 에서 트랙 개별 1 설정은 허용");

    let pe1: i64 = sqlx::query_scalar("SELECT prioritize_enrolled FROM univ_tracks WHERE id = ?")
        .bind(tid1).fetch_one(&pool).await.unwrap();
    let pe2: i64 = sqlx::query_scalar("SELECT prioritize_enrolled FROM univ_tracks WHERE id = ?")
        .bind(tid2).fetch_one(&pool).await.unwrap();
    assert_eq!(pe1, 1, "그 모집단위만 재학생 우선");
    assert_eq!(pe2, 0, "다른 모집단위는 영향 없음");
    let upe: i64 = sqlx::query_scalar("SELECT prioritize_enrolled FROM universities WHERE id = ?")
        .bind(uid).fetch_one(&pool).await.unwrap();
    assert_eq!(upe, 0, "대학은 0 유지");
}

/// 대학 값이 **바뀌지 않는** UPDATE 는 cascade 하지 않는다 (0→0).
/// 대학=0 에서 관리자가 고른 트랙별 1 이 무관한 대학 UPDATE 로 지워지면 안 된다.
#[tokio::test]
async fn trigger_no_cascade_when_univ_value_unchanged() {
    let pool = common::create_test_pool().await;
    let uid = insert_univ(&pool, "한국대").await;
    let tid = insert_track(&pool, uid, "의학").await;
    sqlx::query("UPDATE univ_tracks SET prioritize_enrolled = 1 WHERE id = ?")
        .bind(tid).execute(&pool).await.unwrap();

    // 값이 같은 UPDATE (0→0)
    sqlx::query("UPDATE universities SET prioritize_enrolled = 0 WHERE id = ?")
        .bind(uid).execute(&pool).await.unwrap();

    let pe: i64 = sqlx::query_scalar("SELECT prioritize_enrolled FROM univ_tracks WHERE id = ?")
        .bind(tid).fetch_one(&pool).await.unwrap();
    assert_eq!(pe, 1, "값 무변경 UPDATE 는 관리자가 고른 트랙 설정을 건드리지 않는다");
}

// ── create_track / update_track 핸들러 가드 ────────────────────────

#[tokio::test]
async fn create_track_handler_400_when_univ_prioritize_and_track_0() {
    // 대학=1인데 트랙 prioritize=false로 생성 → 핸들러에서 친절한 400
    let pool = common::create_test_pool().await;
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name, prioritize_enrolled) VALUES ('한국대', 1) RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();

    let body = CreateTrackBody {
        track_name: "컴공".to_string(),
        unit_quota: None,
        prioritize_enrolled: false,
    };
    let res = create_track(
        State(common::make_state(pool)),
        axum::extract::Path(uid),
        axum::Json(body),
    )
    .await;
    assert_eq!(res.unwrap_err().0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn update_track_handler_400_when_univ_prioritize_and_downgrade() {
    // 대학=1인 트랙을 prioritize=false로 UPDATE → 핸들러 400
    let pool = common::create_test_pool().await;
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name, prioritize_enrolled) VALUES ('한국대', 1) RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name, prioritize_enrolled) VALUES (?, '컴공', 1) RETURNING id",
    )
    .bind(uid).fetch_one(&pool).await.unwrap();

    let body = UpdateTrackBody {
        track_name: None,
        unit_quota: None,
        prioritize_enrolled: Some(false),
    };
    let res = update_track(
        State(common::make_state(pool)),
        axum::extract::Path(tid),
        axum::Json(body),
    )
    .await;
    assert_eq!(res.unwrap_err().0, StatusCode::BAD_REQUEST);
}
