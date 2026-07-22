//! 전수 커버리지 마무리: 조회(GET) 계열과 남은 핸들러.
//!
//! 조회 API는 "터지지 않으면 통과"로 쓰기 쉬워서, 정렬·필터가 조용히 사라져도
//! 모르고 지나간다. 그래서 정렬 픽스처는 기대 순서가 **id 오름차순과도, id
//! 내림차순과도 다르도록** 배치한다. 삽입 순서를 그냥 "역순"으로 두면
//! `ORDER BY id DESC` 가 기대값과 우연히 일치해 정렬이 통째로 사라져도 통과한다
//! (이 파일의 초안이 실제로 그 함정에 빠졌고, 변이 검사에서 잡혔다).
//! 픽스처가 여전히 유효한지도 테스트 안에서 `assert_ne!` 로 함께 고정한다.

mod common;

use axum::{
    body::Body,
    extract::{FromRequest, Multipart, Path, Query, State},
    http::{Request, StatusCode},
    Extension,
};
use principal_candidate_manager::{
    audit::{self, Actor, AuditEntry},
    enums::{AuditAction, RoundStatus},
    excel,
    handlers::{
        audit::{export_audit_logs, AuditQuery},
        classes::list_classes,
        external_import::univ_preview,
        round_confirmations::{teacher_confirm_round, teacher_get_confirmation},
        rounds::list_rounds,
        students::grade_options,
        system::get_version,
        universities::{list_all_tracks, list_tracks, list_universities},
    },
    state::AppState,
};
use sqlx::SqlitePool;

fn st(pool: &SqlitePool) -> State<AppState> {
    State(common::make_state(pool.clone()))
}

fn audit_q() -> AuditQuery {
    AuditQuery { page: 1, per_page: 50, round_id: None, action: None, grade: None, class_no: None }
}

async fn response_bytes(resp: axum::response::Response) -> Vec<u8> {
    axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap().to_vec()
}

/// 헤더 이름 → 열 인덱스. 산출물 단언을 열 순서에 결합시킨다
/// (집합 소속만 보면 열이 뒤바뀌어도 통과한다).
fn col_of(header: &[String], name: &str) -> usize {
    header
        .iter()
        .position(|h| h.trim() == name)
        .unwrap_or_else(|| panic!("'{}' 열이 없습니다: {:?}", name, header))
}

// ── 대학 / 모집단위 목록 ─────────────────────────────────────────

/// 삽입 순서를 이름 오름차순과도, **그 역순과도** 어긋나게 넣는다.
/// 단순히 역순으로 넣으면 `ORDER BY id DESC` 가 이름 오름차순과 우연히 일치해
/// 정렬이 통째로 바뀌어도 테스트가 통과한다 (실제로 변이 검사에서 걸렸다).
///   삽입(id): 바다대, 하늘대, 가람대
///   이름 오름차순: 가람대, 바다대, 하늘대  ← id 오름차순·내림차순 어느 쪽과도 다르다
async fn seed_universities(pool: &SqlitePool) -> Vec<i64> {
    let mut ids = Vec::new();
    for (name, quota) in [("바다대", 3i64), ("하늘대", 5), ("가람대", 1)] {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO universities (univ_name, total_quota, prioritize_enrolled)
             VALUES (?, ?, 1) RETURNING id",
        )
        .bind(name)
        .bind(quota)
        .fetch_one(pool)
        .await
        .unwrap();
        ids.push(id);
    }
    ids
}

#[tokio::test]
async fn list_universities_sorts_by_name_not_insertion_order() {
    let pool = common::create_test_pool().await;
    let ids = seed_universities(&pool).await;

    let axum::Json(rows) = list_universities(st(&pool)).await.unwrap();
    let names: Vec<&str> = rows.iter().map(|r| r.univ_name.as_str()).collect();
    assert_eq!(names, vec!["가람대", "바다대", "하늘대"], "대학명 오름차순이어야 함");
    // 픽스처 유효성: 기대 순서가 id 오름차순·내림차순 어느 쪽과도 달라야 한다
    let by_id_asc: Vec<i64> = ids.clone();
    let by_id_desc: Vec<i64> = ids.iter().rev().copied().collect();
    let got: Vec<i64> = rows.iter().map(|r| r.id).collect();
    assert_ne!(got, by_id_asc, "기대 순서가 id 오름차순과 같으면 판별력이 없다");
    assert_ne!(got, by_id_desc, "기대 순서가 id 내림차순과 같으면 판별력이 없다");
    // 정원·재학생우선 플래그도 함께 실려야 화면이 그린다
    assert_eq!(rows[0].total_quota, Some(1));
    assert_eq!(rows[2].total_quota, Some(5));
    assert!(rows.iter().all(|r| r.prioritize_enrolled == 1));
}

