//! 템플릿(양식) 다운로드 → 그대로 채워 업로드 왕복 검증.
//!
//! CLAUDE.md 규칙 5에 따라 Excel 파싱은 헤더 **이름** 기반이다. 그래서 템플릿이
//! 내보내는 헤더와 import의 `require_cols`가 어긋나면, 관리자가 프로그램에서 받은
//! 양식을 그대로 채워 올려도 거부당한다. 양쪽 코드는 각자 정상이고 각자의 테스트도
//! 통과하는데 **사용자 워크플로만 끊기는** 결함이라, 왕복으로만 잡힌다.
//!
//! 판별력 설계:
//! - 기대 헤더 목록을 테스트에 하드코딩하지 않는다. 양쪽이 같이 바뀌어도 통과해버린다.
//! - 샘플 행이 들어 있는 템플릿(학생·학급)은 **템플릿 파일 자체를 그대로 import에
//!   투입**한다. 행 수도 템플릿에서 읽어 기대값으로 쓴다 — 테스트에 상수가 없다.
//! - 샘플 행이 없는 템플릿(기준표·범주표·기초데이터)은 템플릿이 돌려준 헤더 행을
//!   그대로 재사용해 값을 **헤더 이름으로** 채운다. 사전에 없는 헤더가 템플릿에
//!   있으면 조용히 빈 열로 두지 않고 panic — 헤더가 바뀌면 소리 내어 실패한다.

mod common;

use axum::{
    body::Body,
    extract::{FromRequest, Multipart, Path, Query, State},
    http::{header, Request, StatusCode},
};
use principal_candidate_manager::{
    excel,
    handlers::{
        area_data::{
            base_data_import, base_data_template, category_map_import, category_map_template,
            numeric_table_import, numeric_table_template, StudentTypeQuery,
        },
        areas::score_template,
        classes::{classes_template, import_classes},
        students::{
            download_template, enrolled_template, graduated_template, import_enrolled,
            import_graduated, import_students,
        },
    },
    state::AppState,
};
use rust_xlsxwriter::Workbook;
use sqlx::SqlitePool;

// ── 헬퍼 ─────────────────────────────────────────────────────────

fn st(pool: &SqlitePool) -> State<AppState> {
    State(common::make_state(pool.clone()))
}

fn enrolled_q() -> Query<StudentTypeQuery> {
    Query(StudentTypeQuery { student_type: "enrolled".into() })
}

fn graduated_q() -> Query<StudentTypeQuery> {
    Query(StudentTypeQuery { student_type: "graduated".into() })
}

async fn response_bytes(resp: axum::response::Response) -> Vec<u8> {
    axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap().to_vec()
}

fn content_disposition(resp: &axum::response::Response) -> String {
    resp.headers()
        .get(header::CONTENT_DISPOSITION)
        .expect("Content-Disposition 헤더 없음")
        .to_str()
        .unwrap()
        .to_string()
}

