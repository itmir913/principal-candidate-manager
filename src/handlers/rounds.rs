use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Serialize;
use sqlx::FromRow;

use crate::audit::{Actor, AuditEntry};
use crate::enums::{AuditAction, RoundStatus};
use crate::handlers::scoring::run_calculate_scores_on_conn;
use crate::state::AppState;

type ApiError = (StatusCode, String);

#[derive(Serialize, FromRow)]
pub struct RoundRow {
    pub id: i64,
    pub status: RoundStatus,
    pub opened_at: String,
    pub closed_at: Option<String>,
    pub finalized_at: Option<String>,
    /// 마지막 점수 계산 이후 기초데이터가 바뀌었는가 (F-017). 자세한 판정은 `needs_recalc_expr`.
    pub needs_recalc: bool,
}

/// `results` 가 현재 기초데이터보다 낡았는지 판정하는 SQL 조각 — **단일 출처**.
/// `{r}` 자리에 rounds 테이블 별칭이 들어간다.
///
/// 왜 이렇게 파생하는가: `base_data` 에는 타임스탬프 컬럼이 없고 v1 스키마는 출시 후
/// 동결(`11_release_decisions.md` §7)이라 컬럼을 추가할 수 없다. 대신 이미 기록되는
/// 감사 로그(`BASE_DATA_IMPORTED`)의 시각과 `results.calculated_at` 을 비교한다.
///
/// 한계(의도된 과대 근사): `base_data` 는 라운드 스코프가 아니므로 다음 라운드용 데이터를
/// 올려도 현 CLOSED 라운드가 "재계산 필요"로 잡힐 수 있다. 재계산은 추천 상태를 보존하고
/// (§2.4) 부작용이 없으므로, 놓치는 쪽보다 과대 근사를 택했다.
///
/// 점수를 계산한 적이 없으면(`MIN(calculated_at)` = NULL) 비교가 NULL 이라 false 다 —
/// 낡을 결과 자체가 없기 때문이다.
fn needs_recalc_expr(r: &'static str) -> String {
    format!(
        "(CASE WHEN {r}.status = 'CLOSED' AND EXISTS(
             SELECT 1 FROM audit_log al
             WHERE al.action = 'BASE_DATA_IMPORTED'
               AND al.at > (SELECT MIN(res.calculated_at) FROM results res WHERE res.round_id = {r}.id)
         ) THEN 1 ELSE 0 END)"
    )
}

/// 추천 확정·마감 가드용 단일 판정 (F-017). 호출자는 라운드 존재를 이미 확인한 상태여야 한다.
pub(crate) async fn needs_recalc(
    conn: &mut sqlx::SqliteConnection,
    round_id: i64,
) -> Result<bool, ApiError> {
    sqlx::query_scalar(&format!(
        "SELECT {} FROM rounds r WHERE r.id = ?",
        needs_recalc_expr("r")
    ))
    .bind(round_id)
    .fetch_one(conn)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

/// 가드가 돌려줄 안내 — 추천 확정과 마감이 같은 문장을 쓴다.
pub(crate) const NEEDS_RECALC_MSG: &str =
    "기초데이터가 변경되어 점수가 최신이 아닙니다. 점수를 재계산한 뒤 다시 시도하세요.";

pub async fn list_rounds(
    State(state): State<AppState>,
) -> Result<Json<Vec<RoundRow>>, ApiError> {
    let rows = sqlx::query_as::<_, RoundRow>(&format!(
        "SELECT r.id, r.status, r.opened_at, r.closed_at, r.finalized_at,
                {} AS needs_recalc
         FROM rounds r ORDER BY r.id DESC",
        needs_recalc_expr("r")
    ))
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(rows))
}

pub async fn get_current_round(
    State(state): State<AppState>,
) -> Result<Json<Option<RoundRow>>, ApiError> {
    let row = sqlx::query_as::<_, RoundRow>(&format!(
        "SELECT r.id, r.status, r.opened_at, r.closed_at, r.finalized_at,
                {} AS needs_recalc
         FROM rounds r WHERE r.status = 'OPEN' LIMIT 1",
        needs_recalc_expr("r")
    ))
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(row))
}

pub async fn open_round(
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let now = chrono::Utc::now().to_rfc3339();
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // SELECT 후 INSERT 분리 시 TOCTOU race condition 발생 가능 —
    // INSERT ... SELECT ... WHERE NOT EXISTS 로 검사+삽입을 원자적으로 처리한다.
    let id: Option<i64> = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at)
         SELECT 'OPEN', ?
         WHERE NOT EXISTS (SELECT 1 FROM rounds WHERE status IN ('OPEN', 'CLOSED'))
         RETURNING id",
    )
    .bind(&now)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let id = id.ok_or_else(|| {
        (
            StatusCode::CONFLICT,
            "진행 중인 라운드가 있습니다. 모든 라운드가 마감된 후에만 새 라운드를 열 수 있습니다".to_string(),
        )
    })?;

    crate::audit::log(
        &mut *tx,
        AuditEntry {
            actor: Actor::Admin,
            action: AuditAction::RoundOpened,
            round_id: Some(id),
            student_id: None,
            detail: serde_json::json!({}),
        },
    )
    .await?;

    tx.commit()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

