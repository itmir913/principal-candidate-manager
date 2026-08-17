//! 감사 산출물 — 경계값 불변식 (proptest 아님, 결정적 경계 케이스).
//!
//! 인벤토리 U-39(`decide_group` 직접 단위 테스트 0건) / U-40 / U-41(음수 잔여) /
//! U-32(만점 경계) 대응. 각 테스트 첫 줄 주석 = 보장하는 불변식 (E2 요건).

mod common;

use axum::extract::{Query, State};
use principal_candidate_manager::handlers::scoring::{
    decide_group, fill_by_rank_groups, merge_univ_cut, run_calculate_scores_on_conn, GroupStep,
    MergeCand,
};
use principal_candidate_manager::handlers::universities::{export_quota_stats, ExportQuotaQuery};

// ── decide_group 직접 단위 테스트 (U-39) ─────────────────────────

/// 불변식: `decide_group` 은 (확정수 + 그룹크기 <= 잔여) → Take,
/// 아니면 free = 잔여 - 확정수 가 0 이하면 StopClean, 0 초과면 StopTie{free}.
/// **잔여가 음수여도 StopClean** — 아무도 확정하지 않는다(fail-closed).
#[test]
fn decide_group_boundary_matrix() {
    // 정확히 딱 맞음 → Take
    assert_eq!(decide_group(0, 3, 3), GroupStep::Take);
    assert_eq!(decide_group(2, 1, 3), GroupStep::Take);
    // 그룹 사이에서 정원이 깨끗하게 끝남 → StopClean
    assert_eq!(decide_group(3, 1, 3), GroupStep::StopClean);
    assert_eq!(decide_group(3, 5, 3), GroupStep::StopClean);
    // 동점이 정원 경계를 가름 → StopTie
    assert_eq!(decide_group(0, 2, 1), GroupStep::StopTie { free: 1 });
    assert_eq!(decide_group(1, 3, 2), GroupStep::StopTie { free: 1 });
    // 잔여 0 / 음수 (정원 축소 후 used > quota) → 확정 없음
    assert_eq!(decide_group(0, 1, 0), GroupStep::StopClean);
    assert_eq!(decide_group(0, 1, -1), GroupStep::StopClean);
    assert_eq!(decide_group(0, 7, -42), GroupStep::StopClean);
    // 그룹 크기 0 은 호출자가 만들지 않지만, 만들어도 Take 로 무해
    assert_eq!(decide_group(0, 0, 0), GroupStep::Take);
}

/// 불변식: 음수 잔여(U-40 트랙 / U-41 대학)가 채움 함수에 들어와도
/// 확정은 0건이고 수동(tie) 보고도 생기지 않는다 — 정원 축소가 추천을 만들어내지 않는다.
#[test]
fn negative_remaining_confirms_nobody() {
    let items: Vec<(i64, i64)> = vec![(1, 10), (2, 20), (3, 30)];
    for rem in [-1i64, -5, -100] {
        let out = fill_by_rank_groups(&items, Some(rem));
        assert!(out.confirmed.is_empty(), "잔여 {rem} 에서 확정이 생기면 안 된다");
        assert!(out.tie.is_none(), "잔여 {rem} 는 동점 경계가 아니다");
    }

    let pool = vec![vec![
        MergeCand { student_id: 1, track_id: 1, track_rank: 1, univ_rank: 1 },
        MergeCand { student_id: 2, track_id: 1, track_rank: 2, univ_rank: 2 },
    ]];
    for rem in [-1i64, -9] {
        let out = merge_univ_cut(&pool, Some(rem));
        assert!(out.confirmed.is_empty(), "대학 잔여 {rem} 에서 확정이 생기면 안 된다");
        assert!(out.tie.is_none());
    }
}

// ── max_score 경계 (U-32 / M-14) ────────────────────────────────

async fn setup_single_manual_area(pool: &sqlx::SqlitePool, max_score: i64, value: i64) -> i64 {
    common::insert_class(pool, 3, 1).await;
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name, total_quota, prioritize_enrolled)
         VALUES ('한국대', NULL, 0) RETURNING id",
    )
    .fetch_one(pool).await.unwrap();
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name, unit_quota, prioritize_enrolled)
         VALUES (?, '컴공', NULL, 0) RETURNING id",
    )
    .bind(uid).fetch_one(pool).await.unwrap();
    let aid: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, calc_type, max_score, lookup_scope, multi_value)
         VALUES ('면접', 'MANUAL', ?, 'SIMPLE', 0) RETURNING id",
    )
    .bind(max_score).fetch_one(pool).await.unwrap();
    let sid: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled)
         VALUES ('E001', '학생', 3, 1, 1, 1) RETURNING id",
    )
    .fetch_one(pool).await.unwrap();
    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at) VALUES ('OPEN', 'now') RETURNING id",
    )
    .fetch_one(pool).await.unwrap();

    sqlx::query("INSERT INTO base_data (student_id, area_id, track_id, value, multi_value) VALUES (?, ?, NULL, ?, 0)")
        .bind(sid).bind(aid).bind(value.to_string()).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO applications (student_id, track_id, round_id) VALUES (?, ?, ?)")
        .bind(sid).bind(tid).bind(rid).execute(pool).await.unwrap();

    let mut conn = pool.acquire().await.unwrap();
    run_calculate_scores_on_conn(&mut conn, rid, "2026-08-17T00:00:00Z").await.unwrap();
    drop(conn);

    sqlx::query_scalar("SELECT total_score FROM results WHERE round_id = ?")
        .bind(rid).fetch_one(pool).await.unwrap()
}

