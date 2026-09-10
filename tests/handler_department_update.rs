//! 이슈 #32: 마감된 라운드의 학과명 수정.
//!
//! 학과명은 점수 산출에 쓰이지 않고 명단 관리용이므로 CLOSED/FINALIZED 에서도
//! 고칠 수 있어야 한다. 반면 계열·모집단위(track_id)는 선발 단위 자체라 마감 후
//! 바뀌면 안 된다 — 스키마 v2 는 department_name 한 컬럼만 열었고, 이 파일은
//! "열린 것"과 "여전히 닫힌 것"을 양쪽 다 고정한다.
//!
//! 담임 엔드포인트는 FINALIZED 전용이다. 담임이 지난 라운드를 보는 화면은
//! [라운드 결과] 하나뿐이고 그 화면은 FINALIZED 만 보여주므로, 다른 상태를
//! API 만 열어 두면 실제 흐름으로는 지나가지 않는 경로가 남는다.
//! OPEN 은 기존 지원 등록(upsert)이 담당한다 — 그쪽만 담임 확정을 함께
//! 철회하고, 확정·철회는 OPEN 에서만 가능하다. CLOSED 는 관리자가 추천을
//! 확정하는 구간이라 담임이 볼 화면이 없다.

mod common;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use principal_candidate_manager::handlers::applications::{
    teacher_update_application_department, update_application_department, UpdateDepartmentBody,
};
use principal_candidate_manager::handlers::scoring::export_results;
use principal_candidate_manager::excel;
use sqlx::SqlitePool;

// ── 픽스처 ───────────────────────────────────────────────────────

struct Fx {
    sid: i64,
    tid: i64,
    other_tid: i64,
}

/// 학급(1-1) · 재학생 1명 · 대학 1곳 · 모집단위 2개.
/// 모집단위를 둘 두는 이유는 "track_id 를 다른 유효한 값으로 바꾸려는 시도"까지
/// 막히는지 봐야 하기 때문이다. FK 위반으로 실패하면 트리거를 검증한 게 아니다.
async fn setup(pool: &SqlitePool) -> Fx {
    common::insert_class(pool, 1, 1).await;
    let sid: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES ('S001', '홍길동', 1, 1, 1, 1) RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let uid: i64 =
        sqlx::query_scalar("INSERT INTO universities (univ_name) VALUES ('한국대') RETURNING id")
            .fetch_one(pool)
            .await
            .unwrap();
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name) VALUES (?, '자연계열') RETURNING id",
    )
    .bind(uid)
    .fetch_one(pool)
    .await
    .unwrap();
    let other_tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name) VALUES (?, '인문계열') RETURNING id",
    )
    .bind(uid)
    .fetch_one(pool)
    .await
    .unwrap();
    Fx { sid, tid, other_tid }
}

