mod common;

use axum::{
    extract::{Query, State},
};
use principal_candidate_manager::{
    audit::{self, Actor, AuditEntry},
    enums::AuditAction,
    handlers::audit::{list_audit_logs, AuditQuery},
};

// ── 헬퍼 ────────────────────────────────────────────────────────────

async fn insert_log(
    pool: &sqlx::SqlitePool,
    action: AuditAction,
    round_id: Option<i64>,
) {
    let mut conn = pool.acquire().await.unwrap();
    audit::log(
        &mut *conn,
        AuditEntry {
            actor: Actor::Admin,
            action,
            round_id,
            student_id: None,
            detail: serde_json::json!({}),
        },
    )
    .await
    .unwrap();
}

// ── 페이지네이션 경계 ────────────────────────────────────────────────

#[tokio::test]
async fn test_empty_result() {
    let pool = common::create_test_pool().await;
    let state = common::make_state(pool);
    let q = AuditQuery { page: 1, per_page: 50, round_id: None, action: None };
    let axum::Json(page) = list_audit_logs(State(state), Query(q)).await.unwrap();
    assert_eq!(page.total, 0);
    assert!(page.rows.is_empty());
    assert_eq!(page.page, 1);
}

#[tokio::test]
async fn test_pagination_first_page() {
    let pool = common::create_test_pool().await;

    for _ in 0..5 {
        insert_log(&pool, AuditAction::RoundOpened, None).await;
    }

    let state = common::make_state(pool);
    let q = AuditQuery { page: 1, per_page: 3, round_id: None, action: None };
    let axum::Json(page) = list_audit_logs(State(state), Query(q)).await.unwrap();

    assert_eq!(page.total, 5);
    assert_eq!(page.rows.len(), 3);
    assert_eq!(page.page, 1);
    assert_eq!(page.per_page, 3);
}

#[tokio::test]
async fn test_pagination_last_page() {
    let pool = common::create_test_pool().await;

    for _ in 0..5 {
        insert_log(&pool, AuditAction::RoundOpened, None).await;
    }

    let state = common::make_state(pool);
    let q = AuditQuery { page: 2, per_page: 3, round_id: None, action: None };
    let axum::Json(page) = list_audit_logs(State(state), Query(q)).await.unwrap();

    assert_eq!(page.total, 5);
    assert_eq!(page.rows.len(), 2);
    assert_eq!(page.page, 2);
}

#[tokio::test]
async fn test_pagination_beyond_last_page() {
    let pool = common::create_test_pool().await;

    for _ in 0..3 {
        insert_log(&pool, AuditAction::RoundOpened, None).await;
    }

    let state = common::make_state(pool);
    let q = AuditQuery { page: 99, per_page: 10, round_id: None, action: None };
    let axum::Json(page) = list_audit_logs(State(state), Query(q)).await.unwrap();

    assert_eq!(page.total, 3);
    assert_eq!(page.rows.len(), 0);
}

// ── round_id 필터 ───────────────────────────────────────────────────

#[tokio::test]
async fn test_round_id_filter() {
    let pool = common::create_test_pool().await;

    insert_log(&pool, AuditAction::RoundOpened, Some(1)).await;
    insert_log(&pool, AuditAction::RoundClosed, Some(1)).await;
    insert_log(&pool, AuditAction::RoundOpened, Some(2)).await;

    let state = common::make_state(pool);
    let q = AuditQuery { page: 1, per_page: 50, round_id: Some(1), action: None };
    let axum::Json(page) = list_audit_logs(State(state), Query(q)).await.unwrap();

    assert_eq!(page.total, 2);
    assert!(page.rows.iter().all(|r| r.round_id == Some(1)));
}

#[tokio::test]
async fn test_round_id_filter_no_match() {
    let pool = common::create_test_pool().await;

    insert_log(&pool, AuditAction::RoundOpened, Some(1)).await;

    let state = common::make_state(pool);
    let q = AuditQuery { page: 1, per_page: 50, round_id: Some(99), action: None };
    let axum::Json(page) = list_audit_logs(State(state), Query(q)).await.unwrap();

    assert_eq!(page.total, 0);
    assert!(page.rows.is_empty());
}

// ── action 필터 ────────────────────────────────────────────────────

#[tokio::test]
async fn test_action_filter() {
    let pool = common::create_test_pool().await;

    insert_log(&pool, AuditAction::RoundOpened, None).await;
    insert_log(&pool, AuditAction::RoundOpened, None).await;
    insert_log(&pool, AuditAction::RoundClosed, None).await;

    let state = common::make_state(pool);
    let q = AuditQuery {
        page: 1,
        per_page: 50,
        round_id: None,
        action: Some("ROUND_OPENED".to_string()),
    };
    let axum::Json(page) = list_audit_logs(State(state), Query(q)).await.unwrap();

    assert_eq!(page.total, 2);
    assert!(page.rows.iter().all(|r| r.action == "ROUND_OPENED"));
}

// ── 최신순 정렬 ────────────────────────────────────────────────────

#[tokio::test]
async fn test_descending_order() {
    let pool = common::create_test_pool().await;

    insert_log(&pool, AuditAction::RoundOpened, None).await;
    insert_log(&pool, AuditAction::RoundClosed, None).await;
    insert_log(&pool, AuditAction::RoundFinalized, None).await;

    let state = common::make_state(pool);
    let q = AuditQuery { page: 1, per_page: 50, round_id: None, action: None };
    let axum::Json(page) = list_audit_logs(State(state), Query(q)).await.unwrap();

    assert_eq!(page.rows.len(), 3);
    // id DESC 순서: 마지막 삽입이 첫 번째
    assert!(page.rows[0].id > page.rows[1].id);
    assert!(page.rows[1].id > page.rows[2].id);
    assert_eq!(page.rows[0].action, "ROUND_FINALIZED");
}

// ── per_page 최대값 클램핑 ──────────────────────────────────────────

#[tokio::test]
async fn test_per_page_clamped_to_200() {
    let pool = common::create_test_pool().await;

    let state = common::make_state(pool);
    let q = AuditQuery { page: 1, per_page: 9999, round_id: None, action: None };
    let axum::Json(page) = list_audit_logs(State(state), Query(q)).await.unwrap();

    assert_eq!(page.per_page, 200);
}