#[tokio::test]
async fn list_universities_empty_is_empty_array() {
    let pool = common::create_test_pool().await;
    let axum::Json(rows) = list_universities(st(&pool)).await.unwrap();
    assert!(rows.is_empty());
}

#[tokio::test]
async fn list_tracks_filters_to_requested_university_and_sorts() {
    let pool = common::create_test_pool().await;
    let ids = seed_universities(&pool).await;
    // 이름 오름차순(경영학·물리학·체육교육)과도, 그 역순과도 어긋나는 삽입 순서
    for t in ["물리학", "체육교육", "경영학"] {
        sqlx::query(
            "INSERT INTO univ_tracks (univ_id, track_name, unit_quota, prioritize_enrolled)
             VALUES (?, ?, 2, 1)",
        )
        .bind(ids[0])
        .bind(t)
        .execute(&pool)
        .await
        .unwrap();
    }
    // 다른 대학의 모집단위는 섞여 나오면 안 된다
    sqlx::query("INSERT INTO univ_tracks (univ_id, track_name, prioritize_enrolled) VALUES (?, '간호학', 1)")
        .bind(ids[1])
        .execute(&pool)
        .await
        .unwrap();

    let axum::Json(rows) = list_tracks(st(&pool), Path(ids[0])).await.unwrap();
    let names: Vec<&str> = rows.iter().map(|r| r.track_name.as_str()).collect();
    assert_eq!(names, vec!["경영학", "물리학", "체육교육"], "모집단위명 오름차순");
    assert!(rows.iter().all(|r| r.univ_id == ids[0]), "다른 대학의 모집단위가 섞였다");
    assert_eq!(rows[0].unit_quota, Some(2));
}

#[tokio::test]
async fn list_tracks_unknown_university_returns_empty() {
    let pool = common::create_test_pool().await;
    let axum::Json(rows) = list_tracks(st(&pool), Path(9999)).await.unwrap();
    assert!(rows.is_empty());
}

#[tokio::test]
async fn list_all_tracks_joins_university_and_sorts_by_univ_then_track() {
    let pool = common::create_test_pool().await;
    let ids = seed_universities(&pool).await;
    // (대학, 모집단위) 삽입 순서를 기대 정렬과도, 그 역순과도 어긋나게
    //   삽입(id): 바다대/체육교육(1), 가람대/의예과(2), 바다대/경영학(3), 가람대/간호학(4)
    //   기대:     가람대/간호학(4), 가람대/의예과(2), 바다대/경영학(3), 바다대/체육교육(1)
    for (uid, track) in [
        (ids[0], "체육교육"), // 바다대
        (ids[2], "의예과"),   // 가람대
        (ids[0], "경영학"),
        (ids[2], "간호학"),
    ] {
        sqlx::query("INSERT INTO univ_tracks (univ_id, track_name, prioritize_enrolled) VALUES (?, ?, 1)")
            .bind(uid)
            .bind(track)
            .execute(&pool)
            .await
            .unwrap();
    }

    let axum::Json(rows) = list_all_tracks(st(&pool)).await.unwrap();
    let pairs: Vec<(&str, &str)> = rows
        .iter()
        .map(|r| (r.univ_name.as_str(), r.track_name.as_str()))
        .collect();
    assert_eq!(
        pairs,
        vec![
            ("가람대", "간호학"),
            ("가람대", "의예과"),
            ("바다대", "경영학"),
            ("바다대", "체육교육"),
        ]
    );
    let got: Vec<i64> = rows.iter().map(|r| r.id).collect();
    assert_ne!(got, vec![1, 2, 3, 4], "기대 순서가 id 오름차순과 같으면 판별력이 없다");
    assert_ne!(got, vec![4, 3, 2, 1], "기대 순서가 id 내림차순과 같으면 판별력이 없다");
    // JOIN으로 실려 오는 대학 정원이 모집단위 행에 붙어야 한다
    assert_eq!(rows[0].total_quota, Some(1), "가람대 정원");
    assert_eq!(rows[3].total_quota, Some(3), "바다대 정원");
}

