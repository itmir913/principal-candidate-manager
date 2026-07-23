mod common;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use principal_candidate_manager::handlers::students::{
    add_enrolled, add_graduated, delete_student, export_students, find_unique_code, list_students,
    upsert_enrolled_by_position, upsert_student, AddEnrolledBody, AddGraduatedBody, ListQuery,
    StudentRecord,
};

fn enrolled_rec(code: &str, name: &str, g: i64, c: i64, s: i64) -> StudentRecord {
    StudentRecord {
        student_code: code.into(),
        name: name.into(),
        is_enrolled: true,
        grade: Some(g),
        class_no: Some(c),
        seq_no: Some(s),
        grad_year: None,
    }
}

fn graduated_rec(code: &str, name: &str, year: i64) -> StudentRecord {
    StudentRecord {
        student_code: code.into(),
        name: name.into(),
        is_enrolled: false,
        grade: None,
        class_no: None,
        seq_no: None,
        grad_year: Some(year),
    }
}

// ── upsert_student ────────────────────────────────────────────────

#[tokio::test]
async fn upsert_student_empty_code_returns_error() {
    let pool = common::create_test_pool().await;
    let mut tx = pool.begin().await.unwrap();
    let rec = StudentRecord {
        student_code: "".into(),
        name: "홍길동".into(),
        is_enrolled: false,
        grade: None,
        class_no: None,
        seq_no: None,
        grad_year: Some(2024),
    };
    let mut ins = 0;
    let mut upd = 0;
    let res = upsert_student(&mut *tx, &rec, &mut ins, &mut upd).await;
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("student_code"));
}

#[tokio::test]
async fn upsert_student_empty_name_returns_error() {
    let pool = common::create_test_pool().await;
    let mut tx = pool.begin().await.unwrap();
    let rec = StudentRecord {
        student_code: "SC001".into(),
        name: "".into(),
        is_enrolled: false,
        grade: None,
        class_no: None,
        seq_no: None,
        grad_year: Some(2024),
    };
    let mut ins = 0;
    let mut upd = 0;
    let res = upsert_student(&mut *tx, &rec, &mut ins, &mut upd).await;
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("name"));
}

#[tokio::test]
async fn upsert_student_enrolled_missing_position_returns_error() {
    let pool = common::create_test_pool().await;
    let mut tx = pool.begin().await.unwrap();
    let rec = StudentRecord {
        student_code: "SC001".into(),
        name: "홍길동".into(),
        is_enrolled: true,
        grade: None,
        class_no: None,
        seq_no: None,
        grad_year: None,
    };
    let mut ins = 0;
    let mut upd = 0;
    let res = upsert_student(&mut *tx, &rec, &mut ins, &mut upd).await;
    assert!(res.is_err());
}

#[tokio::test]
async fn upsert_student_enrolled_class_not_found_returns_error() {
    let pool = common::create_test_pool().await;
    let mut tx = pool.begin().await.unwrap();
    let rec = enrolled_rec("SC001", "홍길동", 1, 1, 1);
    let mut ins = 0;
    let mut upd = 0;
    let res = upsert_student(&mut *tx, &rec, &mut ins, &mut upd).await;
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("학급 목록에 없습니다"));
}

#[tokio::test]
async fn upsert_student_enrolled_inserts_new_student() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let mut tx = pool.begin().await.unwrap();
    let rec = enrolled_rec("SC001", "홍길동", 1, 1, 1);
    let mut ins = 0;
    let mut upd = 0;
    upsert_student(&mut *tx, &rec, &mut ins, &mut upd).await.unwrap();
    tx.commit().await.unwrap();
    assert_eq!(ins, 1);
    assert_eq!(upd, 0);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM students")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn upsert_student_enrolled_updates_existing() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let mut tx = pool.begin().await.unwrap();
    let rec = enrolled_rec("SC001", "홍길동", 1, 1, 1);
    let mut ins = 0;
    let mut upd = 0;
    upsert_student(&mut *tx, &rec, &mut ins, &mut upd).await.unwrap();
    tx.commit().await.unwrap();
    let mut tx2 = pool.begin().await.unwrap();
    let rec2 = enrolled_rec("SC001", "이순신", 1, 1, 1);
    upsert_student(&mut *tx2, &rec2, &mut ins, &mut upd).await.unwrap();
    tx2.commit().await.unwrap();
    assert_eq!(ins, 1);
    assert_eq!(upd, 1);
    let name: String =
        sqlx::query_scalar("SELECT name FROM students WHERE student_code = 'SC001'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(name, "이순신");
}

