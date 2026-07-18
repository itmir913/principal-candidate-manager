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
    let q = AuditQuery { grade: None, class_no: None, page: 1, per_page: 50, round_id: None, action: None };
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
    let q = AuditQuery { grade: None, class_no: None, page: 1, per_page: 3, round_id: None, action: None };
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
    let q = AuditQuery { grade: None, class_no: None, page: 2, per_page: 3, round_id: None, action: None };
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
    let q = AuditQuery { grade: None, class_no: None, page: 99, per_page: 10, round_id: None, action: None };
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
    let q = AuditQuery { grade: None, class_no: None, page: 1, per_page: 50, round_id: Some(1), action: None };
    let axum::Json(page) = list_audit_logs(State(state), Query(q)).await.unwrap();

    assert_eq!(page.total, 2);
    assert!(page.rows.iter().all(|r| r.round_id == Some(1)));
}

#[tokio::test]
async fn test_round_id_filter_no_match() {
    let pool = common::create_test_pool().await;

    insert_log(&pool, AuditAction::RoundOpened, Some(1)).await;

    let state = common::make_state(pool);
    let q = AuditQuery { grade: None, class_no: None, page: 1, per_page: 50, round_id: Some(99), action: None };
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
        grade: None,
        class_no: None,
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
    let q = AuditQuery { grade: None, class_no: None, page: 1, per_page: 50, round_id: None, action: None };
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
    let q = AuditQuery { grade: None, class_no: None, page: 1, per_page: 9999, round_id: None, action: None };
    let axum::Json(page) = list_audit_logs(State(state), Query(q)).await.unwrap();

    assert_eq!(page.per_page, 200);
}

// ── 학급 필터 ───────────────────────────────────────────────────────

async fn insert_teacher_log(pool: &sqlx::SqlitePool, grade: i64, class_no: i64) {
    // audit::log는 Teacher actor의 학급 존재를 검증(담임명 스냅샷)하므로 학급을 먼저 보장
    sqlx::query(
        "INSERT OR IGNORE INTO classes (grade, class_no, teacher_name, password_hash) VALUES (?, ?, '테스트담임', 'x')",
    )
    .bind(grade)
    .bind(class_no)
    .execute(pool)
    .await
    .unwrap();

    let mut conn = pool.acquire().await.unwrap();
    audit::log(
        &mut *conn,
        AuditEntry {
            actor: Actor::Teacher { grade, class_no },
            action: AuditAction::ApplicationSaved,
            round_id: None,
            student_id: None,
            detail: serde_json::json!({}),
        },
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn class_filter_returns_only_that_class() {
    let pool = common::create_test_pool().await;
    insert_teacher_log(&pool, 1, 1).await;
    insert_teacher_log(&pool, 1, 1).await;
    insert_teacher_log(&pool, 2, 3).await;
    insert_log(&pool, AuditAction::RoundOpened, None).await; // 관리자 행

    let state = common::make_state(pool);
    let q = AuditQuery { grade: Some(1), class_no: Some(1), page: 1, per_page: 50, round_id: None, action: None };
    let axum::Json(page) = list_audit_logs(State(state), Query(q)).await.unwrap();

    assert_eq!(page.total, 2, "1학년 1반 행위만");
    assert!(page.rows.iter().all(|r| r.actor_grade == Some(1) && r.actor_class_no == Some(1)));
}

#[tokio::test]
async fn class_filter_excludes_admin_rows() {
    let pool = common::create_test_pool().await;
    insert_log(&pool, AuditAction::RoundOpened, None).await;
    insert_teacher_log(&pool, 2, 3).await;

    let state = common::make_state(pool);
    let q = AuditQuery { grade: Some(2), class_no: Some(3), page: 1, per_page: 50, round_id: None, action: None };
    let axum::Json(page) = list_audit_logs(State(state), Query(q)).await.unwrap();

    assert_eq!(page.total, 1, "관리자(actor_grade NULL) 행은 학급 필터에서 제외");
}

#[tokio::test]
async fn class_filter_grad_teacher_zero_zero() {
    // 졸업생 담당 특수 계정(0/0)도 학급 필터로 조회 가능해야 한다
    let pool = common::create_test_pool().await;
    insert_teacher_log(&pool, 0, 0).await;
    insert_teacher_log(&pool, 1, 1).await;

    let state = common::make_state(pool);
    let q = AuditQuery { grade: Some(0), class_no: Some(0), page: 1, per_page: 50, round_id: None, action: None };
    let axum::Json(page) = list_audit_logs(State(state), Query(q)).await.unwrap();

    assert_eq!(page.total, 1, "0/0 졸업생 담당 행만");
    assert_eq!(page.rows[0].actor_grade, Some(0));
    assert_eq!(page.rows[0].actor_class_no, Some(0));
}

#[tokio::test]
async fn class_filter_combines_with_action_filter() {
    let pool = common::create_test_pool().await;
    insert_teacher_log(&pool, 1, 1).await; // APPLICATION_SAVED
    {
        let mut conn = pool.acquire().await.unwrap();
        audit::log(
            &mut *conn,
            AuditEntry {
                actor: Actor::Teacher { grade: 1, class_no: 1 },
                action: AuditAction::ApplicationDeleted,
                round_id: None,
                student_id: None,
                detail: serde_json::json!({}),
            },
        )
        .await
        .unwrap();
    }

    let state = common::make_state(pool);
    let q = AuditQuery {
        grade: Some(1),
        class_no: Some(1),
        page: 1,
        per_page: 50,
        round_id: None,
        action: Some("APPLICATION_DELETED".to_string()),
    };
    let axum::Json(page) = list_audit_logs(State(state), Query(q)).await.unwrap();

    assert_eq!(page.total, 1, "학급+작업 유형 필터 결합");
    assert_eq!(page.rows[0].action, "APPLICATION_DELETED");
}
