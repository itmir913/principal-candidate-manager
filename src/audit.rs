use axum::http::StatusCode;
use sqlx::SqliteConnection;

use crate::enums::AuditAction;

type ApiError = (StatusCode, String);

/// 감사 로그 행위자. Teacher는 JWT claims의 grade·class_no를 그대로 넣는다.
pub enum Actor {
    Admin,
    Teacher { grade: i64, class_no: i64 },
}

pub struct AuditEntry {
    pub actor: Actor,
    pub action: AuditAction,
    pub round_id: Option<i64>,
    pub student_id: Option<i64>,
    pub detail: serde_json::Value,
}

/// 감사 로그 기록. 반드시 본 작업과 같은 트랜잭션에서 `&mut *tx`로 호출할 것
/// (find_or_create_track과 동일한 커넥션 규칙 — pool 직접 전달 금지).
/// 실패 시 Err → 호출측 `?` 전파 → 본 작업까지 롤백된다 (fail-fast).
pub async fn log(conn: &mut SqliteConnection, entry: AuditEntry) -> Result<(), ApiError> {
    let (actor_type, grade, class_no) = match entry.actor {
        Actor::Admin => ("ADMIN", None, None),
        Actor::Teacher { grade, class_no } => ("TEACHER", Some(grade), Some(class_no)),
    };

    // TEACHER: 행위 시점 담임명 스냅샷.
    // grade=0, class_no=0은 졸업생 담당 가상 계정 — classes 행이 없으므로 고정값 사용.
    // 일반 학급 계정이 없으면 fail-fast.
    let actor_name: Option<String> = match (grade, class_no) {
        (Some(0), Some(0)) => Some("졸업생".to_string()),
        (Some(g), Some(c)) => {
            let row: Option<Option<String>> = sqlx::query_scalar(
                "SELECT teacher_name FROM classes WHERE grade = ? AND class_no = ?",
            )
            .bind(g)
            .bind(c)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            match row {
                Some(name) => name, // teacher_name 컬럼 자체가 NULL이면 None 그대로 허용
                None => {
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("감사 기록 실패: {g}학년 {c}반 계정이 존재하지 않습니다"),
                    ))
                }
            }
        }
        _ => None,
    };

    let detail = serde_json::to_string(&entry.detail)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    sqlx::query(
        "INSERT INTO audit_log \
         (at, actor_type, actor_grade, actor_class_no, actor_name, action, round_id, student_id, detail) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(chrono::Utc::now().to_rfc3339())
    .bind(actor_type)
    .bind(grade)
    .bind(class_no)
    .bind(actor_name)
    .bind(entry.action)
    .bind(entry.round_id)
    .bind(entry.student_id)
    .bind(detail)
    .execute(&mut *conn)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(())
}

/// 지원 관련 액션의 공통 detail: 학생·대학·모집단위 스냅샷.
/// 대상이 반드시 존재하는 시점(본 작업 검증 통과 후)에 호출한다.
pub async fn application_detail(
    conn: &mut SqliteConnection,
    student_id: i64,
    track_id: i64,
) -> Result<serde_json::Value, ApiError> {
    let row: Option<(String, String, String, String)> = sqlx::query_as(
        "SELECT s.student_code, s.name, u.univ_name, t.track_name
         FROM students s, univ_tracks t
         JOIN universities u ON u.id = t.univ_id
         WHERE s.id = ? AND t.id = ?",
    )
    .bind(student_id)
    .bind(track_id)
    .fetch_optional(&mut *conn)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let (student_code, student_name, univ_name, track_name) = row.ok_or((
        StatusCode::INTERNAL_SERVER_ERROR,
        "감사 기록 실패: 학생 또는 모집단위를 찾을 수 없습니다".to_string(),
    ))?;

    Ok(serde_json::json!({
        "student_code": student_code,
        "student_name": student_name,
        "univ_name": univ_name,
        "track_name": track_name,
    }))
}
