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
               S001,홍길동,재학,1,1,1\n\
               S002,이순신,재학,,1,2\n";
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

/// 재학여부 인식 불가 값("휴학" 등)은 silent default(재학생 처리) 없이 전체 거부되어야 한다.
#[tokio::test]
async fn import_students_invalid_enrolled_flag_rejects_all() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let state = common::make_state(pool.clone());

    let csv = "학생코드,이름,재학여부,학년,반,번호\n\
               S001,홍길동,재학,1,1,1\n\
               S002,이순신,휴학,1,1,2\n";
    let (status, axum::Json(result)) =
        import_students(State(state), common::csv_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(result.inserted, 0);
    assert!(result.errors.iter().any(|e| e.contains("재학여부")));

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM students")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "rollback — S001도 저장되면 안 됨");
}

/// 재학여부 빈 값도 오류 — 과거에는 무조건 재학생으로 처리되던 silent fallback 경로.
#[tokio::test]
async fn import_students_empty_enrolled_flag_rejects_all() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let state = common::make_state(pool.clone());

    let csv = "학생코드,이름,재학여부,학년,반,번호\n\
               S001,홍길동,,1,1,1\n";
    let (status, axum::Json(result)) =
        import_students(State(state), common::csv_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(result.inserted, 0);
    assert!(result.errors.iter().any(|e| e.contains("재학여부")));
}

/// 숫자 0/1은 의미가 모호하므로 배제 — 한글 키워드만 허용한다.
#[tokio::test]
async fn import_students_numeric_enrolled_flag_rejects_all() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let state = common::make_state(pool.clone());

    let csv = "학생코드,이름,재학여부,학년,반,번호,졸업연도\n\
               S001,홍길동,1,1,1,1,\n\
               S002,이순신,0,,,,2024\n";
    let (status, axum::Json(result)) =
        import_students(State(state), common::csv_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(result.inserted, 0);
    assert!(result.errors.iter().any(|e| e.contains("재학여부")));

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM students")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "rollback — 숫자 표기 행은 전체 거부");
}

/// '재학'/'재학생'/'졸업'/'졸업생' 한글 키워드가 명세대로 매핑되는지 검증.
/// 과거 버그: '졸업'이 숫자 파싱 실패 → 무조건 재학생(true)으로 뒤집혔다.
#[tokio::test]
async fn import_students_korean_keywords_map_correctly() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let state = common::make_state(pool.clone());

    let csv = "학생코드,이름,재학여부,학년,반,번호,졸업연도\n\
               S001,홍길동,재학,1,1,1,\n\
               S002,이순신,졸업,,,,2024\n\
               S003,박재적,재학생,1,1,2,\n\
               S004,최동문,졸업생,,,,2023\n";
    let (status, axum::Json(result)) =
        import_students(State(state), common::csv_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::OK, "errors: {:?}", result.errors);
    assert_eq!(result.inserted, 4);

    for (code, expected) in [("S001", 1i64), ("S002", 0), ("S003", 1), ("S004", 0)] {
        let is_enrolled: i64 = sqlx::query_scalar(
            "SELECT is_enrolled FROM students WHERE student_code = ?",
        )
        .bind(code)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(is_enrolled, expected, "{} 분류 불일치", code);
    }
}

/// 기존 졸업생이 '졸업' 행 재업로드로 재학생으로 뒤집히지 않아야 한다 (grad_year 보존).
#[tokio::test]
async fn import_students_reupload_keeps_graduate_classification() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let state = common::make_state(pool.clone());

    sqlx::query(
        "INSERT INTO students (student_code, name, is_enrolled, grad_year) VALUES ('G001', '김졸업', 0, 2023)",
    )
    .execute(&pool)
    .await
    .unwrap();

    // 졸업생 행에 학년/반/번호가 채워져 있어도 '졸업' 표기가 우선한다
    let csv = "학생코드,이름,재학여부,학년,반,번호,졸업연도\n\
               G001,김졸업,졸업,3,1,5,2023\n";
    let (status, axum::Json(result)) =
        import_students(State(state), common::csv_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::OK, "errors: {:?}", result.errors);

    let (is_enrolled, grad_year): (i64, Option<i64>) = sqlx::query_as(
        "SELECT is_enrolled, grad_year FROM students WHERE student_code = 'G001'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(is_enrolled, 0, "졸업생 분류 유지");
    assert_eq!(grad_year, Some(2023), "grad_year 소실 금지");
}