#[tokio::test]
async fn upsert_student_graduated_missing_year_returns_error() {
    let pool = common::create_test_pool().await;
    let mut tx = pool.begin().await.unwrap();
    let rec = StudentRecord {
        student_code: "GR001".into(),
        name: "졸업생".into(),
        is_enrolled: false,
        grade: None,
        class_no: None,
        seq_no: None,
        grad_year: None,
    };
    let mut ins = 0;
    let mut upd = 0;
    let res = upsert_student(&mut *tx, &rec, &mut ins, &mut upd).await;
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("grad_year"));
}

#[tokio::test]
async fn upsert_student_graduated_inserts_ok() {
    let pool = common::create_test_pool().await;
    let mut tx = pool.begin().await.unwrap();
    let rec = graduated_rec("GR001", "졸업생", 2024);
    let mut ins = 0;
    let mut upd = 0;
    upsert_student(&mut *tx, &rec, &mut ins, &mut upd).await.unwrap();
    tx.commit().await.unwrap();
    assert_eq!(ins, 1);
}

// ── upsert_enrolled_by_position ───────────────────────────────────

#[tokio::test]
async fn enrolled_by_position_generates_student_code() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let mut tx = pool.begin().await.unwrap();
    let rec = StudentRecord {
        student_code: String::new(),
        name: "홍길동".into(),
        is_enrolled: true,
        grade: Some(1),
        class_no: Some(1),
        seq_no: Some(1),
        grad_year: None,
    };
    let mut ins = 0;
    let mut upd = 0;
    upsert_enrolled_by_position(&mut *tx, &rec, &mut ins, &mut upd).await.unwrap();
    tx.commit().await.unwrap();
    assert_eq!(ins, 1);
    let code: String =
        sqlx::query_scalar("SELECT student_code FROM students WHERE name = '홍길동'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!code.is_empty());
}

#[tokio::test]
async fn enrolled_by_position_updates_existing_by_position() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let mut tx = pool.begin().await.unwrap();
    let rec = StudentRecord {
        student_code: String::new(),
        name: "홍길동".into(),
        is_enrolled: true,
        grade: Some(1),
        class_no: Some(1),
        seq_no: Some(1),
        grad_year: None,
    };
    let mut ins = 0;
    let mut upd = 0;
    upsert_enrolled_by_position(&mut *tx, &rec, &mut ins, &mut upd).await.unwrap();
    let rec2 = StudentRecord { name: "이순신".into(), ..rec };
    upsert_enrolled_by_position(&mut *tx, &rec2, &mut ins, &mut upd).await.unwrap();
    tx.commit().await.unwrap();
    assert_eq!(ins, 1);
    assert_eq!(upd, 1);
    let name: String = sqlx::query_scalar(
        "SELECT name FROM students WHERE grade = 1 AND class_no = 1 AND seq_no = 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(name, "이순신");
}

#[tokio::test]
async fn enrolled_by_position_missing_class_returns_error() {
    let pool = common::create_test_pool().await;
    let mut tx = pool.begin().await.unwrap();
    let rec = StudentRecord {
        student_code: String::new(),
        name: "홍길동".into(),
        is_enrolled: true,
        grade: Some(2),
        class_no: Some(3),
        seq_no: Some(1),
        grad_year: None,
    };
    let mut ins = 0;
    let mut upd = 0;
    let res = upsert_enrolled_by_position(&mut *tx, &rec, &mut ins, &mut upd).await;
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("학급 목록에 없습니다"));
}

