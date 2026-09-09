mod common;

use axum::{extract::State, http::StatusCode, Extension, Json};
use principal_candidate_manager::{
    handlers::app_info::{
        get_app_info, update_app_info, AppInfo, UpdateAppInfoBody, DEFAULT_DESC, DEFAULT_TITLE,
        MAX_DESC_LEN, MAX_TITLE_LEN,
    },
    state::AppState,
};

fn app_state(pool: sqlx::SqlitePool) -> State<AppState> {
    State(common::make_state(pool))
}

fn body(title: &str, desc: &str) -> Json<UpdateAppInfoBody> {
    Json(UpdateAppInfoBody { title: title.into(), desc: desc.into() })
}

async fn save(pool: &sqlx::SqlitePool, title: &str, desc: &str) -> Result<Json<AppInfo>, (StatusCode, String)> {
    update_app_info(app_state(pool.clone()), Extension(common::admin_claims()), body(title, desc)).await
}

// ── 기본값 ────────────────────────────────────────────────────────

/// 설정 전에는 0.2.14까지 화면에 하드코딩돼 있던 문구가 그대로 나와야 한다 —
/// 업그레이드 직후 제목이 빈칸이 되면 안 된다.
#[tokio::test]
async fn unset_returns_default_title_and_desc() {
    let pool = common::create_test_pool().await;
    let Json(info) = get_app_info(app_state(pool)).await.unwrap();
    assert_eq!(info.title, DEFAULT_TITLE);
    assert_eq!(info.desc, DEFAULT_DESC);
}

/// 한쪽만 저장된 상태(수동 편집·부분 실패)에서도 나머지는 기본값으로 채운다.
#[tokio::test]
async fn only_title_set_falls_back_to_default_desc() {
    let pool = common::create_test_pool().await;
    sqlx::query("INSERT INTO app_configs (key, value) VALUES ('app_title', '한빛고등학교')")
        .execute(&pool)
        .await
        .unwrap();

    let Json(info) = get_app_info(app_state(pool)).await.unwrap();
    assert_eq!(info.title, "한빛고등학교");
    assert_eq!(info.desc, DEFAULT_DESC);
}

/// 화면 안의 "설치본 이름" 카드는 이 값으로 표시 여부를 가른다 — 기본 문구를 쓰는 상태에서
/// 카드를 그리면 고정 제품명과 같은 말이 두 번 뜬다.
#[tokio::test]
async fn configured_is_false_until_admin_sets_a_title() {
    let pool = common::create_test_pool().await;

    let Json(before) = get_app_info(app_state(pool.clone())).await.unwrap();
    assert!(!before.configured, "지정 전에는 false");

    let _ = save(&pool, "한빛고", "인원 제한 없는 대학").await.unwrap();

    let Json(after) = get_app_info(app_state(pool)).await.unwrap();
    assert!(after.configured, "지정 후에는 true");
}

/// 설명을 비운 채 제목만 지정한 경우도 "지정했다" — 판단 근거는 제목 행의 존재다.
/// 설명으로 판단하면 학교명만 쓰는 운영에서 카드가 영영 안 뜬다.
#[tokio::test]
async fn configured_is_true_when_only_title_is_set() {
    let pool = common::create_test_pool().await;
    let _ = save(&pool, "한빛고", "").await.unwrap();

    let Json(info) = get_app_info(app_state(pool)).await.unwrap();
    assert!(info.configured);
    assert_eq!(info.desc, "");
}

/// 저장 응답도 곧바로 configured 를 참으로 알려야 한다 — 프론트가 이 응답으로 상태를
/// 갱신하므로, 여기서 false 가 오면 방금 이름을 지정하고도 카드가 안 뜬다.
#[tokio::test]
async fn update_response_reports_configured() {
    let pool = common::create_test_pool().await;
    let Json(saved_info) = save(&pool, "한빛고", "인원 제한 있는 대학").await.unwrap();
    assert!(saved_info.configured);
}

// ── 저장 ──────────────────────────────────────────────────────────

#[tokio::test]
async fn update_persists_and_is_readable() {
    let pool = common::create_test_pool().await;
    let _ = save(&pool, "한빛고 학교장추천", "인원제한 없음").await.unwrap();

    let Json(info) = get_app_info(app_state(pool)).await.unwrap();
    assert_eq!(info.title, "한빛고 학교장추천");
    assert_eq!(info.desc, "인원제한 없음");
}

