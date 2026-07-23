mod common;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
};
use principal_candidate_manager::handlers::universities::{
    create_track, delete_track, delete_university, export_quota_stats, get_quota_stats,
    get_track_recommended_list, update_track, update_university, CreateTrackBody,
    ExportQuotaQuery, UpdateTrackBody, UpdateUnivBody,
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

/// 잔여석 픽스처: 대학 정원 3 / 트랙A 정원 2 · 트랙B 정원 1.
/// 라운드1에서 A에 1명 확정, 라운드2에서 A에 1명 확정 + B에 1명 확정 후 포기.
/// 기대 집계 — A: unit_used 2, B: unit_used 0(포기는 안 셈), 대학 total_used 2.
async fn setup_quota_fixture(pool: &sqlx::SqlitePool) -> (i64, i64, i64, i64, i64) {
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name, total_quota) VALUES ('한국대', 3) RETURNING id",
    )
    .fetch_one(pool).await.unwrap();
    let ta: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name, unit_quota) VALUES (?, 'A컴공', 2) RETURNING id",
    ).bind(uid).fetch_one(pool).await.unwrap();
    let tb: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name, unit_quota) VALUES (?, 'B전자', 1) RETURNING id",
    ).bind(uid).fetch_one(pool).await.unwrap();

    let r1 = insert_round(pool).await;
    sqlx::query("UPDATE rounds SET status = 'FINALIZED' WHERE id = ?")
        .bind(r1).execute(pool).await.unwrap();
    let r2 = insert_round(pool).await;

    let hash = bcrypt::hash("pass", 4u32).unwrap();
    sqlx::query("INSERT INTO classes (grade, class_no, password_hash) VALUES (1, 1, ?) ON CONFLICT DO NOTHING")
        .bind(&hash).execute(pool).await.unwrap();

    // (학생, 트랙, 라운드, recommended, abandoned)
    let plan = [
        ("S001", ta, r1, 1, 0),
        ("S002", ta, r2, 1, 0),
        ("S003", tb, r2, 1, 1), // 확정 후 포기 → 잔여석 집계에서 빠져야 한다
        ("S004", tb, r2, 0, 0), // 미추천 → 집계 대상 아님
    ];
    for (seq, (code, tid, rid, rec, aband)) in plan.into_iter().enumerate() {
        // 공용 insert_student 는 seq_no 를 1로 고정하므로 여기서는 직접 넣는다
        // (학급 내 seq_no 는 유일해야 한다 — idx_students_position)
        let sid: i64 = sqlx::query_scalar(
            "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
             VALUES (?, '학생', 1, 1, ?, 1) RETURNING id",
        ).bind(code).bind(seq as i64 + 1).fetch_one(pool).await.unwrap();
        sqlx::query(
            "INSERT INTO applications (student_id, track_id, round_id, abandoned) VALUES (?, ?, ?, ?)",
        ).bind(sid).bind(tid).bind(rid).bind(aband).execute(pool).await.unwrap();
        sqlx::query(
            "INSERT INTO results (student_id, track_id, round_id, score_detail, total_score, \
             ranking, recommended, calculated_at) VALUES (?, ?, ?, '{}', 0, 1, ?, '2025-01-09')",
        ).bind(sid).bind(tid).bind(rid).bind(rec).execute(pool).await.unwrap();
    }
    (uid, ta, tb, r1, r2)
}

