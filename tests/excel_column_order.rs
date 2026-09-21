//! 규칙 5 — **엑셀 파싱은 헤더 이름 기반**. 열 인덱스 직접 참조 금지.
//!
//! 지금까지 이 규칙을 지킨 것은 코드 관례뿐이었다. 모든 테스트 픽스처가 템플릿과
//! **같은 열 순서**를 쓰고 있었기 때문에, `get_col(cols, &col, "이름")` 을 `cols[1]` 로
//! 바꿔도 전 스위트가 초록이었다(2026-09-21 변이 검사: 7경로 중 6경로 미검출).
//! 즉 규칙 5 에는 기계 방어선이 거의 없었다 — 열 순서가 다른 파일이 실제로 들어오는
//! 날에야 현장에서 드러났을 것이다.
//!
//! 이 파일은 **모든 import 경로**에 열 순서를 바꾼 입력을 먹인 뒤, 값이 제자리에
//! 들어갔는지 본다. "오류가 안 났다"로는 부족하다 — 값이 엉뚱한 필드로 들어가도
//! 오류는 안 나기 때문이다.
//!
//! ## 픽스처는 **회전**시킨다 (뒤집기가 아니라)
//!
//! 처음에는 헤더를 적당히 뒤섞었는데, 그러면 일부 열이 **우연히 정규 위치에 그대로**
//! 남는다. 실제로 졸업생 픽스처에서 `이름` 이 정규 인덱스(1)에 그대로 있어, 위치로
//! 읽는 변이를 **못 잡았다** — 테스트가 있는데 판별력이 0인 상태였다.
//! 회전(rotate)은 고정점이 없으므로 모든 필수 열이 반드시 자리를 옮긴다.
//! 아래 `모든 픽스처가 필수 열을 정규 위치에서 옮긴다` 테스트가 이 성질을 기계로 지킨다.

mod common;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
};
use principal_candidate_manager::enums::{CalcType, CategoryAgg, MatchMode};
use principal_candidate_manager::handlers::area_data::{
    base_data_import, category_map_import, numeric_table_import, StudentTypeQuery,
};
use principal_candidate_manager::handlers::classes::import_classes;
use principal_candidate_manager::handlers::external_import::{map_daegyo_rows, map_univ_rows};
use principal_candidate_manager::handlers::students::{
    import_enrolled, import_graduated, import_students,
};
use principal_candidate_manager::handlers::universities::settings_import;

// ── 정규(템플릿) 순서와 이 파일이 쓰는 순서 ──────────────────────
// 각 쌍은 아래 메타 테스트가 "고정점 없음"을 확인한다.

const CANON_CLASSES: &[&str] = &["학년", "반", "담임명", "비밀번호"];
const FIX_CLASSES: &[&str] = &["반", "담임명", "비밀번호", "학년", "비고"];

const CANON_STUDENTS: &[&str] =
    &["학생코드", "이름", "재학여부", "학년", "반", "번호", "졸업연도"];
const FIX_STUDENTS: &[&str] =
    &["학년", "반", "번호", "졸업연도", "학생코드", "이름", "재학여부"];

const CANON_ENROLLED: &[&str] = &["학년", "반", "번호", "이름"];
const FIX_ENROLLED: &[&str] = &["반", "번호", "이름", "학년", "비고"];

const CANON_GRADUATED: &[&str] = &["학생코드", "이름", "졸업연도"];
const FIX_GRADUATED: &[&str] = &["이름", "졸업연도", "학생코드"];

const CANON_NUMERIC: &[&str] = &["기준값", "점수"];
const FIX_NUMERIC: &[&str] = &["점수", "기준값"];

const CANON_CATEGORY: &[&str] = &["범주", "점수"];
const FIX_CATEGORY: &[&str] = &["점수", "범주"];

const CANON_BASE_ENROLLED: &[&str] = &["학년", "반", "번호", "이름", "값"];
const FIX_BASE_ENROLLED: &[&str] = &["값", "학년", "반", "번호", "이름"];

const CANON_BASE_GRADUATED: &[&str] = &["학생코드", "이름", "값"];
const FIX_BASE_GRADUATED: &[&str] = &["이름", "값", "학생코드"];