async fn xlsx_multipart(bytes: &[u8]) -> Multipart {
    let boundary = "boundary42";
    let mut body: Vec<u8> = Vec::new();
    body.extend_from_slice(
        format!(
            "--{boundary}\r\n\
             Content-Disposition: form-data; name=\"file\"; filename=\"template.xlsx\"\r\n\
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

/// 템플릿 바이트 → (헤더 행, 데이터 행). 빈 행은 제외한다.
fn split_template(bytes: &[u8]) -> (Vec<String>, Vec<Vec<String>>) {
    let mut rows = excel::parse_xlsx_all_rows_raw(bytes).expect("템플릿 xlsx 파싱 실패");
    assert!(!rows.is_empty(), "템플릿에 헤더 행이 없습니다");
    let headers = rows.remove(0);
    assert!(
        headers.iter().any(|h| !h.trim().is_empty()),
        "템플릿 헤더 행이 비어 있습니다"
    );
    let data = rows
        .into_iter()
        .filter(|r| !r.iter().all(|c| c.trim().is_empty()))
        .collect();
    (headers, data)
}

/// 헤더 이름 → 열 인덱스. 없으면 panic (템플릿이 그 열을 더 이상 내보내지 않는다는 뜻).
fn col_index(headers: &[String], name: &str) -> usize {
    headers
        .iter()
        .position(|h| h.trim() == name)
        .unwrap_or_else(|| panic!("템플릿 헤더에 '{}' 열이 없습니다: {:?}", name, headers))
}

/// 템플릿이 돌려준 헤더 행을 **그대로** 쓰고, 값은 헤더 이름으로 지정해 채운다.
///
/// - 사전에 있는데 템플릿 헤더에 없는 이름 → panic (테스트가 헤더를 오해하고 있다)
/// - 템플릿 헤더에 있는데 어느 행에서도 값을 안 준 이름 → panic
///   (조용히 빈 열이 되면 "헤더가 바뀌었는데 테스트는 통과"가 된다)
fn fill_template(headers: &[String], rows: &[Vec<(&str, &str)>]) -> Vec<u8> {
    let mut used: Vec<&str> = Vec::new();
    for row in rows {
        for (k, _) in row {
            assert!(
                headers.iter().any(|h| h.trim() == *k),
                "템플릿 헤더에 '{}' 열이 없습니다: {:?}",
                k,
                headers
            );
            if !used.contains(k) {
                used.push(k);
            }
        }
    }
    for h in headers {
        let h = h.trim();
        if h.is_empty() {
            continue;
        }
        assert!(
            used.contains(&h),
            "템플릿이 내보낸 '{}' 열에 테스트가 값을 채우지 않았습니다 — 헤더가 바뀌었는지 확인하세요",
            h
        );
    }

    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    for (i, h) in headers.iter().enumerate() {
        ws.write_string(0, i as u16, h.as_str()).unwrap();
    }
    for (r, row) in rows.iter().enumerate() {
        for (k, v) in row {
            let i = col_index(headers, k);
            ws.write_string(r as u32 + 1, i as u16, *v).unwrap();
        }
    }
    wb.save_to_buffer().unwrap()
}

/// 템플릿의 학년·반 열을 읽어 필요한 학급을 미리 만든다.
/// (샘플 행의 학년·반 값을 테스트에 상수로 박지 않기 위한 픽스처 준비)
async fn ensure_classes_from(pool: &SqlitePool, headers: &[String], data: &[Vec<String>]) {
    let gi = col_index(headers, "학년");
    let ci = col_index(headers, "반");
    for row in data {
        let (Some(g), Some(c)) = (row.get(gi), row.get(ci)) else { continue };
        let (Ok(g), Ok(c)) = (g.trim().parse::<i64>(), c.trim().parse::<i64>()) else { continue };
        let exists: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM classes WHERE grade = ? AND class_no = ?")
                .bind(g)
                .bind(c)
                .fetch_one(pool)
                .await
                .unwrap();
        if exists == 0 {
            common::insert_class(pool, g, c).await;
        }
    }
}

async fn new_area(pool: &SqlitePool, sql_values: &str) -> i64 {
    sqlx::query_scalar(&format!("INSERT INTO areas {} RETURNING id", sql_values))
        .fetch_one(pool)
        .await
        .unwrap()
}

// ── 1. 샘플 행이 들어 있는 템플릿: 파일 자체를 그대로 재업로드 ────

#[tokio::test]
async fn students_all_template_imports_as_is() {
    let pool = common::create_test_pool_shared().await;
    let tpl = response_bytes(download_template().await.unwrap()).await;
    let (headers, data) = split_template(&tpl);
    assert!(!data.is_empty(), "학생 통합 양식에 샘플 행이 있어야 한다");
    ensure_classes_from(&pool, &headers, &data).await;

    let (status, axum::Json(result)) =
        import_students(st(&pool), xlsx_multipart(&tpl).await).await.unwrap();
    assert_eq!(
        status,
        StatusCode::OK,
        "받은 양식을 그대로 올렸는데 거부됨: {:?}",
        result.errors
    );
    assert_eq!(result.inserted, data.len(), "샘플 행 수만큼 등록되어야 함");

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM students")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(total as usize, data.len());
    // 양식이 재학·졸업 두 종류를 모두 예시하므로 둘 다 저장돼야 한다
    let enrolled: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM students WHERE is_enrolled = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let graduated: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM students WHERE is_enrolled = 0")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(enrolled > 0 && graduated > 0, "재학·졸업 샘플이 모두 저장돼야 함");
}

#[tokio::test]
async fn enrolled_template_imports_as_is() {
    let pool = common::create_test_pool_shared().await;
    let tpl = response_bytes(enrolled_template().await.unwrap()).await;
    let (headers, data) = split_template(&tpl);
    assert!(!data.is_empty(), "재학생 양식에 샘플 행이 있어야 한다");
    ensure_classes_from(&pool, &headers, &data).await;

    let (status, axum::Json(result)) =
        import_enrolled(st(&pool), xlsx_multipart(&tpl).await).await.unwrap();
    assert_eq!(status, StatusCode::OK, "받은 양식 그대로 거부됨: {:?}", result.errors);
    assert_eq!(result.inserted, data.len());

    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM students WHERE is_enrolled = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(n as usize, data.len());
}

#[tokio::test]
async fn graduated_template_imports_as_is() {
    let pool = common::create_test_pool_shared().await;
    let tpl = response_bytes(graduated_template().await.unwrap()).await;
    let (_, data) = split_template(&tpl);
    assert!(!data.is_empty(), "졸업생 양식에 샘플 행이 있어야 한다");

    let (status, axum::Json(result)) =
        import_graduated(st(&pool), xlsx_multipart(&tpl).await).await.unwrap();
    assert_eq!(status, StatusCode::OK, "받은 양식 그대로 거부됨: {:?}", result.errors);
    assert_eq!(result.inserted, data.len());

    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM students WHERE is_enrolled = 0 AND grad_year IS NOT NULL",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(n as usize, data.len(), "졸업연도 열이 실제로 저장돼야 함");
}

#[tokio::test]
async fn classes_template_imports_as_is() {
    let pool = common::create_test_pool_shared().await;
    let tpl = response_bytes(classes_template().await.unwrap()).await;
    let (headers, data) = split_template(&tpl);
    assert!(!data.is_empty(), "학급 양식에 샘플 행이 있어야 한다");

    let (status, axum::Json(result)) =
        import_classes(st(&pool), xlsx_multipart(&tpl).await).await.unwrap();
    assert_eq!(status, StatusCode::OK, "받은 양식 그대로 거부됨: {}", result);
    assert_eq!(result["inserted"], data.len(), "샘플 행 수만큼 학급이 생겨야 함");

    // 신규 학급은 비밀번호가 필수다 — 양식의 비밀번호 열이 실제로 쓰였는지 확인
    let hashed: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM classes WHERE password_hash IS NOT NULL AND password_hash != ''",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(hashed as usize, data.len());
    // 담임명 열도 반영돼야 한다 (템플릿 샘플 값과 대조)
    let ni = col_index(&headers, "담임명");
    let expected = data[0][ni].trim();
    let stored: String = sqlx::query_scalar("SELECT teacher_name FROM classes LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored, expected);
}

// ── 2. 빈 양식: 템플릿 헤더를 그대로 재사용해 채운 뒤 업로드 ──────

#[tokio::test]
async fn numeric_table_template_simple_roundtrips() {
    let pool = common::create_test_pool_shared().await;
    let aid = new_area(
        &pool,
        "(name, max_score, calc_type, match_mode, lookup_scope) \
         VALUES ('봉사', 10000000, 'NUMERIC', 'UPPER', 'SIMPLE')",
    )
    .await;

    let tpl = response_bytes(numeric_table_template(st(&pool), Path(aid)).await.unwrap()).await;
    let (headers, data) = split_template(&tpl);
    assert!(data.is_empty(), "빈 양식이어야 한다");

    let filled = fill_template(
        &headers,
        &[
            vec![("기준값", "0"), ("점수", "0")],
            vec![("기준값", "20"), ("점수", "50.5")],
            vec![("기준값", "40"), ("점수", "100")],
        ],
    );
    let (status, axum::Json(result)) =
        numeric_table_import(st(&pool), Path(aid), xlsx_multipart(&filled).await).await.unwrap();
    assert_eq!(status, StatusCode::OK, "양식 그대로 채운 파일이 거부됨: {:?}", result.errors);

    let rows: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT threshold, score FROM numeric_table WHERE area_id = ? ORDER BY threshold",
    )
    .bind(aid)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows, vec![(0, 0), (2_000_000, 5_050_000), (4_000_000, 10_000_000)]);
}

#[tokio::test]
async fn numeric_table_template_composite_roundtrips() {
    let pool = common::create_test_pool_shared().await;
    let aid = new_area(
        &pool,
        "(name, max_score, calc_type, match_mode, lookup_scope) \
         VALUES ('환산내신', 10000000, 'NUMERIC', 'LOWER', 'COMPOSITE')",
    )
    .await;

    let tpl = response_bytes(numeric_table_template(st(&pool), Path(aid)).await.unwrap()).await;
    let (headers, _) = split_template(&tpl);

    // COMPOSITE 양식은 대학명·모집단위명 열까지 내보내야 한다 — fill_template이 강제한다
    let filled = fill_template(
        &headers,
        &[
            vec![("기준값", "1"), ("점수", "100"), ("대학명", "한국대"), ("모집단위명", "컴퓨터공학")],
            vec![("기준값", "3"), ("점수", "60"), ("대학명", "한국대"), ("모집단위명", "컴퓨터공학")],
        ],
    );
    let (status, axum::Json(result)) =
        numeric_table_import(st(&pool), Path(aid), xlsx_multipart(&filled).await).await.unwrap();
    assert_eq!(status, StatusCode::OK, "COMPOSITE 양식 왕복 실패: {:?}", result.errors);

    // 대학명·모집단위명이 실제로 track에 묶여 저장됐는지 (공통행으로 강등되면 실패)
    let bound: Vec<(i64, i64, String, String)> = sqlx::query_as(
        "SELECT nt.threshold, nt.score, u.univ_name, ut.track_name
         FROM numeric_table nt
         JOIN univ_tracks ut ON ut.id = nt.track_id
         JOIN universities u ON u.id = ut.univ_id
         WHERE nt.area_id = ? ORDER BY nt.threshold",
    )
    .bind(aid)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        bound,
        vec![
            (100_000, 10_000_000, "한국대".into(), "컴퓨터공학".into()),
            (300_000, 6_000_000, "한국대".into(), "컴퓨터공학".into()),
        ]
    );
}

#[tokio::test]
async fn category_map_template_simple_roundtrips() {
    let pool = common::create_test_pool_shared().await;
    let aid = new_area(
        &pool,
        "(name, max_score, calc_type, category_agg, lookup_scope) \
         VALUES ('수상', 10000000, 'CATEGORY', 'MAX', 'SIMPLE')",
    )
    .await;

    let tpl = response_bytes(category_map_template(st(&pool), Path(aid)).await.unwrap()).await;
    let (headers, data) = split_template(&tpl);
    assert!(data.is_empty(), "빈 양식이어야 한다");

    let filled = fill_template(
        &headers,
        &[
            vec![("범주", "최우수상"), ("점수", "30")],
            vec![("범주", "해당없음"), ("점수", "0")],
        ],
    );
    let (status, axum::Json(result)) =
        category_map_import(st(&pool), Path(aid), xlsx_multipart(&filled).await).await.unwrap();
    assert_eq!(status, StatusCode::OK, "양식 그대로 채운 파일이 거부됨: {:?}", result.errors);

    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT category, score FROM category_map WHERE area_id = ? ORDER BY score DESC",
    )
    .bind(aid)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows, vec![("최우수상".into(), 3_000_000), ("해당없음".into(), 0)]);
}

