//! 상태기계 × 쓰기 엔드포인트 매트릭스 테스트.
//!
//! src/docs/11_state_matrix.md의 표를 실행으로 고정한다.
//! 한 케이스 = 표의 한 셀. 거부 셀은 상태코드에 더해
//! rounds·applications·results 세 테이블의 완전 불변까지 단언한다.

mod common;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use principal_candidate_manager::handlers::{
    applications::{
        abandon_application, teacher_abandon_application, teacher_create_application,
        teacher_delete_application, CreateApplicationBody,
    },
    rounds::{close_round, finalize_round, open_round, reopen_round},
    scoring::{calculate_scores, recommend_result, unrecommend_result},
};
use sqlx::SqlitePool;

// ── 픽스처 ────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Debug)]
enum RState {
    /// open_round: rounds 테이블 비어 있음 / 그 외: 존재하지 않는 라운드 id(9999)
    Missing,
    Open,
    Closed,
    Finalized,
}

const ALL_STATES: [RState; 4] = [RState::Missing, RState::Open, RState::Closed, RState::Finalized];

struct Fx {
    sid: i64,
    tid: i64,
    rid: i64,
}

/// 완전한 지원 픽스처: 학급·학생·대학·모집단위·전형요소(NUMERIC UPPER SIMPLE)·
/// 점수 기준·기초데이터까지 전부 갖춰 close_round(점수 계산 포함)가 성공 가능한 상태.
/// 라운드는 요청된 상태로 1개만 삽입한다 (idx_one_active_round 준수).
/// CLOSED/FINALIZED에는 results 행(recommended=0)을 함께 넣어 recommend 계열이 동작한다.
async fn setup(pool: &SqlitePool, state: RState) -> Fx {
    common::insert_class(pool, 1, 1).await;
    let sid: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES ('S001', '홍길동', 1, 1, 1, 1) RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let univ_id: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name) VALUES ('한국대') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name) VALUES (?, '컴공') RETURNING id",
    )
    .bind(univ_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let area_id: i64 = sqlx::query_scalar(
        "INSERT INTO areas (name, calc_type, max_score, match_mode, lookup_scope, teacher_editable) \
         VALUES ('내신', 'NUMERIC', 10000000, 'UPPER', 'SIMPLE', 0) RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO numeric_table (area_id, track_id, threshold, score) VALUES (?, NULL, 0, 500000)",
    )
    .bind(area_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO base_data (student_id, area_id, track_id, value, multi_value) \
         VALUES (?, ?, NULL, '100000', 0)",
    )
    .bind(sid)
    .bind(area_id)
    .execute(pool)
    .await
    .unwrap();

    if state == RState::Missing {
        return Fx { sid, tid, rid: 9999 };
    }

    let rid: i64 = match state {
        RState::Open => sqlx::query_scalar(
            "INSERT INTO rounds (status, opened_at) \
             VALUES ('OPEN', '2025-01-01T00:00:00Z') RETURNING id",
        ),
        RState::Closed => sqlx::query_scalar(
            "INSERT INTO rounds (status, opened_at, closed_at) \
             VALUES ('CLOSED', '2025-01-01T00:00:00Z', '2025-01-02T00:00:00Z') RETURNING id",
        ),
        RState::Finalized => sqlx::query_scalar(
            "INSERT INTO rounds (status, opened_at, closed_at, finalized_at) \
             VALUES ('FINALIZED', '2025-01-01T00:00:00Z', '2025-01-02T00:00:00Z', \
                     '2025-01-03T00:00:00Z') RETURNING id",
        ),
        RState::Missing => unreachable!(),
    }
    .fetch_one(pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO applications (student_id, track_id, round_id, confirmed, abandoned, department_name) \
         VALUES (?, ?, ?, 1, 0, '컴퓨터공학과')",
    )
    .bind(sid)
    .bind(tid)
    .bind(rid)
    .execute(pool)
    .await
    .unwrap();

    if state == RState::Closed || state == RState::Finalized {
        sqlx::query(
            "INSERT INTO results \
             (student_id, track_id, round_id, score_detail, total_score, ranking, recommended, calculated_at) \
             VALUES (?, ?, ?, ?, 500000, 1, 0, '2025-01-02T00:00:00Z')",
        )
        .bind(sid)
        .bind(tid)
        .bind(rid)
        .bind(format!("{{\"{}\":500000}}", area_id))
        .execute(pool)
        .await
        .unwrap();
    }

    Fx { sid, tid, rid }
}

// ── 스냅샷 (거부 셀 불변 단언용) ──────────────────────────────────

type RoundsSnap = Vec<(i64, String, String, Option<String>, Option<String>)>;
type AppsSnap = Vec<(i64, i64, i64, i64, i64, String)>;
type ResultsSnap = Vec<(i64, i64, i64, String, i64, Option<i64>, i64)>;