// ── find_unique_code ──────────────────────────────────────────────

#[tokio::test]
async fn find_unique_code_returns_base_when_no_collision() {
    let pool = common::create_test_pool().await;
    let mut tx = pool.begin().await.unwrap();
    let code = find_unique_code(&mut *tx, "20251101").await.unwrap();
    assert_eq!(code, "20251101");
}

#[tokio::test]
async fn find_unique_code_returns_suffix_on_collision() {
    let pool = common::create_test_pool().await;
    sqlx::query(
        "INSERT INTO students (student_code, name, is_enrolled, grad_year) \
         VALUES ('20251101', '기존학생', 0, 2024)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let mut tx = pool.begin().await.unwrap();
    let code = find_unique_code(&mut *tx, "20251101").await.unwrap();
    assert_eq!(code, "20251101-2");
}

#[tokio::test]
async fn find_unique_code_increments_suffix_until_free() {
    let pool = common::create_test_pool().await;
    for suffix in &["20251101", "20251101-2"] {
        sqlx::query(
            "INSERT INTO students (student_code, name, is_enrolled, grad_year) \
             VALUES (?, '기존학생', 0, 2024)",
        )
        .bind(suffix)
        .execute(&pool)
        .await
        .unwrap();
    }
    let mut tx = pool.begin().await.unwrap();
    let code = find_unique_code(&mut *tx, "20251101").await.unwrap();
    assert_eq!(code, "20251101-3");
}

// ── add_enrolled ──────────────────────────────────────────────────

#[tokio::test]
async fn add_enrolled_inserts_new_student() {
    let pool = common::create_test_pool_shared().await;
    common::insert_class(&pool, 3, 2).await;
    let body = AddEnrolledBody { name: "홍길동".into(), grade: 3, class_no: 2, seq_no: 15 };
    let (status, Json(res)) = add_enrolled(State(common::make_state(pool.clone())), Json(body))
        .await
        .unwrap();
    assert_eq!(status, StatusCode::OK);
    assert_eq!(res.inserted, 1);
    assert_eq!(res.updated, 0);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM students WHERE is_enrolled=1")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn add_enrolled_updates_existing_by_position() {
    let pool = common::create_test_pool_shared().await;
    common::insert_class(&pool, 3, 2).await;
    let body = AddEnrolledBody { name: "홍길동".into(), grade: 3, class_no: 2, seq_no: 15 };
    let _ = add_enrolled(State(common::make_state(pool.clone())), Json(body)).await.unwrap();
    let body2 = AddEnrolledBody { name: "이순신".into(), grade: 3, class_no: 2, seq_no: 15 };
    let (status, Json(res)) = add_enrolled(State(common::make_state(pool.clone())), Json(body2))
        .await
        .unwrap();
    assert_eq!(status, StatusCode::OK);
    assert_eq!(res.updated, 1);
    let name: String =
        sqlx::query_scalar("SELECT name FROM students WHERE grade=3 AND class_no=2 AND seq_no=15")
            .fetch_one(&pool).await.unwrap();
    assert_eq!(name, "이순신");
}

#[tokio::test]
async fn add_enrolled_empty_name_returns_bad_request() {
    let pool = common::create_test_pool_shared().await;
    let body = AddEnrolledBody { name: "   ".into(), grade: 3, class_no: 2, seq_no: 1 };
    let err = add_enrolled(State(common::make_state(pool)), Json(body)).await.unwrap_err();
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn add_enrolled_missing_class_returns_unprocessable() {
    let pool = common::create_test_pool_shared().await;
    let body = AddEnrolledBody { name: "홍길동".into(), grade: 9, class_no: 9, seq_no: 1 };
    let err = add_enrolled(State(common::make_state(pool)), Json(body)).await.unwrap_err();
    assert_eq!(err.0, StatusCode::UNPROCESSABLE_ENTITY);
}

// ── add_graduated ─────────────────────────────────────────────────

#[tokio::test]
async fn add_graduated_inserts_new_student() {
    let pool = common::create_test_pool_shared().await;
    let body = AddGraduatedBody { student_code: "GR001".into(), name: "김철수".into(), grad_year: 2024 };
    let (status, Json(res)) = add_graduated(State(common::make_state(pool.clone())), Json(body))
        .await
        .unwrap();
    assert_eq!(status, StatusCode::OK);
    assert_eq!(res.inserted, 1);
    assert_eq!(res.updated, 0);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM students WHERE is_enrolled=0")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn add_graduated_updates_existing_by_code() {
    let pool = common::create_test_pool_shared().await;
    let body = AddGraduatedBody { student_code: "GR001".into(), name: "김철수".into(), grad_year: 2024 };
    let _ = add_graduated(State(common::make_state(pool.clone())), Json(body)).await.unwrap();
    let body2 = AddGraduatedBody { student_code: "GR001".into(), name: "박영희".into(), grad_year: 2023 };
    let (status, Json(res)) = add_graduated(State(common::make_state(pool.clone())), Json(body2))
        .await
        .unwrap();
    assert_eq!(status, StatusCode::OK);
    assert_eq!(res.updated, 1);
    let (name, year): (String, i64) =
        sqlx::query_as("SELECT name, grad_year FROM students WHERE student_code='GR001'")
            .fetch_one(&pool).await.unwrap();
    assert_eq!(name, "박영희");
    assert_eq!(year, 2023);
}

#[tokio::test]
async fn add_graduated_empty_code_returns_bad_request() {
    let pool = common::create_test_pool_shared().await;
    let body = AddGraduatedBody { student_code: "  ".into(), name: "김철수".into(), grad_year: 2024 };
    let err = add_graduated(State(common::make_state(pool)), Json(body)).await.unwrap_err();
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn add_graduated_empty_name_returns_bad_request() {
    let pool = common::create_test_pool_shared().await;
    let body = AddGraduatedBody { student_code: "GR001".into(), name: "".into(), grad_year: 2024 };
    let err = add_graduated(State(common::make_state(pool)), Json(body)).await.unwrap_err();
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
}

// ── delete_student ────────────────────────────────────────────────

#[tokio::test]
async fn delete_student_no_refs_ok() {
    let pool = common::create_test_pool().await;
    let sid: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, is_enrolled, grad_year) \
         VALUES ('S001', '홍길동', 0, 2024) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    delete_student(State(common::make_state(pool.clone())), Path(sid))
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM students")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn delete_student_with_base_data_returns_conflict() {
    let pool = common::create_test_pool().await;
    let sid: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, is_enrolled, grad_year) \
         VALUES ('S001', '홍길동', 0, 2024) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let aid: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, lookup_scope) \
         VALUES ('내신', 100000, 'NUMERIC', 'SIMPLE') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO base_data (student_id, area_id, track_id, value) VALUES (?, ?, NULL, '10000')",
    )
    .bind(sid)
    .bind(aid)
    .execute(&pool)
    .await
    .unwrap();
    let res = delete_student(State(common::make_state(pool)), Path(sid)).await;
    assert_eq!(res.unwrap_err().0, StatusCode::CONFLICT);
}