/// `get_quota_stats` 는 관리자 "정원 현황" 화면이 잔여석을 읽는 유일한 경로인데
/// 호출하는 테스트가 하나도 없었다. 집계식(recommended=1 AND abandoned=0)이
/// 조용히 바뀌어도 스위트가 초록이었다.
#[tokio::test]
async fn get_quota_stats_counts_recommended_excluding_abandoned() {
    let pool = common::create_test_pool().await;
    let (_uid, ta, tb, r1, r2) = setup_quota_fixture(&pool).await;

    let axum::Json(stats) = get_quota_stats(State(common::make_state(pool))).await.unwrap();

    assert_eq!(stats.all_round_ids, vec![r1, r2], "추천이 있었던 라운드만 오름차순");
    assert_eq!(stats.univs.len(), 1);
    let u = &stats.univs[0];
    assert_eq!(u.total_quota, Some(3));
    assert_eq!(u.total_used, 2, "포기자(S003)와 미추천자(S004)는 대학 집계에서 빠진다");

    let a = u.tracks.iter().find(|t| t.track_id == ta).expect("A트랙");
    assert_eq!((a.unit_quota, a.unit_used), (Some(2), 2), "A: 정원 2, 사용 2 → 잔여 0");
    let b = u.tracks.iter().find(|t| t.track_id == tb).expect("B트랙");
    assert_eq!((b.unit_quota, b.unit_used), (Some(1), 0), "B: 포기로 반환되어 사용 0 → 잔여 1");

    // 라운드별 내역: A는 두 라운드에 1명씩, B는 포기라 내역 없음
    assert_eq!(
        a.by_round.iter().map(|c| (c.round_id, c.count)).collect::<Vec<_>>(),
        vec![(r1, 1), (r2, 1)],
        "A 라운드별 추천 인원",
    );
    assert!(b.by_round.is_empty(), "B는 포기라 라운드별 내역이 없어야 함: {:?}",
        b.by_round.iter().map(|c| (c.round_id, c.count)).collect::<Vec<_>>());
}

// ── get_track_recommended_list ─────────────────────────────────────
//
// 모집단위별 "추천 확정 학생 목록"은 관리자가 정원 현황에서 열어보는 화면인데
// 이 핸들러를 호출하는 테스트가 하나도 없었다. 필터(recommended=1)와 정렬
// (round_id → ranking NULLS LAST → name)이 조용히 바뀌어도 스위트가 초록이었다.

/// (라운드 상태, 모집단위) 위에 결과 행을 만든다. rec/aband/ranking 을 그대로 반영.
async fn seed_result(
    pool: &sqlx::SqlitePool,
    track_id: i64,
    round_id: i64,
    code: &str,
    name: &str,
    seq_no: i64,
    ranking: Option<i64>,
    rec: i64,
    aband: i64,
) -> i64 {
    let sid: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES (?, ?, 1, 1, ?, 1) RETURNING id",
    )
    .bind(code).bind(name).bind(seq_no)
    .fetch_one(pool).await.unwrap();
    sqlx::query("INSERT INTO applications (student_id, track_id, round_id, abandoned) VALUES (?, ?, ?, ?)")
        .bind(sid).bind(track_id).bind(round_id).bind(aband)
        .execute(pool).await.unwrap();
    sqlx::query(
        "INSERT INTO results (student_id, track_id, round_id, score_detail, total_score, \
         ranking, recommended, calculated_at) VALUES (?, ?, ?, '{}', 0, ?, ?, '2025-01-09')",
    )
    .bind(sid).bind(track_id).bind(round_id).bind(ranking).bind(rec)
    .execute(pool).await.unwrap();
    sid
}

async fn two_rounds(pool: &sqlx::SqlitePool) -> (i64, i64) {
    let r1: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at, finalized_at) \
         VALUES ('FINALIZED', '2025-01-01', '2025-01-05') RETURNING id",
    ).fetch_one(pool).await.unwrap();
    let r2: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at, closed_at) \
         VALUES ('CLOSED', '2025-02-01', '2025-02-05') RETURNING id",
    ).fetch_one(pool).await.unwrap();
    (r1, r2)
}

#[tokio::test]
async fn get_track_recommended_list_returns_only_recommended_rows_with_abandoned_flag() {
    let pool = common::create_test_pool().await;
    let hash = bcrypt::hash("pass", 4u32).unwrap();
    sqlx::query("INSERT INTO classes (grade, class_no, password_hash) VALUES (1, 1, ?)")
        .bind(&hash).execute(&pool).await.unwrap();
    let uid = insert_univ(&pool, "한국대").await;
    let tid = insert_track(&pool, uid, "컴공").await;
    let other = insert_track(&pool, uid, "전자").await;
    let (r1, r2) = two_rounds(&pool).await;

    let rec = seed_result(&pool, tid, r1, "S001", "확정자", 1, Some(1), 1, 0).await;
    let _not_rec = seed_result(&pool, tid, r1, "S002", "미확정자", 2, Some(2), 0, 0).await;
    // 포기자는 정원에서 빠지지만 "추천했었다"는 이력은 남는다 → 목록에는 표시하되 플래그로 구분
    let aband = seed_result(&pool, tid, r2, "S003", "포기자", 3, Some(5), 1, 1).await;
    // 다른 모집단위 확정자는 새면 안 된다
    let _other = seed_result(&pool, other, r2, "S004", "타트랙", 4, Some(1), 1, 0).await;

    let axum::Json(rows) =
        get_track_recommended_list(State(common::make_state(pool)), Path(tid)).await.unwrap();

    assert_eq!(
        rows.iter().map(|r| (r.student_id, r.abandoned)).collect::<Vec<_>>(),
        vec![(rec, false), (aband, true)],
        "recommended=1 행만, 포기 여부는 플래그로: {:?}",
        rows.iter().map(|r| (&r.name, r.abandoned)).collect::<Vec<_>>(),
    );
    assert_eq!(rows[0].student_code, "S001");
    assert_eq!(rows[0].ranking, Some(1));
    assert_eq!(rows[0].is_enrolled, true);
}

