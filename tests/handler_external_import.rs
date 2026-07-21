/// 외부(대교협) 석차연명부 가져오기 핸들러 검증.
/// 세션 3 감사 후속: multi_value 차단, 파일 내 중복=error, 빈 데이터 거부, 전체 rollback.
mod common;

use axum::{
    body::Body,
    extract::{FromRequest, Multipart, Path, State},
    http::{Request, StatusCode},
};
use principal_candidate_manager::handlers::external_import::{daegyo_import, univ_import};
use rust_xlsxwriter::Workbook;

/// 대교협 양식 xlsx 생성: 1행 대학 정보, 2행 헤더, 3행~ 데이터
fn build_daegyo_xlsx(data_rows: &[(i64, i64, i64, &str, &str)]) -> Vec<u8> {
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    ws.write_string(0, 0, "서울-테스트대(본교)-학교장추천-2026").unwrap();
    let headers = ["학년", "반", "번호", "이름", "일반등급", "내점수(환산)", "내등급(환산)"];
    for (i, h) in headers.iter().enumerate() {
        ws.write_string(1, i as u16, *h).unwrap();
    }
    for (r, (grade, class_no, seq_no, name, value)) in data_rows.iter().enumerate() {
        let row = r as u32 + 2;
        ws.write_number(row, 0, *grade as f64).unwrap();
        ws.write_number(row, 1, *class_no as f64).unwrap();
        ws.write_number(row, 2, *seq_no as f64).unwrap();
        ws.write_string(row, 3, *name).unwrap();
        ws.write_string(row, 4, "2.0").unwrap(); // 일반등급 (미사용 경로)
        ws.write_string(row, 5, "제공").unwrap(); // "미제공" 아님 → 내등급(환산) 사용
        ws.write_string(row, 6, *value).unwrap();
    }
    wb.save_to_buffer().unwrap()
}