#[tokio::test]
async fn delete_student_with_application_returns_conflict() {
    let pool = common::create_test_pool().await;
    let hash = bcrypt::hash("pass", 4u32).unwrap();
    sqlx::query("INSERT INTO classes (grade, class_no, password_hash) VALUES (1, 1, ?)")
        .bind(&hash)
        .execute(&pool)
        .await
        .unwrap();
    let sid: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES ('S001', '홍길동', 1, 1, 1, 1) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let univ_id: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name) VALUES ('한국대') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name, unit_quota) VALUES (?, '컴공', 5) RETURNING id",
    )
    .bind(univ_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at) VALUES ('OPEN', '2025-01-01') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO applications (student_id, track_id, round_id, abandoned) \
         VALUES (?, ?, ?, 0)",
    )
    .bind(sid)
    .bind(tid)
    .bind(rid)
    .execute(&pool)
    .await
    .unwrap();
    let res = delete_student(State(common::make_state(pool)), Path(sid)).await;
    assert_eq!(res.unwrap_err().0, StatusCode::CONFLICT);
}

// ── 세션 4 감사 후속: idx_students_position (재학생 위치 유일성) ──

#[tokio::test]
async fn db_rejects_duplicate_enrolled_position() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;

    sqlx::query(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled)
         VALUES ('S001', '홍길동', 1, 1, 1, 1)",
    )
    .execute(&pool)
    .await
    .unwrap();

    // 같은 위치의 다른 재학생 — DB 유니크 인덱스가 최후 방어선
    let res = sqlx::query(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled)
         VALUES ('S002', '이순신', 1, 1, 1, 1)",
    )
    .execute(&pool)
    .await;
    assert!(res.is_err(), "동일 위치 재학생 중복 삽입은 거부되어야 함");
    assert!(res.unwrap_err().to_string().contains("UNIQUE"));
}