async fn new_round(pool: &SqlitePool, status: &str) -> i64 {
    let sql = match status {
        "OPEN" => {
            "INSERT INTO rounds (status, opened_at) \
             VALUES ('OPEN', '2025-01-01T00:00:00Z') RETURNING id"
        }
        "CLOSED" => {
            "INSERT INTO rounds (status, opened_at, closed_at) \
             VALUES ('CLOSED', '2025-01-01T00:00:00Z', '2025-01-02T00:00:00Z') RETURNING id"
        }
        "FINALIZED" => {
            "INSERT INTO rounds (status, opened_at, closed_at, finalized_at) \
             VALUES ('FINALIZED', '2025-01-01T00:00:00Z', '2025-01-02T00:00:00Z', \
                     '2025-01-03T00:00:00Z') RETURNING id"
        }
        _ => unreachable!(),
    };
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

/// 지원 행은 라운드가 OPEN 인 동안 넣는다. 다만 상태는 `UPDATE rounds` 로 직접 올리므로
/// **실제 마감 흐름(close_round → finalize_round)을 거치지 않는다.** FINALIZED 로 올릴 때
/// `trg_require_all_decided_before_finalize` 는 `OLD.status = 'CLOSED'` 조건이라 발화하지
/// 않고, 그래서 여기 만들어지는 FINALIZED 라운드에는 `results` 행이 없다.
///
/// 학과명 경로는 `results` 를 읽지도 쓰지도 않으므로 이 파일의 단언에는 영향이 없다.
/// 다만 **`results` 에 의존하는 기능을 이 헬퍼로 시험하면 통과하면서 아무것도 막지 못한다** —
/// 그때는 `finalize_round` 핸들러를 태우는 픽스처를 따로 만들어라.
async fn apply_then_set_status(
    pool: &SqlitePool,
    fx: &Fx,
    department: &str,
    status: &str,
) -> i64 {
    let rid = new_round(pool, "OPEN").await;
    sqlx::query(
        "INSERT INTO applications (student_id, track_id, round_id, department_name) \
         VALUES (?, ?, ?, ?)",
    )
    .bind(fx.sid)
    .bind(fx.tid)
    .bind(rid)
    .bind(department)
    .execute(pool)
    .await
    .unwrap();

    if status != "OPEN" {
        let (sql, _) = match status {
            "CLOSED" => (
                "UPDATE rounds SET status = 'CLOSED', closed_at = '2025-01-02T00:00:00Z' WHERE id = ?",
                (),
            ),
            "FINALIZED" => (
                "UPDATE rounds SET status = 'FINALIZED', closed_at = '2025-01-02T00:00:00Z', \
                 finalized_at = '2025-01-03T00:00:00Z' WHERE id = ?",
                (),
            ),
            _ => unreachable!(),
        };
        sqlx::query(sql).bind(rid).execute(pool).await.unwrap();
    }
    rid
}

async fn department_of(pool: &SqlitePool, sid: i64, tid: i64, rid: i64) -> String {
    sqlx::query_scalar(
        "SELECT department_name FROM applications \
         WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(sid)
    .bind(tid)
    .bind(rid)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn body(name: &str) -> Json<UpdateDepartmentBody> {
    Json(UpdateDepartmentBody {
        department_name: name.to_string(),
    })
}

fn st(pool: &SqlitePool) -> State<principal_candidate_manager::state::AppState> {
    State(common::make_state(pool.clone()))
}

// ── 관리자: 전 상태 허용 ──────────────────────────────────────────

#[tokio::test]
async fn admin_updates_department_in_every_round_status() {
    for status in ["OPEN", "CLOSED", "FINALIZED"] {
        let pool = common::create_test_pool().await;
        let fx = setup(&pool).await;
        let rid = apply_then_set_status(&pool, &fx, "기계공학과", status).await;

        let got = update_application_department(
            st(&pool),
            Path((fx.sid, fx.tid, rid)),
            body("전기공학과"),
        )
        .await
        .unwrap_or_else(|e| panic!("{status} 라운드에서 관리자 수정 실패: {e:?}"));

        assert_eq!(got, StatusCode::NO_CONTENT, "{status}");
        assert_eq!(
            department_of(&pool, fx.sid, fx.tid, rid).await,
            "전기공학과",
            "{status} 라운드에서 학과명이 반영되지 않았다"
        );
    }
}

#[tokio::test]
async fn admin_update_trims_surrounding_whitespace() {
    let pool = common::create_test_pool().await;
    let fx = setup(&pool).await;
    let rid = apply_then_set_status(&pool, &fx, "기계공학과", "FINALIZED").await;

    update_application_department(
        st(&pool),
        Path((fx.sid, fx.tid, rid)),
        body("  전기공학과  "),
    )
    .await
    .unwrap();

    assert_eq!(department_of(&pool, fx.sid, fx.tid, rid).await, "전기공학과");
}

// ── 담임: FINALIZED 전용 ──────────────────────────────────────────

#[tokio::test]
async fn teacher_updates_department_in_finalized_round() {
    let pool = common::create_test_pool().await;
    let fx = setup(&pool).await;
    let rid = apply_then_set_status(&pool, &fx, "기계공학과", "FINALIZED").await;

    let got = teacher_update_application_department(
        st(&pool),
        Extension(common::teacher_claims(1, 1)),
        Path((fx.sid, fx.tid, rid)),
        body("전기공학과"),
    )
    .await
    .expect("FINALIZED 라운드에서 담임 수정이 되어야 한다");

    assert_eq!(got, StatusCode::NO_CONTENT);
    assert_eq!(department_of(&pool, fx.sid, fx.tid, rid).await, "전기공학과");
}

/// CLOSED 거부는 "아직 안 만든 기능" 이 아니라 의도된 경계다.
/// 담임에게는 CLOSED 라운드를 보는 화면이 없다 — [라운드 결과] 는
/// FINALIZED 만 보여준다. 화면 없는 API 를 열어 두지 않는다.
#[tokio::test]
async fn teacher_update_is_rejected_in_closed_round() {
    let pool = common::create_test_pool().await;
    let fx = setup(&pool).await;
    let rid = apply_then_set_status(&pool, &fx, "기계공학과", "CLOSED").await;

    let err = teacher_update_application_department(
        st(&pool),
        Extension(common::teacher_claims(1, 1)),
        Path((fx.sid, fx.tid, rid)),
        body("전기공학과"),
    )
    .await
    .unwrap_err();

    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert_eq!(department_of(&pool, fx.sid, fx.tid, rid).await, "기계공학과");

    // 같은 라운드를 관리자는 고칠 수 있다 — 급하면 관리자에게 요청하는 경로다
    update_application_department(st(&pool), Path((fx.sid, fx.tid, rid)), body("전기공학과"))
        .await
        .expect("관리자는 CLOSED 에서도 고칠 수 있어야 한다");
    assert_eq!(department_of(&pool, fx.sid, fx.tid, rid).await, "전기공학과");
}

#[tokio::test]
async fn teacher_update_is_rejected_in_open_round() {
    let pool = common::create_test_pool().await;
    let fx = setup(&pool).await;
    let rid = apply_then_set_status(&pool, &fx, "기계공학과", "OPEN").await;

    let err = teacher_update_application_department(
        st(&pool),
        Extension(common::teacher_claims(1, 1)),
        Path((fx.sid, fx.tid, rid)),
        body("전기공학과"),
    )
    .await
    .unwrap_err();

    assert_eq!(
        err.0,
        StatusCode::BAD_REQUEST,
        "OPEN 라운드는 기존 지원 수정 경로가 담당해야 한다 (담임 확정 철회를 함께 하므로)"
    );
    assert_eq!(
        department_of(&pool, fx.sid, fx.tid, rid).await,
        "기계공학과",
        "거부된 요청이 값을 바꿔선 안 된다"
    );
}

#[tokio::test]
async fn grad_teacher_updates_graduated_student() {
    let pool = common::create_test_pool().await;
    let fx = setup(&pool).await;
    // 졸업생은 grade/class_no/seq_no 가 NULL 이고 grad_year 가 있어야 한다 (students CHECK)
    sqlx::query(
        "UPDATE students \
         SET is_enrolled = 0, grade = NULL, class_no = NULL, seq_no = NULL, grad_year = 2024 \
         WHERE id = ?",
    )
    .bind(fx.sid)
    .execute(&pool)
    .await
    .unwrap();
    let rid = apply_then_set_status(&pool, &fx, "기계공학과", "FINALIZED").await;

    // 졸업생 담당 계정은 grade=0/class_no=0 이고 is_enrolled=0 인 학생 전체를 본다
    teacher_update_application_department(
        st(&pool),
        Extension(common::teacher_claims(0, 0)),
        Path((fx.sid, fx.tid, rid)),
        body("전기공학과"),
    )
    .await
    .unwrap();

    assert_eq!(department_of(&pool, fx.sid, fx.tid, rid).await, "전기공학과");
}

#[tokio::test]
async fn teacher_cannot_update_other_class_student() {
    let pool = common::create_test_pool().await;
    let fx = setup(&pool).await;
    let rid = apply_then_set_status(&pool, &fx, "기계공학과", "FINALIZED").await;
    common::insert_class(&pool, 2, 5).await;

    let err = teacher_update_application_department(
        st(&pool),
        Extension(common::teacher_claims(2, 5)),
        Path((fx.sid, fx.tid, rid)),
        body("전기공학과"),
    )
    .await
    .unwrap_err();

    assert_eq!(err.0, StatusCode::FORBIDDEN);
    assert_eq!(department_of(&pool, fx.sid, fx.tid, rid).await, "기계공학과");
}

#[tokio::test]
async fn class_teacher_cannot_touch_graduated_student() {
    let pool = common::create_test_pool().await;
    let fx = setup(&pool).await;
    sqlx::query(
        "UPDATE students          SET is_enrolled = 0, grade = NULL, class_no = NULL, seq_no = NULL, grad_year = 2024          WHERE id = ?",
    )
    .bind(fx.sid)
    .execute(&pool)
    .await
    .unwrap();
    let rid = apply_then_set_status(&pool, &fx, "기계공학과", "FINALIZED").await;

    // 일반 담임(1-1)은 grade/class_no 로 찾으므로 졸업생(둘 다 NULL)을 못 잡는다
    let err = teacher_update_application_department(
        st(&pool),
        Extension(common::teacher_claims(1, 1)),
        Path((fx.sid, fx.tid, rid)),
        body("전기공학과"),
    )
    .await
    .unwrap_err();

    assert_eq!(err.0, StatusCode::FORBIDDEN);
    assert_eq!(department_of(&pool, fx.sid, fx.tid, rid).await, "기계공학과");
}

#[tokio::test]
async fn grad_teacher_cannot_touch_enrolled_student() {
    let pool = common::create_test_pool().await;
    let fx = setup(&pool).await;
    let rid = apply_then_set_status(&pool, &fx, "기계공학과", "FINALIZED").await;

    // 졸업생 담당(0-0)은 is_enrolled = 0 만 본다. 재학생은 남의 담당이다.
    let err = teacher_update_application_department(
        st(&pool),
        Extension(common::teacher_claims(0, 0)),
        Path((fx.sid, fx.tid, rid)),
        body("전기공학과"),
    )
    .await
    .unwrap_err();

    assert_eq!(err.0, StatusCode::FORBIDDEN);
    assert_eq!(department_of(&pool, fx.sid, fx.tid, rid).await, "기계공학과");
}

#[tokio::test]
async fn other_rounds_of_same_application_are_untouched() {
    let pool = common::create_test_pool().await;
    let fx = setup(&pool).await;
    let rid1 = apply_then_set_status(&pool, &fx, "1라운드학과", "FINALIZED").await;
    let rid2 = apply_then_set_status(&pool, &fx, "2라운드학과", "CLOSED").await;

    update_application_department(st(&pool), Path((fx.sid, fx.tid, rid1)), body("고친학과"))
        .await
        .unwrap();

    assert_eq!(department_of(&pool, fx.sid, fx.tid, rid1).await, "고친학과");
    // WHERE 절이 round_id 까지 좁히지 않으면 같은 (학생, 모집단위)의 다른 라운드까지 덮인다.
    // 지난 라운드의 명단은 그 라운드의 기록이므로 함께 바뀌면 이력이 오염된다.
    assert_eq!(
        department_of(&pool, fx.sid, fx.tid, rid2).await,
        "2라운드학과",
        "다른 라운드의 학과명이 함께 바뀌었다"
    );
}

// ── 거부 ─────────────────────────────────────────────────────────

#[tokio::test]
async fn blank_department_is_rejected() {
    for raw in ["", "   ", "\t\n"] {
        let pool = common::create_test_pool().await;
        let fx = setup(&pool).await;
        let rid = apply_then_set_status(&pool, &fx, "기계공학과", "FINALIZED").await;

        let err = update_application_department(st(&pool), Path((fx.sid, fx.tid, rid)), body(raw))
            .await
            .unwrap_err();

        assert_eq!(err.0, StatusCode::BAD_REQUEST, "입력 {raw:?}");
        assert_eq!(
            department_of(&pool, fx.sid, fx.tid, rid).await,
            "기계공학과",
            "입력 {raw:?} 가 값을 지워선 안 된다"
        );
    }
}

#[tokio::test]
async fn teacher_path_also_rejects_blank_department() {
    // 검증은 공용 헬퍼에 있지만, 담임 진입점에서도 단언해 두어야
    // 헬퍼를 우회하는 리팩터링이 생겼을 때 잡힌다.
    let pool = common::create_test_pool().await;
    let fx = setup(&pool).await;
    let rid = apply_then_set_status(&pool, &fx, "기계공학과", "FINALIZED").await;

    let err = teacher_update_application_department(
        st(&pool),
        Extension(common::teacher_claims(1, 1)),
        Path((fx.sid, fx.tid, rid)),
        body("   "),
    )
    .await
    .unwrap_err();

    assert_eq!(err.0, StatusCode::BAD_REQUEST);
    assert_eq!(department_of(&pool, fx.sid, fx.tid, rid).await, "기계공학과");
}

#[tokio::test]
async fn unknown_round_is_404() {
    let pool = common::create_test_pool().await;
    let fx = setup(&pool).await;
    let _ = apply_then_set_status(&pool, &fx, "기계공학과", "FINALIZED").await;

    let err = update_application_department(st(&pool), Path((fx.sid, fx.tid, 9999)), body("전기"))
        .await
        .unwrap_err();
    assert_eq!(err.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn unknown_application_is_404() {
    let pool = common::create_test_pool().await;
    let fx = setup(&pool).await;
    let rid = apply_then_set_status(&pool, &fx, "기계공학과", "FINALIZED").await;

    // 라운드는 있지만 이 모집단위로 지원한 적은 없다
    let err =
        update_application_department(st(&pool), Path((fx.sid, fx.other_tid, rid)), body("전기"))
            .await
            .unwrap_err();
    assert_eq!(err.0, StatusCode::NOT_FOUND);
}

// ── 여전히 닫혀 있어야 하는 것 (DB 레벨) ──────────────────────────

#[tokio::test]
async fn db_still_blocks_track_change_after_close() {
    for status in ["CLOSED", "FINALIZED"] {
        let pool = common::create_test_pool().await;
        let fx = setup(&pool).await;
        let rid = apply_then_set_status(&pool, &fx, "기계공학과", status).await;

        let res = sqlx::query(
            "UPDATE applications SET track_id = ? \
             WHERE student_id = ? AND track_id = ? AND round_id = ?",
        )
        .bind(fx.other_tid)
        .bind(fx.sid)
        .bind(fx.tid)
        .bind(rid)
        .execute(&pool)
        .await;

        assert!(
            res.is_err(),
            "{status} 라운드에서 계열·모집단위 변경이 통과했다 — v2 가 열어야 하는 것은 학과명뿐이다"
        );
    }
}

#[tokio::test]
async fn db_still_blocks_excluded_change_in_finalized_round() {
    let pool = common::create_test_pool().await;
    let fx = setup(&pool).await;
    let rid = apply_then_set_status(&pool, &fx, "기계공학과", "FINALIZED").await;

    let res = sqlx::query(
        "UPDATE applications SET excluded = 1, excluded_reason = '결격' \
         WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(fx.sid)
    .bind(fx.tid)
    .bind(rid)
    .execute(&pool)
    .await;

    assert!(res.is_err(), "FINALIZED 에서 미선발 처리는 계속 막혀야 한다");
}

#[tokio::test]
async fn db_still_blocks_abandon_revert_in_finalized_round() {
    let pool = common::create_test_pool().await;
    let fx = setup(&pool).await;
    let rid = apply_then_set_status(&pool, &fx, "기계공학과", "FINALIZED").await;
    sqlx::query("UPDATE applications SET abandoned = 1 WHERE student_id = ? AND round_id = ?")
        .bind(fx.sid)
        .bind(rid)
        .execute(&pool)
        .await
        .unwrap();

    let res = sqlx::query("UPDATE applications SET abandoned = 0 WHERE student_id = ? AND round_id = ?")
        .bind(fx.sid)
        .bind(rid)
        .execute(&pool)
        .await;

    assert!(res.is_err(), "포기 되돌리기는 계속 막혀야 한다");
}

// ── 박제된 것이 흔들리지 않는지 ──────────────────────────────────

#[tokio::test]
async fn results_snapshot_is_untouched() {
    // CLOSED 와 FINALIZED 둘 다 확인한다. FINALIZED 에는
    // trg_prevent_update_finalized_result 가 걸려 있어, 학과명 수정이
    // results 를 건드리려 들면 조용한 무변화가 아니라 오류로 드러난다.
    for status in ["CLOSED", "FINALIZED"] {
    let pool = common::create_test_pool().await;
    let fx = setup(&pool).await;
    let rid = apply_then_set_status(&pool, &fx, "기계공학과", status).await;
    sqlx::query(
        "INSERT INTO results (student_id, track_id, round_id, score_detail, total_score, \
                              ranking, recommended, calculated_at) \
         VALUES (?, ?, ?, '{\"1\": 9000000}', 9000000, 1, 1, '2025-01-02T00:00:00Z')",
    )
    .bind(fx.sid)
    .bind(fx.tid)
    .bind(rid)
    .execute(&pool)
    .await
    .unwrap();

    let before: (String, i64, Option<i64>, bool) = sqlx::query_as(
        "SELECT score_detail, total_score, ranking, recommended = 1 FROM results \
         WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(fx.sid)
    .bind(fx.tid)
    .bind(rid)
    .fetch_one(&pool)
    .await
    .unwrap();

    update_application_department(st(&pool), Path((fx.sid, fx.tid, rid)), body("전기공학과"))
        .await
        .unwrap();

    let after: (String, i64, Option<i64>, bool) = sqlx::query_as(
        "SELECT score_detail, total_score, ranking, recommended = 1 FROM results \
         WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(fx.sid)
    .bind(fx.tid)
    .bind(rid)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(before, after, "{status} 라운드에서 학과명 수정이 결과 박제를 건드렸다");
    }
}

#[tokio::test]
async fn teacher_confirmation_survives_department_update() {
    let pool = common::create_test_pool().await;
    let fx = setup(&pool).await;
    let rid = apply_then_set_status(&pool, &fx, "기계공학과", "FINALIZED").await;
    sqlx::query(
        "INSERT INTO round_confirmations (round_id, grade, class_no, confirmed_at) \
         VALUES (?, 1, 1, '2025-01-02T00:00:00Z')",
    )
    .bind(rid)
    .execute(&pool)
    .await
    .unwrap();

    teacher_update_application_department(
        st(&pool),
        Extension(common::teacher_claims(1, 1)),
        Path((fx.sid, fx.tid, rid)),
        body("전기공학과"),
    )
    .await
    .unwrap();

    let still: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM round_confirmations WHERE round_id = ?")
            .bind(rid)
            .fetch_one(&pool)
            .await
            .unwrap();

    // 확정·철회는 OPEN 에서만 가능하다. 마감된 라운드의 확정 기록을 지우면
    // 담임이 다시 확정할 방법이 없어 복구 불가능한 상태가 된다.
    assert_eq!(still, 1, "마감된 라운드의 담임 확정이 철회됐다");
}

// ── 감사 기록 ────────────────────────────────────────────────────

#[tokio::test]
async fn audit_log_keeps_previous_and_new_department() {
    let pool = common::create_test_pool().await;
    let fx = setup(&pool).await;
    let rid = apply_then_set_status(&pool, &fx, "기계공학과", "FINALIZED").await;

    update_application_department(st(&pool), Path((fx.sid, fx.tid, rid)), body("전기공학과"))
        .await
        .unwrap();

    let (action, detail, actor): (String, String, String) = sqlx::query_as(
        "SELECT action, detail, actor_type FROM audit_log ORDER BY id DESC LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(action, "APPLICATION_DEPARTMENT_UPDATED");
    assert_eq!(actor, "ADMIN");
    let json: serde_json::Value = serde_json::from_str(&detail).unwrap();
    assert_eq!(json["previous_department_name"], "기계공학과");
    assert_eq!(json["department_name"], "전기공학과");
}

#[tokio::test]
async fn teacher_update_is_logged_as_teacher() {
    let pool = common::create_test_pool().await;
    let fx = setup(&pool).await;
    let rid = apply_then_set_status(&pool, &fx, "기계공학과", "FINALIZED").await;

    teacher_update_application_department(
        st(&pool),
        Extension(common::teacher_claims(1, 1)),
        Path((fx.sid, fx.tid, rid)),
        body("전기공학과"),
    )
    .await
    .unwrap();

    let (actor, grade, class_no): (String, Option<i64>, Option<i64>) = sqlx::query_as(
        "SELECT actor_type, actor_grade, actor_class_no FROM audit_log ORDER BY id DESC LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(actor, "TEACHER");
    assert_eq!((grade, class_no), (Some(1), Some(1)));

    // 전후 값도 담임 경로에서 직접 단언한다. 지금은 관리자와 같은 헬퍼를 쓰지만,
    // 진입점마다 확인해 두어야 헬퍼를 우회하는 리팩터링이 생겼을 때 잡힌다.
    let detail: String = sqlx::query_scalar("SELECT detail FROM audit_log ORDER BY id DESC LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_str(&detail).unwrap();
    assert_eq!(json["previous_department_name"], "기계공학과");
    assert_eq!(json["department_name"], "전기공학과");
}

/// 관리자가 OPEN 라운드에서 학과명을 고쳐도 담임 확정은 유지된다 — **의도된 동작**이다.
/// 담임 전용 경로가 OPEN 을 거부하는 이유가 "확정이 남은 채 값만 바뀐다" 인데,
/// 관리자 경로에는 정확히 그 상황이 생긴다. 학과명은 점수·정원과 무관하고 담임의
/// "입력 완료 확정" 이 뜻하는 바(그 반의 입력이 끝났다)가 달라지지 않으므로 유지한다.
/// 근거는 00_spec §7.5 에 적혀 있다. 이 테스트는 그 판단을 고정한다.
#[tokio::test]
async fn admin_open_update_keeps_teacher_confirmation() {
    let pool = common::create_test_pool().await;
    let fx = setup(&pool).await;
    let rid = apply_then_set_status(&pool, &fx, "기계공학과", "OPEN").await;
    sqlx::query(
        "INSERT INTO round_confirmations (round_id, grade, class_no, confirmed_at)          VALUES (?, 1, 1, '2025-01-02T00:00:00Z')",
    )
    .bind(rid)
    .execute(&pool)
    .await
    .unwrap();

    update_application_department(st(&pool), Path((fx.sid, fx.tid, rid)), body("전기공학과"))
        .await
        .unwrap();

    let still: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM round_confirmations WHERE round_id = ?")
            .bind(rid)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(still, 1, "관리자의 학과명 수정이 담임 확정을 철회했다 (의도와 다름)");
    assert_eq!(department_of(&pool, fx.sid, fx.tid, rid).await, "전기공학과");
}

#[tokio::test]
async fn unchanged_value_writes_no_audit_row() {
    let pool = common::create_test_pool().await;
    let fx = setup(&pool).await;
    let rid = apply_then_set_status(&pool, &fx, "기계공학과", "FINALIZED").await;

    let got = update_application_department(
        st(&pool),
        Path((fx.sid, fx.tid, rid)),
        body("  기계공학과  "),
    )
    .await
    .unwrap();

    assert_eq!(got, StatusCode::NO_CONTENT);
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_log")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 0, "바뀐 것이 없는데 감사 기록이 남았다");
}

// ── 이슈 #32 의 실제 요구: 재계산 없이 명단에 반영 ───────────────

#[tokio::test]
async fn export_reflects_updated_department_without_recalculation() {
    let pool = common::create_test_pool().await;
    let fx = setup(&pool).await;
    let rid = apply_then_set_status(&pool, &fx, "기계공학과", "FINALIZED").await;

    update_application_department(st(&pool), Path((fx.sid, fx.tid, rid)), body("전기공학과"))
        .await
        .unwrap();

    let resp = export_results(st(&pool), Path(rid)).await.unwrap();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    assert!(excel::is_xlsx(&bytes));

    let rows = excel::parse_xlsx_all_rows_raw(&bytes).unwrap();
    let header = &rows[0];
    let col = header
        .iter()
        .position(|c| c == "지원학과")
        .expect("내보내기 헤더에 '지원학과' 열이 있어야 한다");
    let row = rows
        .iter()
        .find(|r| r.iter().any(|c| c == "홍길동"))
        .expect("홍길동 행");

    assert_eq!(
        row[col], "전기공학과",
        "학과명이 results 재계산 없이 내보내기에 반영돼야 한다"
    );
}

// ── v1 → v2 승격 (실제 데이터 위에서) ────────────────────────────

/// 현장 DB 는 v1 으로 만들어졌고 CLOSED/FINALIZED 라운드가 실재한다.
/// 지문 대조(schema_freeze)는 빈 DB 의 sqlite_master 만 비교하므로,
/// "데이터가 있는 v1 DB 에 v2 조각을 적용한 뒤 실제로 동작이 바뀌는가"는
/// 별도로 확인해야 한다. 이 테스트가 승격 경로 자체를 검증한다.
#[tokio::test]
async fn v1_database_with_data_upgrades_to_v2_and_opens_department_only() {
    use principal_candidate_manager::db::{migration_sqls, run_migrations_with};
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    let sqls = migration_sqls();
    assert!(sqls.len() >= 2, "v2 마이그레이션이 등록돼 있어야 한다");

    let opts = SqliteConnectOptions::new()
        .filename(":memory:")
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .unwrap();

    // ① v1 만 적용한 DB 를 만든다 (현장 DB 와 같은 상태)
    run_migrations_with(&pool, &[sqls[0].as_str()]).await.unwrap();
    let ver: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(ver, 1);

    // ② 데이터를 넣고 라운드를 마감한다.
    //    현장 DB 에는 FINALIZED 와 CLOSED 가 함께 있을 수 있으므로 둘 다 만든다
    //    (idx_one_active_round 가 비-FINALIZED 를 하나로 제한하므로 순서가 중요하다).
    let fx = setup(&pool).await;
    let rid = apply_then_set_status(&pool, &fx, "기계공학과", "FINALIZED").await;
    let closed_rid = apply_then_set_status(&pool, &fx, "화학과", "CLOSED").await;

    // v1 에서는 학과명 수정이 막혀 있었다 — 이것이 이슈 #32 의 출발점이다
    let blocked_on_v1 = sqlx::query(
        "UPDATE applications SET department_name = '전기공학과' \
         WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(fx.sid)
    .bind(fx.tid)
    .bind(rid)
    .execute(&pool)
    .await;
    assert!(blocked_on_v1.is_err(), "v1 에서는 학과명 수정이 막혀 있어야 한다");

    // ③ v2 로 승격
    let refs: Vec<&str> = sqls.iter().map(String::as_str).collect();
    run_migrations_with(&pool, &refs).await.unwrap();
    let ver: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(ver, 2, "승격 후 user_version 이 2 여야 한다");

    // ④ 이제 학과명은 열리고
    update_application_department(st(&pool), Path((fx.sid, fx.tid, rid)), body("전기공학과"))
        .await
        .expect("v2 승격 후 학과명 수정이 되어야 한다");
    assert_eq!(department_of(&pool, fx.sid, fx.tid, rid).await, "전기공학과");

    // ⑤ 계열·모집단위는 여전히 닫혀 있다
    let still_blocked = sqlx::query(
        "UPDATE applications SET track_id = ? \
         WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(fx.other_tid)
    .bind(fx.sid)
    .bind(fx.tid)
    .bind(rid)
    .execute(&pool)
    .await;
    assert!(
        still_blocked.is_err(),
        "승격 후에도 계열·모집단위 변경은 막혀야 한다"
    );

    // ⑥ CLOSED 라운드도 같이 열렸는지 — 관리자는 상태를 가리지 않는다
    update_application_department(st(&pool), Path((fx.sid, fx.tid, closed_rid)), body("생명과학과"))
        .await
        .expect("승격 후 CLOSED 라운드의 학과명도 고칠 수 있어야 한다");
    assert_eq!(
        department_of(&pool, fx.sid, fx.tid, closed_rid).await,
        "생명과학과"
    );

    // CLOSED 에서도 계열은 계속 막힌다
    let closed_track = sqlx::query(
        "UPDATE applications SET track_id = ?          WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(fx.other_tid)
    .bind(fx.sid)
    .bind(fx.tid)
    .bind(closed_rid)
    .execute(&pool)
    .await;
    assert!(closed_track.is_err(), "CLOSED 에서도 계열 변경은 막혀야 한다");

    // ⑦ 기존 데이터가 승격 과정에서 사라지지 않았다
    let apps: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM applications")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(apps, 2, "승격이 기존 지원 행을 잃어버렸다");
}