const CANON_SETTINGS: &[&str] = &[
    "대학명", "대학 정원", "대학 재학생우선",
    "모집단위명", "모집단위 정원", "모집단위 재학생우선",
];
const FIX_SETTINGS: &[&str] = &[
    "모집단위명", "모집단위 정원", "모집단위 재학생우선",
    "대학명", "대학 정원", "대학 재학생우선",
];

const CANON_DAEGYO: &[&str] =
    &["학년", "반", "번호", "이름", "일반등급", "내점수(환산)", "내등급(환산)"];
const FIX_DAEGYO: &[&str] = &[
    "석차", "내등급(환산)", "내점수(환산)", "일반등급", "일반점수", "이름", "번호", "반", "학년",
];
const FIX_DAEGYO_FALLBACK: &[&str] =
    &["내등급(환산)", "일반등급", "이름", "내점수(환산)", "번호", "반", "학년"];

const CANON_UNIV: &[&str] = &["학년", "반", "번호", "이름", "등급"];
const FIX_UNIV: &[&str] = &["반", "번호", "이름", "등급", "학년", "비고"];

/// 픽스처가 **실제로 판별력을 갖는지** 확인한다.
///
/// 필수 열이 정규 인덱스에 그대로 남아 있으면, 코드가 위치로 읽도록 망가져도 같은 값이
/// 나와 테스트가 조용히 통과한다. 그 상태로 커밋하면 "테스트를 넣었다"는 기록만 남고
/// 방어선은 없다 — 이 저장소가 반복해서 배운 실패다.
#[test]
fn every_fixture_moves_required_columns_off_their_canonical_position() {
    let pairs: &[(&str, &[&str], &[&str])] = &[
        ("classes", CANON_CLASSES, FIX_CLASSES),
        ("students 통합", CANON_STUDENTS, FIX_STUDENTS),
        ("students 재학생", CANON_ENROLLED, FIX_ENROLLED),
        ("students 졸업생", CANON_GRADUATED, FIX_GRADUATED),
        ("numeric_table", CANON_NUMERIC, FIX_NUMERIC),
        ("category_map", CANON_CATEGORY, FIX_CATEGORY),
        ("base_data 재학생", CANON_BASE_ENROLLED, FIX_BASE_ENROLLED),
        ("base_data 졸업생", CANON_BASE_GRADUATED, FIX_BASE_GRADUATED),
        ("universities 설정", CANON_SETTINGS, FIX_SETTINGS),
        ("대교협", CANON_DAEGYO, FIX_DAEGYO),
        ("대교협(미제공)", CANON_DAEGYO, FIX_DAEGYO_FALLBACK),
        ("유니브", CANON_UNIV, FIX_UNIV),
    ];

    let mut stuck: Vec<String> = Vec::new();
    for (label, canon, fixture) in pairs {
        for (canon_idx, name) in canon.iter().enumerate() {
            let fix_idx = fixture
                .iter()
                .position(|f| f == name)
                .unwrap_or_else(|| panic!("{label} 픽스처에 필수 열 '{name}' 이 없다"));
            if fix_idx == canon_idx {
                stuck.push(format!("{label}: '{name}' 이 정규 위치({canon_idx})에 그대로"));
            }
        }
    }

    assert!(
        stuck.is_empty(),
        "픽스처가 판별력을 잃었다 — 아래 열은 위치로 읽어도 같은 값이 나온다:\n  {}\n\
         헤더를 **회전**시켜라(뒤섞기는 고정점이 남는다).",
        stuck.join("\n  ")
    );
}

// ── 헬퍼 ─────────────────────────────────────────────────────────

fn csv_line(cells: &[&str]) -> String {
    cells.join(",")
}

fn csv_body(header: &[&str], rows: &[&[&str]]) -> String {
    let mut out = csv_line(header);
    for r in rows {
        out.push('\n');
        out.push_str(&csv_line(r));
    }
    out.push('\n');
    out
}

async fn insert_area(
    pool: &sqlx::SqlitePool,
    calc_type: CalcType,
    match_mode: Option<MatchMode>,
    category_agg: Option<CategoryAgg>,
) -> i64 {
    sqlx::query(
        "INSERT INTO areas (name, max_score, calc_type, match_mode, category_agg, lookup_scope, multi_value) \
         VALUES ('요소', 10000000, ?, ?, ?, 'SIMPLE', 0)",
    )
    .bind(calc_type)
    .bind(match_mode)
    .bind(category_agg)
    .execute(pool)
    .await
    .unwrap()
    .last_insert_rowid()
}