#[tokio::test]
async fn update_twice_replaces_previous_value() {
    let pool = common::create_test_pool().await;
    let _ = save(&pool, "첫 제목", "첫 설명").await.unwrap();
    let _ = save(&pool, "둘째 제목", "둘째 설명").await.unwrap();

    let Json(info) = get_app_info(app_state(pool.clone())).await.unwrap();
    assert_eq!(info.title, "둘째 제목");

    // INSERT OR REPLACE 이므로 키당 한 행만 남아야 한다
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM app_configs WHERE key = 'app_title'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 1);
}

/// 설명은 비워 둘 수 있다 — 학교명만 쓰고 부제는 안 쓰는 운영이 있다.
#[tokio::test]
async fn empty_desc_is_allowed() {
    let pool = common::create_test_pool().await;
    let _ = save(&pool, "한빛고", "").await.unwrap();

    let Json(info) = get_app_info(app_state(pool)).await.unwrap();
    assert_eq!(info.desc, "");
}

#[tokio::test]
async fn title_and_desc_are_trimmed() {
    let pool = common::create_test_pool().await;
    let _ = save(&pool, "  한빛고  ", "  인원제한 없음  ").await.unwrap();

    let Json(info) = get_app_info(app_state(pool)).await.unwrap();
    assert_eq!(info.title, "한빛고");
    assert_eq!(info.desc, "인원제한 없음");
}

// ── 거부 ──────────────────────────────────────────────────────────

/// 제목은 화면의 유일한 식별자다. 빈 값이면 인스턴스를 구분할 수 없다.
#[tokio::test]
async fn empty_title_is_rejected() {
    let pool = common::create_test_pool().await;
    let err = save(&pool, "", "설명").await.expect_err("빈 제목은 거부");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn whitespace_only_title_is_rejected() {
    let pool = common::create_test_pool().await;
    let err = save(&pool, "   ", "설명").await.expect_err("공백뿐인 제목은 거부");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);

    // 거부된 요청이 값을 남기면 안 된다
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM app_configs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 0, "거부된 요청은 아무것도 저장하지 않는다");
}

#[tokio::test]
async fn title_at_limit_is_accepted_and_over_limit_is_rejected() {
    let pool = common::create_test_pool().await;

    let at_limit = "가".repeat(MAX_TITLE_LEN);
    let _ = save(&pool, &at_limit, "").await.expect("상한 길이는 허용");

    let over = "가".repeat(MAX_TITLE_LEN + 1);
    let err = save(&pool, &over, "").await.expect_err("상한 초과는 거부");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn desc_over_limit_is_rejected() {
    let pool = common::create_test_pool().await;

    let at_limit = "나".repeat(MAX_DESC_LEN);
    let _ = save(&pool, "제목", &at_limit).await.expect("상한 길이는 허용");

    let over = "나".repeat(MAX_DESC_LEN + 1);
    let err = save(&pool, "제목", &over).await.expect_err("상한 초과는 거부");
    assert_eq!(err.0, StatusCode::BAD_REQUEST);
}

/// 길이 상한은 문자 수 기준이다 — 바이트로 세면 한글이 1/3만 들어간다.
#[tokio::test]
async fn length_limit_counts_characters_not_bytes() {
    let pool = common::create_test_pool().await;
    // 한글 40자 = UTF-8 120바이트. 바이트 기준이면 여기서 거부된다.
    let korean = "한".repeat(MAX_TITLE_LEN);
    assert!(korean.len() > MAX_TITLE_LEN, "전제: 바이트 길이는 상한을 넘는다");
    let _ = save(&pool, &korean, "").await.expect("문자 수 기준이면 허용된다");
}

// ── 감사 로그 ─────────────────────────────────────────────────────

#[tokio::test]
async fn update_writes_audit_log() {
    let pool = common::create_test_pool().await;
    let _ = save(&pool, "한빛고", "인원제한 없음").await.unwrap();

    let (action, detail): (String, String) =
        sqlx::query_as("SELECT action, detail FROM audit_log ORDER BY id DESC LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(action, "APP_INFO_UPDATED");
    assert!(detail.contains("한빛고"), "바뀐 제목이 detail 에 남는다: {detail}");
}

/// 거부된 요청은 감사 로그도 남기지 않는다 (트랜잭션 진입 전에 검증한다).
#[tokio::test]
async fn rejected_update_writes_no_audit_log() {
    let pool = common::create_test_pool().await;
    let _ = save(&pool, "", "설명").await;

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_log")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}