#[tokio::test]
async fn category_map_template_composite_roundtrips() {
    let pool = common::create_test_pool_shared().await;
    let aid = new_area(
        &pool,
        "(name, max_score, calc_type, category_agg, lookup_scope) \
         VALUES ('대학별가산', 10000000, 'CATEGORY', 'MAX', 'COMPOSITE')",
    )
    .await;

    let tpl = response_bytes(category_map_template(st(&pool), Path(aid)).await.unwrap()).await;
    let (headers, _) = split_template(&tpl);

    let filled = fill_template(
        &headers,
        &[
            vec![
                ("범주", "우수"),
                ("점수", "10"),
                ("대학명", "한국대"),
                ("모집단위명", "컴퓨터공학"),
            ],
            // 0점 기준(해당하지 않음)은 모집단위마다 필수
            vec![
                ("범주", "해당없음"),
                ("점수", "0"),
                ("대학명", "한국대"),
                ("모집단위명", "컴퓨터공학"),
            ],
        ],
    );
    let (status, axum::Json(result)) =
        category_map_import(st(&pool), Path(aid), xlsx_multipart(&filled).await).await.unwrap();
    assert_eq!(status, StatusCode::OK, "COMPOSITE 범주 양식 왕복 실패: {:?}", result.errors);

    let bound: Vec<(String, i64, String, String)> = sqlx::query_as(
        "SELECT cm.category, cm.score, u.univ_name, ut.track_name
         FROM category_map cm
         JOIN univ_tracks ut ON ut.id = cm.track_id
         JOIN universities u ON u.id = ut.univ_id
         WHERE cm.area_id = ? ORDER BY cm.score DESC",
    )
    .bind(aid)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        bound,
        vec![
            ("우수".into(), 1_000_000, "한국대".into(), "컴퓨터공학".into()),
            ("해당없음".into(), 0, "한국대".into(), "컴퓨터공학".into()),
        ]
    );
}

