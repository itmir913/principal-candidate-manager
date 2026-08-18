//! 감사 재현 테스트 — **의도적으로 실패한다.**
//!
//! 지점: 2-77 / U-05 / M-13
//! `export_quota_stats`(universities.rs:688-690)의 라운드 열 라벨은
//! `format!("{}차 추천", i + 1)` — `stats.all_round_ids` 배열의 **인덱스** 기반이다.
//! 그런데 `all_round_ids`(universities.rs:590-593)는
//! `results JOIN applications ... WHERE recommended = 1 AND abandoned = 0` 의 결과에서
//! 뽑히므로 **추천 확정이 0건인 라운드는 배열에서 빠진다.**
//!
//! 따라서 1차에서 확정이 0건(전원 미선발)이면 2차의 추천 수가 "1차 추천" 열에 적힌다.
//! 엑셀을 받은 사람은 2차에 확정된 학생을 1차 확정으로 읽는다.

mod common;

use axum::extract::{Query, State};
use principal_candidate_manager::handlers::universities::{export_quota_stats, ExportQuotaQuery};

#[tokio::test]
async fn round_column_label_matches_actual_round_id() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 3, 1).await;

    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name, total_quota, prioritize_enrolled)
         VALUES ('한국대', 5, 0) RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name, unit_quota, prioritize_enrolled)
         VALUES (?, '컴공', 5, 0) RETURNING id",
    )
    .bind(uid).fetch_one(&pool).await.unwrap();

    let s1: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled)
         VALUES ('E001', '1차지원자', 3, 1, 1, 1) RETURNING id",
    ).fetch_one(&pool).await.unwrap();
    let s2: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled)
         VALUES ('E002', '2차지원자', 3, 1, 2, 1) RETURNING id",
    ).fetch_one(&pool).await.unwrap();

    // 1차 라운드: 지원자 1명, 전원 미선발 → 추천 확정 0건 → FINALIZED
    sqlx::query("INSERT INTO rounds (id, status, opened_at, closed_at, finalized_at)
                 VALUES (1, 'FINALIZED', 'now', 'now', 'now')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO applications (student_id, track_id, round_id, excluded, excluded_reason)
                 VALUES (?, ?, 1, 1, '서류 미비')")
        .bind(s1).bind(tid).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO results (student_id, track_id, round_id, total_score, ranking, recommended, calculated_at)
                 VALUES (?, ?, 1, 1000000, 1, 0, 'now')")
        .bind(s1).bind(tid).execute(&pool).await.unwrap();

    // 2차 라운드: 지원자 1명, 추천 확정 1건
    sqlx::query("INSERT INTO rounds (id, status, opened_at, closed_at)
                 VALUES (2, 'CLOSED', 'now', 'now')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO applications (student_id, track_id, round_id) VALUES (?, ?, 2)")
        .bind(s2).bind(tid).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO results (student_id, track_id, round_id, total_score, ranking, recommended, calculated_at)
                 VALUES (?, ?, 2, 2000000, 1, 1, 'now')")
        .bind(s2).bind(tid).execute(&pool).await.unwrap();

    let resp = export_quota_stats(
        State(common::make_state(pool.clone())),
        Query(ExportQuotaQuery { univ_id: None }),
    )
    .await
    .unwrap();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap().to_vec();
    let rows = principal_candidate_manager::excel::parse_xlsx_sheet_rows(&bytes, "정원 현황").unwrap();

    let header = &rows[0];
    let round_cols: Vec<&String> = header.iter().filter(|h| h.ends_with("차 추천")).collect();

    assert_eq!(round_cols.len(), 1, "추천이 있는 라운드는 2차 하나뿐이므로 라운드 열도 1개");
    // 그 열의 값은 round_id=2 의 추천 수(1명)다. 라벨도 "2차 추천"이어야 한다.
    let value_col = header.iter().position(|h| h.ends_with("차 추천")).unwrap();
    assert_eq!(rows[1][value_col], "1", "그 열의 값은 2차에서 확정된 1명");
    assert_eq!(
        round_cols[0], "2차 추천",
        "라운드 열 라벨은 실제 round_id 를 따라야 한다 (현재는 배열 인덱스 i+1 이라 '1차 추천'이 된다)"
    );
}