fn row(cells: &[&str]) -> Vec<String> {
    cells.iter().map(|s| s.to_string()).collect()
}

// ── classes ──────────────────────────────────────────────────────

#[tokio::test]
async fn import_classes_reads_by_header_name() {
    let pool = common::create_test_pool().await;
    let state = common::make_state(pool.clone());

    let body = csv_body(FIX_CLASSES, &[&["7", "홍길동", "pass1234", "2", "무시됨"]]);
    let (status, _) = import_classes(State(state), common::csv_multipart(&body).await)
        .await
        .unwrap();
    assert_eq!(status, StatusCode::OK, "열 순서가 달라도 받아야 한다");

    let (g, c, t): (i64, i64, String) =
        sqlx::query_as("SELECT grade, class_no, teacher_name FROM classes")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((g, c, t.as_str()), (2, 7, "홍길동"), "열 위치로 읽고 있다");
}

// ── students (통합 / 재학생 / 졸업생) ────────────────────────────

#[tokio::test]
async fn import_students_reads_by_header_name() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 2, 7).await;
    let state = common::make_state(pool.clone());

    let body = csv_body(FIX_STUDENTS, &[&["2", "7", "3", "", "S001", "홍길동", "재학"]]);
    let (status, _) = import_students(State(state), common::csv_multipart(&body).await)
        .await
        .unwrap();
    assert_eq!(status, StatusCode::OK);

    let (code, name, g, c, s): (String, String, i64, i64, i64) = sqlx::query_as(
        "SELECT student_code, name, grade, class_no, seq_no FROM students",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        (code.as_str(), name.as_str(), g, c, s),
        ("S001", "홍길동", 2, 7, 3),
        "열 위치로 읽고 있다 — 이름·학생코드가 바뀌어 들어갔을 수 있다"
    );
}

#[tokio::test]
async fn import_enrolled_reads_by_header_name() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 2, 7).await;
    let state = common::make_state(pool.clone());

    let body = csv_body(FIX_ENROLLED, &[&["7", "3", "홍길동", "2", "무시됨"]]);
    let (status, _) = import_enrolled(State(state), common::csv_multipart(&body).await)
        .await
        .unwrap();
    assert_eq!(status, StatusCode::OK);

    let (name, g, c, s): (String, i64, i64, i64) =
        sqlx::query_as("SELECT name, grade, class_no, seq_no FROM students")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((name.as_str(), g, c, s), ("홍길동", 2, 7, 3));
}

#[tokio::test]
async fn import_graduated_reads_by_header_name() {
    let pool = common::create_test_pool().await;
    let state = common::make_state(pool.clone());

    let body = csv_body(FIX_GRADUATED, &[&["홍길동", "2024", "G001"]]);
    let (status, _) = import_graduated(State(state), common::csv_multipart(&body).await)
        .await
        .unwrap();
    assert_eq!(status, StatusCode::OK);

    let (code, name, year): (String, String, i64) =
        sqlx::query_as("SELECT student_code, name, grad_year FROM students")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        (code.as_str(), name.as_str(), year),
        ("G001", "홍길동", 2024),
        "학생코드와 이름이 뒤바뀌어 들어갔다면 열 위치로 읽은 것이다"
    );
}

// ── area_data (기준표 / 범주표 / 기초데이터) ─────────────────────

#[tokio::test]
async fn numeric_table_import_reads_by_header_name() {
    let pool = common::create_test_pool().await;
    let aid = insert_area(&pool, CalcType::Numeric, Some(MatchMode::Upper), None).await;
    let state = common::make_state(pool.clone());

    // 기준값과 점수가 뒤바뀌어도 둘 다 숫자라 **파싱은 통과한다** — 조용히 틀린
    // 점수표가 저장된다. 그래서 이 경로는 값 위치를 반드시 단언해야 한다.
    let body = csv_body(FIX_NUMERIC, &[&["8.5", "3"]]);
    let (status, axum::Json(result)) =
        numeric_table_import(State(state), Path(aid), common::csv_multipart(&body).await)
            .await
            .unwrap();
    assert_eq!(status, StatusCode::OK, "errors: {:?}", result.errors);

    let (th, sc): (i64, i64) =
        sqlx::query_as("SELECT threshold, score FROM numeric_table WHERE area_id = ?")
            .bind(aid)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        (th, sc),
        (300000, 850000),
        "기준값과 점수가 뒤바뀌었다 — 열 위치로 읽고 있다"
    );
}