#[tokio::test]
async fn base_data_template_enrolled_roundtrips() {
    let pool = common::create_test_pool_shared().await;
    common::insert_class(&pool, 3, 1).await;
    sqlx::query(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled)
         VALUES ('20250001', '홍길동', 3, 1, 1, 1)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let aid = new_area(
        &pool,
        "(name, max_score, calc_type, match_mode, lookup_scope) \
         VALUES ('봉사시간', 10000000, 'NUMERIC', 'UPPER', 'SIMPLE')",
    )
    .await;

    let tpl = response_bytes(
        base_data_template(st(&pool), Path(aid), enrolled_q()).await.unwrap(),
    )
    .await;
    let (headers, data) = split_template(&tpl);
    assert!(data.is_empty(), "재학생 기초데이터 양식은 빈 양식이어야 한다");

    let filled = fill_template(
        &headers,
        &[vec![
            ("학년", "3"),
            ("반", "1"),
            ("번호", "1"),
            ("이름", "홍길동"),
            ("값", "35.5"),
        ]],
    );
    let (status, axum::Json(result)) =
        base_data_import(st(&pool), Path(aid), enrolled_q(), xlsx_multipart(&filled).await)
            .await
            .unwrap();
    assert_eq!(status, StatusCode::OK, "재학생 양식 왕복 실패: {:?}", result.errors);

    let stored: String = sqlx::query_scalar(
        "SELECT value FROM base_data bd JOIN students s ON s.id = bd.student_id
         WHERE bd.area_id = ? AND s.student_code = '20250001'",
    )
    .bind(aid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(stored, "3550000", "35.5시간 → ×100000 정수");
}

#[tokio::test]
async fn base_data_template_graduated_prefills_students_and_roundtrips() {
    let pool = common::create_test_pool_shared().await;
    for (code, name) in [("20240002", "김철수"), ("20240001", "이영희")] {
        sqlx::query(
            "INSERT INTO students (student_code, name, is_enrolled, grad_year)
             VALUES (?, ?, 0, 2024)",
        )
        .bind(code)
        .bind(name)
        .execute(&pool)
        .await
        .unwrap();
    }
    let aid = new_area(
        &pool,
        "(name, max_score, calc_type, match_mode, lookup_scope) \
         VALUES ('봉사시간', 10000000, 'NUMERIC', 'UPPER', 'SIMPLE')",
    )
    .await;

    let tpl = response_bytes(
        base_data_template(st(&pool), Path(aid), graduated_q()).await.unwrap(),
    )
    .await;
    let (headers, data) = split_template(&tpl);
    // 졸업생 양식은 명단을 미리 채워 준다 — 학생코드 오름차순
    assert_eq!(data.len(), 2, "졸업생 2명이 미리 채워져야 함");
    let ci = col_index(&headers, "학생코드");
    let ni = col_index(&headers, "이름");
    assert_eq!(
        (data[0][ci].as_str(), data[0][ni].as_str()),
        ("20240001", "이영희"),
        "학생코드 오름차순으로 채워져야 함"
    );
    assert_eq!((data[1][ci].as_str(), data[1][ni].as_str()), ("20240002", "김철수"));

    // 미리 채워진 명단에 '값'만 적어 그대로 올린다 (실제 사용 방식)
    let filled = fill_template(
        &headers,
        &[
            vec![("학생코드", &data[0][ci]), ("이름", &data[0][ni]), ("값", "10")],
            vec![("학생코드", &data[1][ci]), ("이름", &data[1][ni]), ("값", "20")],
        ],
    );
    let (status, axum::Json(result)) =
        base_data_import(st(&pool), Path(aid), graduated_q(), xlsx_multipart(&filled).await)
            .await
            .unwrap();
    assert_eq!(status, StatusCode::OK, "졸업생 양식 왕복 실패: {:?}", result.errors);
    assert_eq!(result.rows, 2);

    let stored: Vec<(String, String)> = sqlx::query_as(
        "SELECT s.student_code, bd.value FROM base_data bd
         JOIN students s ON s.id = bd.student_id
         WHERE bd.area_id = ? ORDER BY s.student_code",
    )
    .bind(aid)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        stored,
        vec![
            ("20240001".to_string(), "1000000".to_string()),
            ("20240002".to_string(), "2000000".to_string()),
        ]
    );
}