// ── 학급 목록 ────────────────────────────────────────────────────

#[tokio::test]
async fn list_classes_sorts_and_appends_graduate_sentinel_only_when_graduates_exist() {
    let pool = common::create_test_pool().await;
    // 삽입 순서를 (학년, 반) 오름차순과도, 그 역순과도 어긋나게
    //   삽입: (3,1), (1,2), (3,2)  /  기대: (1,2), (3,1), (3,2)
    for (g, c) in [(3i64, 1i64), (1, 2), (3, 2)] {
        common::insert_class(&pool, g, c).await;
    }

    let axum::Json(rows) = list_classes(st(&pool)).await.unwrap();
    let seq: Vec<(i64, i64)> = rows.iter().map(|r| (r.grade, r.class_no)).collect();
    assert_eq!(seq, vec![(1, 2), (3, 1), (3, 2)], "학년·반 오름차순");
    assert!(
        !rows.iter().any(|r| r.grade == 0),
        "졸업생이 없으면 졸업생 항목이 나오면 안 된다"
    );

    // 졸업생이 생기면 sentinel(0/0) 항목이 맨 뒤에 붙는다
    sqlx::query(
        "INSERT INTO students (student_code, name, is_enrolled, grad_year)
         VALUES ('20240001', '이영희', 0, 2024)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let axum::Json(rows) = list_classes(st(&pool)).await.unwrap();
    assert_eq!(rows.len(), 4);
    let last = rows.last().unwrap();
    assert_eq!((last.grade, last.class_no), (0, 0));
    assert_eq!(last.teacher_name.as_deref(), Some("졸업생"));
}

#[tokio::test]
async fn list_classes_ignores_enrolled_students_for_sentinel() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    sqlx::query(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled)
         VALUES ('20250001', '홍길동', 1, 1, 1, 1)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let axum::Json(rows) = list_classes(st(&pool)).await.unwrap();
    assert_eq!(rows.len(), 1, "재학생만 있으면 졸업생 항목이 붙으면 안 된다");
}

// ── 라운드 목록 ──────────────────────────────────────────────────

#[tokio::test]
async fn list_rounds_returns_newest_first_with_timestamps() {
    let pool = common::create_test_pool().await;
    sqlx::query(
        "INSERT INTO rounds (status, opened_at, closed_at, finalized_at)
         VALUES ('FINALIZED', '2025-01-01T00:00:00Z', '2025-02-01T00:00:00Z', '2025-03-01T00:00:00Z')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO rounds (status, opened_at) VALUES ('OPEN', '2025-04-01T00:00:00Z')")
        .execute(&pool)
        .await
        .unwrap();

    let axum::Json(rows) = list_rounds(st(&pool)).await.unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].id, 2, "최신 라운드가 먼저");
    assert_eq!(rows[0].status, RoundStatus::Open);
    assert_eq!(rows[1].status, RoundStatus::Finalized);
    // 마감·확정 시각이 실려야 화면이 라운드 이력을 그린다
    assert_eq!(rows[1].closed_at.as_deref(), Some("2025-02-01T00:00:00Z"));
    assert_eq!(rows[1].finalized_at.as_deref(), Some("2025-03-01T00:00:00Z"));
    assert!(rows[0].closed_at.is_none());
}

// ── 학년·반 옵션 ─────────────────────────────────────────────────

#[tokio::test]
async fn grade_options_lists_only_enrolled_positions_deduplicated() {
    let pool = common::create_test_pool().await;
    for (g, c) in [(3i64, 2i64), (3, 1), (1, 1)] {
        common::insert_class(&pool, g, c).await;
    }
    // 3-2에 두 명 (seq_no는 학급 내 유일해야 하므로 다르게) → DISTINCT로 한 번만
    for (code, name, g, c, s) in [
        ("20250001", "홍길동", 3i64, 2i64, 1i64),
        ("20250002", "김영수", 3, 2, 2),
        ("20250003", "박민지", 3, 1, 1),
        ("20250004", "최지훈", 1, 1, 1),
    ] {
        sqlx::query(
            "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled)
             VALUES (?, ?, ?, ?, ?, 1)",
        )
        .bind(code).bind(name).bind(g).bind(c).bind(s)
        .execute(&pool)
        .await
        .unwrap();
    }
    // 졸업생은 학년·반이 NULL — 옵션에 끼어들면 안 된다
    sqlx::query(
        "INSERT INTO students (student_code, name, is_enrolled, grad_year)
         VALUES ('20240001', '이영희', 0, 2024)",
    )
    .execute(&pool)
    .await
    .unwrap();

    let axum::Json(opts) = grade_options(st(&pool)).await.unwrap();
    assert_eq!(opts.grades, vec![1, 3], "학년 오름차순");
    assert_eq!(opts.by_grade.get("3").unwrap(), &vec![1, 2], "3학년 반 오름차순·중복 제거");
    assert_eq!(opts.by_grade.get("1").unwrap(), &vec![1]);
    assert_eq!(opts.by_grade.len(), 2, "졸업생이 만든 학년 항목이 있으면 안 된다");
}

