/// 세션 B: import·개별 CRUD 핸들러별 감사 로그 기록 검증.
///
/// 각 AuditAction 변형(≥1개), import 성공→1건·실패→0건,
/// delete 시 삭제 전 이름 스냅샷 포함 여부를 확인한다.
mod common;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use principal_candidate_manager::{
    enums::{CalcType, LookupScope, MatchMode},
    handlers::{
        area_data::{base_data_import, category_map_import, numeric_table_import, StudentTypeQuery},
        areas::{create_area, delete_area, update_area, CreateAreaBody, UpdateAreaBody},
        classes::{delete_class, import_classes, upsert_class, UpsertClassBody},
        students::{
            add_enrolled, add_graduated, delete_student, import_enrolled, import_graduated,
            import_students, AddEnrolledBody, AddGraduatedBody,
        },
        universities::{
            create_track, create_university, delete_track, delete_university, update_track,
            update_university, CreateTrackBody, CreateUnivBody, UpdateTrackBody, UpdateUnivBody,
        },
    },
    score::Score,
};

// ── 픽스처 헬퍼 ───────────────────────────────────────────────────

async fn insert_univ(pool: &sqlx::SqlitePool, name: &str) -> i64 {
    sqlx::query_scalar("INSERT INTO universities (univ_name) VALUES (?) RETURNING id")
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn insert_track_row(pool: &sqlx::SqlitePool, univ_id: i64, name: &str) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name) VALUES (?, ?) RETURNING id",
    )
    .bind(univ_id)
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn insert_area_numeric(pool: &sqlx::SqlitePool) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, match_mode, lookup_scope) \
         VALUES ('내신', 10000000, 'NUMERIC', 'UPPER', 'SIMPLE') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn insert_area_category(pool: &sqlx::SqlitePool) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, category_agg, lookup_scope, multi_value) \
         VALUES ('활동', 10000000, 'CATEGORY', 'MAX', 'SIMPLE', 0) RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn insert_graduated_student(pool: &sqlx::SqlitePool, code: &str) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO students (student_code, name, is_enrolled, grad_year) \
         VALUES (?, '테스트', 0, 2024) RETURNING id",
    )
    .bind(code)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn enrolled_csv() -> &'static str {
    "학년,반,담임명,비밀번호\n1,1,홍길동,pass1234\n"
}

fn audit_count_action<'a>(pool: &'a sqlx::SqlitePool, action: &'a str) -> impl std::future::Future<Output = i64> + 'a {
    async move {
        sqlx::query_scalar("SELECT COUNT(*) FROM audit_log WHERE action = ?")
            .bind(action)
            .fetch_one(pool)
            .await
            .unwrap()
    }
}

fn total_audit_count(pool: &sqlx::SqlitePool) -> impl std::future::Future<Output = i64> + '_ {
    async move {
        sqlx::query_scalar("SELECT COUNT(*) FROM audit_log")
            .fetch_one(pool)
            .await
            .unwrap()
    }
}

// ═══════════════════════════════════════════════════════
// ClassesImported
// ═══════════════════════════════════════════════════════

#[tokio::test]
async fn classes_imported_success_writes_one_log() {
    let pool = common::create_test_pool().await;
    let state = common::make_state(pool.clone());
    let csv = "학년,반,담임명,비밀번호\n1,1,홍길동,pass1234\n1,2,이순신,pass5678\n";
    let (status, _) = import_classes(State(state), common::csv_multipart(csv).await)
        .await
        .unwrap();
    assert_eq!(status, StatusCode::OK);
    assert_eq!(audit_count_action(&pool, "CLASSES_IMPORTED").await, 1);
}

#[tokio::test]
async fn classes_imported_failure_writes_no_log() {
    let pool = common::create_test_pool().await;
    let state = common::make_state(pool.clone());
    // 비밀번호 길이 미달 → 422
    let csv = "학년,반,담임명,비밀번호\n1,1,홍길동,pass1234\n1,2,이순신,ab\n";
    let (status, _) = import_classes(State(state), common::csv_multipart(csv).await)
        .await
        .unwrap();
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(total_audit_count(&pool).await, 0);
}