#[tokio::test]
async fn base_data_template_graduated_composite_prefills_every_track() {
    let pool = common::create_test_pool_shared().await;
    sqlx::query(
        "INSERT INTO students (student_code, name, is_enrolled, grad_year)
         VALUES ('20240001', '이영희', 0, 2024)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name, prioritize_enrolled) VALUES ('한국대', 1) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    for t in ["컴퓨터공학", "기계공학"] {
        sqlx::query("INSERT INTO univ_tracks (univ_id, track_name, prioritize_enrolled) VALUES (?, ?, 1)")
            .bind(uid)
            .bind(t)
            .execute(&pool)
            .await
            .unwrap();
    }
    let aid = new_area(
        &pool,
        "(name, max_score, calc_type, match_mode, lookup_scope) \
         VALUES ('환산점수', 10000000, 'NUMERIC', 'UPPER', 'COMPOSITE')",
    )
    .await;

    let tpl = response_bytes(
        base_data_template(st(&pool), Path(aid), graduated_q()).await.unwrap(),
    )
    .await;
    let (headers, data) = split_template(&tpl);
    // 졸업생 1명 × 모집단위 2개 = 2행이 미리 채워져야 한다
    assert_eq!(data.len(), 2, "학생×모집단위 조합만큼 채워져야 함");
    let ui = col_index(&headers, "대학명");
    let ti = col_index(&headers, "모집단위명");
    let mut tracks: Vec<&str> = data.iter().map(|r| r[ti].as_str()).collect();
    tracks.sort();
    assert_eq!(tracks, vec!["기계공학", "컴퓨터공학"]);
    assert!(data.iter().all(|r| r[ui] == "한국대"));

    let ci = col_index(&headers, "학생코드");
    let ni = col_index(&headers, "이름");
    let filled = fill_template(
        &headers,
        &data
            .iter()
            .enumerate()
            .map(|(i, r)| {
                vec![
                    ("학생코드", r[ci].as_str()),
                    ("이름", r[ni].as_str()),
                    ("값", if i == 0 { "70" } else { "80" }),
                    ("대학명", r[ui].as_str()),
                    ("모집단위명", r[ti].as_str()),
                ]
            })
            .collect::<Vec<_>>(),
    );
    let (status, axum::Json(result)) =
        base_data_import(st(&pool), Path(aid), graduated_q(), xlsx_multipart(&filled).await)
            .await
            .unwrap();
    assert_eq!(status, StatusCode::OK, "COMPOSITE 졸업생 양식 왕복 실패: {:?}", result.errors);

    // 모집단위별로 서로 다른 값이 각자 트랙에 붙어야 한다 (공통행 강등이면 실패)
    let bound: Vec<(String, String)> = sqlx::query_as(
        "SELECT ut.track_name, bd.value FROM base_data bd
         JOIN univ_tracks ut ON ut.id = bd.track_id
         WHERE bd.area_id = ? ORDER BY ut.track_name",
    )
    .bind(aid)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(bound.len(), 2, "두 모집단위 모두 track_id가 채워져야 함");
    assert!(bound.iter().all(|(_, v)| v == "7000000" || v == "8000000"));
    assert_ne!(bound[0].1, bound[1].1, "모집단위별 값이 뒤섞이면 안 됨");
}