#[tokio::test]
async fn get_track_recommended_list_orders_by_round_then_ranking_nulls_last() {
    let pool = common::create_test_pool().await;
    let hash = bcrypt::hash("pass", 4u32).unwrap();
    sqlx::query("INSERT INTO classes (grade, class_no, password_hash) VALUES (1, 1, ?)")
        .bind(&hash).execute(&pool).await.unwrap();
    let uid = insert_univ(&pool, "한국대").await;
    let tid = insert_track(&pool, uid, "컴공").await;
    let (r1, r2) = two_rounds(&pool).await;

    // 삽입 순서를 기대 정렬과 일부러 어긋나게 둔다 — ORDER BY 가 없어도 통과하면 안 된다.
    //
    // 이름은 **기대 정렬의 역순**이 되도록 A~D 접두어를 붙인다. 이름을 설명적으로
    // ("라운드1_1위" 식) 지으면 이름 오름차순이 기대 정렬과 우연히 일치해서,
    // ORDER BY 가 `s.name` 하나로 줄어들어도 테스트가 통과해 버린다(실제로 그랬다).
    // 같은 라운드·같은 순위 두 명(S005, S003)으로 세 번째 키인 이름 정렬을,
    // 순위 미계산(S004)으로 NULLS LAST 를 각각 고정한다.
    seed_result(&pool, tid, r2, "S004", "A_2차_순위없음", 4, None, 1, 0).await;
    seed_result(&pool, tid, r1, "S002", "C_1차_2위", 2, Some(2), 1, 0).await;
    seed_result(&pool, tid, r2, "S003", "B_2차_1위", 3, Some(1), 1, 0).await;
    seed_result(&pool, tid, r1, "S001", "D_1차_1위", 1, Some(1), 1, 0).await;
    seed_result(&pool, tid, r2, "S005", "A_2차_1위동점", 5, Some(1), 1, 0).await;

    let axum::Json(rows) =
        get_track_recommended_list(State(common::make_state(pool)), Path(tid)).await.unwrap();

    assert_eq!(
        rows.iter().map(|r| (r.round_id, r.student_code.as_str())).collect::<Vec<_>>(),
        vec![(r1, "S001"), (r1, "S002"), (r2, "S005"), (r2, "S003"), (r2, "S004")],
        "라운드 오름차순 → 순위 오름차순 → (동점은 이름 오름차순) → 순위 미계산은 맨 뒤",
    );
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

// export_quota_stats 는 2시트(①"전체 명단" ②"정원 현황")를 내보낸다.
// 정원·잔여석 집계는 두 번째 "정원 현황" 시트에 있으므로 그 시트를 직접 연다
// (첫 시트를 읽으면 지원자 명단이 나온다).
fn parse_rows(bytes: &[u8]) -> Vec<Vec<String>> {
    principal_candidate_manager::excel::parse_xlsx_sheet_rows(bytes, "정원 현황").unwrap()
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
    assert!(cd.contains("전체_명단_정원현황"), "파일명에 '전체_명단_정원현황' 포함: {cd}");
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
    assert!(cd.contains("_명단_정원현황_"), "파일명 패턴: {cd}");
}

#[tokio::test]
async fn export_quota_stats_content_disposition_all_vs_filtered() {
    let pool = common::create_test_pool().await;
    let uid = insert_univ(&pool, "고려대").await;
    insert_track(&pool, uid, "의대").await;

    let (_, cd_all) = export_bytes(&pool, None).await;
    let (_, cd_filtered) = export_bytes(&pool, Some(uid)).await;

    assert!(cd_all.contains("전체_명단_정원현황"), "전체 경로 파일명: {cd_all}");
    assert!(cd_filtered.contains("고려대"), "필터 경로 파일명에 대학명: {cd_filtered}");
    assert!(cd_filtered.contains("_명단_정원현황_"), "필터 경로 파일명 패턴: {cd_filtered}");
}

/// 기존 export 테스트들은 행 수·대학명·파일명만 봤고 픽스처에 라운드도 results 도
/// 없어서 **정원 산술이 한 번도 실행되지 않았다**. 잔여인원 계산식
/// `(quota - used).max(0)` 이 틀려도 전 스위트가 초록이었다.
#[tokio::test]
async fn export_quota_stats_cells_carry_remaining_seat_numbers() {
    let pool = common::create_test_pool().await;
    let _ = setup_quota_fixture(&pool).await;

    let (bytes, _) = export_bytes(&pool, None).await;
    let rows = parse_rows(&bytes);
    let header = &rows[0];
    let col_of = |h: &str| header.iter().position(|c| c == h)
        .unwrap_or_else(|| panic!("헤더에 '{}' 없음: {:?}", h, header));

    // 값이 "어느 열 아래에" 있는지까지 결합해서 단언한다
    let (c_track, c_quota, c_used, c_left) =
        (col_of("모집단위"), col_of("모집단위 정원"), col_of("추천인원"), col_of("잔여인원"));
    let (c_uq, c_uused, c_uleft) =
        (col_of("대학 전체 정원"), col_of("대학 추천인원"), col_of("대학 잔여인원"));

    let a = rows[1..].iter().find(|r| r[c_track] == "A컴공").expect("A컴공 행");
    assert_eq!(
        (a[c_quota].as_str(), a[c_used].as_str(), a[c_left].as_str()),
        ("2", "2", "0"),
        "A: 정원 2 · 추천 2 · 잔여 0 이어야 함: {a:?}",
    );

    let b = rows[1..].iter().find(|r| r[c_track] == "B전자").expect("B전자 행");
    assert_eq!(
        (b[c_quota].as_str(), b[c_used].as_str(), b[c_left].as_str()),
        ("1", "0", "1"),
        "B: 포기자는 세지 않으므로 추천 0 · 잔여 1 이어야 함: {b:?}",
    );

    // 대학 전체 열은 두 행 모두 같은 값
    for r in [a, b] {
        assert_eq!(
            (r[c_uq].as_str(), r[c_uused].as_str(), r[c_uleft].as_str()),
            ("3", "2", "1"),
            "대학: 정원 3 · 추천 2 · 잔여 1 이어야 함: {r:?}",
        );
    }
}

/// 정원 무제한(NULL)은 숫자가 아니라 "무제한" 문자열로 나가야 한다.
/// 0 이나 빈 칸으로 나가면 관리자가 정원 소진으로 오독한다.
#[tokio::test]
async fn export_quota_stats_unlimited_quota_renders_as_text() {
    let pool = common::create_test_pool().await;
    let uid = insert_univ(&pool, "무제한대").await; // total_quota NULL
    insert_track(&pool, uid, "자유전공").await;      // unit_quota NULL

    let (bytes, _) = export_bytes(&pool, None).await;
    let rows = parse_rows(&bytes);
    let header = &rows[0];
    let col_of = |h: &str| header.iter().position(|c| c == h).unwrap();

    let r = &rows[1];
    for h in ["모집단위 정원", "잔여인원", "대학 전체 정원", "대학 잔여인원"] {
        assert_eq!(r[col_of(h)], "무제한", "'{h}' 열은 '무제한' 이어야 함: {r:?}");
    }
    assert_eq!(r[col_of("추천인원")], "0", "추천 인원은 무제한이어도 숫자");
}

#[tokio::test]
async fn export_quota_stats_empty_db_returns_header_only() {
    let pool = common::create_test_pool().await;

    let (bytes, cd) = export_bytes(&pool, None).await;
    let rows = parse_rows(&bytes);
    // 헤더 행만 존재
    assert_eq!(rows.len(), 1, "빈 DB → 헤더 행만");
    assert_eq!(rows[0][0], "대학명", "첫 번째 헤더 열");
    assert!(cd.contains("전체_명단_정원현황"), "빈 DB 파일명: {cd}");
}

/// 라운드별 추천 인원 열("1차 추천", "2차 추천", …)은 `all_round_ids` 순서로 붙는
/// 동적 열이라 헤더 라벨과 셀 값이 어긋나기 쉽다. 기존 export 테스트는 정원·잔여만
/// 봤고, 픽스처의 라운드별 인원이 모두 같아 열이 뒤바뀌어도 통과했다.
#[tokio::test]
async fn export_quota_stats_round_columns_carry_per_round_counts() {
    let pool = common::create_test_pool().await;
    let hash = bcrypt::hash("pass", 4u32).unwrap();
    sqlx::query("INSERT INTO classes (grade, class_no, password_hash) VALUES (1, 1, ?)")
        .bind(&hash).execute(&pool).await.unwrap();

    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name, total_quota) VALUES ('한국대', 10) RETURNING id",
    ).fetch_one(&pool).await.unwrap();
    let ta: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name, unit_quota) VALUES (?, 'A컴공', 10) RETURNING id",
    ).bind(uid).fetch_one(&pool).await.unwrap();
    let tb: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name, unit_quota) VALUES (?, 'B전자', 10) RETURNING id",
    ).bind(uid).fetch_one(&pool).await.unwrap();
    let (r1, r2) = two_rounds(&pool).await;

    // A: 1차 2명 · 2차 1명 / B: 1차 0명 · 2차 3명 — 라운드축·트랙축 모두 비대칭
    let plan = [
        (ta, r1), (ta, r1), (ta, r2),
        (tb, r2), (tb, r2), (tb, r2),
    ];
    for (i, (tid, rid)) in plan.into_iter().enumerate() {
        seed_result(&pool, tid, rid, &format!("S{:03}", i + 1), "학생", i as i64 + 1, Some(1), 1, 0).await;
    }

    let (bytes, _) = export_bytes(&pool, None).await;
    let rows = parse_rows(&bytes);
    let header = &rows[0];
    let col_of = |h: &str| header.iter().position(|c| c == h)
        .unwrap_or_else(|| panic!("헤더에 '{}' 없음: {:?}", h, header));
    let (c_track, c1, c2) = (col_of("모집단위"), col_of("1차 추천"), col_of("2차 추천"));
    assert!(c1 < c2, "라운드 열은 라운드 오름차순으로 붙어야 함: {header:?}");

    let a = rows[1..].iter().find(|r| r[c_track] == "A컴공").expect("A컴공 행");
    assert_eq!(
        (a[c1].as_str(), a[c2].as_str()), ("2", "1"),
        "A: 1차 2명 · 2차 1명 이어야 함: {a:?}",
    );
    let b = rows[1..].iter().find(|r| r[c_track] == "B전자").expect("B전자 행");
    assert_eq!(
        (b[c1].as_str(), b[c2].as_str()), ("0", "3"),
        "B: 1차 0명 · 2차 3명 이어야 함(내역 없는 라운드는 0): {b:?}",
    );
}

