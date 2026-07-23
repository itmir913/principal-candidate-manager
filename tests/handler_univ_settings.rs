//! 대학·모집단위 설정 일괄 Import·Export (template/export/preview/import).
//!
//! - template→채움→import, export→import 왕복(헤더 하드코딩 금지)
//! - 거부 3종 세트: 상태코드 + DB 불변 + 오류의 행번호·원인
//! - prioritize 불변식·마감 라운드 가드·정원 허용
//! - preview 가 DB 를 바꾸지 않음(읽기 전용)
//! - cascade 미리보기

mod common;

use axum::{
    body::Body,
    extract::{FromRequest, Multipart, State},
    http::{Request, StatusCode},
};
use principal_candidate_manager::handlers::universities::{
    settings_export, settings_import, settings_preview, settings_template,
};
use sqlx::SqlitePool;

// ── 헬퍼 ─────────────────────────────────────────────────────────

async fn xlsx_multipart(bytes: &[u8]) -> Multipart {
    let boundary = "b42";
    let mut body: Vec<u8> = Vec::new();
    body.extend_from_slice(
        format!(
            "--{boundary}\r\n\
             Content-Disposition: form-data; name=\"file\"; filename=\"f.xlsx\"\r\n\
             Content-Type: application/octet-stream\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    let req = Request::builder()
        .method("POST")
        .header("content-type", format!("multipart/form-data; boundary={boundary}"))
        .body(Body::from(body))
        .unwrap();
    Multipart::from_request(req, &()).await.unwrap()
}

async fn response_bytes(resp: axum::response::Response) -> Vec<u8> {
    axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap().to_vec()
}

fn st(pool: &SqlitePool) -> State<principal_candidate_manager::state::AppState> {
    State(common::make_state(pool.clone()))
}

/// import 호출 → (status, inserted, updated, errors)
async fn do_import(pool: &SqlitePool, mp: Multipart) -> (StatusCode, i64, i64, Vec<String>) {
    let (status, axum::Json(v)) = settings_import(st(pool), mp).await.unwrap();
    let errors = v["errors"].as_array().unwrap().iter()
        .map(|e| e.as_str().unwrap().to_string()).collect();
    (status, v["inserted"].as_i64().unwrap(), v["updated"].as_i64().unwrap(), errors)
}

async fn insert_univ(pool: &SqlitePool, name: &str, quota: Option<i64>, prio: i64) -> i64 {
    sqlx::query_scalar("INSERT INTO universities (univ_name, total_quota, prioritize_enrolled) VALUES (?, ?, ?) RETURNING id")
        .bind(name).bind(quota).bind(prio).fetch_one(pool).await.unwrap()
}
async fn insert_track(pool: &SqlitePool, uid: i64, name: &str, quota: Option<i64>, prio: i64) -> i64 {
    sqlx::query_scalar("INSERT INTO univ_tracks (univ_id, track_name, unit_quota, prioritize_enrolled) VALUES (?, ?, ?, ?) RETURNING id")
        .bind(uid).bind(name).bind(quota).bind(prio).fetch_one(pool).await.unwrap()
}
async fn insert_closed_round(pool: &SqlitePool) {
    sqlx::query("INSERT INTO rounds (status, opened_at, closed_at) VALUES ('CLOSED', '2025-01-01', '2025-01-02')")
        .execute(pool).await.unwrap();
}
async fn univ_state(pool: &SqlitePool, name: &str) -> (Option<i64>, i64) {
    sqlx::query_as("SELECT total_quota, prioritize_enrolled FROM universities WHERE univ_name = ?")
        .bind(name).fetch_one(pool).await.unwrap()
}
async fn track_state(pool: &SqlitePool, uid: i64, name: &str) -> (Option<i64>, i64) {
    sqlx::query_as("SELECT unit_quota, prioritize_enrolled FROM univ_tracks WHERE univ_id = ? AND track_name = ?")
        .bind(uid).bind(name).fetch_one(pool).await.unwrap()
}

const HEADER: &str = "대학명,대학 정원,대학 재학생우선,모집단위명,모집단위 정원,모집단위 재학생우선";

// ══════════════════════════════════════════════════════════════════
//  왕복 (헤더 하드코딩 금지)
// ══════════════════════════════════════════════════════════════════

/// export 가 쓰는 헤더와 import 의 require_cols 가 어긋나면 정상 절차가 거부된다.
/// 내보낸 xlsx 를 그대로 재import → 성공 + 값 불변(idempotent).
#[tokio::test]
async fn export_then_import_roundtrip_is_idempotent() {
    let pool = common::create_test_pool().await;
    let u1 = insert_univ(&pool, "한국대", Some(5), 0).await;
    insert_track(&pool, u1, "인문", Some(3), 0).await;
    insert_track(&pool, u1, "자연", None, 0).await;
    let _u2 = insert_univ(&pool, "무트랙대", None, 1).await; // 모집단위 0개 대학

    let bytes = response_bytes(settings_export(st(&pool)).await.unwrap()).await;
    let (status, inserted, updated, errors) = do_import(&pool, xlsx_multipart(&bytes).await).await;

    assert_eq!(status, StatusCode::OK, "왕복 import 성공: {:?}", errors);
    assert_eq!((inserted, updated), (0, 0), "동일 데이터 재import 는 변경 0건이어야 함");
    assert_eq!(univ_state(&pool, "한국대").await, (Some(5), 0));
    assert_eq!(track_state(&pool, u1, "인문").await, (Some(3), 0));
    assert_eq!(track_state(&pool, u1, "자연").await, (None, 0), "무제한 트랙 정원 유지");
    assert_eq!(univ_state(&pool, "무트랙대").await, (None, 1), "무트랙 대학도 왕복");
}

/// 빈 template 의 헤더를 **읽어서** 데이터를 채워 import → 새 대학·모집단위 생성.
/// 헤더를 테스트에 하드코딩하지 않는다.
#[tokio::test]
async fn template_headers_feed_importer() {
    let pool = common::create_test_pool().await;
    let tbytes = response_bytes(settings_template().await.unwrap()).await;
    let rows = principal_candidate_manager::excel::parse_xlsx_all_rows_raw(&tbytes).unwrap();
    let headers = rows[0].join(",");

    // 템플릿 헤더 + 새 데이터 한 대학(모집단위 1개) — 헤더는 위에서 읽은 것 그대로 사용
    let csv = format!("{}\n새대학,10,아니오,새전공,4,아니오\n", headers);
    let (status, inserted, _updated, errors) =
        do_import(&pool, common::csv_multipart(&csv).await).await;

    assert_eq!(status, StatusCode::OK, "템플릿 헤더로 채운 파일 import 성공: {:?}", errors);
    assert_eq!(inserted, 2, "대학 1 + 모집단위 1 생성");
    let uid: i64 = sqlx::query_scalar("SELECT id FROM universities WHERE univ_name = '새대학'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(univ_state(&pool, "새대학").await, (Some(10), 0));
    assert_eq!(track_state(&pool, uid, "새전공").await, (Some(4), 0));
}

// ══════════════════════════════════════════════════════════════════
//  생성/수정
// ══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn import_creates_and_updates_upsert_only() {
    let pool = common::create_test_pool().await;
    let u1 = insert_univ(&pool, "한국대", Some(5), 0).await;
    insert_track(&pool, u1, "인문", Some(3), 0).await;
    let _keep = insert_univ(&pool, "안건드림대", Some(9), 0).await; // 파일에 없음 → 유지

    // 한국대 정원 5→7, 인문 정원 3→2, + 새 모집단위 자연, + 새 대학 서울대(무트랙)
    let csv = format!(
        "{h}\n\
         한국대,7,아니오,인문,2,아니오\n\
         한국대,7,아니오,자연,무제한,아니오\n\
         서울대,무제한,예,,,\n",
        h = HEADER
    );
    let (status, inserted, updated, errors) =
        do_import(&pool, common::csv_multipart(&csv).await).await;

    assert_eq!(status, StatusCode::OK, "{:?}", errors);
    // 생성: 자연 트랙 + 서울대 대학 = 2 / 수정: 한국대(정원) + 인문(정원) = 2
    assert_eq!((inserted, updated), (2, 2), "생성 2 · 수정 2");
    assert_eq!(univ_state(&pool, "한국대").await, (Some(7), 0));
    assert_eq!(track_state(&pool, u1, "인문").await, (Some(2), 0));
    assert_eq!(track_state(&pool, u1, "자연").await, (None, 0));
    assert_eq!(univ_state(&pool, "서울대").await, (None, 1));
    assert_eq!(univ_state(&pool, "안건드림대").await, (Some(9), 0), "파일에 없는 대학은 유지(삭제 아님)");
}

// ══════════════════════════════════════════════════════════════════
//  거부 3종 세트 (상태코드 + DB 불변 + 행번호·원인)
// ══════════════════════════════════════════════════════════════════

/// 헤더만 있고 데이터 0행 → 422 거부
#[tokio::test]
async fn reject_empty_data_file() {
    let pool = common::create_test_pool().await;
    let csv = format!("{}\n", HEADER);
    let (status, _i, _u, errors) = do_import(&pool, common::csv_multipart(&csv).await).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(errors.iter().any(|e| e.contains("빈 파일")), "원인 안내: {:?}", errors);
}

/// 파일 내 모집단위 중복 → 422 + 행번호 + 원인, DB 불변
#[tokio::test]
async fn reject_duplicate_track_in_file() {
    let pool = common::create_test_pool().await;
    let csv = format!(
        "{h}\n한국대,5,아니오,인문,3,아니오\n한국대,5,아니오,인문,2,아니오\n",
        h = HEADER
    );
    let (status, _i, _u, errors) = do_import(&pool, common::csv_multipart(&csv).await).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(errors.iter().any(|e| e.contains("중복") && e.contains("3행")), "행번호+원인: {:?}", errors);
    let cnt: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM universities").fetch_one(&pool).await.unwrap();
    assert_eq!(cnt, 0, "rollback — 아무것도 저장되면 안 됨");
}

/// 대학=예인데 모집단위=아니오 → 불변식 위반 422, DB 불변
#[tokio::test]
async fn reject_univ_yes_track_no() {
    let pool = common::create_test_pool().await;
    let csv = format!("{h}\n한국대,5,예,인문,3,아니오\n", h = HEADER);
    let (status, _i, _u, errors) = do_import(&pool, common::csv_multipart(&csv).await).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(errors.iter().any(|e| e.contains("재학생 우선") || e.contains("재학생우선")), "{:?}", errors);
    let cnt: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM universities").fetch_one(&pool).await.unwrap();
    assert_eq!(cnt, 0);
}

/// 같은 대학의 반복 대학값 불일치 → 422 + 행번호
#[tokio::test]
async fn reject_inconsistent_repeated_univ_values() {
    let pool = common::create_test_pool().await;
    let csv = format!(
        "{h}\n한국대,5,아니오,인문,3,아니오\n한국대,8,아니오,자연,3,아니오\n",
        h = HEADER
    );
    let (status, _i, _u, errors) = do_import(&pool, common::csv_multipart(&csv).await).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(errors.iter().any(|e| e.contains("3행") && e.contains("다릅니다")), "{:?}", errors);
}

/// 재학생우선/정원 토큰 오류 → 422 + 행번호
#[tokio::test]
async fn reject_invalid_tokens() {
    let pool = common::create_test_pool().await;
    let csv = format!("{h}\n한국대,다섯,아니오,인문,3,글쎄\n", h = HEADER);
    let (status, _i, _u, errors) = do_import(&pool, common::csv_multipart(&csv).await).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(errors.iter().any(|e| e.contains("정원") && e.contains("2행")), "정원 오류: {:?}", errors);
    assert!(errors.iter().any(|e| e.contains("재학생우선") && e.contains("2행")), "재학생우선 오류: {:?}", errors);
}

/// 모집단위명 빈 행에 모집단위 정원/재학생우선이 채워짐 → 모순 422
#[tokio::test]
async fn reject_blank_track_name_with_values() {
    let pool = common::create_test_pool().await;
    let csv = format!("{h}\n한국대,5,아니오,,3,아니오\n", h = HEADER);
    let (status, _i, _u, errors) = do_import(&pool, common::csv_multipart(&csv).await).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(errors.iter().any(|e| e.contains("모집단위명이 비었는데")), "{:?}", errors);
}

// ══════════════════════════════════════════════════════════════════
//  마감 라운드 가드
// ══════════════════════════════════════════════════════════════════

/// CLOSED 라운드 중 기존 대학의 재학생우선 변경 → 409, DB 불변
#[tokio::test]
async fn prioritize_change_blocked_when_closed_round() {
    let pool = common::create_test_pool().await;
    let _u = insert_univ(&pool, "한국대", Some(5), 0).await;
    insert_closed_round(&pool).await;

    let csv = format!("{h}\n한국대,5,예,,,\n", h = HEADER); // prio 0→1
    let err = settings_import(st(&pool), common::csv_multipart(&csv).await).await.unwrap_err();
    assert_eq!(err.0, StatusCode::CONFLICT, "마감 중 재학생우선 변경은 409");

    assert_eq!(univ_state(&pool, "한국대").await, (Some(5), 0), "409 시 값 불변(rollback)");
}

/// CLOSED 라운드 중에도 정원 변경은 허용
#[tokio::test]
async fn quota_change_allowed_when_closed_round() {
    let pool = common::create_test_pool().await;
    let _u = insert_univ(&pool, "한국대", Some(5), 0).await;
    insert_closed_round(&pool).await;

    let csv = format!("{h}\n한국대,9,아니오,,,\n", h = HEADER); // 정원만 5→9, prio 불변
    let (status, _i, updated, errors) = do_import(&pool, common::csv_multipart(&csv).await).await;
    assert_eq!(status, StatusCode::OK, "{:?}", errors);
    assert_eq!(updated, 1);
    assert_eq!(univ_state(&pool, "한국대").await, (Some(9), 0));
}

// ══════════════════════════════════════════════════════════════════
//  cascade
// ══════════════════════════════════════════════════════════════════

/// 대학 재학생우선 0→1 이면 파일에 없는 기존 모집단위도 트리거로 함께 1 이 된다.
#[tokio::test]
async fn univ_prioritize_cascades_to_untouched_track() {
    let pool = common::create_test_pool().await;
    let u = insert_univ(&pool, "한국대", Some(5), 0).await;
    insert_track(&pool, u, "인문", Some(3), 0).await; // 파일에 넣지 않는다

    // 대학만 0→1 (모집단위 행 없음)
    let csv = format!("{h}\n한국대,5,예,,,\n", h = HEADER);
    let (status, _i, _u, errors) = do_import(&pool, common::csv_multipart(&csv).await).await;
    assert_eq!(status, StatusCode::OK, "{:?}", errors);

    assert_eq!(univ_state(&pool, "한국대").await, (Some(5), 1));
    assert_eq!(track_state(&pool, u, "인문").await, (Some(3), 1), "cascade 로 트랙도 1");
}

// ══════════════════════════════════════════════════════════════════
//  preview (읽기 전용)
// ══════════════════════════════════════════════════════════════════

/// preview 는 diff 를 돌려주되 DB 를 절대 바꾸지 않는다.
#[tokio::test]
async fn preview_reports_changes_without_writing() {
    let pool = common::create_test_pool().await;
    let u = insert_univ(&pool, "한국대", Some(5), 0).await;
    insert_track(&pool, u, "인문", Some(3), 0).await;

    let csv = format!("{h}\n한국대,7,아니오,인문,2,아니오\n", h = HEADER);
    let axum::Json(p) = settings_preview(st(&pool), common::csv_multipart(&csv).await).await.unwrap();

    assert!(p.errors.is_empty(), "유효 파일: {:?}", p.errors);
    assert!(p.changes.iter().any(|c| c.kind == "univ"), "대학 정원 변경 diff");
    assert!(p.changes.iter().any(|c| c.kind == "track"), "모집단위 정원 변경 diff");
    // DB 는 그대로여야 한다
    assert_eq!(univ_state(&pool, "한국대").await, (Some(5), 0), "preview 는 쓰지 않는다");
    assert_eq!(track_state(&pool, u, "인문").await, (Some(3), 0));
}

/// preview 는 오류가 있어도 200 으로 errors 를 담아 돌려준다(모달이 표시).
#[tokio::test]
async fn preview_returns_errors_as_200() {
    let pool = common::create_test_pool().await;
    let csv = format!("{h}\n한국대,5,예,인문,3,아니오\n", h = HEADER); // 불변식 위반
    let axum::Json(p) = settings_preview(st(&pool), common::csv_multipart(&csv).await).await.unwrap();
    assert!(!p.errors.is_empty(), "오류가 담겨야 함");
    assert!(p.changes.is_empty(), "오류 시 diff 없음");
}

/// preview 는 마감 라운드로 차단되는 재학생우선 변경에 blocked 플래그를 세운다.
#[tokio::test]
async fn preview_flags_blocked_when_closed() {
    let pool = common::create_test_pool().await;
    let _u = insert_univ(&pool, "한국대", Some(5), 0).await;
    insert_closed_round(&pool).await;

    let csv = format!("{h}\n한국대,5,예,,,\n", h = HEADER); // prio 변경
    let axum::Json(p) = settings_preview(st(&pool), common::csv_multipart(&csv).await).await.unwrap();
    assert!(p.has_blocked, "마감 중 재학생우선 변경은 blocked");
    assert!(!p.closed_round_labels.is_empty(), "마감 라운드 라벨 노출");
}
