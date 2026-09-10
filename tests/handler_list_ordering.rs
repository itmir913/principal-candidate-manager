//! 목록 정렬 회귀 테스트.
//!
//! 이 저장소의 목록 쿼리는 오랫동안 `ORDER BY r.track_id` / `ut.id` 처럼 **등록 순번**으로
//! 정렬했다. 화면에서는 순서가 뒤죽박죽으로 보이고(대학이 가나다순이 아니다), 동률 tiebreak
//! 이 없는 자리는 실행마다 순서가 달라질 수 있었다. 특히 졸업생은 grade/class_no/seq_no 가
//! 전부 NULL 이라, 그 세 열로만 정렬하는 **페이지네이션** 쿼리는 같은 학생을 두 페이지에
//! 보여 주거나 아예 빠뜨릴 수 있었다.
//!
//! 픽스처가 중요하다: 대학을 **가나다 역순으로 등록**한다(track_id 순 = 가나다 역순).
//! 등록순 정렬이 남아 있으면 이 테스트는 반드시 실패한다.

mod common;

use axum::extract::{Path, Query, State};
use principal_candidate_manager::handlers::scoring::{
    fetch_teacher_results, get_results, ResultQuery,
};
use principal_candidate_manager::handlers::students::{list_students, ListQuery};
use sqlx::SqlitePool;

/// 대학 3곳을 가나다 **역순**으로 등록한다 → id 1=한양, 2=서울, 3=고려.
/// 각 대학에 모집단위 2개를 역순으로 둔다.
async fn seed_univs(pool: &SqlitePool) {
    for (uid, name) in [(1i64, "한양대학교"), (2, "서울대학교"), (3, "고려대학교")] {
        sqlx::query(
            "INSERT INTO universities (id, univ_name, total_quota, prioritize_enrolled) VALUES (?, ?, NULL, 0)",
        )
        .bind(uid).bind(name).execute(pool).await.unwrap();
        // 모집단위도 역순 등록: (uid*10+1)=나학과, (uid*10+2)=가학과
        for (off, tname) in [(1i64, "나학과"), (2, "가학과")] {
            sqlx::query(
                "INSERT INTO univ_tracks (id, univ_id, track_name, unit_quota, prioritize_enrolled) \
                 VALUES (?, ?, ?, NULL, 0)",
            )
            .bind(uid * 10 + off).bind(uid).bind(tname).execute(pool).await.unwrap();
        }
    }
}

async fn seed_round(pool: &SqlitePool, status: &str) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at, closed_at, finalized_at) VALUES (?, '2026-01-01T00:00:00Z', '2026-01-02T00:00:00Z', '2026-01-03T00:00:00Z') RETURNING id",
    )
    .bind(status).fetch_one(pool).await.unwrap()
}

async fn seed_student(pool: &SqlitePool, code: &str, name: &str, seq: i64) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES (?, ?, 3, 1, ?, 1) RETURNING id",
    )
    .bind(code).bind(name).bind(seq).fetch_one(pool).await.unwrap()
}

async fn apply_and_score(pool: &SqlitePool, sid: i64, tid: i64, rid: i64, score: i64, ranking: i64) {
    sqlx::query(
        "INSERT INTO applications (student_id, track_id, round_id, department_name) VALUES (?, ?, ?, '학과')",
    )
    .bind(sid).bind(tid).bind(rid).execute(pool).await.unwrap();
    sqlx::query(
        "INSERT INTO results (student_id, track_id, round_id, score_detail, total_score, ranking, recommended, calculated_at) \
         VALUES (?, ?, ?, '{}', ?, ?, 0, '2026-01-02T01:00:00Z')",
    )
    .bind(sid).bind(tid).bind(rid).bind(score).bind(ranking).execute(pool).await.unwrap();
}

async fn seed_class(pool: &SqlitePool) {
    sqlx::query("INSERT INTO classes (grade, class_no, teacher_name, password_hash) VALUES (3, 1, '김담임', 'x')")
        .execute(pool).await.unwrap();
}

// ── 1. 관리자 [결과] 탭 ───────────────────────────────────────────────

