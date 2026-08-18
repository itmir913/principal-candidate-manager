//! 감사 재현 테스트 — **의도적으로 실패한다.**
//!
//! 지점: 4-14 / U-28 (+ 표시 결합 U-20)
//! `POST /api/students/enrolled/add`(students.rs:724 `add_enrolled`)는 body 의
//! `grade`/`class_no`/`seq_no` 를 범위 검증 없이 `StudentRecord` 로 넘긴다.
//! DB 에도 `students.seq_no` 범위 CHECK 가 없다(002-students.sql).
//! 학년·반은 `classes` FK 가 사실상 양수를 강제하지만 **번호(seq_no)에는 아무 관문이 없다.**
//!
//! 결과: 0·음수 번호의 재학생이 저장되고
//!  - `list_students` 의 `ORDER BY ... seq_no` 에서 맨 앞에 온다
//!  - 담임 결과 화면 `ResultsTab.vue:288-292 studentsByRound` 의
//!    `(a.seq_no ?? 999) - (b.seq_no ?? 999)` 정렬에서도 맨 앞에 온다
//!  - 기초데이터 엑셀은 학년+반+번호로 학생을 찾으므로 음수 번호를 적어야만 매칭된다

mod common;

use axum::extract::State;
use axum::http::StatusCode;
use principal_candidate_manager::handlers::students::{add_enrolled, AddEnrolledBody};

#[tokio::test]
async fn add_enrolled_rejects_non_positive_seq_no() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 3, 1).await;
    let state = common::make_state(pool.clone());

    for bad in [0i64, -7] {
        let res = add_enrolled(
            State(state.clone()),
            axum::Json(AddEnrolledBody {
                name: format!("학생{bad}"),
                grade: 3,
                class_no: 1,
                seq_no: bad,
            }),
        )
        .await;
        assert!(
            matches!(&res, Err((StatusCode::BAD_REQUEST, _)) | Err((StatusCode::UNPROCESSABLE_ENTITY, _))),
            "seq_no={bad} 는 거부되어야 한다 (학년·반은 1 이상을 강제하는데 번호만 무제한). 실제: {:?}",
            res.as_ref().map(|(s, _)| *s)
        );
    }

    let stored: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM students WHERE seq_no <= 0")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored, 0, "0·음수 번호 재학생이 저장되면 안 된다");
}