/// 정원을 나중에 줄이면 확정 인원이 정원을 넘을 수 있다. 잔여인원이 음수로
/// 나가면 관리자가 "-1명 남음"을 보게 된다 — `.max(0)` 클램프가 이를 막는다.
/// 기존 픽스처는 정원과 확정 인원이 정확히 맞아떨어져 클램프가 발동한 적이 없다.
#[tokio::test]
async fn export_quota_stats_clamps_negative_remaining_to_zero() {
    let pool = common::create_test_pool().await;
    let hash = bcrypt::hash("pass", 4u32).unwrap();
    sqlx::query("INSERT INTO classes (grade, class_no, password_hash) VALUES (1, 1, ?)")
        .bind(&hash).execute(&pool).await.unwrap();

    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name, total_quota) VALUES ('한국대', 1) RETURNING id",
    ).fetch_one(&pool).await.unwrap();
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name, unit_quota) VALUES (?, '컴공', 1) RETURNING id",
    ).bind(uid).fetch_one(&pool).await.unwrap();
    let (r1, _r2) = two_rounds(&pool).await;

    seed_result(&pool, tid, r1, "S001", "학생", 1, Some(1), 1, 0).await;
    seed_result(&pool, tid, r1, "S002", "학생", 2, Some(2), 1, 0).await;

    let (bytes, _) = export_bytes(&pool, None).await;
    let rows = parse_rows(&bytes);
    let header = &rows[0];
    let col_of = |h: &str| header.iter().position(|c| c == h)
        .unwrap_or_else(|| panic!("헤더에 '{}' 없음: {:?}", h, header));
    let r = &rows[1];

    // 추천 인원은 실제 값(2)을 그대로, 잔여만 0 으로 클램프
    assert_eq!(r[col_of("추천인원")], "2", "실제 확정 인원: {r:?}");
    assert_eq!(r[col_of("잔여인원")], "0", "정원 1 - 확정 2 = -1 → 0 으로 클램프: {r:?}");
    assert_eq!(r[col_of("대학 추천인원")], "2", "대학 확정 인원: {r:?}");
    assert_eq!(r[col_of("대학 잔여인원")], "0", "대학 잔여도 음수 금지: {r:?}");
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