// ═══════════════════════════════════════════════════════
// ClassSaved
// ═══════════════════════════════════════════════════════

#[tokio::test]
async fn class_saved_writes_one_log() {
    let pool = common::create_test_pool().await;
    upsert_class(
        State(common::make_state(pool.clone())),
        Path((1i64, 1i64)),
        Json(UpsertClassBody {
            teacher_name: Some("홍길동".into()),
            password: Some("pass1234".into()),
        }),
    )
    .await
    .unwrap();
    assert_eq!(audit_count_action(&pool, "CLASS_SAVED").await, 1);
}

// ═══════════════════════════════════════════════════════
// ClassDeleted — 삭제 전 teacher_name 스냅샷
// ═══════════════════════════════════════════════════════

#[tokio::test]
async fn class_deleted_detail_has_pre_delete_teacher_name() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 2, 3).await;
    // teacher_name을 알려진 값으로 설정
    sqlx::query("UPDATE classes SET teacher_name = '김담임' WHERE grade = 2 AND class_no = 3")
        .execute(&pool)
        .await
        .unwrap();

    delete_class(State(common::make_state(pool.clone())), Path((2i64, 3i64)))
        .await
        .unwrap();

    assert_eq!(audit_count_action(&pool, "CLASS_DELETED").await, 1);
    let detail_str: String =
        sqlx::query_scalar("SELECT detail FROM audit_log WHERE action = 'CLASS_DELETED'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let detail: serde_json::Value = serde_json::from_str(&detail_str).unwrap();
    assert_eq!(detail["teacher_name"], "김담임");
}

// ═══════════════════════════════════════════════════════
// StudentsImported (import_students / import_enrolled / import_graduated)
// ═══════════════════════════════════════════════════════

#[tokio::test]
async fn students_imported_all_success_writes_one_log() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let state = common::make_state(pool.clone());
    let csv = "학생코드,이름,재학여부,학년,반,번호\nS001,홍길동,재학,1,1,1\n";
    let (status, _) = import_students(State(state), common::csv_multipart(csv).await)
        .await
        .unwrap();
    assert_eq!(status, StatusCode::OK);
    assert_eq!(audit_count_action(&pool, "STUDENTS_IMPORTED").await, 1);
}

#[tokio::test]
async fn students_imported_failure_writes_no_log() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let state = common::make_state(pool.clone());
    // 재학생인데 학년 누락 → 422
    let csv = "학생코드,이름,재학여부,학년,반,번호\nS001,홍길동,재학,,1,1\n";
    let (status, _) = import_students(State(state), common::csv_multipart(csv).await)
        .await
        .unwrap();
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(total_audit_count(&pool).await, 0);
}

#[tokio::test]
async fn students_imported_enrolled_success_writes_log_with_source() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let state = common::make_state(pool.clone());
    let csv = "학생코드,이름,학년,반,번호\nS001,홍길동,1,1,1\n";
    let (status, _) = import_enrolled(State(state), common::csv_multipart(csv).await)
        .await
        .unwrap();
    assert_eq!(status, StatusCode::OK);
    assert_eq!(audit_count_action(&pool, "STUDENTS_IMPORTED").await, 1);
    let detail_str: String =
        sqlx::query_scalar("SELECT detail FROM audit_log WHERE action = 'STUDENTS_IMPORTED'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let detail: serde_json::Value = serde_json::from_str(&detail_str).unwrap();
    assert_eq!(detail["source"], "enrolled");
}

