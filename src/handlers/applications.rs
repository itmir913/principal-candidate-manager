use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::{auth::TeacherClaims, state::AppState};

// ── 비밀번호 변경 ──────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ChangePasswordBody {
    pub new_password: String,
}

pub async fn teacher_change_password(
    State(state): State<AppState>,
    Extension(claims): Extension<TeacherClaims>,
    Json(body): Json<ChangePasswordBody>,
) -> Result<StatusCode, ApiError> {
    if body.new_password.len() < 4 {
        return Err((StatusCode::BAD_REQUEST, "비밀번호는 4자 이상이어야 합니다".into()));
    }
    let hash = bcrypt::hash(&body.new_password, bcrypt::DEFAULT_COST)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    sqlx::query("UPDATE classes SET password_hash = ? WHERE grade = ? AND class_no = ?")
        .bind(&hash)
        .bind(claims.grade)
        .bind(claims.class_no)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

type ApiError = (StatusCode, String);

#[derive(Serialize, FromRow)]
pub struct ApplicationRow {
    pub student_id: i64,
    pub univ_id: i64,
    pub round_id: i64,
    pub confirmed: i64,
    pub abandoned: i64,
    pub student_code: String,
    pub name: String,
    pub grade: Option<i64>,
    pub class_no: Option<i64>,
    pub seq_no: Option<i64>,
    pub is_enrolled: i64,
    pub univ_name: String,
    pub track_name: String,
}

#[derive(Serialize, FromRow)]
pub struct StudentRow {
    pub id: i64,
    pub student_code: String,
    pub name: String,
    pub grade: Option<i64>,
    pub class_no: Option<i64>,
    pub seq_no: Option<i64>,
    pub is_enrolled: i64,
}

#[derive(Deserialize)]
pub struct ApplicationListQuery {
    pub round_id: Option<i64>,
    pub univ_id: Option<i64>,
}

#[derive(Deserialize)]
pub struct TeacherAppListQuery {
    pub round_id: Option<i64>,
}

#[derive(Deserialize)]
pub struct CreateApplicationBody {
    pub student_id: i64,
    pub univ_id: i64,
    pub round_id: i64,
}

// ── Admin ──────────────────────────────────────────────────────────

pub async fn admin_list_applications(
    State(state): State<AppState>,
    Query(q): Query<ApplicationListQuery>,
) -> Result<Json<Vec<ApplicationRow>>, ApiError> {
    let rows = sqlx::query_as::<_, ApplicationRow>(
        "SELECT a.student_id, a.univ_id, a.round_id, a.confirmed, a.abandoned,
                s.student_code, s.name, s.grade, s.class_no, s.seq_no, s.is_enrolled,
                u.univ_name, u.track_name
         FROM applications a
         JOIN students s ON a.student_id = s.id
         JOIN universities u ON a.univ_id = u.id
         WHERE (? IS NULL OR a.round_id = ?)
           AND (? IS NULL OR a.univ_id = ?)
         ORDER BY u.univ_name, u.track_name, s.grade, s.class_no, s.seq_no",
    )
    .bind(q.round_id).bind(q.round_id)
    .bind(q.univ_id).bind(q.univ_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(rows))
}

pub async fn abandon_application(
    State(state): State<AppState>,
    Path((sid, uid, rid)): Path<(i64, i64, i64)>,
) -> Result<StatusCode, ApiError> {
    sqlx::query(
        "UPDATE applications SET abandoned = 1
         WHERE student_id = ? AND univ_id = ? AND round_id = ?",
    )
    .bind(sid).bind(uid).bind(rid)
    .execute(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

// ── Teacher ────────────────────────────────────────────────────────

pub async fn teacher_list_students(
    State(state): State<AppState>,
    Extension(claims): Extension<TeacherClaims>,
) -> Result<Json<Vec<StudentRow>>, ApiError> {
    let rows = sqlx::query_as::<_, StudentRow>(
        "SELECT id, student_code, name, grade, class_no, seq_no, is_enrolled
         FROM students WHERE grade = ? AND class_no = ?
         ORDER BY seq_no",
    )
    .bind(claims.grade)
    .bind(claims.class_no)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(rows))
}

pub async fn teacher_list_applications(
    State(state): State<AppState>,
    Extension(claims): Extension<TeacherClaims>,
    Query(q): Query<TeacherAppListQuery>,
) -> Result<Json<Vec<ApplicationRow>>, ApiError> {
    let rows = sqlx::query_as::<_, ApplicationRow>(
        "SELECT a.student_id, a.univ_id, a.round_id, a.confirmed, a.abandoned,
                s.student_code, s.name, s.grade, s.class_no, s.seq_no, s.is_enrolled,
                u.univ_name, u.track_name
         FROM applications a
         JOIN students s ON a.student_id = s.id
         JOIN universities u ON a.univ_id = u.id
         WHERE s.grade = ? AND s.class_no = ?
           AND (? IS NULL OR a.round_id = ?)
         ORDER BY s.seq_no, u.univ_name",
    )
    .bind(claims.grade).bind(claims.class_no)
    .bind(q.round_id).bind(q.round_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(rows))
}

pub async fn teacher_create_application(
    State(state): State<AppState>,
    Extension(claims): Extension<TeacherClaims>,
    Json(body): Json<CreateApplicationBody>,
) -> Result<StatusCode, ApiError> {
    let round_status: Option<String> = sqlx::query_scalar(
        "SELECT status FROM rounds WHERE id = ?",
    )
    .bind(body.round_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match round_status.as_deref() {
        Some("OPEN") => {}
        Some(_) => return Err((StatusCode::BAD_REQUEST, "라운드가 OPEN 상태가 아닙니다".into())),
        None => return Err((StatusCode::NOT_FOUND, "라운드를 찾을 수 없습니다".into())),
    }

    let ok: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM students WHERE id = ? AND grade = ? AND class_no = ?)",
    )
    .bind(body.student_id)
    .bind(claims.grade)
    .bind(claims.class_no)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if !ok {
        return Err((StatusCode::FORBIDDEN, "해당 학생은 담당 학급이 아닙니다".into()));
    }

    sqlx::query(
        "INSERT OR IGNORE INTO applications (student_id, univ_id, round_id, confirmed, abandoned)
         VALUES (?, ?, ?, 1, 0)",
    )
    .bind(body.student_id)
    .bind(body.univ_id)
    .bind(body.round_id)
    .execute(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::CREATED)
}

pub async fn teacher_delete_application(
    State(state): State<AppState>,
    Extension(claims): Extension<TeacherClaims>,
    Path((sid, uid, rid)): Path<(i64, i64, i64)>,
) -> Result<StatusCode, ApiError> {
    let round_status: Option<String> = sqlx::query_scalar(
        "SELECT status FROM rounds WHERE id = ?",
    )
    .bind(rid)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if round_status.as_deref() != Some("OPEN") {
        return Err((StatusCode::BAD_REQUEST, "OPEN 라운드의 지원만 취소할 수 있습니다".into()));
    }

    let ok: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM students WHERE id = ? AND grade = ? AND class_no = ?)",
    )
    .bind(sid)
    .bind(claims.grade)
    .bind(claims.class_no)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if !ok {
        return Err((StatusCode::FORBIDDEN, "해당 학생은 담당 학급이 아닙니다".into()));
    }

    sqlx::query(
        "DELETE FROM applications WHERE student_id = ? AND univ_id = ? AND round_id = ?",
    )
    .bind(sid).bind(uid).bind(rid)
    .execute(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{auth::TeacherClaims, db::create_test_pool, state::AppState};
    use axum::{
        extract::{Path, State},
        http::StatusCode,
        Extension, Json,
    };

    fn make_state(pool: sqlx::SqlitePool) -> AppState {
        AppState { db: pool, jwt_secret: "test".into() }
    }

    fn teacher(grade: i64, class_no: i64) -> TeacherClaims {
        TeacherClaims { role: "teacher".into(), grade, class_no, exp: 9_999_999_999 }
    }

    /// 기본 픽스처: 학급 1-1, 학생 S001, 대학 1개, OPEN 라운드 반환 (sid, uid, rid)
    async fn setup(pool: &sqlx::SqlitePool) -> (i64, i64, i64) {
        let hash = bcrypt::hash("pass", 4u32).unwrap();
        sqlx::query(
            "INSERT INTO classes (grade, class_no, password_hash) VALUES (1, 1, ?)",
        )
        .bind(&hash)
        .execute(pool)
        .await
        .unwrap();

        let sid: i64 = sqlx::query_scalar(
            "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
             VALUES ('S001', '홍길동', 1, 1, 1, 1) RETURNING id",
        )
        .fetch_one(pool)
        .await
        .unwrap();

        let uid: i64 = sqlx::query_scalar(
            "INSERT INTO universities (univ_name, track_name, capacity) \
             VALUES ('서울대', '컴공', 5) RETURNING id",
        )
        .fetch_one(pool)
        .await
        .unwrap();

        let rid: i64 = sqlx::query_scalar(
            "INSERT INTO rounds (status, opened_at) VALUES ('OPEN', '2025-01-01T00:00:00Z') RETURNING id",
        )
        .fetch_one(pool)
        .await
        .unwrap();

        (sid, uid, rid)
    }

    // ── teacher_create_application ────────────────────────────────────

    #[tokio::test]
    async fn create_application_open_round_ok() {
        let pool = create_test_pool().await;
        let (sid, uid, rid) = setup(&pool).await;
        let res = teacher_create_application(
            State(make_state(pool.clone())),
            Extension(teacher(1, 1)),
            Json(CreateApplicationBody { student_id: sid, univ_id: uid, round_id: rid }),
        )
        .await;
        assert_eq!(res.unwrap(), StatusCode::CREATED);
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM applications")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn create_application_duplicate_is_silently_ignored() {
        let pool = create_test_pool().await;
        let (sid, uid, rid) = setup(&pool).await;
        let body = || Json(CreateApplicationBody { student_id: sid, univ_id: uid, round_id: rid });
        teacher_create_application(
            State(make_state(pool.clone())),
            Extension(teacher(1, 1)),
            body(),
        )
        .await
        .unwrap();
        // 두 번째 INSERT OR IGNORE — 에러 없이 통과해야 함
        let res = teacher_create_application(
            State(make_state(pool.clone())),
            Extension(teacher(1, 1)),
            body(),
        )
        .await;
        assert!(res.is_ok());
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM applications")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count, 1); // 여전히 1건
    }

    #[tokio::test]
    async fn create_application_closed_round_returns_bad_request() {
        let pool = create_test_pool().await;
        let (sid, uid, _) = setup(&pool).await;
        let rid: i64 = sqlx::query_scalar(
            "INSERT INTO rounds (status, opened_at, closed_at) \
             VALUES ('CLOSED', '2025-01-01T00:00:00Z', '2025-01-02T00:00:00Z') RETURNING id",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let res = teacher_create_application(
            State(make_state(pool)),
            Extension(teacher(1, 1)),
            Json(CreateApplicationBody { student_id: sid, univ_id: uid, round_id: rid }),
        )
        .await;
        assert_eq!(res.unwrap_err().0, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn create_application_round_not_found_returns_not_found() {
        let pool = create_test_pool().await;
        let (sid, uid, _) = setup(&pool).await;
        let res = teacher_create_application(
            State(make_state(pool)),
            Extension(teacher(1, 1)),
            Json(CreateApplicationBody { student_id: sid, univ_id: uid, round_id: 9999 }),
        )
        .await;
        assert_eq!(res.unwrap_err().0, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn create_application_student_not_in_class_returns_forbidden() {
        let pool = create_test_pool().await;
        let (sid, uid, rid) = setup(&pool).await;
        // student S001은 1-1 소속인데 2-2 담임이 지원 시도
        let res = teacher_create_application(
            State(make_state(pool)),
            Extension(teacher(2, 2)),
            Json(CreateApplicationBody { student_id: sid, univ_id: uid, round_id: rid }),
        )
        .await;
        assert_eq!(res.unwrap_err().0, StatusCode::FORBIDDEN);
    }

    // ── teacher_delete_application ────────────────────────────────────

    #[tokio::test]
    async fn delete_application_open_round_ok() {
        let pool = create_test_pool().await;
        let (sid, uid, rid) = setup(&pool).await;
        sqlx::query(
            "INSERT INTO applications (student_id, univ_id, round_id, confirmed, abandoned) \
             VALUES (?, ?, ?, 1, 0)",
        )
        .bind(sid).bind(uid).bind(rid)
        .execute(&pool)
        .await
        .unwrap();
        teacher_delete_application(
            State(make_state(pool.clone())),
            Extension(teacher(1, 1)),
            Path((sid, uid, rid)),
        )
        .await
        .unwrap();
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM applications")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn delete_application_closed_round_returns_bad_request() {
        let pool = create_test_pool().await;
        let (sid, uid, _) = setup(&pool).await;
        // CLOSED 라운드 직접 삽입
        let rid: i64 = sqlx::query_scalar(
            "INSERT INTO rounds (status, opened_at, closed_at) \
             VALUES ('CLOSED', '2025-01-01T00:00:00Z', '2025-01-02T00:00:00Z') RETURNING id",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO applications (student_id, univ_id, round_id, confirmed, abandoned) \
             VALUES (?, ?, ?, 1, 0)",
        )
        .bind(sid).bind(uid).bind(rid)
        .execute(&pool)
        .await
        .unwrap();
        let res = teacher_delete_application(
            State(make_state(pool)),
            Extension(teacher(1, 1)),
            Path((sid, uid, rid)),
        )
        .await;
        assert_eq!(res.unwrap_err().0, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn delete_application_wrong_class_returns_forbidden() {
        let pool = create_test_pool().await;
        let (sid, uid, rid) = setup(&pool).await;
        sqlx::query(
            "INSERT INTO applications (student_id, univ_id, round_id, confirmed, abandoned) \
             VALUES (?, ?, ?, 1, 0)",
        )
        .bind(sid).bind(uid).bind(rid)
        .execute(&pool)
        .await
        .unwrap();
        let res = teacher_delete_application(
            State(make_state(pool)),
            Extension(teacher(2, 2)), // 틀린 학급
            Path((sid, uid, rid)),
        )
        .await;
        assert_eq!(res.unwrap_err().0, StatusCode::FORBIDDEN);
    }

    // ── abandon_application ───────────────────────────────────────────

    #[tokio::test]
    async fn abandon_application_sets_abandoned_flag() {
        let pool = create_test_pool().await;
        let (sid, uid, rid) = setup(&pool).await;
        sqlx::query(
            "INSERT INTO applications (student_id, univ_id, round_id, confirmed, abandoned) \
             VALUES (?, ?, ?, 1, 0)",
        )
        .bind(sid).bind(uid).bind(rid)
        .execute(&pool)
        .await
        .unwrap();
        abandon_application(State(make_state(pool.clone())), Path((sid, uid, rid)))
            .await
            .unwrap();
        let abandoned: i64 = sqlx::query_scalar(
            "SELECT abandoned FROM applications \
             WHERE student_id = ? AND univ_id = ? AND round_id = ?",
        )
        .bind(sid).bind(uid).bind(rid)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(abandoned, 1);
    }
}