#[tokio::test]
async fn import_students_success_commits() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let state = common::make_state(pool.clone());

    let csv = "학생코드,이름,재학여부,학년,반,번호\n\
               S001,홍길동,재학,1,1,1\n\
               S002,이순신,재학,1,1,2\n";
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

// ── 세션 3 감사 후속: 파일 내 중복 행 = error / 신규 학급 비밀번호 필수 ──

#[tokio::test]
async fn import_students_duplicate_code_in_file_rejects_all() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let state = common::make_state(pool.clone());

    // 같은 학생코드가 두 행 — 마지막 행이 조용히 이기면 안 됨
    let csv = "학생코드,이름,재학여부,학년,반,번호,졸업연도\n\
               S001,홍길동,재학,1,1,1,\n\
               S001,홍길순,졸업,,,,2024\n";
    let (status, axum::Json(result)) =
        import_students(State(state), common::csv_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(result.errors[0].contains("중복"));

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM students")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "rollback — 1행도 저장되면 안 됨");
}

#[tokio::test]
async fn import_enrolled_duplicate_position_in_file_rejects_all() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let state = common::make_state(pool.clone());

    let csv = "학년,반,번호,이름\n1,1,1,홍길동\n1,1,1,이순신\n";
    let (status, axum::Json(result)) =
        import_enrolled(State(state), common::csv_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(result.errors[0].contains("중복"));

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM students")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn import_graduated_duplicate_code_in_file_rejects_all() {
    let pool = common::create_test_pool().await;
    let state = common::make_state(pool.clone());

    let csv = "학생코드,이름,졸업연도\nG001,김철수,2024\nG001,김영희,2023\n";
    let (status, axum::Json(result)) =
        import_graduated(State(state), common::csv_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(result.errors[0].contains("중복"));

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM students")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn import_classes_duplicate_class_in_file_rejects_all() {
    let pool = common::create_test_pool().await;
    let state = common::make_state(pool.clone());

    // 같은 (학년, 반)이 두 행 — 두 번째 비밀번호가 조용히 채택되면 안 됨
    let csv = "학년,반,담임명,비밀번호\n1,1,홍길동,pass1234\n1,1,이순신,pass5678\n";
    let (status, axum::Json(result)) =
        import_classes(State(state), common::csv_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(result["errors"][0].as_str().unwrap().contains("중복"));

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM classes")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn import_classes_new_class_without_password_rejects_all() {
    // 신규 학급 + 비밀번호 누락 → NOT NULL 500이 아니라 행 오류 422
    let pool = common::create_test_pool().await;
    let state = common::make_state(pool.clone());

    let csv = "학년,반,담임명,비밀번호\n1,1,홍길동,\n";
    let (status, axum::Json(result)) =
        import_classes(State(state), common::csv_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(result["errors"][0].as_str().unwrap().contains("비밀번호"));

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM classes")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn import_classes_existing_class_without_password_updates_name_only() {
    // 기존 학급은 비밀번호 없이 담임명만 갱신 가능 (기존 동작 보존)
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let state = common::make_state(pool.clone());

    let csv = "학년,반,담임명,비밀번호\n1,1,새담임,\n";
    let (status, axum::Json(result)) =
        import_classes(State(state), common::csv_multipart(csv).await)
            .await
            .unwrap();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["updated"], 1);

    let name: String = sqlx::query_scalar(
        "SELECT teacher_name FROM classes WHERE grade = 1 AND class_no = 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(name, "새담임");
}
