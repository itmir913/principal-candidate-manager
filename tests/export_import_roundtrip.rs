//! Export → Import 왕복 검증.
//!
//! 사용자의 실제 워크플로는 "내보내기 받아서 수정 후 다시 업로드"다.
//! export가 쓰는 헤더와 import의 require_cols가 어긋나면 정상 절차가 거부되므로,
//! 내보낸 xlsx를 그대로 재import해 성공 + 데이터 동일함을 고정한다.
//! 손상된 xlsx가 panic 없이 400으로 거부되는 것도 함께 검증한다.

mod common;

use axum::{
    body::Body,
    extract::{FromRequest, Multipart, Path, Query, State},
    http::{Request, StatusCode},
};
use principal_candidate_manager::handlers::{
    area_data::{
        base_data_export, base_data_import, category_map_export, category_map_import,
        numeric_table_export, numeric_table_import, StudentTypeQuery,
    },
    classes::{export_classes, import_classes},
    students::{
        export_enrolled, export_graduated, export_students, import_enrolled, import_graduated,
        import_students,
    },
};
use sqlx::SqlitePool;

// ── 헬퍼 ─────────────────────────────────────────────────────────

/// xlsx 바이트를 multipart file 필드로 감싼다
async fn xlsx_multipart(bytes: &[u8]) -> Multipart {
    let boundary = "boundary42";
    let mut body: Vec<u8> = Vec::new();
    body.extend_from_slice(
        format!(
            "--{boundary}\r\n\
             Content-Disposition: form-data; name=\"file\"; filename=\"export.xlsx\"\r\n\
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
    axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec()
}

fn st(pool: &SqlitePool) -> State<principal_candidate_manager::state::AppState> {
    State(common::make_state(pool.clone()))
}

fn enrolled_q() -> Query<StudentTypeQuery> {
    Query(StudentTypeQuery { student_type: "enrolled".into() })
}

fn graduated_q() -> Query<StudentTypeQuery> {
    Query(StudentTypeQuery { student_type: "graduated".into() })
}

// ── 점수 기준 (numeric_table / category_map) ─────────────────────

#[tokio::test]
async fn numeric_table_simple_roundtrip() {
    let pool = common::create_test_pool_shared().await;
    let aid: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, match_mode, lookup_scope) \
         VALUES ('내신', 10000000, 'NUMERIC', 'UPPER', 'SIMPLE') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // 소수·음수 포함 기준표 등록
    let csv = "기준값,점수\n-0.5,0\n3.5,80\n4,100\n";
    let (status, _) = numeric_table_import(st(&pool), Path(aid), common::csv_multipart(csv).await)
        .await
        .unwrap();
    assert_eq!(status, StatusCode::OK);

    let before: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT threshold, score FROM numeric_table WHERE area_id = ? ORDER BY threshold",
    )
    .bind(aid)
    .fetch_all(&pool)
    .await
    .unwrap();

    // export → 그대로 재import
    let resp = numeric_table_export(st(&pool), Path(aid)).await.unwrap();
    let xlsx = response_bytes(resp).await;
    let (status, axum::Json(result)) =
        numeric_table_import(st(&pool), Path(aid), xlsx_multipart(&xlsx).await)
            .await
            .unwrap();
    assert_eq!(status, StatusCode::OK, "export 산출물 재import 실패: {:?}", result.errors);

    let after: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT threshold, score FROM numeric_table WHERE area_id = ? ORDER BY threshold",
    )
    .bind(aid)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(before, after, "왕복 후 기준표가 동일해야 함");
}

#[tokio::test]
async fn numeric_table_composite_roundtrip() {
    // COMPOSITE: 대학명·모집단위명 열 포함 왕복 — resolve_track이 기존 트랙을 재사용해야 함
    let pool = common::create_test_pool_shared().await;
    let aid: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, match_mode, lookup_scope) \
         VALUES ('환산내신', 10000000, 'NUMERIC', 'UPPER', 'COMPOSITE') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let csv = "기준값,점수,대학명,모집단위명\n0,0,,\n3,80,,\n0,0,한국대,컴공\n4,90,한국대,컴공\n";
    let (status, _) = numeric_table_import(st(&pool), Path(aid), common::csv_multipart(csv).await)
        .await
        .unwrap();
    assert_eq!(status, StatusCode::OK);

    let count_before: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM numeric_table WHERE area_id = ?")
            .bind(aid)
            .fetch_one(&pool)
            .await
            .unwrap();
    let univ_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM universities")
        .fetch_one(&pool)
        .await
        .unwrap();

    let resp = numeric_table_export(st(&pool), Path(aid)).await.unwrap();
    let xlsx = response_bytes(resp).await;
    let (status, axum::Json(result)) =
        numeric_table_import(st(&pool), Path(aid), xlsx_multipart(&xlsx).await)
            .await
            .unwrap();
    assert_eq!(status, StatusCode::OK, "재import 실패: {:?}", result.errors);
    assert!(result.warnings.is_empty(), "기존 트랙 재사용 — 자동 추가 경고가 없어야 함: {:?}", result.warnings);

    let count_after: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM numeric_table WHERE area_id = ?")
            .bind(aid)
            .fetch_one(&pool)
            .await
            .unwrap();
    let univ_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM universities")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count_before, count_after);
    assert_eq!(univ_before, univ_after, "대학이 중복 생성되면 안 됨");
}