#[tokio::test]
async fn db_allows_multiple_graduated_students() {
    // 졸업생은 위치가 전부 NULL — 부분 인덱스(is_enrolled=1)에 걸리지 않아야 함
    let pool = common::create_test_pool().await;

    for (code, name) in [("G001", "김철수"), ("G002", "김영희")] {
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

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM students WHERE is_enrolled = 0")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 2);
}

// ── E3: 마감 라운드 결과가 있는 학생의 재학/졸업 구분 변경 차단 ────
//
// results.ranking(대학 순위)은 close_round 시점 저장값이고 모집단위 순위(track_rank)는
// 라이브 계산이다. 재학/졸업 구분은 재학생 우선 트랙의 정렬 키이므로, CLOSED 라운드에
// 결과가 있는 학생의 구분이 바뀌면 두 순위의 기준 시점이 어긋난다.
// (E1의 prioritize 가드와 같은 결함, 다른 입력 경로)

/// 지정 상태의 라운드 + 그 학생의 결과 행 하나를 만든다.
async fn seed_round_result(pool: &sqlx::SqlitePool, student_code: &str, status: &str) -> i64 {
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name) VALUES ('한국대') RETURNING id",
    )
    .fetch_one(pool).await.unwrap();
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name) VALUES (?, '컴공') RETURNING id",
    )
    .bind(uid).fetch_one(pool).await.unwrap();
    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at) VALUES (?, '2025-01-01T00:00:00Z') RETURNING id",
    )
    .bind(status).fetch_one(pool).await.unwrap();
    let sid: i64 = sqlx::query_scalar("SELECT id FROM students WHERE student_code = ?")
        .bind(student_code).fetch_one(pool).await.unwrap();
    sqlx::query(
        "INSERT INTO applications (student_id, track_id, round_id, department_name) \
         VALUES (?, ?, ?, '학과')",
    )
    .bind(sid).bind(tid).bind(rid).execute(pool).await.unwrap();
    sqlx::query(
        "INSERT INTO results \
         (student_id, track_id, round_id, score_detail, total_score, ranking, calculated_at) \
         VALUES (?, ?, ?, '{}', 100000, 1, '2025-01-02T00:00:00Z')",
    )
    .bind(sid).bind(tid).bind(rid).execute(pool).await.unwrap();
    rid
}