async fn snapshot(pool: &SqlitePool) -> (RoundsSnap, AppsSnap, ResultsSnap) {
    let rounds: RoundsSnap = sqlx::query_as(
        "SELECT id, status, opened_at, closed_at, finalized_at FROM rounds ORDER BY id",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    let apps: AppsSnap = sqlx::query_as(
        "SELECT student_id, track_id, round_id, confirmed, abandoned, department_name \
         FROM applications ORDER BY student_id, track_id, round_id",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    let results: ResultsSnap = sqlx::query_as(
        "SELECT student_id, track_id, round_id, score_detail, total_score, ranking, recommended \
         FROM results ORDER BY student_id, track_id, round_id",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    (rounds, apps, results)
}

// ── 엔드포인트 디스패치 ───────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
enum Ep {
    OpenRound,
    CloseRound,
    ReopenRound,
    FinalizeRound,
    Calculate,
    Recommend,
    Unrecommend,
    AdminAbandon,
    TeacherCreate,
    TeacherDelete,
    TeacherAbandon,
}

async fn call(ep: Ep, pool: &SqlitePool, fx: &Fx) -> StatusCode {
    let st = common::make_state(pool.clone());
    match ep {
        Ep::OpenRound => match open_round(State(st)).await {
            Ok((s, _)) => s,
            Err((s, _)) => s,
        },
        Ep::CloseRound => match close_round(State(st), Path(fx.rid)).await {
            Ok(_) => StatusCode::OK,
            Err((s, _)) => s,
        },
        Ep::ReopenRound => match reopen_round(State(st), Path(fx.rid)).await {
            Ok(s) => s,
            Err((s, _)) => s,
        },
        Ep::FinalizeRound => match finalize_round(State(st), Path(fx.rid)).await {
            Ok(s) => s,
            Err((s, _)) => s,
        },
        Ep::Calculate => match calculate_scores(State(st), Path(fx.rid)).await {
            Ok(_) => StatusCode::OK,
            Err((s, _)) => s,
        },
        Ep::Recommend => match recommend_result(State(st), Path((fx.sid, fx.tid, fx.rid))).await {
            Ok(s) => s,
            Err((s, _)) => s,
        },
        Ep::Unrecommend => {
            match unrecommend_result(State(st), Path((fx.sid, fx.tid, fx.rid))).await {
                Ok(s) => s,
                Err((s, _)) => s,
            }
        }
        Ep::AdminAbandon => {
            match abandon_application(State(st), Path((fx.sid, fx.tid, fx.rid))).await {
                Ok(s) => s,
                Err((s, _)) => s,
            }
        }
        Ep::TeacherCreate => {
            let body = CreateApplicationBody {
                student_id: fx.sid,
                track_id: fx.tid,
                round_id: fx.rid,
                department_name: "컴퓨터공학과".into(),
                base_data_entries: vec![],
            };
            match teacher_create_application(
                State(st),
                Extension(common::teacher_claims(1, 1)),
                Json(body),
            )
            .await
            {
                Ok(s) => s,
                Err((s, _)) => s,
            }
        }
        Ep::TeacherDelete => {
            match teacher_delete_application(
                State(st),
                Extension(common::teacher_claims(1, 1)),
                Path((fx.sid, fx.tid, fx.rid)),
            )
            .await
            {
                Ok(s) => s,
                Err((s, _)) => s,
            }
        }
        Ep::TeacherAbandon => {
            match teacher_abandon_application(
                State(st),
                Extension(common::teacher_claims(1, 1)),
                Path((fx.sid, fx.tid, fx.rid)),
            )
            .await
            {
                Ok(s) => s,
                Err((s, _)) => s,
            }
        }
    }
}

/// 표의 한 행(엔드포인트 하나 × 상태 4개)을 실행한다.
/// expected는 [라운드 없음, OPEN, CLOSED, FINALIZED] 순서.
/// 거부 셀(4xx)은 세 테이블 스냅샷 완전 일치까지 단언한다.
async fn assert_matrix_row(ep: Ep, expected: [StatusCode; 4]) {
    for (state, exp) in ALL_STATES.into_iter().zip(expected) {
        let pool = common::create_test_pool().await;
        let fx = setup(&pool, state).await;
        let before = snapshot(&pool).await;

        let got = call(ep, &pool, &fx).await;
        assert_eq!(got, exp, "{ep:?} × {state:?}: 기대 {exp}, 실제 {got}");

        if exp.is_client_error() {
            let after = snapshot(&pool).await;
            assert_eq!(
                before, after,
                "{ep:?} × {state:?}: 거부 시 rounds/applications/results는 단 한 행도 변하면 안 됨"
            );
        }
    }
}

// ── 매트릭스: 관리자 ──────────────────────────────────────────────

#[tokio::test]
async fn matrix_open_round() {
    use StatusCode as S;
    assert_matrix_row(Ep::OpenRound, [S::CREATED, S::CONFLICT, S::CONFLICT, S::CREATED]).await;
}

#[tokio::test]
async fn matrix_close_round() {
    use StatusCode as S;
    assert_matrix_row(Ep::CloseRound, [S::NOT_FOUND, S::OK, S::NOT_FOUND, S::NOT_FOUND]).await;
}

#[tokio::test]
async fn matrix_reopen_round() {
    use StatusCode as S;
    assert_matrix_row(
        Ep::ReopenRound,
        [S::NOT_FOUND, S::NOT_FOUND, S::NO_CONTENT, S::NOT_FOUND],
    )
    .await;
}

#[tokio::test]
async fn matrix_finalize_round() {
    use StatusCode as S;
    assert_matrix_row(
        Ep::FinalizeRound,
        [S::NOT_FOUND, S::NOT_FOUND, S::NO_CONTENT, S::NOT_FOUND],
    )
    .await;
}

#[tokio::test]
async fn matrix_calculate_scores() {
    use StatusCode as S;
    assert_matrix_row(Ep::Calculate, [S::NOT_FOUND, S::BAD_REQUEST, S::OK, S::BAD_REQUEST]).await;
}

#[tokio::test]
async fn matrix_recommend() {
    use StatusCode as S;
    assert_matrix_row(
        Ep::Recommend,
        [S::BAD_REQUEST, S::BAD_REQUEST, S::NO_CONTENT, S::BAD_REQUEST],
    )
    .await;
}

#[tokio::test]
async fn matrix_unrecommend() {
    use StatusCode as S;
    assert_matrix_row(
        Ep::Unrecommend,
        [S::BAD_REQUEST, S::BAD_REQUEST, S::NO_CONTENT, S::BAD_REQUEST],
    )
    .await;
}

#[tokio::test]
async fn matrix_admin_abandon() {
    use StatusCode as S;
    assert_matrix_row(
        Ep::AdminAbandon,
        [S::BAD_REQUEST, S::BAD_REQUEST, S::BAD_REQUEST, S::NO_CONTENT],
    )
    .await;
}

// ── 매트릭스: 담임 ────────────────────────────────────────────────

#[tokio::test]
async fn matrix_teacher_create() {
    use StatusCode as S;
    assert_matrix_row(
        Ep::TeacherCreate,
        [S::NOT_FOUND, S::CREATED, S::BAD_REQUEST, S::BAD_REQUEST],
    )
    .await;
}

#[tokio::test]
async fn matrix_teacher_delete() {
    use StatusCode as S;
    assert_matrix_row(
        Ep::TeacherDelete,
        [S::BAD_REQUEST, S::NO_CONTENT, S::BAD_REQUEST, S::BAD_REQUEST],
    )
    .await;
}

#[tokio::test]
async fn matrix_teacher_abandon() {
    use StatusCode as S;
    assert_matrix_row(
        Ep::TeacherAbandon,
        [S::BAD_REQUEST, S::BAD_REQUEST, S::BAD_REQUEST, S::NO_CONTENT],
    )
    .await;
}

// ── DB 방어선: FINALIZED results UPDATE 차단 트리거 ───────────────
// idx_one_active_round·trg_prevent_delete_closed_result의 직접 SQL 테스트는
// tests/handler_rounds.rs에 이미 존재한다 (중복 작성 금지).

#[tokio::test]
async fn db_rejects_result_update_in_finalized_round() {
    let pool = common::create_test_pool().await;
    let fx = setup(&pool, RState::Finalized).await;

    // 핸들러를 우회한 직접 SQL UPDATE도 트리거가 차단해야 한다
    let res = sqlx::query(
        "UPDATE results SET recommended = 1 \
         WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(fx.sid)
    .bind(fx.tid)
    .bind(fx.rid)
    .execute(&pool)
    .await;
    assert!(res.is_err(), "FINALIZED 라운드 results UPDATE는 트리거가 차단해야 함");
    assert!(res.unwrap_err().to_string().contains("FINALIZED"));

    let recommended: i64 = sqlx::query_scalar(
        "SELECT recommended FROM results WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(fx.sid)
    .bind(fx.tid)
    .bind(fx.rid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(recommended, 0, "차단 후 recommended는 원래 값(0) 유지");
}

#[tokio::test]
async fn db_allows_result_update_in_closed_round() {
    // CLOSED에서는 recommend/unrecommend가 UPDATE해야 하므로 허용 유지
    let pool = common::create_test_pool().await;
    let fx = setup(&pool, RState::Closed).await;

    sqlx::query(
        "UPDATE results SET recommended = 1 \
         WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(fx.sid)
    .bind(fx.tid)
    .bind(fx.rid)
    .execute(&pool)
    .await
    .unwrap();

    let recommended: i64 = sqlx::query_scalar(
        "SELECT recommended FROM results WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(fx.sid)
    .bind(fx.tid)
    .bind(fx.rid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(recommended, 1);
}