#[tokio::test]
async fn students_imported_graduated_success_writes_log_with_source() {
    let pool = common::create_test_pool().await;
    let state = common::make_state(pool.clone());
    let csv = "학생코드,이름,졸업연도\nGR001,김철수,2024\n";
    let (status, _) = import_graduated(State(state), common::csv_multipart(csv).await)
        .await
        .unwrap();
    assert_eq!(status, StatusCode::OK);
    assert_eq!(audit_count_action(&pool, "STUDENTS_IMPORTED").await, 1);
    let detail_str: String =
        sqlx::query_scalar("SELECT detail FROM audit_log WHERE action = 'STUDENTS_IMPORTED'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let detail: serde_json::Value = serde_json::from_str(&detail_str).unwrap();
    assert_eq!(detail["source"], "graduated");
}

// ═══════════════════════════════════════════════════════
// StudentAdded
// ═══════════════════════════════════════════════════════

#[tokio::test]
async fn student_added_enrolled_writes_one_log() {
    let pool = common::create_test_pool_shared().await;
    common::insert_class(&pool, 1, 1).await;
    add_enrolled(
        State(common::make_state(pool.clone())),
        Json(AddEnrolledBody { name: "홍길동".into(), grade: 1, class_no: 1, seq_no: 1 }),
    )
    .await
    .unwrap();
    assert_eq!(audit_count_action(&pool, "STUDENT_ADDED").await, 1);
}

#[tokio::test]
async fn student_added_graduated_writes_one_log() {
    let pool = common::create_test_pool_shared().await;
    add_graduated(
        State(common::make_state(pool.clone())),
        Json(AddGraduatedBody {
            student_code: "GR001".into(),
            name: "김철수".into(),
            grad_year: 2024,
        }),
    )
    .await
    .unwrap();
    assert_eq!(audit_count_action(&pool, "STUDENT_ADDED").await, 1);
}

// ═══════════════════════════════════════════════════════
// StudentDeleted — 삭제 전 이름 스냅샷
// ═══════════════════════════════════════════════════════

#[tokio::test]
async fn student_deleted_detail_has_pre_delete_name() {
    let pool = common::create_test_pool().await;
    let sid = insert_graduated_student(&pool, "S001").await;

    delete_student(State(common::make_state(pool.clone())), Path(sid))
        .await
        .unwrap();

    assert_eq!(audit_count_action(&pool, "STUDENT_DELETED").await, 1);
    let detail_str: String =
        sqlx::query_scalar("SELECT detail FROM audit_log WHERE action = 'STUDENT_DELETED'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let detail: serde_json::Value = serde_json::from_str(&detail_str).unwrap();
    assert_eq!(detail["student_code"], "S001");
    assert!(detail.get("name").is_some());
}

// ═══════════════════════════════════════════════════════
// AreaCreated / AreaUpdated / AreaDeleted
// ═══════════════════════════════════════════════════════

#[tokio::test]
async fn area_created_writes_one_log() {
    let pool = common::create_test_pool().await;
    create_area(
        State(common::make_state(pool.clone())),
        Json(CreateAreaBody {
            name: "교과".into(),
            max_score: Score::from_raw(10_000_000),
            calc_type: CalcType::Numeric,
            teacher_editable: false,
            lookup_scope: LookupScope::Simple,
            match_mode: Some(MatchMode::Upper),
            category_agg: None,
            multi_value: false,
        }),
    )
    .await
    .unwrap();
    assert_eq!(audit_count_action(&pool, "AREA_CREATED").await, 1);
}

#[tokio::test]
async fn area_updated_writes_one_log() {
    let pool = common::create_test_pool().await;
    let aid = insert_area_numeric(&pool).await;
    update_area(
        State(common::make_state(pool.clone())),
        Path(aid),
        Json(UpdateAreaBody { name: Some("내신수정".into()), teacher_editable: None }),
    )
    .await
    .unwrap();
    assert_eq!(audit_count_action(&pool, "AREA_UPDATED").await, 1);
}

#[tokio::test]
async fn area_deleted_detail_has_pre_delete_name() {
    let pool = common::create_test_pool().await;
    let aid = insert_area_numeric(&pool).await;

    delete_area(State(common::make_state(pool.clone())), Path(aid))
        .await
        .unwrap();

    assert_eq!(audit_count_action(&pool, "AREA_DELETED").await, 1);
    let detail_str: String =
        sqlx::query_scalar("SELECT detail FROM audit_log WHERE action = 'AREA_DELETED'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let detail: serde_json::Value = serde_json::from_str(&detail_str).unwrap();
    assert_eq!(detail["name"], "내신");
}

// ═══════════════════════════════════════════════════════
// ScoreTableImported (numeric + category)
// ═══════════════════════════════════════════════════════

#[tokio::test]
async fn numeric_table_imported_success_writes_one_log() {
    let pool = common::create_test_pool_shared().await;
    let aid = insert_area_numeric(&pool).await;
    let state = common::make_state(pool.clone());
    let csv = "기준값,점수\n10,100\n0,0\n";
    let (status, _) =
        numeric_table_import(State(state), Path(aid), common::csv_multipart(csv).await)
            .await
            .unwrap();
    assert_eq!(status, StatusCode::OK);
    assert_eq!(audit_count_action(&pool, "SCORE_TABLE_IMPORTED").await, 1);
}

#[tokio::test]
async fn numeric_table_imported_failure_writes_no_log() {
    let pool = common::create_test_pool_shared().await;
    let aid = insert_area_numeric(&pool).await;
    let state = common::make_state(pool.clone());
    // 단조성 위반 → 422
    let csv = "기준값,점수\n10.0,90.0\n5.0,50.0\n1.0,80.0\n";
    let (status, _) =
        numeric_table_import(State(state), Path(aid), common::csv_multipart(csv).await)
            .await
            .unwrap();
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(total_audit_count(&pool).await, 0);
}

#[tokio::test]
async fn category_map_imported_success_writes_one_log() {
    let pool = common::create_test_pool_shared().await;
    let aid = insert_area_category(&pool).await;
    let state = common::make_state(pool.clone());
    let csv = "범주,점수\n활동,100\n미활동,0\n";
    let (status, _) =
        category_map_import(State(state), Path(aid), common::csv_multipart(csv).await)
            .await
            .unwrap();
    assert_eq!(status, StatusCode::OK);
    assert_eq!(audit_count_action(&pool, "SCORE_TABLE_IMPORTED").await, 1);
}

// ═══════════════════════════════════════════════════════
// BaseDataImported
// ═══════════════════════════════════════════════════════

#[tokio::test]
async fn base_data_imported_success_writes_one_log() {
    let pool = common::create_test_pool_shared().await;
    let aid = insert_area_numeric(&pool).await;
    insert_graduated_student(&pool, "S001").await;
    // numeric_table 먼저 설정
    sqlx::query(
        "INSERT INTO numeric_table (area_id, track_id, threshold, score) VALUES (?, NULL, 0, 0)",
    )
    .bind(aid)
    .execute(&pool)
    .await
    .unwrap();
    let state = common::make_state(pool.clone());
    let csv = "학생코드,이름,값\nS001,테스트,5.0\n";
    let q = Query(StudentTypeQuery { student_type: "graduated".into() });
    let (status, _) =
        base_data_import(State(state), Path(aid), q, common::csv_multipart(csv).await)
            .await
            .unwrap();
    assert_eq!(status, StatusCode::OK);
    assert_eq!(audit_count_action(&pool, "BASE_DATA_IMPORTED").await, 1);
}

#[tokio::test]
async fn base_data_imported_failure_writes_no_log() {
    let pool = common::create_test_pool_shared().await;
    let aid = insert_area_numeric(&pool).await;
    insert_graduated_student(&pool, "S001").await;
    sqlx::query(
        "INSERT INTO numeric_table (area_id, track_id, threshold, score) VALUES (?, NULL, 0, 0)",
    )
    .bind(aid)
    .execute(&pool)
    .await
    .unwrap();
    let state = common::make_state(pool.clone());
    // 중복 행 → 422
    let csv = "학생코드,이름,값\nS001,테스트,5.0\nS001,테스트,3.0\n";
    let q = Query(StudentTypeQuery { student_type: "graduated".into() });
    let (status, _) =
        base_data_import(State(state), Path(aid), q, common::csv_multipart(csv).await)
            .await
            .unwrap();
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(total_audit_count(&pool).await, 0);
}

// ═══════════════════════════════════════════════════════
// UniversityCreated / UniversityUpdated / UniversityDeleted
// ═══════════════════════════════════════════════════════

#[tokio::test]
async fn university_created_writes_one_log() {
    let pool = common::create_test_pool().await;
    create_university(
        State(common::make_state(pool.clone())),
        Json(CreateUnivBody {
            univ_name: "한국대".into(),
            total_quota: None,
            prioritize_enrolled: false,
        }),
    )
    .await
    .unwrap();
    assert_eq!(audit_count_action(&pool, "UNIVERSITY_CREATED").await, 1);
}

#[tokio::test]
async fn university_updated_writes_one_log() {
    let pool = common::create_test_pool().await;
    let uid = insert_univ(&pool, "한국대").await;
    update_university(
        State(common::make_state(pool.clone())),
        Path(uid),
        Json(UpdateUnivBody {
            univ_name: Some("한국대학교".into()),
            total_quota: None,
            prioritize_enrolled: None,
        }),
    )
    .await
    .unwrap();
    assert_eq!(audit_count_action(&pool, "UNIVERSITY_UPDATED").await, 1);
    let detail_str: String =
        sqlx::query_scalar("SELECT detail FROM audit_log WHERE action = 'UNIVERSITY_UPDATED'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let detail: serde_json::Value = serde_json::from_str(&detail_str).unwrap();
    assert_eq!(detail["univ_name"], "한국대학교");
}

#[tokio::test]
async fn university_deleted_detail_has_pre_delete_name() {
    let pool = common::create_test_pool().await;
    let uid = insert_univ(&pool, "삭제대학교").await;

    delete_university(State(common::make_state(pool.clone())), Path(uid))
        .await
        .unwrap();

    assert_eq!(audit_count_action(&pool, "UNIVERSITY_DELETED").await, 1);
    let detail_str: String =
        sqlx::query_scalar("SELECT detail FROM audit_log WHERE action = 'UNIVERSITY_DELETED'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let detail: serde_json::Value = serde_json::from_str(&detail_str).unwrap();
    assert_eq!(detail["univ_name"], "삭제대학교");
}

// ═══════════════════════════════════════════════════════
// TrackCreated / TrackUpdated / TrackDeleted
// ═══════════════════════════════════════════════════════

#[tokio::test]
async fn track_created_writes_one_log() {
    let pool = common::create_test_pool().await;
    let uid = insert_univ(&pool, "한국대").await;
    create_track(
        State(common::make_state(pool.clone())),
        Path(uid),
        Json(CreateTrackBody {
            track_name: "컴공".into(),
            unit_quota: None,
            prioritize_enrolled: false,
        }),
    )
    .await
    .unwrap();
    assert_eq!(audit_count_action(&pool, "TRACK_CREATED").await, 1);
    let detail_str: String =
        sqlx::query_scalar("SELECT detail FROM audit_log WHERE action = 'TRACK_CREATED'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let detail: serde_json::Value = serde_json::from_str(&detail_str).unwrap();
    assert_eq!(detail["univ_name"], "한국대");
    assert_eq!(detail["track_name"], "컴공");
}

#[tokio::test]
async fn track_updated_writes_one_log() {
    let pool = common::create_test_pool().await;
    let uid = insert_univ(&pool, "한국대").await;
    let tid = insert_track_row(&pool, uid, "컴공").await;
    update_track(
        State(common::make_state(pool.clone())),
        Path(tid),
        Json(UpdateTrackBody {
            track_name: Some("소프트웨어학과".into()),
            unit_quota: None,
            prioritize_enrolled: None,
        }),
    )
    .await
    .unwrap();
    assert_eq!(audit_count_action(&pool, "TRACK_UPDATED").await, 1);
}

#[tokio::test]
async fn track_deleted_detail_has_pre_delete_names() {
    let pool = common::create_test_pool().await;
    let uid = insert_univ(&pool, "삭제대").await;
    let tid = insert_track_row(&pool, uid, "삭제트랙").await;

    delete_track(State(common::make_state(pool.clone())), Path(tid))
        .await
        .unwrap();

    assert_eq!(audit_count_action(&pool, "TRACK_DELETED").await, 1);
    let detail_str: String =
        sqlx::query_scalar("SELECT detail FROM audit_log WHERE action = 'TRACK_DELETED'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let detail: serde_json::Value = serde_json::from_str(&detail_str).unwrap();
    assert_eq!(detail["univ_name"], "삭제대");
    assert_eq!(detail["track_name"], "삭제트랙");
}