#[tokio::test]
async fn category_map_import_reads_by_header_name() {
    let pool = common::create_test_pool().await;
    let aid = insert_area(&pool, CalcType::Category, None, Some(CategoryAgg::Sum)).await;
    let state = common::make_state(pool.clone());

    // CATEGORY 는 "가장 낮은 점수를 0점으로" 기준 행을 요구한다 — 함께 넣는다.
    let body = csv_body(FIX_CATEGORY, &[&["0", "해당없음"], &["7.25", "반장"]]);
    let (status, axum::Json(result)) =
        category_map_import(State(state), Path(aid), common::csv_multipart(&body).await)
            .await
            .unwrap();
    assert_eq!(status, StatusCode::OK, "errors: {:?}", result.errors);

    let sc: i64 = sqlx::query_scalar(
        "SELECT score FROM category_map WHERE area_id = ? AND category = '반장'",
    )
    .bind(aid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(sc, 725000);
}

#[tokio::test]
async fn base_data_import_graduated_reads_by_header_name() {
    let pool = common::create_test_pool().await;
    let aid = insert_area(&pool, CalcType::Manual, None, None).await;
    sqlx::query(
        "INSERT INTO students (student_code, name, is_enrolled, grad_year) \
         VALUES ('G001', '홍길동', 0, 2024)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let state = common::make_state(pool.clone());

    let body = csv_body(FIX_BASE_GRADUATED, &[&["홍길동", "4.5", "G001"]]);
    let q = Query(StudentTypeQuery { student_type: "graduated".to_string() });
    let (status, axum::Json(result)) =
        base_data_import(State(state), Path(aid), q, common::csv_multipart(&body).await)
            .await
            .unwrap();
    assert_eq!(status, StatusCode::OK, "errors: {:?}", result.errors);

    let value: String = sqlx::query_scalar("SELECT value FROM base_data WHERE area_id = ?")
        .bind(aid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(value, "450000", "값 열이 아니라 다른 열을 읽었다");
}

#[tokio::test]
async fn base_data_import_enrolled_reads_by_header_name() {
    let pool = common::create_test_pool().await;
    let aid = insert_area(&pool, CalcType::Manual, None, None).await;
    common::insert_class(&pool, 2, 7).await;
    sqlx::query(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES ('S001', '홍길동', 2, 7, 3, 1)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let state = common::make_state(pool.clone());

    let body = csv_body(FIX_BASE_ENROLLED, &[&["4.5", "2", "7", "3", "홍길동"]]);
    let q = Query(StudentTypeQuery { student_type: "enrolled".to_string() });
    let (status, axum::Json(result)) =
        base_data_import(State(state), Path(aid), q, common::csv_multipart(&body).await)
            .await
            .unwrap();
    assert_eq!(status, StatusCode::OK, "errors: {:?}", result.errors);

    let value: String = sqlx::query_scalar("SELECT value FROM base_data WHERE area_id = ?")
        .bind(aid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(value, "450000");
}

// ── universities 설정 ────────────────────────────────────────────

#[tokio::test]
async fn settings_import_reads_by_header_name() {
    let pool = common::create_test_pool().await;
    let state = common::make_state(pool.clone());

    // 재학생우선은 대학과 모집단위가 어긋나면 별도 업무 규칙에 걸린다
    // ("재학생 우선 대학의 모집단위는 모두 '예'"). 그래서 구분은 **정원**으로 한다 —
    // 위치로 읽으면 대학 5 / 모집단위 2 가 서로 뒤바뀐다.
    let body = csv_body(
        FIX_SETTINGS,
        &[&["컴퓨터공학부", "2", "아니오", "가대학", "5", "아니오"]],
    );
    let (status, axum::Json(result)) =
        settings_import(State(state), common::csv_multipart(&body).await)
            .await
            .unwrap();
    assert_eq!(status, StatusCode::OK, "결과: {result}");

    let (uname, uquota, uprio): (String, i64, i64) =
        sqlx::query_as("SELECT univ_name, total_quota, prioritize_enrolled FROM universities")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((uname.as_str(), uquota, uprio), ("가대학", 5, 0));

    let (tname, tquota): (String, i64) =
        sqlx::query_as("SELECT track_name, unit_quota FROM univ_tracks")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        (tname.as_str(), tquota),
        ("컴퓨터공학부", 2),
        "대학 열과 모집단위 열이 섞여 들어갔다 — 열 위치로 읽고 있다"
    );
}

// ── 외부 양식(대교협 / 유니브) ───────────────────────────────────
//
// 이 둘은 `parse_file_rows_with_headers` 가 아니라 원시 행을 직접 받는다. 그래서
// 매핑 함수를 바이트 디코딩에서 떼어 냈다 — 유니브 양식은 .xls(OLE2 컨테이너)라
// 테스트에서 파일을 만들 수 없고, 그 탓에 이 경로는 통째로 검사 밖이었다
// (기존 테스트는 "비 .xls 를 거부한다" 하나뿐).

#[tokio::test]
async fn map_daegyo_rows_reads_by_header_name() {
    // 1행: 제목, 2행: 헤더, 3행~: 데이터. 실제 파일에 있는 미사용 열(일반점수·석차)도 넣는다.
    let rows = vec![
        row(&["서울-테스트대(본교)-학교장추천-2026"]),
        row(FIX_DAEGYO),
        row(&["12", "1.5", "915.0", "2.0", "912.5", "홍길동", "7", "3", "2"]),
    ];

    let parsed = map_daegyo_rows(&rows).expect("열 순서가 달라도 파싱돼야 한다");

    assert_eq!(parsed.univ_name, "테스트대(본교)");
    assert_eq!(parsed.records.len(), 1);
    let (line, cells) = &parsed.records[0];
    assert_eq!(*line, 3, "엑셀 행 번호는 1-based 원본 기준");
    assert_eq!(
        cells,
        &vec!["2".to_string(), "3".to_string(), "7".to_string(),
              "홍길동".to_string(), "1.5".to_string()],
        "[학년, 반, 번호, 이름, 값] 순서로 들어가야 한다 — 열 위치로 읽고 있다"
    );
}

#[tokio::test]
async fn map_daegyo_rows_uses_plain_grade_when_converted_score_is_missing() {
    // `내점수(환산)`이 "미제공"이면 `일반등급`을 쓴다. 열 순서를 바꿔도 그 선택이 유지돼야 한다.
    let rows = vec![
        row(&["서울-테스트대-학교장추천-2026"]),
        row(FIX_DAEGYO_FALLBACK),
        row(&["9.9", "2.0", "홍길동", "미제공", "7", "3", "2"]),
    ];

    let parsed = map_daegyo_rows(&rows).expect("파싱 실패");
    assert_eq!(
        parsed.records[0].1[4], "2.0",
        "미제공이면 일반등급(2.0)이어야 한다 — 9.9 면 환산등급을 잘못 집은 것이다"
    );
}

#[tokio::test]
async fn map_univ_rows_reads_by_header_name() {
    // 1행 B열: 대학명, 6행(index 5): 헤더, 7행~: 데이터.
    let rows = vec![
        row(&["대학", "테스트대"]),
        row(&["학과", "컴퓨터공학부"]),
        row(&["전형", "학교장추천"]),
        row(&[]),
        row(&[]),
        row(FIX_UNIV),
        row(&["3", "7", "홍길동", "1.5", "2", "무시됨"]),
    ];

    let parsed = map_univ_rows(&rows).expect("열 순서가 달라도 파싱돼야 한다");

    assert_eq!(parsed.univ_name, "테스트대");
    assert_eq!(parsed.records.len(), 1);
    let (line, cells) = &parsed.records[0];
    assert_eq!(*line, 7);
    assert_eq!(
        cells,
        &vec!["2".to_string(), "3".to_string(), "7".to_string(),
              "홍길동".to_string(), "1.5".to_string()],
        "[학년, 반, 번호, 이름, 값] 순서로 들어가야 한다"
    );
}

#[tokio::test]
async fn map_univ_rows_rejects_missing_required_column() {
    // 헤더 이름 기반이라는 것은 **없으면 오류**라는 뜻이기도 하다.
    let rows = vec![
        row(&["대학", "테스트대"]), row(&[]), row(&[]), row(&[]), row(&[]),
        row(&["등급", "번호", "반", "학년"]),   // 이름 열 없음
        row(&["1.5", "7", "3", "2"]),
    ];

    let err = map_univ_rows(&rows).expect_err("필수 열이 없으면 거부해야 한다");
    assert!(err.contains("이름"), "어느 열이 없는지 알려야 한다: {err}");
}