pub async fn close_round(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // BEGIN IMMEDIATE: 이 시점부터 다른 커넥션의 쓰기(base_data 수정 등)를 차단한다.
    // 검증 → status 변경 → 점수 계산 전체가 단일 원자적 블록.
    // sqlx 관리 트랜잭션: 오류 경로에서 tx drop 시 자동 ROLLBACK — status 변경도 함께 취소되어
    // round는 OPEN 유지, 커넥션 오염 없음.
    let mut tx = state
        .db
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 1. base_data 누락 검증
    let missing: Vec<(String, String, String, String, String)> = sqlx::query_as(
        "SELECT a.name, s.name, s.student_code, u.univ_name, ut.track_name
         FROM applications ap
         JOIN students s ON s.id = ap.student_id
         JOIN univ_tracks ut ON ut.id = ap.track_id
         JOIN universities u ON u.id = ut.univ_id
         CROSS JOIN areas a
         WHERE ap.round_id = ?
           AND NOT EXISTS (
             SELECT 1 FROM base_data bd
             WHERE bd.student_id = ap.student_id AND bd.area_id = a.id
               AND CASE WHEN a.lookup_scope = 'COMPOSITE'
                        THEN bd.track_id = ap.track_id
                        ELSE bd.track_id IS NULL END
           )
         LIMIT 5",
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if !missing.is_empty() {
        let details: Vec<String> = missing
            .iter()
            .map(|(area, student, code, univ, track)| {
                format!("전형요소 '{}': {} {} 지원자 {} ({})", area, univ, track, student, code)
            })
            .collect();
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("기초데이터 누락으로 라운드를 종료할 수 없습니다:\n{}", details.join("\n")),
        ));
    }

    // 2. OPEN → CLOSED 상태 변경 (점수 계산 실패 시 ROLLBACK으로 함께 취소됨)
    let now = chrono::Utc::now().to_rfc3339();
    let affected = sqlx::query(
        "UPDATE rounds SET status = 'CLOSED', closed_at = ? WHERE id = ? AND status = 'OPEN'",
    )
    .bind(&now)
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .rows_affected();

    if affected == 0 {
        return Err((StatusCode::NOT_FOUND, format!("라운드 id={} 없거나 이미 CLOSED", id)));
    }

    // 3. 점수 계산 — 실패 시 tx drop으로 자동 ROLLBACK, round는 OPEN으로 복귀
    let count = run_calculate_scores_on_conn(&mut *tx, id, &now)
        .await
        .map_err(|e| (StatusCode::UNPROCESSABLE_ENTITY, e))?;

    crate::audit::log(
        &mut *tx,
        AuditEntry {
            actor: Actor::Admin,
            action: AuditAction::RoundClosed,
            round_id: Some(id),
            student_id: None,
            detail: serde_json::json!({ "calculated": count }),
        },
    )
    .await?;

    tx.commit()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({ "calculated": count })))
}

pub async fn reopen_round(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let affected = sqlx::query(
        "UPDATE rounds SET status = 'OPEN', closed_at = NULL WHERE id = ? AND status = 'CLOSED'",
    )
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .rows_affected();

    if affected == 0 {
        return Err((StatusCode::NOT_FOUND, "라운드를 찾을 수 없거나 CLOSED 상태가 아닙니다".into()));
    }

    // 추천 플래그 및 순위 초기화 — 재계산 전 stale 데이터 노출 방지
    sqlx::query(
        "UPDATE results SET recommended = 0, ranking = NULL WHERE round_id = ?",
    )
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 미선발 플래그 초기화 — 추천과 동일하게 재개 시 리셋
    // (rounds.status가 이미 OPEN으로 변경된 후이므로 trg_prevent_update_closed_application 비활성)
    sqlx::query(
        "UPDATE applications SET excluded = 0, excluded_reason = NULL WHERE round_id = ?",
    )
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    crate::audit::log(
        &mut *tx,
        AuditEntry {
            actor: Actor::Admin,
            action: AuditAction::RoundReopened,
            round_id: Some(id),
            student_id: None,
            detail: serde_json::json!({}),
        },
    )
    .await?;

    tx.commit()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize, FromRow)]
struct UndecidedApplication {
    student_code: String,
    student_name: String,
    grade: i64,
    class_no: i64,
    univ_name: String,
    track_name: String,
}

#[derive(Serialize, FromRow)]
struct TrackOverQuota {
    track_name: String,
    univ_name: String,
    unit_quota: i64,
    total_recommended: i64,
}

#[derive(Serialize, FromRow)]
struct UnivOverQuota {
    univ_name: String,
    total_quota: i64,
    total_recommended: i64,
}