// ── E1: CLOSED 라운드 중 재학생 우선 변경 차단 ────────────────────
// results.ranking 은 마감 시점 저장값, 화면·자동 추천의 모집단위 순위는 라이브 계산.
// CLOSED 중 설정만 바꾸면 두 기준이 어긋난다.

async fn insert_round_with_status(pool: &sqlx::SqlitePool, status: &str) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at, closed_at, finalized_at) \
         VALUES (?, '2025-01-01', '2025-01-02', '2025-01-03') RETURNING id",
    )
    .bind(status)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn univ_body(pe: Option<bool>, quota: Option<Option<i64>>, name: Option<String>) -> UpdateUnivBody {
    UpdateUnivBody { univ_name: name, total_quota: quota, prioritize_enrolled: pe }
}

#[tokio::test]
async fn update_university_prioritize_409_when_closed_round_exists() {
    let pool = common::create_test_pool().await;
    let uid = insert_univ(&pool, "한국대").await;
    insert_round_with_status(&pool, "CLOSED").await;

    let res = update_university(
        State(common::make_state(pool.clone())),
        Path(uid),
        axum::Json(univ_body(Some(true), None, None)),
    )
    .await;
    assert_eq!(res.unwrap_err().0, StatusCode::CONFLICT);

    let pe: i64 = sqlx::query_scalar("SELECT prioritize_enrolled FROM universities WHERE id = ?")
        .bind(uid).fetch_one(&pool).await.unwrap();
    assert_eq!(pe, 0, "409 시 값이 바뀌면 안 된다");
}

