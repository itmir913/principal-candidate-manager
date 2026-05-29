mod common;

use axum::{extract::State, http::StatusCode, Extension, Json};
use principal_candidate_manager::handlers::auth::{
    admin_login, admin_status, change_admin_password, teacher_login, AdminLoginBody,
    ChangePasswordBody, TeacherLoginBody,
};

async fn make_state() -> principal_candidate_manager::state::AppState {
    let pool = common::create_test_pool().await;
    sqlx::query("INSERT INTO app_configs (key, value) VALUES ('admin_password_hash', '')")
        .execute(&pool)
        .await
        .unwrap();
    common::make_state(pool)
}

async fn insert_class_pw(
    pool: &sqlx::SqlitePool,
    grade: i64,
    class_no: i64,
    pw: &str,
) {
    let hash = bcrypt::hash(pw, 4u32).unwrap();
    sqlx::query("INSERT INTO classes (grade, class_no, password_hash) VALUES (?, ?, ?)")
        .bind(grade)
        .bind(class_no)
        .bind(hash)
        .execute(pool)
        .await
        .unwrap();
}

// ── admin_status ─────────────────────────────────────────────────

#[tokio::test]
async fn admin_status_not_initialized() {
    let state = make_state().await;
    let Json(res) = admin_status(State(state)).await.unwrap();
    assert!(!res.initialized);
}

#[tokio::test]
async fn admin_status_initialized_after_first_login() {
    let state = make_state().await;
    let _ = admin_login(
        State(state.clone()),
        Json(AdminLoginBody { password: "anypassword".into() }),
    )
    .await
    .unwrap();
    let Json(res) = admin_status(State(state)).await.unwrap();
    assert!(res.initialized);
}

// ── admin_login ───────────────────────────────────────────────────

#[tokio::test]
async fn admin_login_first_call_sets_hash_and_returns_token() {
    let state = make_state().await;
    let res = admin_login(
        State(state.clone()),
        Json(AdminLoginBody { password: "init_pw_123".into() }),
    )
    .await;
    assert!(res.is_ok(), "{:?}", res.err());
    let hash: String =
        sqlx::query_scalar("SELECT value FROM app_configs WHERE key = 'admin_password_hash'")
            .fetch_one(&state.db)
            .await
            .unwrap();
    assert!(!hash.is_empty());
    assert!(!res.unwrap().0.token.is_empty());
}

#[tokio::test]
async fn admin_login_correct_password_succeeds() {
    let state = make_state().await;
    let _ = admin_login(
        State(state.clone()),
        Json(AdminLoginBody { password: "correct_pw".into() }),
    )
    .await
    .unwrap();
    let res = admin_login(
        State(state),
        Json(AdminLoginBody { password: "correct_pw".into() }),
    )
    .await;
    assert!(res.is_ok());
}

#[tokio::test]
async fn admin_login_wrong_password_returns_unauthorized() {
    let state = make_state().await;
    let _ = admin_login(
        State(state.clone()),
        Json(AdminLoginBody { password: "correct".into() }),
    )
    .await
    .unwrap();
    let res = admin_login(
        State(state),
        Json(AdminLoginBody { password: "wrong".into() }),
    )
    .await;
    assert_eq!(res.unwrap_err().0, StatusCode::UNAUTHORIZED);
}

// ── teacher_login ─────────────────────────────────────────────────

#[tokio::test]
async fn teacher_login_success() {
    let state = make_state().await;
    insert_class_pw(&state.db, 1, 1, "pass1234").await;
    let res = teacher_login(
        State(state),
        Json(TeacherLoginBody { grade: 1, class_no: 1, password: "pass1234".into() }),
    )
    .await;
    assert!(res.is_ok());
    assert!(!res.unwrap().0.token.is_empty());
}

#[tokio::test]
async fn teacher_login_class_not_found_returns_not_found() {
    let state = make_state().await;
    let res = teacher_login(
        State(state),
        Json(TeacherLoginBody { grade: 9, class_no: 9, password: "any".into() }),
    )
    .await;
    assert_eq!(res.unwrap_err().0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn teacher_login_wrong_password_returns_unauthorized() {
    let state = make_state().await;
    insert_class_pw(&state.db, 2, 3, "correct").await;
    let res = teacher_login(
        State(state),
        Json(TeacherLoginBody { grade: 2, class_no: 3, password: "wrong".into() }),
    )
    .await;
    assert_eq!(res.unwrap_err().0, StatusCode::UNAUTHORIZED);
}

// ── change_admin_password ─────────────────────────────────────────

#[tokio::test]
async fn change_admin_password_wrong_current_returns_unauthorized() {
    let state = make_state().await;
    let _ = admin_login(
        State(state.clone()),
        Json(AdminLoginBody { password: "current_pw".into() }),
    )
    .await
    .unwrap();
    let res = change_admin_password(
        State(state),
        Extension(common::admin_claims()),
        Json(ChangePasswordBody {
            current_password: "wrong_pw".into(),
            new_password: "newpassword123".into(),
        }),
    )
    .await;
    assert_eq!(res.unwrap_err().0, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn change_admin_password_new_too_short_returns_bad_request() {
    let state = make_state().await;
    let _ = admin_login(
        State(state.clone()),
        Json(AdminLoginBody { password: "current_pw".into() }),
    )
    .await
    .unwrap();
    let res = change_admin_password(
        State(state),
        Extension(common::admin_claims()),
        Json(ChangePasswordBody {
            current_password: "current_pw".into(),
            new_password: "short".into(),
        }),
    )
    .await;
    assert_eq!(res.unwrap_err().0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn change_admin_password_success_allows_new_password() {
    let state = make_state().await;
    let _ = admin_login(
        State(state.clone()),
        Json(AdminLoginBody { password: "old_password".into() }),
    )
    .await
    .unwrap();
    change_admin_password(
        State(state.clone()),
        Extension(common::admin_claims()),
        Json(ChangePasswordBody {
            current_password: "old_password".into(),
            new_password: "new_password_123".into(),
        }),
    )
    .await
    .unwrap();

    let ok = admin_login(
        State(state.clone()),
        Json(AdminLoginBody { password: "new_password_123".into() }),
    )
    .await;
    assert!(ok.is_ok());

    let fail = admin_login(
        State(state),
        Json(AdminLoginBody { password: "old_password".into() }),
    )
    .await;
    assert_eq!(fail.unwrap_err().0, StatusCode::UNAUTHORIZED);
}