// ── 3. 템플릿 가드: 잘못된 전형요소·student_type은 즉시 거부 ─────

#[tokio::test]
async fn numeric_table_template_rejects_non_numeric_area() {
    let pool = common::create_test_pool_shared().await;
    let aid = new_area(
        &pool,
        "(name, max_score, calc_type, category_agg, lookup_scope) \
         VALUES ('수상', 10000000, 'CATEGORY', 'MAX', 'SIMPLE')",
    )
    .await;
    let err = numeric_table_template(st(&pool), Path(aid)).await.unwrap_err();
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(err.1.contains("RANGE"), "메시지: {}", err.1);
}

#[tokio::test]
async fn category_map_template_rejects_non_category_area() {
    let pool = common::create_test_pool_shared().await;
    let aid = new_area(
        &pool,
        "(name, max_score, calc_type, match_mode, lookup_scope) \
         VALUES ('봉사', 10000000, 'NUMERIC', 'UPPER', 'SIMPLE')",
    )
    .await;
    let err = category_map_template(st(&pool), Path(aid)).await.unwrap_err();
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(err.1.contains("CATEGORY"), "메시지: {}", err.1);
}

#[tokio::test]
async fn templates_reject_unknown_area() {
    let pool = common::create_test_pool_shared().await;
    for res in [
        numeric_table_template(st(&pool), Path(9999)).await,
        category_map_template(st(&pool), Path(9999)).await,
        base_data_template(st(&pool), Path(9999), enrolled_q()).await,
    ] {
        let err = res.unwrap_err();
        assert_eq!(err.0, StatusCode::NOT_FOUND, "없는 전형요소는 404여야 함: {}", err.1);
    }
}