/// file + univ_name + track_name 3필드 multipart 요청 생성
async fn import_multipart(xlsx: &[u8], univ_name: &str, track_name: &str) -> Multipart {
    let boundary = "boundary42";
    let mut body: Vec<u8> = Vec::new();
    body.extend_from_slice(
        format!(
            "--{boundary}\r\n\
             Content-Disposition: form-data; name=\"file\"; filename=\"data.xlsx\"\r\n\
             Content-Type: application/octet-stream\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(xlsx);
    body.extend_from_slice(
        format!(
            "\r\n--{boundary}\r\n\
             Content-Disposition: form-data; name=\"univ_name\"\r\n\r\n\
             {univ_name}\r\n\
             --{boundary}\r\n\
             Content-Disposition: form-data; name=\"track_name\"\r\n\r\n\
             {track_name}\r\n\
             --{boundary}--\r\n"
        )
        .as_bytes(),
    );
    let req = Request::builder()
        .method("POST")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();
    Multipart::from_request(req, &()).await.unwrap()
}

async fn insert_enrolled_student(pool: &sqlx::SqlitePool, code: &str, grade: i64, class_no: i64, seq_no: i64, name: &str) {
    sqlx::query(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES (?, ?, ?, ?, ?, 1)",
    )
    .bind(code).bind(name).bind(grade).bind(class_no).bind(seq_no)
    .execute(pool)
    .await
    .unwrap();
}

async fn insert_composite_area(pool: &sqlx::SqlitePool, calc_type: &str, multi_value: i64) -> i64 {
    sqlx::query(
        "INSERT INTO areas (name, max_score, calc_type, match_mode, category_agg, lookup_scope, multi_value) \
         VALUES (?, 10000000, ?, ?, ?, 'COMPOSITE', ?)",
    )
    .bind(format!("ext_{calc_type}_{multi_value}"))
    .bind(calc_type)
    .bind(if calc_type == "NUMERIC" { Some("UPPER") } else { None })
    .bind(if calc_type == "CATEGORY" { Some("SUM") } else { None })
    .bind(multi_value)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_rowid()
}

#[tokio::test]
async fn daegyo_import_success_commits() {
    let pool = common::create_test_pool_shared().await;
    common::insert_class(&pool, 3, 1).await;
    insert_enrolled_student(&pool, "E001", 3, 1, 1, "홍길동").await;
    insert_enrolled_student(&pool, "E002", 3, 1, 2, "이순신").await;
    let aid = insert_composite_area(&pool, "NUMERIC", 0).await;
    let state = common::make_state(pool.clone());

    let xlsx = build_daegyo_xlsx(&[(3, 1, 1, "홍길동", "1.5"), (3, 1, 2, "이순신", "2.5")]);
    let (status, axum::Json(result)) = daegyo_import(
        State(state),
        Path(aid),
        import_multipart(&xlsx, "테스트대", "컴퓨터공학부").await,
    )
    .await
    .unwrap();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(result.rows, 2);

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM base_data WHERE area_id = ?")
        .bind(aid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 2);
}

#[tokio::test]
async fn daegyo_import_duplicate_student_rejects_all() {
    // 파일 내 같은 학생 두 행 → 마지막 행이 조용히 이기면 안 됨 (중복=error)
    let pool = common::create_test_pool_shared().await;
    common::insert_class(&pool, 3, 1).await;
    insert_enrolled_student(&pool, "E001", 3, 1, 1, "홍길동").await;
    let aid = insert_composite_area(&pool, "NUMERIC", 0).await;
    let state = common::make_state(pool.clone());

    let xlsx = build_daegyo_xlsx(&[(3, 1, 1, "홍길동", "1.5"), (3, 1, 1, "홍길동", "3.0")]);
    let (status, axum::Json(result)) = daegyo_import(
        State(state),
        Path(aid),
        import_multipart(&xlsx, "테스트대", "컴퓨터공학부").await,
    )
    .await
    .unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(result.errors[0].contains("중복"));
    // 행 번호는 엑셀 원본 기준 — 중복 행은 4행 (1행 정보, 2행 헤더, 3·4행 데이터)
    assert!(result.errors[0].starts_with("4행"), "실제 오류: {}", result.errors[0]);

    let base_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM base_data WHERE area_id = ?")
        .bind(aid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(base_count, 0, "rollback — 부분 저장 없음");

    let univ_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM universities")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(univ_count, 0, "자동 생성 대학도 rollback 되어야 함");
}

#[tokio::test]
async fn daegyo_import_multi_value_area_rejected() {
    // 복수값(CATEGORY SUM) 전형요소: 값 변경 재업로드 시 기존 행이 남아
    // SUM 이중 합산이 가능하므로 외부 가져오기 자체를 400으로 거부
    let pool = common::create_test_pool_shared().await;
    common::insert_class(&pool, 3, 1).await;
    insert_enrolled_student(&pool, "E001", 3, 1, 1, "홍길동").await;
    let aid = insert_composite_area(&pool, "CATEGORY", 1).await;
    let state = common::make_state(pool.clone());

    let xlsx = build_daegyo_xlsx(&[(3, 1, 1, "홍길동", "수상")]);
    let result = daegyo_import(
        State(state),
        Path(aid),
        import_multipart(&xlsx, "테스트대", "컴퓨터공학부").await,
    )
    .await;
    let Err(err) = result else { panic!("오류가 반환되어야 함") };

    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(err.1.contains("복수값"));

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM base_data WHERE area_id = ?")
        .bind(aid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn daegyo_import_empty_data_rejected() {
    // 데이터 0행 파일 → 트랙만 생성되고 200이 나오면 안 됨
    let pool = common::create_test_pool_shared().await;
    let aid = insert_composite_area(&pool, "NUMERIC", 0).await;
    let state = common::make_state(pool.clone());

    let xlsx = build_daegyo_xlsx(&[]);
    let result = daegyo_import(
        State(state),
        Path(aid),
        import_multipart(&xlsx, "테스트대", "컴퓨터공학부").await,
    )
    .await;
    let Err(err) = result else { panic!("오류가 반환되어야 함") };

    assert_eq!(err.0, StatusCode::BAD_REQUEST);

    let univ_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM universities")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(univ_count, 0, "대학이 생성되면 안 됨");
}

#[tokio::test]
async fn daegyo_import_reupload_replaces_single_value() {
    // 단일값 전형요소 재업로드: 값이 교체되고 행이 늘어나지 않아야 함
    let pool = common::create_test_pool_shared().await;
    common::insert_class(&pool, 3, 1).await;
    insert_enrolled_student(&pool, "E001", 3, 1, 1, "홍길동").await;
    let aid = insert_composite_area(&pool, "NUMERIC", 0).await;

    for value in ["1.5", "2.0"] {
        let state = common::make_state(pool.clone());
        let xlsx = build_daegyo_xlsx(&[(3, 1, 1, "홍길동", value)]);
        let (status, _) = daegyo_import(
            State(state),
            Path(aid),
            import_multipart(&xlsx, "테스트대", "컴퓨터공학부").await,
        )
        .await
        .unwrap();
        assert_eq!(status, StatusCode::OK);
    }

    let rows: Vec<String> =
        sqlx::query_scalar("SELECT value FROM base_data WHERE area_id = ?")
            .bind(aid)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(rows.len(), 1, "재업로드 시 행이 누적되면 안 됨");
    assert_eq!(rows[0], "200000", "마지막 업로드 값으로 교체되어야 함");
}

// ── 세션 5: 거부 경로 보충 ────────────────────────────────────────
// 공통 단언 3종: ① 상태코드 ② 테이블 불변(rollback) ③ 행 번호+원인 메시지

/// 셀을 문자열 그대로 쓰는 대교협 xlsx 빌더 — 학년/반/번호 파싱 실패 케이스용
fn build_daegyo_xlsx_str(data_rows: &[[&str; 5]]) -> Vec<u8> {
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    ws.write_string(0, 0, "서울-테스트대(본교)-학교장추천-2026").unwrap();
    let headers = ["학년", "반", "번호", "이름", "일반등급", "내점수(환산)", "내등급(환산)"];
    for (i, h) in headers.iter().enumerate() {
        ws.write_string(1, i as u16, *h).unwrap();
    }
    for (r, [grade, class_no, seq_no, name, value]) in data_rows.iter().enumerate() {
        let row = r as u32 + 2;
        ws.write_string(row, 0, *grade).unwrap();
        ws.write_string(row, 1, *class_no).unwrap();
        ws.write_string(row, 2, *seq_no).unwrap();
        ws.write_string(row, 3, *name).unwrap();
        ws.write_string(row, 4, "2.0").unwrap();
        ws.write_string(row, 5, "제공").unwrap();
        ws.write_string(row, 6, *value).unwrap();
    }
    wb.save_to_buffer().unwrap()
}

async fn assert_no_side_effects(pool: &sqlx::SqlitePool, aid: i64) {
    let base_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM base_data WHERE area_id = ?")
        .bind(aid)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(base_count, 0, "rollback — 부분 저장 없음");
    let univ_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM universities")
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(univ_count, 0, "자동 생성 대학도 rollback 되어야 함");
}

#[tokio::test]
async fn daegyo_import_unregistered_student_rejects_all() {
    // DB에 없는 학생 → 422 + 위치·이름 안내, 전체 rollback
    let pool = common::create_test_pool_shared().await;
    common::insert_class(&pool, 3, 1).await;
    insert_enrolled_student(&pool, "E001", 3, 1, 1, "홍길동").await;
    let aid = insert_composite_area(&pool, "NUMERIC", 0).await;
    let state = common::make_state(pool.clone());

    // 3반 9번은 미등록 — 등록된 1번 행이 있어도 전체 거부
    let xlsx = build_daegyo_xlsx(&[(3, 1, 1, "홍길동", "1.5"), (3, 3, 9, "미등록", "2.0")]);
    let (status, axum::Json(result)) = daegyo_import(
        State(state),
        Path(aid),
        import_multipart(&xlsx, "테스트대", "컴퓨터공학부").await,
    )
    .await
    .unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(result.rows, 0);
    assert!(result.errors[0].starts_with("4행"), "실제 오류: {}", result.errors[0]);
    assert!(
        result.errors[0].contains("등록된 재학생을 찾을 수 없습니다"),
        "실제 오류: {}",
        result.errors[0]
    );
    assert!(result.errors[0].contains("3학년 3반 9번"), "실제 오류: {}", result.errors[0]);
    assert_no_side_effects(&pool, aid).await;
}

#[tokio::test]
async fn daegyo_import_missing_header_returns_bad_request() {
    // 헤더에서 '이름' 열 제거 → 400 + 어느 열이 없는지 안내
    let pool = common::create_test_pool_shared().await;
    let aid = insert_composite_area(&pool, "NUMERIC", 0).await;
    let state = common::make_state(pool.clone());

    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    ws.write_string(0, 0, "서울-테스트대(본교)-학교장추천-2026").unwrap();
    for (i, h) in ["학년", "반", "번호", "일반등급", "내점수(환산)", "내등급(환산)"].iter().enumerate() {
        ws.write_string(1, i as u16, *h).unwrap();
    }
    let xlsx = wb.save_to_buffer().unwrap();

    let res = daegyo_import(
        State(state),
        Path(aid),
        import_multipart(&xlsx, "테스트대", "컴퓨터공학부").await,
    )
    .await;
    let Err(err) = res else { panic!("헤더 누락 시 거부되어야 함") };

    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(err.1.contains("'이름'"), "실제 오류: {}", err.1);
    assert_no_side_effects(&pool, aid).await;
}

// ── 석차 값 없음/변환 실패 = 행 건너뛰기 + warning ─────────────────
// 전출·자퇴 학생은 외부 프로그램이 등급을 '-'/공백으로 내보낸다.
// 전체 거부하면 관리자가 매 업로드마다 원본을 손봐야 하므로 해당 행만 건너뛴다.

/// 실제 대교협 파일 재현: 전출 학생 행은 점수·등급 4개 열이 모두 `'-`(아포스트로피 포함),
/// `석차` 열에는 숫자가 남아 있다. 미사용 열(`일반점수`·`석차`)도 실제 파일대로 넣는다.
fn build_daegyo_xlsx_real_transfer_row() -> Vec<u8> {
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    ws.write_string(0, 0, "서울-테스트대(본교)-학교장추천-2026").unwrap();
    let headers = [
        "학년", "반", "번호", "이름", "일반점수", "일반등급", "내점수(환산)", "내등급(환산)", "석차",
    ];
    for (i, h) in headers.iter().enumerate() {
        ws.write_string(1, i as u16, *h).unwrap();
    }
    // 3행: 정상 학생
    ws.write_number(2, 0, 3.0).unwrap();
    ws.write_number(2, 1, 6.0).unwrap();
    ws.write_number(2, 2, 1.0).unwrap();
    ws.write_string(2, 3, "홍길동").unwrap();
    ws.write_string(2, 4, "912.5").unwrap();
    ws.write_string(2, 5, "2.0").unwrap();
    ws.write_string(2, 6, "915.0").unwrap();
    ws.write_string(2, 7, "1.5").unwrap();
    ws.write_number(2, 8, 12.0).unwrap();
    // 4행: 전출 학생 — 사용자 제공 샘플 그대로
    ws.write_number(3, 0, 3.0).unwrap();
    ws.write_number(3, 1, 6.0).unwrap();
    ws.write_number(3, 2, 20.0).unwrap();
    ws.write_string(3, 3, "홍길동").unwrap();
    for c in 4..8u16 {
        ws.write_string(3, c, "'-").unwrap();
    }
    ws.write_number(3, 8, 335.0).unwrap();
    wb.save_to_buffer().unwrap()
}

#[tokio::test]
async fn daegyo_import_real_transfer_row_skips_only_that_student() {
    let pool = common::create_test_pool_shared().await;
    common::insert_class(&pool, 3, 6).await;
    insert_enrolled_student(&pool, "E001", 3, 6, 1, "홍길동").await;
    insert_enrolled_student(&pool, "E002", 3, 6, 20, "홍길동").await;
    let aid = insert_composite_area(&pool, "NUMERIC", 0).await;
    let state = common::make_state(pool.clone());

    let (status, axum::Json(result)) = daegyo_import(
        State(state),
        Path(aid),
        import_multipart(&build_daegyo_xlsx_real_transfer_row(), "테스트대", "컴퓨터공학부").await,
    )
    .await
    .unwrap();

    assert_eq!(status, StatusCode::OK, "전출 학생 1명 때문에 전체 거부되면 안 됨");
    assert_eq!(result.rows, 1);
    assert!(result.errors.is_empty(), "실제 오류: {:?}", result.errors);

    let skip = result
        .warnings
        .iter()
        .find(|w| w.contains("건너뜀"))
        .unwrap_or_else(|| panic!("건너뜀 경고 없음: {:?}", result.warnings));
    assert!(skip.starts_with("4행"), "실제 경고: {}", skip);
    assert!(skip.contains("3학년 6반 20번 홍길동"), "실제 경고: {}", skip);
    // 아포스트로피가 셀 값에 포함된 원본 그대로 표시되어야 관리자가 파일에서 찾을 수 있다
    assert!(skip.contains("'-"), "원본 값이 경고에 없음: {}", skip);

    let saved: Vec<(i64, String)> =
        sqlx::query_as("SELECT student_id, value FROM base_data WHERE area_id = ?")
            .bind(aid)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(saved.len(), 1, "정상 학생만 저장");
    assert_eq!(saved[0].1, "150000", "내등급(환산) 1.5 → ×100000");
}

#[tokio::test]
async fn daegyo_import_value_parse_error_skips_row_only() {
    let pool = common::create_test_pool_shared().await;
    common::insert_class(&pool, 3, 1).await;
    insert_enrolled_student(&pool, "E001", 3, 1, 1, "홍길동").await;
    insert_enrolled_student(&pool, "E002", 3, 1, 2, "전출생").await;
    let aid = insert_composite_area(&pool, "NUMERIC", 0).await;
    let state = common::make_state(pool.clone());

    let xlsx = build_daegyo_xlsx(&[(3, 1, 1, "홍길동", "1.5"), (3, 1, 2, "전출생", "-")]);
    let (status, axum::Json(result)) = daegyo_import(
        State(state),
        Path(aid),
        import_multipart(&xlsx, "테스트대", "컴퓨터공학부").await,
    )
    .await
    .unwrap();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(result.rows, 1, "정상 행만 저장");
    assert!(result.errors.is_empty(), "실제 오류: {:?}", result.errors);

    let skip = result
        .warnings
        .iter()
        .find(|w| w.contains("건너뜀"))
        .unwrap_or_else(|| panic!("건너뜀 경고 없음: {:?}", result.warnings));
    assert!(skip.starts_with("4행"), "실제 경고: {}", skip);
    assert!(skip.contains("3학년 1반 2번 전출생"), "실제 경고: {}", skip);
    assert!(skip.contains("숫자 변환 실패"), "실제 경고: {}", skip);

    let saved: Vec<String> = sqlx::query_scalar("SELECT value FROM base_data WHERE area_id = ?")
        .bind(aid)
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(saved, vec!["150000".to_string()], "건너뛴 학생 행은 저장되지 않음");
}

#[tokio::test]
async fn daegyo_import_missing_value_skips_row_only() {
    let pool = common::create_test_pool_shared().await;
    common::insert_class(&pool, 3, 1).await;
    insert_enrolled_student(&pool, "E001", 3, 1, 1, "홍길동").await;
    insert_enrolled_student(&pool, "E002", 3, 1, 2, "자퇴생").await;
    let aid = insert_composite_area(&pool, "NUMERIC", 0).await;
    let state = common::make_state(pool.clone());

    let xlsx = build_daegyo_xlsx(&[(3, 1, 1, "홍길동", "1.5"), (3, 1, 2, "자퇴생", "")]);
    let (status, axum::Json(result)) = daegyo_import(
        State(state),
        Path(aid),
        import_multipart(&xlsx, "테스트대", "컴퓨터공학부").await,
    )
    .await
    .unwrap();

    assert_eq!(status, StatusCode::OK);
    assert_eq!(result.rows, 1);
    let skip = result
        .warnings
        .iter()
        .find(|w| w.contains("건너뜀"))
        .unwrap_or_else(|| panic!("건너뜀 경고 없음: {:?}", result.warnings));
    assert!(skip.starts_with("4행"), "실제 경고: {}", skip);
    assert!(skip.contains("석차 값이 비어 있어"), "실제 경고: {}", skip);
}

#[tokio::test]
async fn daegyo_import_all_rows_skipped_rejects() {
    // 값 열을 잘못 고른 파일이 "완료 — 0건"으로 조용히 넘어가면 안 됨
    let pool = common::create_test_pool_shared().await;
    common::insert_class(&pool, 3, 1).await;
    insert_enrolled_student(&pool, "E001", 3, 1, 1, "홍길동").await;
    let aid = insert_composite_area(&pool, "NUMERIC", 0).await;
    let state = common::make_state(pool.clone());

    let xlsx = build_daegyo_xlsx(&[(3, 1, 1, "홍길동", "-")]);
    let (status, axum::Json(result)) = daegyo_import(
        State(state),
        Path(aid),
        import_multipart(&xlsx, "테스트대", "컴퓨터공학부").await,
    )
    .await
    .unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(result.rows, 0);
    assert!(result.errors[0].contains("저장된 행이 없습니다"), "실제 오류: {}", result.errors[0]);
    // 건너뛴 사유는 rollback 시에도 관리자에게 보여야 한다
    assert!(result.warnings.iter().any(|w| w.contains("건너뜀")), "실제 경고: {:?}", result.warnings);
    assert_no_side_effects(&pool, aid).await;
}

#[tokio::test]
async fn daegyo_import_non_numeric_grade_rejects_all() {
    let pool = common::create_test_pool_shared().await;
    common::insert_class(&pool, 3, 1).await;
    insert_enrolled_student(&pool, "E001", 3, 1, 1, "홍길동").await;
    let aid = insert_composite_area(&pool, "NUMERIC", 0).await;
    let state = common::make_state(pool.clone());

    let xlsx = build_daegyo_xlsx_str(&[["삼", "1", "1", "홍길동", "1.5"]]);
    let (status, axum::Json(result)) = daegyo_import(
        State(state),
        Path(aid),
        import_multipart(&xlsx, "테스트대", "컴퓨터공학부").await,
    )
    .await
    .unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(result.errors[0].starts_with("3행"), "실제 오류: {}", result.errors[0]);
    assert!(result.errors[0].contains("학년"), "실제 오류: {}", result.errors[0]);
    assert!(result.errors[0].contains("숫자 변환 실패"), "실제 오류: {}", result.errors[0]);
    assert_no_side_effects(&pool, aid).await;
}

#[tokio::test]
async fn daegyo_import_simple_area_rejected() {
    // 외부 가져오기는 COMPOSITE(대학별 환산) 전형요소 전용 — SIMPLE은 400
    let pool = common::create_test_pool_shared().await;
    common::insert_class(&pool, 3, 1).await;
    insert_enrolled_student(&pool, "E001", 3, 1, 1, "홍길동").await;
    let aid: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, match_mode, lookup_scope, multi_value) \
         VALUES ('내신', 10000000, 'NUMERIC', 'UPPER', 'SIMPLE', 0) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let state = common::make_state(pool.clone());

    let xlsx = build_daegyo_xlsx(&[(3, 1, 1, "홍길동", "1.5")]);
    let res = daegyo_import(
        State(state),
        Path(aid),
        import_multipart(&xlsx, "테스트대", "컴퓨터공학부").await,
    )
    .await;
    let Err(err) = res else { panic!("SIMPLE 전형요소는 거부되어야 함") };

    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(err.1.contains("대학별 환산점수"), "실제 오류: {}", err.1);
    assert_no_side_effects(&pool, aid).await;
}

#[tokio::test]
async fn univ_import_non_xls_file_returns_bad_request() {
    // 유니브 양식은 .xls (BIFF) — xlsx 바이트를 올리면 400
    // (정상 .xls 파싱 경로는 테스트에서 BIFF 파일을 생성할 수 없어 미커버 — do_import
    //  공통 로직은 daegyo 테스트가 커버한다)
    let pool = common::create_test_pool_shared().await;
    let aid = insert_composite_area(&pool, "NUMERIC", 0).await;
    let state = common::make_state(pool.clone());

    let xlsx = build_daegyo_xlsx(&[(3, 1, 1, "홍길동", "1.5")]);
    let res = univ_import(
        State(state),
        Path(aid),
        import_multipart(&xlsx, "테스트대", "컴퓨터공학부").await,
    )
    .await;
    let Err(err) = res else { panic!(".xls가 아닌 파일은 거부되어야 함") };

    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(err.1.contains(".xls"), "실제 오류: {}", err.1);
    assert_no_side_effects(&pool, aid).await;
}
