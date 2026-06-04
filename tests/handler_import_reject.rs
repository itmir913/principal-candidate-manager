/// 모든 import 핸들러의 "오류 시 전체 거부" 원칙을 검증한다.
/// 오류 행이 하나라도 있으면 HTTP 422 + rollback, 성공 시 HTTP 200 + commit.
mod common;

use axum::{extract::State, http::StatusCode};
use principal_candidate_manager::handlers::{
    classes::import_classes,
    students::{import_enrolled, import_graduated, import_students},
};

// ── import_classes ────────────────────────────────────────────────

#[tokio::test]
async fn import_classes_error_rejects_all() {
    let pool = common::create_test_pool().await;
    let state = common::make_state(pool.clone());

    // 2행: 비밀번호가 4자 미만(="ab") → 오류 → 전체 거부
    let csv = "학년,반,담임명,비밀번호\n1,1,홍길동,pass1234\n1,2,이순신,ab\n";
    let (status, axum::Json(result)) =
        import_classes(State(state), common::csv_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(result["inserted"], 0);
    assert!(!result["errors"].as_array().unwrap().is_empty());

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM classes")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "rollback — 1행도 저장되면 안 됨");
}

#[tokio::test]
async fn import_classes_success_commits() {
    let pool = common::create_test_pool().await;
    let state = common::make_state(pool.clone());

    let csv = "학년,반,담임명,비밀번호\n1,1,홍길동,pass1234\n1,2,이순신,pass5678\n2,1,김철수,pass9012\n";
    let (status, axum::Json(result)) =
        import_classes(State(state), common::csv_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["inserted"], 3);

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM classes")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 3);
}

// ── import_students ───────────────────────────────────────────────

#[tokio::test]
async fn import_students_error_rejects_all() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let state = common::make_state(pool.clone());

    // S001: 정상, S002: 재학생인데 학년 누락 → 오류
    let csv = "학생코드,이름,재학여부,학년,반,번호\n\
               S001,홍길동,1,1,1,1\n\
               S002,이순신,1,,1,2\n";
    let (status, axum::Json(result)) =
        import_students(State(state), common::csv_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(result.inserted, 0);
    assert!(!result.errors.is_empty());

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM students")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "rollback — S001도 저장되면 안 됨");
}

#[tokio::test]
async fn import_students_success_commits() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let state = common::make_state(pool.clone());

    let csv = "학생코드,이름,재학여부,학년,반,번호\n\
               S001,홍길동,1,1,1,1\n\
               S002,이순신,1,1,1,2\n";
    let (status, axum::Json(result)) =
        import_students(State(state), common::csv_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(result.inserted, 2);
    assert!(result.errors.is_empty());
}

// ── import_enrolled ───────────────────────────────────────────────

#[tokio::test]
async fn import_enrolled_error_rejects_all() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let state = common::make_state(pool.clone());

    // 이름 누락 행 포함 → 전체 거부
    let csv = "이름,학년,반,번호\n홍길동,1,1,1\n,1,1,2\n";
    let (status, axum::Json(result)) =
        import_enrolled(State(state), common::csv_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(result.inserted, 0);
    assert!(!result.errors.is_empty());

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM students")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "rollback — 홍길동도 저장되면 안 됨");
}

#[tokio::test]
async fn import_enrolled_success_commits() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let state = common::make_state(pool.clone());

    let csv = "이름,학년,반,번호\n홍길동,1,1,1\n이순신,1,1,2\n";
    let (status, axum::Json(result)) =
        import_enrolled(State(state), common::csv_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(result.inserted, 2);
    assert!(result.errors.is_empty());
}

// ── import_classes: 추가 유효성 검증 ──────────────────────────────

#[tokio::test]
async fn import_classes_missing_required_column_returns_bad_request() {
    // "학년" 열 없음 → require_cols 실패 → 400 (데이터 처리 전 거부)
    let pool = common::create_test_pool().await;
    let state = common::make_state(pool.clone());

    let csv = "반,비밀번호\n1,pass1234\n";
    let res = import_classes(State(state), common::csv_multipart(csv).await).await;
    assert_eq!(res.unwrap_err().0, StatusCode::BAD_REQUEST);

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM classes")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn import_classes_empty_teacher_name_rejects_all() {
    // 담임명 열이 있지만 빈 셀 → 전체 거부
    let pool = common::create_test_pool().await;
    let state = common::make_state(pool.clone());

    // 1행: 정상, 2행: 담임명 비어 있음 → 전체 거부
    let csv = "학년,반,담임명,비밀번호\n1,1,홍길동,pass1234\n1,2,,pass5678\n";
    let (status, axum::Json(result)) =
        import_classes(State(state), common::csv_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(result["inserted"], 0);
    assert!(!result["errors"].as_array().unwrap().is_empty());

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM classes")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "rollback — 1행도 저장되면 안 됨");
}

#[tokio::test]
async fn import_classes_missing_teacher_name_column_returns_bad_request() {
    // "담임명" 열 없음 → require_cols 실패 → 400
    let pool = common::create_test_pool().await;
    let state = common::make_state(pool.clone());

    let csv = "학년,반,비밀번호\n1,1,pass1234\n";
    let res = import_classes(State(state), common::csv_multipart(csv).await).await;
    assert_eq!(res.unwrap_err().0, StatusCode::BAD_REQUEST);

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM classes")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn import_classes_grade_zero_rejects_all() {
    // grade=0은 졸업생 sentinel — import에서는 유효하지 않은 값
    let pool = common::create_test_pool().await;
    let state = common::make_state(pool.clone());

    // 1행: 정상, 2행: grade=0 → 2행 오류 → 전체 거부
    let csv = "학년,반,담임명,비밀번호\n1,1,홍길동,pass1234\n0,2,이순신,pass5678\n";
    let (status, axum::Json(result)) =
        import_classes(State(state), common::csv_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(result["inserted"], 0);

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM classes")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "grade=0 포함 시 전체 rollback");
}

#[tokio::test]
async fn import_classes_non_numeric_grade_rejects_all() {
    // grade = "abc" → 숫자 변환 실패 → 전체 거부
    let pool = common::create_test_pool().await;
    let state = common::make_state(pool.clone());

    let csv = "학년,반,담임명,비밀번호\nabc,1,홍길동,pass1234\n";
    let (status, axum::Json(result)) =
        import_classes(State(state), common::csv_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(result["inserted"], 0);
}

// ── import_enrolled: 추가 유효성 검증 ─────────────────────────────

#[tokio::test]
async fn import_enrolled_whitespace_name_rejects_all() {
    // 이름이 공백만 있는 경우 — get_col trim 후 빈 문자열 → 오류
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let state = common::make_state(pool.clone());

    let csv = "이름,학년,반,번호\n홍길동,1,1,1\n   ,1,1,2\n";
    let (status, axum::Json(result)) =
        import_enrolled(State(state), common::csv_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(result.inserted, 0);
    assert!(!result.errors.is_empty());

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM students")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "rollback — 홍길동도 저장되면 안 됨");
}

#[tokio::test]
async fn import_enrolled_class_not_found_rejects_all() {
    // 존재하지 않는 학급(2학년 1반) → 오류 → 전체 거부
    let pool = common::create_test_pool().await;
    // 1학년 1반만 등록, 2학년 1반은 미등록
    common::insert_class(&pool, 1, 1).await;
    let state = common::make_state(pool.clone());

    let csv = "이름,학년,반,번호\n홍길동,1,1,1\n이순신,2,1,1\n";
    let (status, axum::Json(result)) =
        import_enrolled(State(state), common::csv_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(result.inserted, 0);
    assert!(!result.errors.is_empty());

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM students")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "rollback — 홍길동도 저장되면 안 됨");
}

#[tokio::test]
async fn import_enrolled_missing_required_column_returns_bad_request() {
    // "이름" 열 없음 → require_cols 실패 → 400
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let state = common::make_state(pool.clone());

    let csv = "학년,반,번호\n1,1,1\n";
    // ImportResult가 Debug를 구현하지 않으므로 unwrap_err() 대신 match 사용
    match import_enrolled(State(state), common::csv_multipart(csv).await).await {
        Ok(_) => panic!("400 BAD_REQUEST가 예상됨"),
        Err((status, _)) => assert_eq!(status, StatusCode::BAD_REQUEST),
    }
}

// ── import_graduated ──────────────────────────────────────────────

#[tokio::test]
async fn import_graduated_error_rejects_all() {
    let pool = common::create_test_pool().await;
    let state = common::make_state(pool.clone());

    // 이름 누락 행 포함 → 전체 거부
    let csv = "학생코드,이름,졸업연도\nS001,홍길동,2023\nS002,,2023\n";
    let (status, axum::Json(result)) =
        import_graduated(State(state), common::csv_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(result.inserted, 0);
    assert!(!result.errors.is_empty());

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM students")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "rollback — S001도 저장되면 안 됨");
}

#[tokio::test]
async fn import_graduated_success_commits() {
    let pool = common::create_test_pool().await;
    let state = common::make_state(pool.clone());

    let csv = "학생코드,이름,졸업연도\nS001,홍길동,2023\nS002,이순신,2023\n";
    let (status, axum::Json(result)) =
        import_graduated(State(state), common::csv_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(result.inserted, 2);
    assert!(result.errors.is_empty());
}

#[tokio::test]
async fn import_graduated_non_numeric_year_rejects_all() {
    // 졸업연도 = "abc" → parse_i64 실패 → grad_year=None → upsert_student 오류 → 전체 거부
    let pool = common::create_test_pool().await;
    let state = common::make_state(pool.clone());

    let csv = "학생코드,이름,졸업연도\nS001,홍길동,2023\nS002,이순신,abc\n";
    let (status, axum::Json(result)) =
        import_graduated(State(state), common::csv_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(result.inserted, 0);
    assert!(!result.errors.is_empty());

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM students")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "rollback — S001도 저장되면 안 됨");
}

#[tokio::test]
async fn import_graduated_missing_required_column_returns_bad_request() {
    // "졸업연도" 열 없음 → require_cols 실패 → 400
    let pool = common::create_test_pool().await;
    let state = common::make_state(pool.clone());

    let csv = "학생코드,이름\nS001,홍길동\n";
    match import_graduated(State(state), common::csv_multipart(csv).await).await {
        Ok(_) => panic!("400 BAD_REQUEST가 예상됨"),
        Err((status, _)) => assert_eq!(status, StatusCode::BAD_REQUEST),
    }
}

#[tokio::test]
async fn import_classes_xls_format_returns_bad_request() {
    // .xls(OLE2) 형식 업로드 → 400 (지원하지 않는 포맷)
    let pool = common::create_test_pool().await;
    let state = common::make_state(pool.clone());

    // OLE2 매직 바이트로 시작하는 가짜 xls 파일
    let fake_xls = b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1some_content";
    let boundary = "boundary42";
    let body = format!(
        "--{boundary}\r\n\
         Content-Disposition: form-data; name=\"file\"; filename=\"data.xls\"\r\n\
         Content-Type: application/vnd.ms-excel\r\n\r\n",
    );
    let mut body_bytes = body.into_bytes();
    body_bytes.extend_from_slice(fake_xls);
    body_bytes.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    use axum::{body::Body, extract::{FromRequest, Multipart}, http::Request};
    let req = Request::builder()
        .method("POST")
        .header("content-type", format!("multipart/form-data; boundary={boundary}"))
        .body(Body::from(body_bytes))
        .unwrap();
    let mp = Multipart::from_request(req, &()).await.unwrap();

    let res = import_classes(State(state), mp).await;
    assert_eq!(res.unwrap_err().0, StatusCode::BAD_REQUEST);
}
