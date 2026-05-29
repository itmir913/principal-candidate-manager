mod common;

use axum::{
    extract::{Path, State},
    http::StatusCode,
};
use principal_candidate_manager::handlers::students::{
    delete_student, find_unique_code, upsert_enrolled_by_position, upsert_student, StudentRecord,
};

fn enrolled_rec(code: &str, name: &str, g: i64, c: i64, s: i64) -> StudentRecord {
    StudentRecord {
        student_code: code.into(),
        name: name.into(),
        is_enrolled: 1,
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
        is_enrolled: 0,
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
        is_enrolled: 0,
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
        is_enrolled: 0,
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
        is_enrolled: 1,
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
        is_enrolled: 0,
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
        is_enrolled: 1,
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
        is_enrolled: 1,
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
        is_enrolled: 1,
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
        "INSERT INTO applications (student_id, track_id, round_id, confirmed, abandoned) \
         VALUES (?, ?, ?, 1, 0)",
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