/// 재학 → 졸업 전환은 CLOSED 라운드에 결과가 있으면 거부된다.
#[tokio::test]
async fn upsert_student_enrolled_to_graduated_blocked_when_closed_round() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let mut tx = pool.begin().await.unwrap();
    let (mut ins, mut upd) = (0usize, 0usize);
    upsert_student(&mut *tx, &enrolled_rec("S1", "홍길동", 1, 1, 1), &mut ins, &mut upd)
        .await.unwrap();
    tx.commit().await.unwrap();
    seed_round_result(&pool, "S1", "CLOSED").await;

    let mut tx2 = pool.begin().await.unwrap();
    let res = upsert_student(&mut *tx2, &graduated_rec("S1", "홍길동", 2024), &mut ins, &mut upd).await;

    let err = res.expect_err("마감 라운드에 결과가 있으면 구분 변경 거부");
    assert!(err.contains("마감된 라운드"), "사유에 라운드 명시: {}", err);
    assert!(err.contains("졸업"), "바꾸려던 구분 명시: {}", err);
    assert!(err.contains("재오픈"), "탈출구 안내: {}", err);
    drop(tx2);
    let still: i64 = sqlx::query_scalar("SELECT is_enrolled FROM students WHERE student_code = 'S1'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(still, 1, "거부됐으므로 구분은 그대로");
}

/// 졸업 → 재학 전환도 같은 기준으로 거부된다.
#[tokio::test]
async fn upsert_student_graduated_to_enrolled_blocked_when_closed_round() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let mut tx = pool.begin().await.unwrap();
    let (mut ins, mut upd) = (0usize, 0usize);
    upsert_student(&mut *tx, &graduated_rec("S1", "홍길동", 2024), &mut ins, &mut upd)
        .await.unwrap();
    tx.commit().await.unwrap();
    seed_round_result(&pool, "S1", "CLOSED").await;

    let mut tx2 = pool.begin().await.unwrap();
    let res = upsert_student(&mut *tx2, &enrolled_rec("S1", "홍길동", 1, 1, 1), &mut ins, &mut upd).await;

    let err = res.expect_err("마감 라운드에 결과가 있으면 구분 변경 거부");
    assert!(err.contains("재학"), "바꾸려던 구분 명시: {}", err);
}

/// OPEN 라운드는 막지 않는다 — 마감 전이라 저장 순위가 아직 없다.
#[tokio::test]
async fn upsert_student_enrollment_flip_allowed_when_round_open() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let mut tx = pool.begin().await.unwrap();
    let (mut ins, mut upd) = (0usize, 0usize);
    upsert_student(&mut *tx, &enrolled_rec("S1", "홍길동", 1, 1, 1), &mut ins, &mut upd)
        .await.unwrap();
    tx.commit().await.unwrap();
    seed_round_result(&pool, "S1", "OPEN").await;

    let mut tx2 = pool.begin().await.unwrap();
    upsert_student(&mut *tx2, &graduated_rec("S1", "홍길동", 2024), &mut ins, &mut upd)
        .await.expect("OPEN 라운드는 막지 않는다");
    tx2.commit().await.unwrap();
    let now: i64 = sqlx::query_scalar("SELECT is_enrolled FROM students WHERE student_code = 'S1'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(now, 0);
}

/// FINALIZED 라운드도 막지 않는다 — 추천이 끝나 순위가 더 쓰이지 않는다.
#[tokio::test]
async fn upsert_student_enrollment_flip_allowed_when_round_finalized() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let mut tx = pool.begin().await.unwrap();
    let (mut ins, mut upd) = (0usize, 0usize);
    upsert_student(&mut *tx, &enrolled_rec("S1", "홍길동", 1, 1, 1), &mut ins, &mut upd)
        .await.unwrap();
    tx.commit().await.unwrap();
    seed_round_result(&pool, "S1", "FINALIZED").await;

    let mut tx2 = pool.begin().await.unwrap();
    upsert_student(&mut *tx2, &graduated_rec("S1", "홍길동", 2024), &mut ins, &mut upd)
        .await.expect("FINALIZED 라운드는 막지 않는다");
}