#[tokio::test]
async fn update_track_prioritize_409_when_closed_round_exists() {
    let pool = common::create_test_pool().await;
    let uid = insert_univ(&pool, "한국대").await;
    let tid = insert_track(&pool, uid, "컴공").await;
    insert_round_with_status(&pool, "CLOSED").await;

    let res = update_track(
        State(common::make_state(pool.clone())),
        Path(tid),
        axum::Json(UpdateTrackBody {
            track_name: None,
            unit_quota: None,
            prioritize_enrolled: Some(true),
        }),
    )
    .await;
    assert_eq!(res.unwrap_err().0, StatusCode::CONFLICT);

    let pe: i64 = sqlx::query_scalar("SELECT prioritize_enrolled FROM univ_tracks WHERE id = ?")
        .bind(tid).fetch_one(&pool).await.unwrap();
    assert_eq!(pe, 0, "409 시 값이 바뀌면 안 된다");
}

#[tokio::test]
async fn update_university_quota_allowed_when_closed_round_exists() {
    // 정원은 저장 순위에 영향이 없으므로 CLOSED 중에도 허용 (불필요하게 조이지 않는다)
    let pool = common::create_test_pool().await;
    let uid = insert_univ(&pool, "한국대").await;
    insert_round_with_status(&pool, "CLOSED").await;

    let st = update_university(
        State(common::make_state(pool.clone())),
        Path(uid),
        axum::Json(univ_body(None, Some(Some(3)), None)),
    )
    .await
    .unwrap();
    assert_eq!(st, StatusCode::NO_CONTENT);

    let q: Option<i64> = sqlx::query_scalar("SELECT total_quota FROM universities WHERE id = ?")
        .bind(uid).fetch_one(&pool).await.unwrap();
    assert_eq!(q, Some(3));
}