#[tokio::test]
async fn base_data_template_rejects_bad_student_type() {
    let pool = common::create_test_pool_shared().await;
    let aid = new_area(
        &pool,
        "(name, max_score, calc_type, match_mode, lookup_scope) \
         VALUES ('봉사', 10000000, 'NUMERIC', 'UPPER', 'SIMPLE')",
    )
    .await;
    let err = base_data_template(
        st(&pool),
        Path(aid),
        Query(StudentTypeQuery { student_type: "everyone".into() }),
    )
    .await
    .unwrap_err();
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
}

// ── 4. 내장 점수 기준 샘플 파일 ──────────────────────────────────

/// 프로그램에 내장돼 배포되는 점수 기준 샘플이 실제 import를 통과하는지.
/// 샘플이 통과하지 못하면 관리자는 받은 예시를 그대로 쓸 수 없다.
async fn sample_bytes(name: &str) -> Vec<u8> {
    let resp = score_template(Path(name.to_string())).await.unwrap();
    assert!(
        content_disposition(&resp).contains(name),
        "파일명에 템플릿 이름이 들어가야 함"
    );
    response_bytes(resp).await
}

#[tokio::test]
async fn numeric_score_samples_import_into_matching_area() {
    // (샘플명, match_mode) — 기준값이 커질수록 점수가 오르면 UPPER, 내리면 LOWER
    for (name, mode, scope) in [
        ("grade", "LOWER", "COMPOSITE"),
        ("attendance", "LOWER", "SIMPLE"),
        ("volunteer", "UPPER", "SIMPLE"),
    ] {
        let pool = common::create_test_pool_shared().await;
        let aid = new_area(
            &pool,
            &format!(
                "(name, max_score, calc_type, match_mode, lookup_scope) \
                 VALUES ('{}', 10000000, 'NUMERIC', '{}', '{}')",
                name, mode, scope
            ),
        )
        .await;
        let bytes = sample_bytes(name).await;
        let (_, data) = split_template(&bytes);
        assert!(!data.is_empty(), "{} 샘플에 데이터 행이 없습니다", name);

        let (status, axum::Json(result)) =
            numeric_table_import(st(&pool), Path(aid), xlsx_multipart(&bytes).await)
                .await
                .unwrap();
        assert_eq!(
            status,
            StatusCode::OK,
            "내장 샘플 '{}'을(를) 그대로 올렸는데 거부됨: {:?}",
            name,
            result.errors
        );
        assert_eq!(result.rows, data.len(), "{} 샘플 행이 전부 등록돼야 함", name);
    }
}