/// CLOSED 라운드가 있어도 **그 학생의 결과가 없으면** 막지 않는다.
#[tokio::test]
async fn upsert_student_enrollment_flip_allowed_when_student_has_no_result() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let mut tx = pool.begin().await.unwrap();
    let (mut ins, mut upd) = (0usize, 0usize);
    upsert_student(&mut *tx, &enrolled_rec("S1", "홍길동", 1, 1, 1), &mut ins, &mut upd)
        .await.unwrap();
    upsert_student(&mut *tx, &enrolled_rec("S2", "김철수", 1, 1, 2), &mut ins, &mut upd)
        .await.unwrap();
    tx.commit().await.unwrap();
    seed_round_result(&pool, "S2", "CLOSED").await; // 결과는 S2 에만

    let mut tx2 = pool.begin().await.unwrap();
    upsert_student(&mut *tx2, &graduated_rec("S1", "홍길동", 2024), &mut ins, &mut upd)
        .await.expect("결과가 없는 학생은 막지 않는다");
}

/// 구분이 그대로면 CLOSED 중에도 이름 수정은 허용된다(가드는 전환에만 걸린다).
#[tokio::test]
async fn upsert_student_name_only_update_allowed_when_closed_round() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let mut tx = pool.begin().await.unwrap();
    let (mut ins, mut upd) = (0usize, 0usize);
    upsert_student(&mut *tx, &enrolled_rec("S1", "홍길동", 1, 1, 1), &mut ins, &mut upd)
        .await.unwrap();
    tx.commit().await.unwrap();
    seed_round_result(&pool, "S1", "CLOSED").await;

    let mut tx2 = pool.begin().await.unwrap();
    upsert_student(&mut *tx2, &enrolled_rec("S1", "홍길순", 1, 1, 1), &mut ins, &mut upd)
        .await.expect("구분 무변경 — 이름 수정은 허용");
    tx2.commit().await.unwrap();
    let name: String = sqlx::query_scalar("SELECT name FROM students WHERE student_code = 'S1'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(name, "홍길순");
}

/// 신규 학생 추가는 CLOSED 중에도 허용된다(마감 라운드에 결과가 있을 수 없다).
#[tokio::test]
async fn upsert_student_new_insert_allowed_when_closed_round() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let mut tx = pool.begin().await.unwrap();
    let (mut ins, mut upd) = (0usize, 0usize);
    upsert_student(&mut *tx, &enrolled_rec("S1", "홍길동", 1, 1, 1), &mut ins, &mut upd)
        .await.unwrap();
    tx.commit().await.unwrap();
    seed_round_result(&pool, "S1", "CLOSED").await;

    let mut tx2 = pool.begin().await.unwrap();
    upsert_student(&mut *tx2, &graduated_rec("S9", "신입", 2024), &mut ins, &mut upd)
        .await.expect("신규 학생은 막지 않는다");
}

// ── list_students: 필터·페이지네이션(G 세션 보강) ────────────────────

fn list_query(grade: Option<i64>, class_no: Option<i64>, page: i64, per_page: i64) -> Query<ListQuery> {
    Query(ListQuery { grade, class_no, is_enrolled: None, page, per_page })
}

#[tokio::test]
async fn list_students_grade_class_filter_returns_only_matching_class() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    common::insert_class(&pool, 1, 2).await;
    let mut tx = pool.begin().await.unwrap();
    let (mut ins, mut upd) = (0usize, 0usize);
    for (code, seq) in [("E1", 1), ("E2", 2), ("E3", 3)] {
        upsert_student(&mut *tx, &enrolled_rec(code, code, 1, 1, seq), &mut ins, &mut upd).await.unwrap();
    }
    upsert_student(&mut *tx, &enrolled_rec("E4", "E4", 1, 2, 1), &mut ins, &mut upd).await.unwrap();
    tx.commit().await.unwrap();

    let Json(page) = list_students(State(common::make_state(pool)), list_query(Some(1), Some(1), 1, 50))
        .await
        .unwrap();
    assert_eq!(page.total, 3, "1반 소속 3명만 집계되어야 함(2반 제외)");
    assert_eq!(page.rows.len(), 3);
    let codes: Vec<&str> = page.rows.iter().map(|r| r.student_code.as_str()).collect();
    assert_eq!(codes, vec!["E1", "E2", "E3"], "seq_no 오름차순 정렬이어야 함");
}