#[tokio::test]
async fn grade_options_empty_when_no_enrolled_students() {
    let pool = common::create_test_pool().await;
    sqlx::query(
        "INSERT INTO students (student_code, name, is_enrolled, grad_year)
         VALUES ('20240001', '이영희', 0, 2024)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let axum::Json(opts) = grade_options(st(&pool)).await.unwrap();
    assert!(opts.grades.is_empty());
    assert!(opts.by_grade.is_empty());
}

// ── 감사 기록 스냅샷 (audit::application_detail) ─────────────────

#[tokio::test]
async fn application_detail_snapshots_student_and_track_names() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    let sid: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled)
         VALUES ('20250001', '홍길동', 1, 1, 1, 1) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name) VALUES ('한국대') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name) VALUES (?, '컴퓨터공학') RETURNING id",
    )
    .bind(uid)
    .fetch_one(&pool)
    .await
    .unwrap();
    // 다른 대학의 동명 모집단위 — track_id가 아니라 이름으로 골라 오면 여기서 걸린다
    let other: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name) VALUES ('민국대') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO univ_tracks (univ_id, track_name) VALUES (?, '컴퓨터공학')")
        .bind(other)
        .execute(&pool)
        .await
        .unwrap();

    let mut conn = pool.acquire().await.unwrap();
    let detail = audit::application_detail(&mut conn, sid, tid).await.unwrap();
    assert_eq!(detail["student_code"], "20250001");
    assert_eq!(detail["student_name"], "홍길동");
    assert_eq!(detail["univ_name"], "한국대");
    assert_eq!(detail["track_name"], "컴퓨터공학");
}