#[tokio::test]
async fn category_map_roundtrip() {
    let pool = common::create_test_pool_shared().await;
    let aid: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, category_agg, lookup_scope, multi_value) \
         VALUES ('활동', 10000000, 'CATEGORY', 'MAX', 'SIMPLE', 0) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let csv = "범주,점수\n해당없음,0\n수상,5.5\n임원,10\n";
    let (status, _) = category_map_import(st(&pool), Path(aid), common::csv_multipart(csv).await)
        .await
        .unwrap();
    assert_eq!(status, StatusCode::OK);

    let before: Vec<(String, i64)> = sqlx::query_as(
        "SELECT category, score FROM category_map WHERE area_id = ? ORDER BY category",
    )
    .bind(aid)
    .fetch_all(&pool)
    .await
    .unwrap();

    let resp = category_map_export(st(&pool), Path(aid)).await.unwrap();
    let xlsx = response_bytes(resp).await;
    let (status, axum::Json(result)) =
        category_map_import(st(&pool), Path(aid), xlsx_multipart(&xlsx).await)
            .await
            .unwrap();
    assert_eq!(status, StatusCode::OK, "재import 실패: {:?}", result.errors);

    let after: Vec<(String, i64)> = sqlx::query_as(
        "SELECT category, score FROM category_map WHERE area_id = ? ORDER BY category",
    )
    .bind(aid)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(before, after, "왕복 후 범주표가 동일해야 함");
}

// ── 학반 / 학생 ──────────────────────────────────────────────────

