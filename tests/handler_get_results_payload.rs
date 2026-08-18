//! 수정 검증 라운드 — `1db8ff0`(F-014) 이후 `get_results` 응답 크기 실측.
//!
//! 프론트가 라운드 전체를 받도록 바뀌었으므로 "전체 조회 응답이 얼마나 큰가"를
//! 추측이 아니라 실측으로 남긴다. 전형요소 10개(score_detail 키 10개)·재학생·
//! 한글 이름·대학/모집단위/학과명이 있는 현실적인 행을 만들어 직렬화 바이트를 잰다.

mod common;

use axum::extract::{Path, Query, State};
use principal_candidate_manager::handlers::scoring::{get_results, ResultQuery};

#[tokio::test]
async fn measure_get_results_payload_per_row() {
    let pool = common::create_test_pool().await;
    let n_areas = 10usize;
    let n_rows = 200i64;

    sqlx::query("INSERT INTO classes (grade, class_no, teacher_name, password_hash) VALUES (3, 1, '김담임', 'x')")
        .execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO universities (id, univ_name, total_quota, prioritize_enrolled) \
         VALUES (1, '서울대학교', 5, 1)",
    ).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO univ_tracks (id, univ_id, track_name, unit_quota, prioritize_enrolled) \
         VALUES (1, 1, '지역균형선발전형', 3, 1)",
    ).execute(&pool).await.unwrap();
    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at, closed_at) \
         VALUES ('CLOSED', '2026-01-01T00:00:00Z', '2026-01-02T00:00:00Z') RETURNING id",
    ).fetch_one(&pool).await.unwrap();

    // score_detail: {"1":1234500,...} 전형요소 10개
    let detail: String = {
        let body: Vec<String> = (1..=n_areas).map(|i| format!("\"{i}\":1234500")).collect();
        format!("{{{}}}", body.join(","))
    };

    for i in 1..=n_rows {
        let sid: i64 = sqlx::query_scalar(
            "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
             VALUES (?, '홍길동', 3, 1, ?, 1) RETURNING id",
        ).bind(format!("2026{i:04}")).bind(i).fetch_one(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO applications (student_id, track_id, round_id, department_name) \
             VALUES (?, 1, ?, '컴퓨터공학부')",
        ).bind(sid).bind(rid).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO results (student_id, track_id, round_id, score_detail, total_score, ranking, recommended, calculated_at) \
             VALUES (?, 1, ?, ?, 12345000, ?, 0, '2026-01-02T01:00:00Z')",
        ).bind(sid).bind(rid).bind(&detail).bind(i).execute(&pool).await.unwrap();
    }

    let rows = get_results(
        State(common::make_state(pool.clone())),
        Path(rid),
        Query(ResultQuery { track_id: None }),
    ).await.expect("조회 성공").0;
    assert_eq!(rows.len() as i64, n_rows);

    let json = serde_json::to_string(&rows).unwrap();
    let per_row = json.len() as f64 / n_rows as f64;
    println!(
        "PAYLOAD: 행 {} 개 / 전형요소 {} 개 → 총 {} bytes / 행당 {:.1} bytes",
        n_rows, n_areas, json.len(), per_row
    );
    for n in [500i64, 1000, 3000, 10000] {
        println!("PAYLOAD: 라운드 {n} 행 추정 = {:.0} KB", per_row * n as f64 / 1024.0);
    }
    // 회귀 감지용 느슨한 상한 — 행당 1 KB 를 넘으면 응답 계약이 크게 바뀐 것이다.
    assert!(per_row < 1024.0, "행당 {per_row:.1} bytes — 응답이 예상보다 크다");
}