#[tokio::test]
async fn application_detail_fails_fast_when_target_missing() {
    let pool = common::create_test_pool().await;
    let mut conn = pool.acquire().await.unwrap();
    // 스냅샷 대상이 없으면 빈 detail로 조용히 기록하지 않고 오류여야 한다
    let err = audit::application_detail(&mut conn, 1, 1).await.unwrap_err();
    assert_eq!(err.0, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(err.1.contains("감사 기록 실패"), "메시지: {}", err.1);
}

// ── 감사 기록 내보내기 ───────────────────────────────────────────

async fn log_admin(pool: &SqlitePool, action: AuditAction, round_id: Option<i64>) {
    let mut conn = pool.acquire().await.unwrap();
    audit::log(
        &mut conn,
        AuditEntry { actor: Actor::Admin, action, round_id, student_id: None, detail: serde_json::json!({"k": "v"}) },
    )
    .await
    .unwrap();
}

async fn log_teacher(pool: &SqlitePool, grade: i64, class_no: i64, action: AuditAction) {
    let mut conn = pool.acquire().await.unwrap();
    audit::log(
        &mut conn,
        AuditEntry {
            actor: Actor::Teacher { grade, class_no },
            action,
            round_id: None,
            student_id: None,
            detail: serde_json::json!({}),
        },
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn export_audit_logs_writes_columns_in_declared_order() {
    let pool = common::create_test_pool().await;
    sqlx::query("INSERT INTO classes (grade, class_no, teacher_name, password_hash) VALUES (2, 5, '박담임', 'x')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO classes (grade, class_no, teacher_name, password_hash) VALUES (0, 0, NULL, 'x')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO rounds (status, opened_at) VALUES ('OPEN', '2025-01-01T00:00:00Z')")
        .execute(&pool)
        .await
        .unwrap();

    log_admin(&pool, AuditAction::RoundOpened, Some(1)).await;
    log_teacher(&pool, 2, 5, AuditAction::ApplicationSaved).await;
    log_teacher(&pool, 0, 0, AuditAction::ApplicationSaved).await;

    let resp = export_audit_logs(st(&pool), Query(audit_q())).await.unwrap();
    let bytes = response_bytes(resp).await;
    let rows = excel::parse_xlsx_sheet_rows(&bytes, "감사기록").unwrap();
    let header = rows[0].clone();
    let (c_actor, c_action, c_round, c_detail) =
        (col_of(&header, "행위자"), col_of(&header, "행위"), col_of(&header, "라운드"), col_of(&header, "상세"));
    assert_eq!(col_of(&header, "시각"), 0, "시각이 첫 열이어야 함");

    // 최신순(id DESC): 졸업생 담당 → 2학년 5반 → 관리자
    assert_eq!(rows.len(), 4, "헤더 + 3행");
    assert_eq!(rows[1][c_actor], "졸업생 담당", "0학년 0반으로 표기하면 안 된다");
    assert_eq!(rows[2][c_actor], "2학년 5반 박담임");
    assert_eq!(rows[3][c_actor], "관리자");

    assert_eq!(rows[3][c_action], "ROUND_OPENED");
    assert_eq!(rows[3][c_round], "1", "라운드 열에 round_id가 들어가야 함");
    assert_eq!(rows[3][c_detail], r#"{"k":"v"}"#);
    // 관리자 행 외에는 라운드가 비어 있어야 한다
    assert!(rows[1].get(c_round).map(|s| s.is_empty()).unwrap_or(true));
    // 시각은 실제로 기록돼야 한다 (빈 칸이면 감사 기록의 의미가 없다)
    assert!(rows[1][0].contains('T') && rows[1][0].len() > 10, "시각: {:?}", rows[1][0]);
}

#[tokio::test]
async fn export_audit_logs_honours_filters() {
    let pool = common::create_test_pool().await;
    sqlx::query("INSERT INTO classes (grade, class_no, teacher_name, password_hash) VALUES (2, 5, '박담임', 'x')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO classes (grade, class_no, teacher_name, password_hash) VALUES (3, 1, '김담임', 'x')")
        .execute(&pool)
        .await
        .unwrap();
    // 진행 중 라운드는 전체에서 하나뿐(idx_one_active_round) — 1차는 확정, 2차만 열어 둔다
    sqlx::query(
        "INSERT INTO rounds (status, opened_at, closed_at, finalized_at)
         VALUES ('FINALIZED', '2025-01-01T00:00:00Z', '2025-02-01T00:00:00Z', '2025-02-02T00:00:00Z')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO rounds (status, opened_at) VALUES ('OPEN', '2025-03-01T00:00:00Z')")
        .execute(&pool)
        .await
        .unwrap();
    log_admin(&pool, AuditAction::RoundOpened, Some(1)).await;
    log_admin(&pool, AuditAction::RoundClosed, Some(2)).await;
    log_teacher(&pool, 2, 5, AuditAction::ApplicationSaved).await;
    log_teacher(&pool, 3, 1, AuditAction::ApplicationSaved).await;

    let data_rows = |bytes: &[u8]| -> usize {
        excel::parse_xlsx_sheet_rows(bytes, "감사기록").unwrap().len() - 1
    };

    let all = response_bytes(export_audit_logs(st(&pool), Query(audit_q())).await.unwrap()).await;
    assert_eq!(data_rows(&all), 4);

    let by_round = AuditQuery { round_id: Some(2), ..audit_q() };
    let b = response_bytes(export_audit_logs(st(&pool), Query(by_round)).await.unwrap()).await;
    assert_eq!(data_rows(&b), 1, "round_id 필터가 적용돼야 함");

    let by_action = AuditQuery { action: Some("APPLICATION_SAVED".into()), ..audit_q() };
    let b = response_bytes(export_audit_logs(st(&pool), Query(by_action)).await.unwrap()).await;
    assert_eq!(data_rows(&b), 2, "action 필터가 적용돼야 함");

    let by_class = AuditQuery { grade: Some(3), class_no: Some(1), ..audit_q() };
    let b = response_bytes(export_audit_logs(st(&pool), Query(by_class)).await.unwrap()).await;
    let rows = excel::parse_xlsx_sheet_rows(&b, "감사기록").unwrap();
    assert_eq!(rows.len() - 1, 1, "학급 필터가 적용돼야 함");
    let c_actor = col_of(&rows[0], "행위자");
    assert_eq!(rows[1][c_actor], "3학년 1반 김담임");
}

#[tokio::test]
async fn export_audit_logs_puts_ip_in_its_own_column() {
    let pool = common::create_test_pool().await;
    let mut conn = pool.acquire().await.unwrap();
    audit::log_with_ip(
        &mut conn,
        AuditEntry {
            actor: Actor::Admin,
            action: AuditAction::DbBackupDownloaded,
            round_id: None,
            student_id: None,
            detail: serde_json::json!({}),
        },
        Some("192.168.0.7".into()),
    )
    .await
    .unwrap();
    drop(conn);

    let bytes = response_bytes(export_audit_logs(st(&pool), Query(audit_q())).await.unwrap()).await;
    let rows = excel::parse_xlsx_sheet_rows(&bytes, "감사기록").unwrap();
    let c_ip = col_of(&rows[0], "IP");
    assert_eq!(rows[1][c_ip], "192.168.0.7");
    // IP는 마지막 열 — 기존 열 순서를 밀어내면 안 된다
    assert_eq!(c_ip, rows[0].len() - 1);
}

#[tokio::test]
async fn export_audit_logs_empty_still_writes_header() {
    let pool = common::create_test_pool().await;
    let bytes = response_bytes(export_audit_logs(st(&pool), Query(audit_q())).await.unwrap()).await;
    let rows = excel::parse_xlsx_sheet_rows(&bytes, "감사기록").unwrap();
    assert_eq!(rows.len(), 1, "데이터가 없어도 헤더는 있어야 함");
    col_of(&rows[0], "시각");
    col_of(&rows[0], "IP");
}

// ── 담임 확정 조회 ───────────────────────────────────────────────

async fn seed_round_and_class(pool: &SqlitePool) -> i64 {
    common::insert_class(pool, 1, 1).await;
    // 같은 학년 다른 반 — 학년만 비교하는 조회는 여기서 걸린다
    common::insert_class(pool, 1, 2).await;
    common::insert_class(pool, 2, 2).await;
    sqlx::query_scalar("INSERT INTO rounds (status, opened_at) VALUES ('OPEN', '2025-01-01T00:00:00Z') RETURNING id")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn teacher_get_confirmation_reports_unconfirmed_then_confirmed() {
    let pool = common::create_test_pool_shared().await;
    let rid = seed_round_and_class(&pool).await;

    let axum::Json(before) = teacher_get_confirmation(
        st(&pool),
        Extension(common::teacher_claims(1, 1)),
        Path(rid),
    )
    .await
    .unwrap();
    assert!(!before.confirmed);
    assert!(before.confirmed_at.is_none());

    teacher_confirm_round(st(&pool), Extension(common::teacher_claims(1, 1)), Path(rid))
        .await
        .unwrap();

    let axum::Json(after) = teacher_get_confirmation(
        st(&pool),
        Extension(common::teacher_claims(1, 1)),
        Path(rid),
    )
    .await
    .unwrap();
    assert!(after.confirmed);
    assert!(after.confirmed_at.is_some(), "확정 시각이 실려야 화면에 표시된다");
}

#[tokio::test]
async fn teacher_get_confirmation_is_scoped_to_own_class() {
    let pool = common::create_test_pool_shared().await;
    let rid = seed_round_and_class(&pool).await;
    teacher_confirm_round(st(&pool), Extension(common::teacher_claims(1, 1)), Path(rid))
        .await
        .unwrap();

    // 같은 학년 옆 반 담임 — 학년만 맞춰 보는 조회라면 여기서 남의 확정이 새어 나온다
    let axum::Json(same_grade) = teacher_get_confirmation(
        st(&pool),
        Extension(common::teacher_claims(1, 2)),
        Path(rid),
    )
    .await
    .unwrap();
    assert!(!same_grade.confirmed, "같은 학년 옆 반에 남의 확정이 새어 나왔다");
    assert!(same_grade.confirmed_at.is_none());

    // 학년·반이 모두 다른 담임
    let axum::Json(other) = teacher_get_confirmation(
        st(&pool),
        Extension(common::teacher_claims(2, 2)),
        Path(rid),
    )
    .await
    .unwrap();
    assert!(!other.confirmed, "다른 학급의 확정이 새어 나왔다");
}

#[tokio::test]
async fn teacher_get_confirmation_unknown_round_is_404() {
    let pool = common::create_test_pool_shared().await;
    seed_round_and_class(&pool).await;
    let err = teacher_get_confirmation(
        st(&pool),
        Extension(common::teacher_claims(1, 1)),
        Path(9999),
    )
    .await
    .unwrap_err();
    assert_eq!(err.0, StatusCode::NOT_FOUND, "없는 라운드를 '미확정'으로 답하면 안 된다");
}

// ── 유니브(석차연명부) 미리보기 ──────────────────────────────────

async fn file_multipart(bytes: &[u8]) -> Multipart {
    let boundary = "boundary42";
    let mut body: Vec<u8> = Vec::new();
    body.extend_from_slice(
        format!(
            "--{boundary}\r\n\
             Content-Disposition: form-data; name=\"file\"; filename=\"rank.xls\"\r\n\
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

async fn composite_area(pool: &SqlitePool) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO areas (name, max_score, calc_type, match_mode, lookup_scope)
         VALUES ('환산점수', 10000000, 'NUMERIC', 'UPPER', 'COMPOSITE') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn univ_preview_rejects_non_xls_file() {
    // BIFF(.xls) 바이너리는 테스트에서 생성할 수 없으므로 정상 경로는 미검증이다.
    // 대신 "다른 형식을 올렸을 때 조용히 빈 미리보기를 주지 않는다"를 고정한다.
    let pool = common::create_test_pool().await;
    let aid = composite_area(&pool).await;

    // xlsx(PK) 파일을 유니브 양식이라며 올린 경우
    let mut wb = rust_xlsxwriter::Workbook::new();
    wb.add_worksheet().write_string(0, 0, "학년").unwrap();
    let xlsx = wb.save_to_buffer().unwrap();
    let err = univ_preview(st(&pool), Path(aid), file_multipart(&xlsx).await)
        .await
        .err()
        .expect("거부되어야 함");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(err.1.contains(".xls"), "메시지: {}", err.1);

    // CSV를 올린 경우도 같은 거부
    let err = univ_preview(st(&pool), Path(aid), file_multipart(b"a,b\n1,2\n").await)
        .await
        .err()
        .expect("거부되어야 함");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn univ_preview_rejects_ole2_header_without_valid_workbook() {
    // OLE2 매직만 흉내 낸 손상 파일 — panic 없이 400이어야 하고,
    // "행 0건"으로 조용히 통과해서도 안 된다.
    let pool = common::create_test_pool().await;
    let aid = composite_area(&pool).await;
    let mut fake = b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1".to_vec();
    fake.extend_from_slice(&[0u8; 512]);
    let err = univ_preview(st(&pool), Path(aid), file_multipart(&fake).await)
        .await
        .err()
        .expect("거부되어야 함");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn univ_preview_unknown_area_is_404_before_reading_file() {
    let pool = common::create_test_pool().await;
    let err = univ_preview(st(&pool), Path(9999), file_multipart(b"anything").await)
        .await
        .err()
        .expect("거부되어야 함");
    assert_eq!(err.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn univ_preview_requires_a_file() {
    let pool = common::create_test_pool().await;
    let aid = composite_area(&pool).await;
    let boundary = "boundary42";
    let req = Request::builder()
        .method("POST")
        .header("content-type", format!("multipart/form-data; boundary={boundary}"))
        .body(Body::from(format!("--{boundary}--\r\n")))
        .unwrap();
    let mp = Multipart::from_request(req, &()).await.unwrap();
    let err = univ_preview(st(&pool), Path(aid), mp).await.err().expect("거부되어야 함");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert!(err.1.contains("파일이 없습니다"), "메시지: {}", err.1);
}

// ── 버전 ─────────────────────────────────────────────────────────

#[tokio::test]
async fn get_version_reports_the_built_package_version() {
    let axum::Json(v) = get_version().await;
    assert_eq!(v.version, env!("CARGO_PKG_VERSION"));
    // "0.0.0" 같은 자리표시자를 반환하고 있지는 않은지 (업데이트 안내가 이 값에 의존)
    let parts: Vec<&str> = v.version.split('.').collect();
    assert_eq!(parts.len(), 3, "semver 3자리여야 함: {}", v.version);
    assert!(parts.iter().all(|p| p.parse::<u32>().is_ok()), "숫자가 아님: {}", v.version);
    assert_ne!(v.version, "0.0.0");
}