#[tokio::test]
async fn classes_roundtrip_keeps_names_and_hashes() {
    // export에는 비밀번호가 없다 — 기존 학급 재import는 담임명만 갱신하고 해시는 보존
    let pool = common::create_test_pool_shared().await;
    let csv = "학년,반,담임명,비밀번호\n3,1,담임A,pass1234\n3,2,담임B,pass5678\n";
    let (status, _) = import_classes(st(&pool), common::csv_multipart(csv).await).await.unwrap();
    assert_eq!(status, StatusCode::OK);

    let before: Vec<(i64, i64, String, String)> = sqlx::query_as(
        "SELECT grade, class_no, teacher_name, password_hash FROM classes ORDER BY grade, class_no",
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    let resp = export_classes(st(&pool)).await.unwrap();
    let xlsx = response_bytes(resp).await;
    let (status, axum::Json(result)) =
        import_classes(st(&pool), xlsx_multipart(&xlsx).await).await.unwrap();
    assert_eq!(status, StatusCode::OK, "재import 실패: {:?}", result["errors"]);

    let after: Vec<(i64, i64, String, String)> = sqlx::query_as(
        "SELECT grade, class_no, teacher_name, password_hash FROM classes ORDER BY grade, class_no",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(before, after, "왕복 후 학반(해시 포함)이 동일해야 함");
}

async fn students_snapshot(pool: &SqlitePool) -> Vec<(String, String, Option<i64>, Option<i64>, Option<i64>, i64, Option<i64>)> {
    sqlx::query_as(
        "SELECT student_code, name, grade, class_no, seq_no, is_enrolled, grad_year \
         FROM students ORDER BY student_code",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn students_full_roundtrip() {
    // 전체 학생(재학+졸업 혼합) export → import 왕복
    let pool = common::create_test_pool_shared().await;
    common::insert_class(&pool, 3, 1).await;
    let csv = "학생코드,이름,재학여부,학년,반,번호,졸업연도\n\
               E001,홍길동,재학,3,1,1,\n\
               G001,김졸업,졸업,,,,2024\n";
    let (status, _) = import_students(st(&pool), common::csv_multipart(csv).await).await.unwrap();
    assert_eq!(status, StatusCode::OK);

    let before = students_snapshot(&pool).await;

    let resp = export_students(st(&pool)).await.unwrap();
    let xlsx = response_bytes(resp).await;
    let (status, axum::Json(result)) =
        import_students(st(&pool), xlsx_multipart(&xlsx).await).await.unwrap();
    assert_eq!(status, StatusCode::OK, "재import 실패: {:?}", result.errors);

    assert_eq!(before, students_snapshot(&pool).await, "왕복 후 학생 명단이 동일해야 함");
}

#[tokio::test]
async fn students_enrolled_roundtrip() {
    let pool = common::create_test_pool_shared().await;
    common::insert_class(&pool, 3, 1).await;
    let csv = "이름,학년,반,번호\n홍길동,3,1,1\n이순신,3,1,2\n";
    let (status, _) = import_enrolled(st(&pool), common::csv_multipart(csv).await).await.unwrap();
    assert_eq!(status, StatusCode::OK);

    let before = students_snapshot(&pool).await;

    let resp = export_enrolled(st(&pool)).await.unwrap();
    let xlsx = response_bytes(resp).await;
    let (status, axum::Json(result)) =
        import_enrolled(st(&pool), xlsx_multipart(&xlsx).await).await.unwrap();
    assert_eq!(status, StatusCode::OK, "재import 실패: {:?}", result.errors);

    assert_eq!(before, students_snapshot(&pool).await, "왕복 후 재학생 명단이 동일해야 함");
}

#[tokio::test]
async fn students_graduated_roundtrip() {
    let pool = common::create_test_pool_shared().await;
    let csv = "학생코드,이름,졸업연도\nG001,김졸업,2024\nG002,박졸업,2023\n";
    let (status, _) = import_graduated(st(&pool), common::csv_multipart(csv).await).await.unwrap();
    assert_eq!(status, StatusCode::OK);

    let before = students_snapshot(&pool).await;

    let resp = export_graduated(st(&pool)).await.unwrap();
    let xlsx = response_bytes(resp).await;
    let (status, axum::Json(result)) =
        import_graduated(st(&pool), xlsx_multipart(&xlsx).await).await.unwrap();
    assert_eq!(status, StatusCode::OK, "재import 실패: {:?}", result.errors);

    assert_eq!(before, students_snapshot(&pool).await, "왕복 후 졸업생 명단이 동일해야 함");
}

// ── 기초데이터 ───────────────────────────────────────────────────

#[tokio::test]
async fn base_data_graduated_roundtrip() {
    // 졸업생 전용 DB: export(학생코드/이름/값) 헤더가 graduated import와 호환
    let pool = common::create_test_pool_shared().await;
    sqlx::query(
        "INSERT INTO students (student_code, name, is_enrolled, grad_year) VALUES ('G001', '김졸업', 0, 2024)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let aid: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, match_mode, lookup_scope) \
         VALUES ('내신', 10000000, 'NUMERIC', 'UPPER', 'SIMPLE') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let csv = "학생코드,이름,값\nG001,김졸업,3.75\n";
    let (status, _) =
        base_data_import(st(&pool), Path(aid), graduated_q(), common::csv_multipart(csv).await)
            .await
            .unwrap();
    assert_eq!(status, StatusCode::OK);

    let before: Vec<(i64, String)> = sqlx::query_as(
        "SELECT student_id, value FROM base_data WHERE area_id = ? ORDER BY student_id",
    )
    .bind(aid)
    .fetch_all(&pool)
    .await
    .unwrap();

    let resp = base_data_export(st(&pool), Path(aid), graduated_q()).await.unwrap();
    let xlsx = response_bytes(resp).await;
    let (status, axum::Json(result)) =
        base_data_import(st(&pool), Path(aid), graduated_q(), xlsx_multipart(&xlsx).await)
            .await
            .unwrap();
    assert_eq!(status, StatusCode::OK, "재import 실패: {:?}", result.errors);

    let after: Vec<(i64, String)> = sqlx::query_as(
        "SELECT student_id, value FROM base_data WHERE area_id = ? ORDER BY student_id",
    )
    .bind(aid)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(before, after, "왕복 후 기초데이터가 동일해야 함");
}

/// 재학생 base_data 왕복: export가 import와 동일한 학년/반/번호/이름/값 헤더로
/// 내보내므로 "내려받아 수정 후 재업로드" 흐름이 성립한다.
/// (과거에는 학생코드 헤더로 내보내 재import가 400이었다 — 세션 5 후속에서 대칭화)
#[tokio::test]
async fn base_data_enrolled_roundtrip() {
    let pool = common::create_test_pool_shared().await;
    common::insert_class(&pool, 3, 1).await;
    sqlx::query(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES ('E001', '홍길동', 3, 1, 1, 1)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let aid: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, match_mode, lookup_scope) \
         VALUES ('내신', 10000000, 'NUMERIC', 'UPPER', 'SIMPLE') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let csv = "학년,반,번호,이름,값\n3,1,1,홍길동,4.2\n";
    let (status, _) =
        base_data_import(st(&pool), Path(aid), enrolled_q(), common::csv_multipart(csv).await)
            .await
            .unwrap();
    assert_eq!(status, StatusCode::OK);

    let before: Vec<(i64, String)> = sqlx::query_as(
        "SELECT student_id, value FROM base_data WHERE area_id = ? ORDER BY student_id",
    )
    .bind(aid)
    .fetch_all(&pool)
    .await
    .unwrap();

    let resp = base_data_export(st(&pool), Path(aid), enrolled_q()).await.unwrap();
    let xlsx = response_bytes(resp).await;
    let (status, axum::Json(result)) =
        base_data_import(st(&pool), Path(aid), enrolled_q(), xlsx_multipart(&xlsx).await)
            .await
            .unwrap();
    assert_eq!(status, StatusCode::OK, "재import 실패: {:?}", result.errors);

    let after: Vec<(i64, String)> = sqlx::query_as(
        "SELECT student_id, value FROM base_data WHERE area_id = ? ORDER BY student_id",
    )
    .bind(aid)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(before, after, "왕복 후 기초데이터가 동일해야 함");
}

/// COMPOSITE 재학생 왕복: 모집단위별 행 + 공통 테이블 행(track NULL)이 모두 내보내지고
/// (과거 INNER JOIN은 공통 행을 누락) 기존 트랙 재사용으로 그대로 재import된다.
#[tokio::test]
async fn base_data_enrolled_composite_roundtrip() {
    let pool = common::create_test_pool_shared().await;
    common::insert_class(&pool, 3, 1).await;
    sqlx::query(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES ('E001', '홍길동', 3, 1, 1, 1)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let aid: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, match_mode, lookup_scope) \
         VALUES ('환산내신', 10000000, 'NUMERIC', 'UPPER', 'COMPOSITE') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // 모집단위 행(한국대/컴공 자동 생성) + 공통 행(대학명·모집단위명 공란)
    let csv = "학년,반,번호,이름,값,대학명,모집단위명\n\
               3,1,1,홍길동,4.2,한국대,컴공\n\
               3,1,1,홍길동,3.5,,\n";
    let (status, _) =
        base_data_import(st(&pool), Path(aid), enrolled_q(), common::csv_multipart(csv).await)
            .await
            .unwrap();
    assert_eq!(status, StatusCode::OK);

    let before: Vec<(i64, Option<i64>, String)> = sqlx::query_as(
        "SELECT student_id, track_id, value FROM base_data WHERE area_id = ? \
         ORDER BY COALESCE(track_id, 0), student_id",
    )
    .bind(aid)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(before.len(), 2, "모집단위 행 + 공통 행");

    let resp = base_data_export(st(&pool), Path(aid), enrolled_q()).await.unwrap();
    let xlsx = response_bytes(resp).await;
    let (status, axum::Json(result)) =
        base_data_import(st(&pool), Path(aid), enrolled_q(), xlsx_multipart(&xlsx).await)
            .await
            .unwrap();
    assert_eq!(status, StatusCode::OK, "재import 실패: {:?}", result.errors);
    assert!(result.warnings.is_empty(), "기존 트랙 재사용 — 자동 추가 경고가 없어야 함: {:?}", result.warnings);

    let after: Vec<(i64, Option<i64>, String)> = sqlx::query_as(
        "SELECT student_id, track_id, value FROM base_data WHERE area_id = ? \
         ORDER BY COALESCE(track_id, 0), student_id",
    )
    .bind(aid)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(before, after, "공통 행 포함 왕복 후 기초데이터가 동일해야 함");
}

/// 혼합 DB: export는 student_type별로 분리되어 각자 자기 타입 헤더·행만 담고,
/// 각각 자기 타입으로 재import해도 상대 타입 데이터를 건드리지 않는다.
#[tokio::test]
async fn base_data_export_separates_student_types() {
    let pool = common::create_test_pool_shared().await;
    common::insert_class(&pool, 3, 1).await;
    sqlx::query(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES ('E001', '홍길동', 3, 1, 1, 1)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO students (student_code, name, is_enrolled, grad_year) \
         VALUES ('G001', '김졸업', 0, 2024)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let aid: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, match_mode, lookup_scope) \
         VALUES ('내신', 10000000, 'NUMERIC', 'UPPER', 'SIMPLE') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let (status, _) = base_data_import(
        st(&pool), Path(aid), enrolled_q(),
        common::csv_multipart("학년,반,번호,이름,값\n3,1,1,홍길동,4.2\n").await,
    )
    .await
    .unwrap();
    assert_eq!(status, StatusCode::OK);
    let (status, _) = base_data_import(
        st(&pool), Path(aid), graduated_q(),
        common::csv_multipart("학생코드,이름,값\nG001,김졸업,3.75\n").await,
    )
    .await
    .unwrap();
    assert_eq!(status, StatusCode::OK);

    let snapshot = |pool: SqlitePool, aid: i64| async move {
        sqlx::query_as::<_, (i64, String)>(
            "SELECT student_id, value FROM base_data WHERE area_id = ? ORDER BY student_id",
        )
        .bind(aid)
        .fetch_all(&pool)
        .await
        .unwrap()
    };
    let before = snapshot(pool.clone(), aid).await;
    assert_eq!(before.len(), 2);

    // 재학생 export: 재학생 행 1개만, 재학생 헤더
    let resp = base_data_export(st(&pool), Path(aid), enrolled_q()).await.unwrap();
    let enrolled_xlsx = response_bytes(resp).await;
    let rows = principal_candidate_manager::excel::parse_xlsx_all_rows_raw(&enrolled_xlsx).unwrap();
    assert_eq!(rows.len(), 2, "헤더 + 재학생 1행: {:?}", rows);
    assert!(rows[0].iter().any(|c| c == "학년"), "재학생 헤더: {:?}", rows[0]);
    assert!(rows[1].iter().any(|c| c == "홍길동"));

    // 졸업생 export: 졸업생 행 1개만, 졸업생 헤더
    let resp = base_data_export(st(&pool), Path(aid), graduated_q()).await.unwrap();
    let graduated_xlsx = response_bytes(resp).await;
    let rows = principal_candidate_manager::excel::parse_xlsx_all_rows_raw(&graduated_xlsx).unwrap();
    assert_eq!(rows.len(), 2, "헤더 + 졸업생 1행: {:?}", rows);
    assert!(rows[0].iter().any(|c| c == "학생코드"), "졸업생 헤더: {:?}", rows[0]);
    assert!(rows[1].iter().any(|c| c == "G001"));

    // 각자 재import — 상대 타입 데이터 불변
    let (status, _) =
        base_data_import(st(&pool), Path(aid), enrolled_q(), xlsx_multipart(&enrolled_xlsx).await)
            .await
            .unwrap();
    assert_eq!(status, StatusCode::OK);
    let (status, _) =
        base_data_import(st(&pool), Path(aid), graduated_q(), xlsx_multipart(&graduated_xlsx).await)
            .await
            .unwrap();
    assert_eq!(status, StatusCode::OK);

    assert_eq!(before, snapshot(pool.clone(), aid).await, "타입별 왕복 후 전체 기초데이터 동일");
}

// ── 손상 파일 내성 ───────────────────────────────────────────────

#[tokio::test]
async fn truncated_xlsx_returns_400_without_panic() {
    // PK 매직은 유효하지만 내용이 잘린 xlsx → panic 없이 400
    let pool = common::create_test_pool_shared().await;
    let aid: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, match_mode, lookup_scope) \
         VALUES ('내신', 10000000, 'NUMERIC', 'UPPER', 'SIMPLE') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO numeric_table (area_id, track_id, threshold, score) VALUES (?, NULL, 0, 0)")
        .bind(aid)
        .execute(&pool)
        .await
        .unwrap();

    let resp = numeric_table_export(st(&pool), Path(aid)).await.unwrap();
    let full = response_bytes(resp).await;
    let truncated = &full[..full.len() / 2];

    let res = numeric_table_import(st(&pool), Path(aid), xlsx_multipart(truncated).await).await;
    let Err(err) = res else { panic!("잘린 xlsx는 거부되어야 함") };
    assert_eq!(err.0, StatusCode::BAD_REQUEST);

    // 기존 기준표 보존
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM numeric_table WHERE area_id = ?")
        .bind(aid)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}
