//! 감사 재현 테스트 — **의도적으로 실패한다.**
//!
//! 지점: 4-31 / 4-35 / U-29 / U-30 / M-8
//! Excel 설정 import 경로는 `parse_quota`(universities.rs:836-847)로 정원 `>= 1` 또는
//! "무제한"만 허용한다. 그런데 JSON API(1-53 / 1-60 / 1-63 / 1-65)에는 범위 검증이
//! 전혀 없어 0·음수 정원이 그대로 저장된다.
//!
//! 관리자 UI도 막지 못한다: `UniversitiesTab.vue:604` 는
//! `parseInt(e.target.value) || 1` 이라 "0"은 조용히 1로 바뀌고(Fail-Fast 위반)
//! "-3"은 truthy 라 그대로 통과한다. `min="1"` 은 input 이벤트에 강제되지 않는다.
//!
//! 저장된 음수 정원은 엑셀 "모집단위 정원"·"대학 전체 정원" 열에 그대로 기록된다
//! (universities.rs:722, 736 `write_number(... q as f64)`).

mod common;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use principal_candidate_manager::handlers::universities::{
    create_track, create_university, update_track, update_university,
    CreateTrackBody, CreateUnivBody, UpdateTrackBody, UpdateUnivBody,
};

#[tokio::test]
async fn create_university_rejects_non_positive_total_quota() {
    let pool = common::create_test_pool().await;
    let state = common::make_state(pool.clone());

    for bad in [0i64, -3] {
        let res = create_university(
            State(state.clone()),
            axum::Json(CreateUnivBody {
                univ_name: format!("대학{bad}"),
                total_quota: Some(Some(bad)),
                prioritize_enrolled: false,
            }),
        )
        .await;
        assert!(
            matches!(&res, Err((StatusCode::BAD_REQUEST, _))),
            "total_quota={bad} 는 400으로 거부되어야 한다 (Excel 경로 parse_quota 와 동일 기준). 실제: {:?}",
            res.as_ref().map(|(s, _)| *s).map_err(|(s, m)| (*s, m.clone()))
        );
    }
}

#[tokio::test]
async fn update_university_rejects_non_positive_total_quota() {
    let pool = common::create_test_pool().await;
    let state = common::make_state(pool.clone());
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name, total_quota, prioritize_enrolled)
         VALUES ('한국대', 5, 0) RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();

    let res = update_university(
        State(state),
        Path(uid),
        axum::Json(UpdateUnivBody {
            univ_name: None,
            total_quota: Some(Some(-3)),
            prioritize_enrolled: None,
        }),
    )
    .await;
    assert!(
        matches!(&res, Err((StatusCode::BAD_REQUEST, _))),
        "total_quota=-3 는 400으로 거부되어야 한다. 실제: {res:?}"
    );

    let stored: Option<i64> = sqlx::query_scalar("SELECT total_quota FROM universities WHERE id = ?")
        .bind(uid).fetch_one(&pool).await.unwrap();
    assert_ne!(stored, Some(-3), "음수 정원이 DB에 저장되면 안 된다");
}

#[tokio::test]
async fn create_track_rejects_non_positive_unit_quota() {
    let pool = common::create_test_pool().await;
    let state = common::make_state(pool.clone());
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name, total_quota, prioritize_enrolled)
         VALUES ('한국대', NULL, 0) RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();

    let res = create_track(
        State(state),
        Path(uid),
        axum::Json(CreateTrackBody {
            track_name: "컴공".into(),
            unit_quota: Some(Some(0)),
            prioritize_enrolled: false,
        }),
    )
    .await;
    assert!(
        matches!(&res, Err((StatusCode::BAD_REQUEST, _))),
        "unit_quota=0 은 400으로 거부되어야 한다. 실제: {res:?}"
    );
}

#[tokio::test]
async fn update_track_rejects_non_positive_unit_quota() {
    let pool = common::create_test_pool().await;
    let state = common::make_state(pool.clone());
    let uid: i64 = sqlx::query_scalar(
        "INSERT INTO universities (univ_name, total_quota, prioritize_enrolled)
         VALUES ('한국대', NULL, 0) RETURNING id",
    )
    .fetch_one(&pool).await.unwrap();
    let tid: i64 = sqlx::query_scalar(
        "INSERT INTO univ_tracks (univ_id, track_name, unit_quota, prioritize_enrolled)
         VALUES (?, '컴공', 3, 0) RETURNING id",
    )
    .bind(uid).fetch_one(&pool).await.unwrap();

    let res = update_track(
        State(state),
        Path(tid),
        axum::Json(UpdateTrackBody {
            track_name: None,
            unit_quota: Some(Some(-1)),
            prioritize_enrolled: None,
        }),
    )
    .await;
    assert!(
        matches!(&res, Err((StatusCode::BAD_REQUEST, _))),
        "unit_quota=-1 은 400으로 거부되어야 한다. 실제: {res:?}"
    );

    let stored: Option<i64> = sqlx::query_scalar("SELECT unit_quota FROM univ_tracks WHERE id = ?")
        .bind(tid).fetch_one(&pool).await.unwrap();
    assert_ne!(stored, Some(-1), "음수 정원이 DB에 저장되면 안 된다");
}
