/// 외부(대교협) 석차연명부 가져오기 핸들러 검증.
/// 세션 3 감사 후속: multi_value 차단, 파일 내 중복=error, 빈 데이터 거부, 전체 rollback.
mod common;

use axum::{
    body::Body,
    extract::{FromRequest, Multipart, Path, State},
    http::{Request, StatusCode},
};
use principal_candidate_manager::handlers::external_import::daegyo_import;
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