#[tokio::test]
async fn update_university_same_prioritize_value_allowed_when_closed() {
    // 폼이 전 필드를 함께 보내므로, 값이 그대로면 이름/정원 수정은 통과해야 한다
    let pool = common::create_test_pool().await;
    let uid = insert_univ(&pool, "한국대").await;
    insert_round_with_status(&pool, "CLOSED").await;

    let st = update_university(
        State(common::make_state(pool.clone())),
        Path(uid),
        axum::Json(univ_body(Some(false), None, Some("한국대학교".into()))),
    )
    .await
    .unwrap();
    assert_eq!(st, StatusCode::NO_CONTENT);

    let name: String = sqlx::query_scalar("SELECT univ_name FROM universities WHERE id = ?")
        .bind(uid).fetch_one(&pool).await.unwrap();
    assert_eq!(name, "한국대학교");
}

#[tokio::test]
async fn update_university_prioritize_allowed_when_open_or_finalized() {
    for status in ["OPEN", "FINALIZED"] {
        let pool = common::create_test_pool().await;
        let uid = insert_univ(&pool, "한국대").await;
        insert_round_with_status(&pool, status).await;

        let st = update_university(
            State(common::make_state(pool.clone())),
            Path(uid),
            axum::Json(univ_body(Some(true), None, None)),
        )
        .await
        .unwrap_or_else(|e| panic!("{status} 라운드에서는 허용되어야 함: {}", e.1));
        assert_eq!(st, StatusCode::NO_CONTENT);

        let pe: i64 = sqlx::query_scalar("SELECT prioritize_enrolled FROM universities WHERE id = ?")
            .bind(uid).fetch_one(&pool).await.unwrap();
        assert_eq!(pe, 1, "{status} 라운드에서는 변경이 반영되어야 함");
    }
}

// ── 신규: 전체 명단 시트 + 정원 현황의 지원/포기 열 ────────────────

/// export_quota_stats 는 2시트다: ①"전체 명단"(전 라운드 지원자, 라운드 열 포함)
/// ②"정원 현황"(지원·추천·포기·잔여). 두 시트가 모두 채워지는지 확인한다.
#[tokio::test]
async fn export_quota_stats_roster_sheet_and_applied_abandoned_columns() {
    let pool = common::create_test_pool().await;
    let hash = bcrypt::hash("pass", 4u32).unwrap();
    sqlx::query("INSERT INTO classes (grade, class_no, password_hash) VALUES (1, 1, ?)")
        .bind(&hash).execute(&pool).await.unwrap();
    let uid = insert_univ(&pool, "한국대").await;
    let tid = insert_track(&pool, uid, "컴공").await;
    let (r1, r2) = two_rounds(&pool).await;

    // 1차 추천 1명 / 2차 추천 1명 / 2차 포기 1명 — 전체 라운드 누적
    seed_result(&pool, tid, r1, "S001", "에이", 1, Some(1), 1, 0).await;
    seed_result(&pool, tid, r2, "S002", "비",   2, Some(1), 1, 0).await;
    seed_result(&pool, tid, r2, "S003", "씨",   3, Some(2), 1, 1).await;

    let (bytes, cd) = export_bytes(&pool, None).await;
    assert!(cd.contains("전체_명단_정원현황"), "파일명: {cd}");

    // ① 전체 명단 — 라운드 열 포함, 지원자 3명 전원
    let roster = principal_candidate_manager::excel::parse_xlsx_sheet_rows(&bytes, "전체 명단").unwrap();
    assert!(roster[0].iter().any(|h| h == "라운드"), "명단에 '라운드' 열: {:?}", roster[0]);
    assert!(roster[0].iter().any(|h| h == "모집단위 순위"), "명단에 '모집단위 순위' 열");
    assert_eq!(roster.len(), 4, "헤더 + 지원자 3명: {roster:?}");

    // ② 정원 현황 — 지원 3 / 추천 2(포기 제외) / 포기 1
    let summary = principal_candidate_manager::excel::parse_xlsx_sheet_rows(&bytes, "정원 현황").unwrap();
    let header = &summary[0];
    let col_of = |h: &str| header.iter().position(|c| c == h)
        .unwrap_or_else(|| panic!("헤더에 '{}' 없음: {:?}", h, header));
    let data = &summary[1];
    assert_eq!(data[col_of("지원 인원")], "3", "지원 3명: {data:?}");
    assert_eq!(data[col_of("추천인원")], "2", "추천(포기 제외) 2명: {data:?}");
    assert_eq!(data[col_of("포기 인원")], "1", "포기 1명: {data:?}");
}