#[tokio::test]
async fn get_results_orders_by_univ_name_then_track_name() {
    let pool = common::create_test_pool().await;
    seed_class(&pool).await;
    seed_univs(&pool).await;
    let rid = seed_round(&pool, "CLOSED").await;

    // 대학 3 × 모집단위 2 에 학생 1명씩
    let mut n = 0i64;
    for uid in [1i64, 2, 3] {
        for off in [1i64, 2] {
            n += 1;
            let sid = seed_student(&pool, &format!("2026{n:04}"), "홍길동", n).await;
            apply_and_score(&pool, sid, uid * 10 + off, rid, 1_000_00, 1).await;
        }
    }

    let rows = get_results(
        State(common::make_state(pool.clone())),
        Path(rid),
        Query(ResultQuery { track_id: None }),
    ).await.expect("조회 성공").0;

    let got: Vec<String> = rows.iter().map(|r| format!("{} {}", r.univ_name, r.track_name)).collect();
    assert_eq!(
        got,
        vec![
            "고려대학교 가학과", "고려대학교 나학과",
            "서울대학교 가학과", "서울대학교 나학과",
            "한양대학교 가학과", "한양대학교 나학과",
        ],
        "대학·모집단위 가나다순이어야 한다 (등록순 track_id 정렬이면 역순으로 나온다)"
    );
}

#[tokio::test]
async fn get_results_breaks_ties_by_student_code() {
    let pool = common::create_test_pool().await;
    seed_class(&pool).await;
    seed_univs(&pool).await;
    let rid = seed_round(&pool, "CLOSED").await;

    // 같은 모집단위, 같은 점수·같은 ranking → tiebreak 이 없으면 순서가 비결정적이다.
    // 학번을 등록 역순으로 넣어 rowid 순서와 학번 순서를 어긋나게 만든다.
    for (i, code) in ["20260003", "20260001", "20260002"].iter().enumerate() {
        let sid = seed_student(&pool, code, "홍길동", (i + 1) as i64).await;
        apply_and_score(&pool, sid, 11, rid, 5_000_00, 1).await;
    }

    let rows = get_results(
        State(common::make_state(pool.clone())),
        Path(rid),
        Query(ResultQuery { track_id: None }),
    ).await.expect("조회 성공").0;

    let codes: Vec<&str> = rows.iter().map(|r| r.student_code.as_str()).collect();
    assert_eq!(codes, vec!["20260001", "20260002", "20260003"], "동점 행은 학번순으로 고정되어야 한다");
}

// ── 2. 담임 [라운드 결과] ─────────────────────────────────────────────

#[tokio::test]
async fn teacher_results_order_each_student_by_univ_name() {
    let pool = common::create_test_pool().await;
    seed_class(&pool).await;
    seed_univs(&pool).await;
    let rid = seed_round(&pool, "FINALIZED").await;

    // 한 학생이 세 대학에 지원(등록 역순 track_id) → 학생 안에서 가나다순이어야 한다
    let sid = seed_student(&pool, "20260001", "홍길동", 1).await;
    for uid in [1i64, 2, 3] {
        apply_and_score(&pool, sid, uid * 10 + 1, rid, 1_000_00, 1).await;
    }

    let rows = fetch_teacher_results(&pool, &common::teacher_claims(3, 1), None)
        .await.expect("조회 성공");

    let names: Vec<&str> = rows.iter().map(|r| r.univ_name.as_str()).collect();
    assert_eq!(names, vec!["고려대학교", "서울대학교", "한양대학교"]);
}

// ── 3. 학생 목록 페이지네이션 ────────────────────────────────────────

#[tokio::test]
async fn graduated_student_pagination_has_no_gap_or_duplicate() {
    let pool = common::create_test_pool().await;

    // 졸업생은 grade/class_no/seq_no 가 모두 NULL 이다. 이 세 열로만 정렬하면
    // 전 행이 동률이 되어 LIMIT/OFFSET 페이지 경계가 흔들린다.
    for i in 1..=10 {
        sqlx::query(
            "INSERT INTO students (student_code, name, is_enrolled, grad_year) VALUES (?, '졸업생', 0, 2025)",
        )
        .bind(format!("2025{:04}", 11 - i)) // 학번을 등록 역순으로
        .execute(&pool).await.unwrap();
    }

    let state = common::make_state(pool.clone());
    let mut seen: Vec<String> = Vec::new();
    for page in 1..=3 {
        let p = list_students(
            State(state.clone()),
            Query(ListQuery { grade: None, class_no: None, is_enrolled: Some(0), page, per_page: 4 }),
        ).await.expect("조회 성공").0;
        assert_eq!(p.total, 10);
        seen.extend(p.rows.into_iter().map(|r| r.student_code));
    }

    let mut sorted = seen.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), 10, "페이지 사이에 중복·누락이 없어야 한다: {seen:?}");
    assert_eq!(seen, sorted, "졸업생 목록은 학번순으로 고정되어야 한다");
}