#[tokio::test]
async fn list_students_pagination_offset_matches_page_minus_one_times_per_page() {
    // 재학생 4명(1반 seq1~3, 2반 seq1) + 졸업생 1명 = 총 5명.
    // 정렬은 is_enrolled DESC, grade, class_no, seq_no 이므로
    // 순서는 E1,E2,E3,E4,G1. per_page=2일 때 2페이지는 offset=2 → E3,E4가 나와야 한다.
    // offset이 page*per_page(=4)로 잘못 계산되면 2페이지에 G1만 남는다.
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    common::insert_class(&pool, 1, 2).await;
    let mut tx = pool.begin().await.unwrap();
    let (mut ins, mut upd) = (0usize, 0usize);
    for (code, seq) in [("E1", 1), ("E2", 2), ("E3", 3)] {
        upsert_student(&mut *tx, &enrolled_rec(code, code, 1, 1, seq), &mut ins, &mut upd).await.unwrap();
    }
    upsert_student(&mut *tx, &enrolled_rec("E4", "E4", 1, 2, 1), &mut ins, &mut upd).await.unwrap();
    upsert_student(&mut *tx, &graduated_rec("G1", "G1", 2024), &mut ins, &mut upd).await.unwrap();
    tx.commit().await.unwrap();

    let state = common::make_state(pool);
    let Json(page2) = list_students(State(state.clone()), list_query(None, None, 2, 2)).await.unwrap();
    assert_eq!(page2.total, 5, "필터 없을 때 전체 5명 집계");
    let codes: Vec<&str> = page2.rows.iter().map(|r| r.student_code.as_str()).collect();
    assert_eq!(codes, vec!["E3", "E4"], "offset=(page-1)*per_page=2 이어야 함");

    let Json(page3) = list_students(State(state), list_query(None, None, 3, 2)).await.unwrap();
    let codes3: Vec<&str> = page3.rows.iter().map(|r| r.student_code.as_str()).collect();
    assert_eq!(codes3, vec!["G1"], "3페이지엔 졸업생 1명만 남아야 함");
}

/// 전체 목록 다운로드(export_students)는 화면 목록과 같은 기준으로 정렬돼야 한다:
/// 재학생 먼저(is_enrolled DESC) → 학년·반·번호 → 학생코드.
/// is_enrolled 를 빼면 grade=NULL 인 졸업생이 맨 위로 올라와 화면과 어긋난다(회귀 방지).
#[tokio::test]
async fn export_students_sorts_enrolled_before_graduated() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;

    // 졸업생을 먼저 넣어 삽입 순서가 기대 정렬과 어긋나게 둔다
    sqlx::query("INSERT INTO students (student_code, name, is_enrolled, grad_year) VALUES ('20240001', '졸업', 0, 2024)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) VALUES ('E002', '재학2', 1, 1, 2, 1)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) VALUES ('E001', '재학1', 1, 1, 1, 1)")
        .execute(&pool).await.unwrap();

    let resp = export_students(State(common::make_state(pool))).await.unwrap();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let rows = principal_candidate_manager::excel::parse_xlsx_all_rows_raw(&bytes).unwrap();
    let h = &rows[0];
    let c_code = h.iter().position(|c| c == "학생코드").unwrap();
    let order: Vec<&str> = rows[1..].iter().map(|r| r[c_code].as_str()).collect();

    assert_eq!(
        order,
        vec!["E001", "E002", "20240001"],
        "재학생(번호순) 먼저, 졸업생은 코드가 작아도 맨 뒤: {order:?}",
    );
}
