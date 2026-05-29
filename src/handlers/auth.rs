use axum::{extract::State, http::StatusCode, Extension, Json};
use serde::{Deserialize, Serialize};

use crate::{auth, state::AppState};

#[derive(Deserialize)]
pub struct AdminLoginBody {
    pub password: String,
}

#[derive(Deserialize)]
pub struct TeacherLoginBody {
    pub grade: i64,
    pub class_no: i64,
    pub password: String,
}

#[derive(Deserialize)]
pub struct ChangePasswordBody {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub token: String,
}

type ApiError = (StatusCode, String);

#[derive(Serialize)]
pub struct AdminStatusResponse {
    pub initialized: bool,
}

pub async fn admin_status(
    State(state): State<AppState>,
) -> Result<Json<AdminStatusResponse>, ApiError> {
    let hash: Option<String> = sqlx::query_scalar(
        "SELECT value FROM app_configs WHERE key = 'admin_password_hash'",
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let initialized = hash.map(|h| !h.is_empty()).unwrap_or(false);
    Ok(Json(AdminStatusResponse { initialized }))
}

pub async fn admin_login(
    State(state): State<AppState>,
    Json(body): Json<AdminLoginBody>,
) -> Result<Json<TokenResponse>, ApiError> {
    let hash: Option<String> = sqlx::query_scalar(
        "SELECT value FROM app_configs WHERE key = 'admin_password_hash'",
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let hash = hash.ok_or((StatusCode::INTERNAL_SERVER_ERROR, "설정값 없음".into()))?;

    if hash.is_empty() {
        let new_hash = bcrypt::hash(&body.password, bcrypt::DEFAULT_COST)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        sqlx::query("UPDATE app_configs SET value = ? WHERE key = 'admin_password_hash'")
            .bind(&new_hash)
            .execute(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    } else {
        let ok = bcrypt::verify(&body.password, &hash)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        if !ok {
            return Err((StatusCode::UNAUTHORIZED, "비밀번호가 틀렸습니다".into()));
        }
    }

    let token = auth::encode_admin_token(&state.jwt_secret)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(TokenResponse { token }))
}

pub async fn teacher_login(
    State(state): State<AppState>,
    Json(body): Json<TeacherLoginBody>,
) -> Result<Json<TokenResponse>, ApiError> {
    let hash: Option<String> = sqlx::query_scalar(
        "SELECT COALESCE(password_hash, '') FROM classes WHERE grade = ? AND class_no = ?",
    )
    .bind(body.grade)
    .bind(body.class_no)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let hash = hash.ok_or((StatusCode::NOT_FOUND, "해당 반이 없습니다".into()))?;

    if hash.is_empty() {
        return Err((StatusCode::UNAUTHORIZED, "비밀번호가 설정되지 않았습니다".into()));
    }

    let ok = bcrypt::verify(&body.password, &hash)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !ok {
        return Err((StatusCode::UNAUTHORIZED, "비밀번호가 틀렸습니다".into()));
    }

    let token = auth::encode_teacher_token(body.grade, body.class_no, &state.jwt_secret)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(TokenResponse { token }))
}

pub async fn change_admin_password(
    State(state): State<AppState>,
    Extension(_claims): Extension<auth::AdminClaims>,
    Json(body): Json<ChangePasswordBody>,
) -> Result<StatusCode, ApiError> {
    let current_hash: String = sqlx::query_scalar(
        "SELECT value FROM app_configs WHERE key = 'admin_password_hash'",
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "설정값 없음".into()))?;

    let ok = bcrypt::verify(&body.current_password, &current_hash)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !ok {
        return Err((StatusCode::UNAUTHORIZED, "현재 비밀번호가 틀렸습니다".into()));
    }

    if body.new_password.len() < 8 {
        return Err((StatusCode::BAD_REQUEST, "새 비밀번호는 8자 이상이어야 합니다".into()));
    }

    // bcrypt는 CPU 집약 — DB 접근 전 미리 계산
    let new_hash = bcrypt::hash(&body.new_password, bcrypt::DEFAULT_COST)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    sqlx::query("UPDATE app_configs SET value = ? WHERE key = 'admin_password_hash'")
        .bind(&new_hash)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{auth::AdminClaims, db::create_test_pool, state::AppState};
    use axum::{extract::State, http::StatusCode, Extension, Json};

    const JWT: &str = "test_secret_auth";

    async fn make_state() -> AppState {
        let pool = create_test_pool().await;
        sqlx::query("INSERT INTO app_configs (key, value) VALUES ('admin_password_hash', '')")
            .execute(&pool)
            .await
            .unwrap();
        AppState { db: pool, jwt_secret: JWT.to_string() }
    }

    async fn insert_class_pw(pool: &sqlx::SqlitePool, grade: i64, class_no: i64, pw: &str) {
        let hash = bcrypt::hash(pw, 4u32).unwrap();
        sqlx::query(
            "INSERT INTO classes (grade, class_no, password_hash) VALUES (?, ?, ?)",
        )
        .bind(grade).bind(class_no).bind(hash)
        .execute(pool).await.unwrap();
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
        admin_login(
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
        admin_login(
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
        admin_login(
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
        admin_login(
            State(state.clone()),
            Json(AdminLoginBody { password: "current_pw".into() }),
        )
        .await
        .unwrap();
        let dummy_claims = AdminClaims { role: "admin".into(), exp: 9_999_999_999 };
        let res = change_admin_password(
            State(state),
            Extension(dummy_claims),
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
        admin_login(
            State(state.clone()),
            Json(AdminLoginBody { password: "current_pw".into() }),
        )
        .await
        .unwrap();
        let dummy_claims = AdminClaims { role: "admin".into(), exp: 9_999_999_999 };
        let res = change_admin_password(
            State(state),
            Extension(dummy_claims),
            Json(ChangePasswordBody {
                current_password: "current_pw".into(),
                new_password: "short".into(), // 5자 < 8자
            }),
        )
        .await;
        assert_eq!(res.unwrap_err().0, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn change_admin_password_success_allows_new_password() {
        let state = make_state().await;
        admin_login(
            State(state.clone()),
            Json(AdminLoginBody { password: "old_password".into() }),
        )
        .await
        .unwrap();
        let dummy_claims = AdminClaims { role: "admin".into(), exp: 9_999_999_999 };
        change_admin_password(
            State(state.clone()),
            Extension(dummy_claims),
            Json(ChangePasswordBody {
                current_password: "old_password".into(),
                new_password: "new_password_123".into(),
            }),
        )
        .await
        .unwrap();

        // 새 비밀번호로 로그인 성공
        let ok = admin_login(
            State(state.clone()),
            Json(AdminLoginBody { password: "new_password_123".into() }),
        )
        .await;
        assert!(ok.is_ok());

        // 기존 비밀번호 거부
        let fail = admin_login(
            State(state),
            Json(AdminLoginBody { password: "old_password".into() }),
        )
        .await;
        assert_eq!(fail.unwrap_err().0, StatusCode::UNAUTHORIZED);
    }
}