pub async fn finalize_round(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    // BEGIN IMMEDIATE: 상태 검증·정원 검증·status UPDATE를 원자적으로 처리.
    // 트랜잭션 없이 개별 pool 쿼리를 사용하면 두 요청이 동시에 CLOSED를 확인하고 둘 다 UPDATE할 수 있다.
    // sqlx 관리 트랜잭션: 오류 경로에서 tx drop 시 자동 ROLLBACK — 커넥션 오염 없음
    let mut tx = state.db.begin_with("BEGIN IMMEDIATE").await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // CLOSED 상태인지 확인 (UPDATE WHERE status='CLOSED' 가드와 이중 방어)
    let status: Option<String> = sqlx::query_scalar("SELECT status FROM rounds WHERE id = ?")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if status.as_deref() != Some("CLOSED") {
        return Err((StatusCode::NOT_FOUND, "라운드를 찾을 수 없거나 CLOSED 상태가 아닙니다".into()));
    }

    // 기초데이터가 계산 이후에 바뀌었으면 마감할 수 없다 (F-017).
    // 낡은 총점·순위로 결과를 확정하면 되돌릴 수 없다.
    if needs_recalc(&mut *tx, id).await? {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, NEEDS_RECALC_MSG.into()));
    }

    // 미결정 지원 검증 — 추천도 제외도 되지 않은 지원이 있으면 마감 불가.
    // COALESCE(r.recommended, 0) = 0: results 행 없음(점수 미계산)도 미결정에 포함한다(LEFT JOIN).
    // 이는 silent fallback이 아닌 "results 없음 = 미결정"이라는 의도된 3상태(추천/제외/미결정) 판정.
    let undecided: Vec<UndecidedApplication> = sqlx::query_as(
        "SELECT s.student_code, s.name AS student_name, s.grade, s.class_no,
                u.univ_name, ut.track_name
         FROM applications a
         JOIN students s      ON s.id  = a.student_id
         JOIN univ_tracks ut  ON ut.id = a.track_id
         JOIN universities u  ON u.id  = ut.univ_id
         LEFT JOIN results r  ON r.student_id = a.student_id
                             AND r.track_id   = a.track_id
                             AND r.round_id   = a.round_id
         WHERE a.round_id = ?
           AND a.excluded = 0
           AND COALESCE(r.recommended, 0) = 0
         ORDER BY u.univ_name, ut.track_name, s.grade, s.class_no, s.student_code",
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if !undecided.is_empty() {
        let body = serde_json::json!({
            "error": "추천 또는 제외가 결정되지 않은 지원자가 있어 라운드를 마감할 수 없습니다",
            "undecided": undecided,
        });
        return Err((StatusCode::UNPROCESSABLE_ENTITY, body.to_string()));
    }

    // 모집단위 정원 초과 검증 (unit_quota IS NOT NULL인 트랙만)
    let track_violations: Vec<TrackOverQuota> = sqlx::query_as(
        "SELECT ut.track_name, u.univ_name, ut.unit_quota,
                COUNT(*) AS total_recommended
         FROM results r
         JOIN applications a ON a.student_id = r.student_id
                             AND a.track_id  = r.track_id
                             AND a.round_id  = r.round_id
         JOIN univ_tracks ut ON ut.id = r.track_id
         JOIN universities u  ON u.id  = ut.univ_id
         WHERE r.recommended = 1 AND a.abandoned = 0
           AND ut.unit_quota IS NOT NULL
         GROUP BY ut.id
         HAVING COUNT(*) > ut.unit_quota
         LIMIT 5",
    )
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 대학 전체 정원 초과 검증 (total_quota IS NOT NULL인 대학만)
    let univ_violations: Vec<UnivOverQuota> = sqlx::query_as(
        "SELECT u.univ_name, u.total_quota,
                COUNT(*) AS total_recommended
         FROM results r
         JOIN applications a ON a.student_id = r.student_id
                             AND a.track_id  = r.track_id
                             AND a.round_id  = r.round_id
         JOIN univ_tracks ut ON ut.id = r.track_id
         JOIN universities u  ON u.id  = ut.univ_id
         WHERE r.recommended = 1 AND a.abandoned = 0
           AND u.total_quota IS NOT NULL
         GROUP BY u.id
         HAVING COUNT(*) > u.total_quota
         LIMIT 5",
    )
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if !track_violations.is_empty() || !univ_violations.is_empty() {
        let body = serde_json::json!({
            "error": "정원 초과로 라운드를 확정할 수 없습니다",
            "track_violations": track_violations,
            "univ_violations": univ_violations,
        });
        return Err((StatusCode::UNPROCESSABLE_ENTITY, body.to_string()));
    }

    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE rounds SET status = 'FINALIZED', finalized_at = ? WHERE id = ? AND status = 'CLOSED'",
    )
    .bind(&now)
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    crate::audit::log(
        &mut *tx,
        AuditEntry {
            actor: Actor::Admin,
            action: AuditAction::RoundFinalized,
            round_id: Some(id),
            student_id: None,
            detail: serde_json::json!({}),
        },
    )
    .await?;

    tx.commit().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}