#[tokio::test]
async fn category_score_samples_import_into_matching_area() {
    for name in ["award", "extracurricular", "penalty"] {
        let pool = common::create_test_pool_shared().await;
        // penalty는 음수 점수(감점)라 만점 0인 순수 감점 전형요소로 등록된다
        let aid = new_area(
            &pool,
            &format!(
                "(name, max_score, calc_type, category_agg, lookup_scope) \
                 VALUES ('{}', 10000000, 'CATEGORY', 'MAX', 'SIMPLE')",
                name
            ),
        )
        .await;
        let bytes = sample_bytes(name).await;
        let (_, data) = split_template(&bytes);
        assert!(!data.is_empty(), "{} 샘플에 데이터 행이 없습니다", name);

        let (status, axum::Json(result)) =
            category_map_import(st(&pool), Path(aid), xlsx_multipart(&bytes).await)
                .await
                .unwrap();
        assert_eq!(
            status,
            StatusCode::OK,
            "내장 샘플 '{}'을(를) 그대로 올렸는데 거부됨: {:?}",
            name,
            result.errors
        );
        assert_eq!(result.rows, data.len(), "{} 샘플 행이 전부 등록돼야 함", name);
    }
}

#[tokio::test]
async fn score_template_unknown_name_is_404() {
    let err = score_template(Path("nonexistent".to_string())).await.unwrap_err();
    assert_eq!(err.0, StatusCode::NOT_FOUND);
    // 경로 조작 시도도 목록 검사에서 먼저 막혀야 한다 (임베드 조회까지 가면 안 됨)
    let err = score_template(Path("../Cargo.toml".to_string())).await.unwrap_err();
    assert_eq!(err.0, StatusCode::NOT_FOUND);
    assert!(err.1.contains("존재하지 않는"), "메시지: {}", err.1);
}

// ── 5. 다운로드 응답 형식 ────────────────────────────────────────

#[tokio::test]
async fn templates_are_served_as_xlsx_attachments() {
    let pool = common::create_test_pool_shared().await;
    let aid = new_area(
        &pool,
        "(name, max_score, calc_type, match_mode, lookup_scope) \
         VALUES ('봉사', 10000000, 'NUMERIC', 'UPPER', 'SIMPLE')",
    )
    .await;

    let responses = vec![
        download_template().await.unwrap(),
        enrolled_template().await.unwrap(),
        graduated_template().await.unwrap(),
        classes_template().await.unwrap(),
        numeric_table_template(st(&pool), Path(aid)).await.unwrap(),
        base_data_template(st(&pool), Path(aid), enrolled_q()).await.unwrap(),
        score_template(Path("grade".to_string())).await.unwrap(),
    ];

    for resp in responses {
        let ct = resp
            .headers()
            .get(header::CONTENT_TYPE)
            .expect("Content-Type 없음")
            .to_str()
            .unwrap()
            .to_string();
        let cd = content_disposition(&resp);
        assert!(
            ct.contains("spreadsheetml.sheet"),
            "xlsx MIME이 아니면 브라우저가 파일을 잘못 연다: {}",
            ct
        );
        assert!(cd.starts_with("attachment;"), "다운로드가 아니라 인라인으로 열린다: {}", cd);
        assert!(cd.contains(".xlsx"), "파일명 확장자 누락: {}", cd);
        let bytes = response_bytes(resp).await;
        assert!(excel::is_xlsx(&bytes), "본문이 실제 xlsx가 아님");
    }
}