/// 불변식: MANUAL 값이 만점과 정확히 같으면 그대로, 초과하면 만점으로 절단,
/// 만점이 0 이면 총점은 0 이하 — 어떤 경우에도 total_score <= Σ max_score.
#[tokio::test]
async fn max_score_boundary_caps_exactly() {
    for (max_score, value, want) in [
        (1_000_000i64, 1_000_000i64, 1_000_000i64), // 정확히 만점
        (1_000_000, 999_999, 999_999),              // 만점 직전
        (1_000_000, 1_000_001, 1_000_000),          // 만점 +1 → 절단
        (0, 5_000_000, 0),                          // 만점 0 → 0
        (1_000_000, -300_000, -300_000),            // 감점(하한 없음)
    ] {
        let pool = common::create_test_pool().await;
        let total = setup_single_manual_area(&pool, max_score, value).await;
        assert_eq!(total, want, "max_score={max_score} value={value}");
        assert!(total <= max_score, "총점 {total} 이 Σ 만점 {max_score} 를 넘었다");
        pool.close().await;
    }
}

// ── 음수 정원이 엑셀 산출물에 그대로 기록된다 (U-29 / U-30 후속) ──

/// 불변식(현행 동작 고정): JSON API 로 저장된 음수 정원은 거부되지 않고
/// 엑셀 "모집단위 정원"·"대학 전체 정원" 열에 **음수 그대로** 기록되며,
/// 잔여 열만 `.max(0)` 으로 0 이 된다. 입력 관문이 생기면 이 테스트가 깨져야 한다.
#[tokio::test]
async fn negative_quota_is_written_verbatim_into_excel() {
    let pool = common::create_test_pool().await;
    sqlx::query("INSERT INTO universities (id, univ_name, total_quota, prioritize_enrolled) VALUES (1, '한국대', -2, 0)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO univ_tracks (id, univ_id, track_name, unit_quota, prioritize_enrolled) VALUES (1, 1, '컴공', -3, 0)")
        .execute(&pool).await.unwrap();

    let resp = export_quota_stats(
        State(common::make_state(pool.clone())),
        Query(ExportQuotaQuery { univ_id: None }),
    )
    .await
    .unwrap();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap().to_vec();
    let rows = principal_candidate_manager::excel::parse_xlsx_sheet_rows(&bytes, "정원 현황").unwrap();

    // 헤더: 대학명, 모집단위, 모집단위 정원, 지원 인원, 추천인원, 포기 인원, 잔여인원,
    //       대학 전체 정원, 대학 추천인원, 대학 잔여인원
    assert_eq!(rows[1][2], "-3", "모집단위 정원이 음수 그대로 기록된다");
    assert_eq!(rows[1][6], "0", "잔여인원은 .max(0) 으로 0");
    assert_eq!(rows[1][7], "-2", "대학 전체 정원이 음수 그대로 기록된다");
    assert_eq!(rows[1][9], "0", "대학 잔여인원은 .max(0) 으로 0");
}

// ── CLOSED 라운드에서 base_data UPSERT 후 results 가 낡는다 ───────

/// 불변식(현행 동작 고정): CLOSED 라운드 지원자의 base_data 는
/// `INSERT OR REPLACE` 로 갱신 가능하고(트리거는 명시 DELETE 만 차단),
/// 그때 `results.total_score` 는 **자동 갱신되지 않는다**.
/// 즉 재계산 전까지 화면·엑셀 총점은 현재 기초데이터와 불일치한다.
#[tokio::test]
async fn base_data_upsert_in_closed_round_leaves_results_stale() {
    let pool = common::create_test_pool().await;
    let total_before = setup_single_manual_area(&pool, 10_000_000, 5_000_000).await;
    assert_eq!(total_before, 5_000_000);

    // 라운드를 CLOSED 로 전환
    sqlx::query("UPDATE rounds SET status = 'CLOSED', closed_at = 'now'")
        .execute(&pool).await.unwrap();

    // CLOSED 인데도 기초데이터 UPSERT 는 통과한다
    sqlx::query(
        "INSERT OR REPLACE INTO base_data (student_id, area_id, track_id, value, multi_value)
         SELECT student_id, area_id, track_id, '9000000', multi_value FROM base_data",
    )
    .execute(&pool).await.unwrap();

    let total_after: i64 = sqlx::query_scalar("SELECT total_score FROM results")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(
        total_after, 5_000_000,
        "results 는 갱신되지 않는다 — 화면·엑셀은 재계산 전까지 낡은 총점을 보여준다"
    );
    let base_now: String = sqlx::query_scalar("SELECT value FROM base_data").fetch_one(&pool).await.unwrap();
    assert_eq!(base_now, "9000000", "기초데이터는 이미 바뀌어 있다");
}
