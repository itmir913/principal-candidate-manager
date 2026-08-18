use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Response,
    Extension, Json,
};
use rust_xlsxwriter::Workbook;
use serde::{Deserialize, Serialize, Serializer};
use sqlx::{FromRow, Row};
use std::collections::HashMap;

use crate::{
    audit::{Actor, AuditEntry},
    auth::TeacherClaims,
    enums::{AuditAction, CalcType, CategoryAgg, LookupScope, MatchMode, RoundStatus},
    excel, score::Score, state::AppState,
};

type ApiError = (StatusCode, String);

/// 모집단위 track_rank 윈도우 함수 SQL 단편 — 단일 출처.
///
/// 인자는 SQL 테이블 별칭이다: r(results), ut(univ_tracks), s(students).
/// 호출부마다 별칭이 다르므로(r/ut/s 또는 r2/ut2/s2) 감싸는 쿼리의 FROM·JOIN 절과
/// 반드시 일치시킬 것. 어긋나면 컴파일은 되고 런타임에 "no such column"으로 깨진다.
///
/// `partition_by_round` 의 판단 기준은 **쿼리가 여러 라운드를 걸치는가** 하나뿐이다
/// (CTE인지 인라인인지와는 무관 — 두 형태 모두 양쪽 값에 존재한다):
/// - true  → `PARTITION BY {r}.track_id, {r}.round_id`
///   여러 라운드 행이 섞이는 쿼리. 빠뜨리면 라운드 간 순위가 한 덩어리로 매겨진다.
/// - false → `PARTITION BY {r}.track_id`
///   `WHERE round_id = ?` 로 단일 라운드가 이미 고정된 쿼리.
///
/// 별칭은 `&'static str` — 호출부에서 반드시 리터럴이어야 한다. 이 함수는 SQL을
/// 문자열로 조립하므로 런타임 값을 받으면 인젝션 경로가 된다.
fn track_rank_window(r: &'static str, ut: &'static str, s: &'static str, partition_by_round: bool) -> String {
    let partition = if partition_by_round {
        format!("{r}.track_id, {r}.round_id")
    } else {
        format!("{r}.track_id")
    };
    format!(
        "CAST(RANK() OVER (
                    PARTITION BY {partition}
                    ORDER BY
                        CASE WHEN {ut}.prioritize_enrolled = 1 THEN {s}.is_enrolled ELSE NULL END DESC NULLS LAST,
                        {r}.total_score DESC
                ) AS INTEGER) AS track_rank"
    )
}

fn score_detail_as_map<S: Serializer>(val: &str, s: S) -> Result<S::Ok, S::Error> {
    use serde::ser::{Error, SerializeMap};
    let raw: HashMap<String, i64> = serde_json::from_str(val)
        .map_err(|e| S::Error::custom(format!("score_detail JSON 파싱 실패: {}", e)))?;
    let mut m = s.serialize_map(Some(raw.len()))?;
    for (k, v) in &raw {
        m.serialize_entry(k, &Score::from_raw(*v))?;
    }
    m.end()
}

#[derive(FromRow)]
pub struct AreaRow {
    pub id: i64,
    pub name: String,
    pub calc_type: CalcType,
    pub max_score: i64,
    pub match_mode: Option<MatchMode>,
    pub category_agg: Option<CategoryAgg>,
    pub lookup_scope: LookupScope,
}

#[derive(FromRow)]
struct AppRef {
    student_id: i64,
    track_id: i64,
    student_code: String,
    name: String,
    univ_name: String,
    track_name: String,
}

pub struct StudentTrackCtx {
    pub student_code: String,
    pub student_name: String,
    pub univ_name: String,
    pub track_name: String,
}

#[derive(Serialize, FromRow)]
pub struct ResultRow {
    pub student_id: i64,
    pub track_id: i64,
    pub round_id: i64,
    pub total_score: Score,
    #[serde(serialize_with = "score_detail_as_map")]
    pub score_detail: String,
    pub ranking: Option<i64>,
    pub track_rank: Option<i64>,
    pub recommended: bool,
    pub abandoned: bool,
    pub excluded: bool,
    pub excluded_reason: Option<String>,
    pub student_code: String,
    pub name: String,
    pub grade: Option<i64>,
    pub class_no: Option<i64>,
    pub seq_no: Option<i64>,
    pub is_enrolled: bool,
    pub univ_name: String,
    pub track_name: String,
    pub department_name: String,
}

#[derive(Deserialize)]
pub struct ResultQuery {
    pub track_id: Option<i64>,
}

// ── Scoring helpers ───────────────────────────────────────────────

pub fn lookup_range_score(value: i64, rows: &[(i64, i64)], direction: MatchMode) -> Result<i64, String> {
    match direction {
        MatchMode::Upper => rows
            .iter()
            .filter(|(th, _)| value >= *th)
            .max_by_key(|(th, _)| *th)
            .map(|(_, sc)| *sc)
            .ok_or_else(|| {
                format!("UPPER 매칭 실패: 값 {}에 해당하는 구간 항목이 없습니다 (모든 하한치보다 낮습니다)", value as f64 / 100_000.0)
            }),
        MatchMode::Lower => {
            if rows.is_empty() {
                return Err(format!("LOWER 매칭 실패: 구간 테이블이 비어 있습니다"));
            }
            // threshold가 허용 상한선 역할: value <= threshold인 행 중 최소 threshold 선택.
            // value가 최대 threshold를 초과하면("5일 이상: 5점") 최대 threshold 행의 점수 사용.
            Ok(rows
                .iter()
                .filter(|(th, _)| value <= *th)
                .min_by_key(|(th, _)| *th)
                .map(|(_, sc)| *sc)
                .unwrap_or_else(|| {
                    // rows is non-empty here, so max_by_key always returns Some
                    rows.iter().max_by_key(|(th, _)| *th).map(|(_, sc)| *sc).unwrap()
                }))
        }
        MatchMode::Exact => rows
            .iter()
            .find(|(th, _)| *th == value)
            .map(|(_, sc)| *sc)
            .ok_or_else(|| format!("EXACT 매칭 실패: 값 {}에 해당하는 구간 항목이 없습니다", value as f64 / 100_000.0)),
    }
}

/// 확정 점수 계산 — base_data에서 값을 읽어 `compute_area_score` 헬퍼로 위임한다.
/// 담임 미리보기(`teacher_area_score_preview`)도 반드시 같은 헬퍼를 통과하도록 짜여 있어,
/// 두 경로의 시맨틱이 갈릴 여지가 없다.
pub async fn calc_area_score(
    db: &mut sqlx::SqliteConnection,
    student_id: i64,
    area: &AreaRow,
    track_id: i64,
    ctx: &StudentTrackCtx,
) -> Result<i64, String> {
    // COMPOSITE 전형요소는 모집단위별 데이터 사용, SIMPLE은 전역 데이터
    let lookup_track: Option<i64> = if area.lookup_scope == LookupScope::Composite {
        Some(track_id)
    } else {
        None
    };

    let meta = AreaMeta {
        id: area.id,
        name: &area.name,
        max_score: area.max_score,
        match_mode: area.match_mode,
        category_agg: area.category_agg,
    };

    let wrap = |msg: String| format!(
        "전형요소 '{}': {} {} 지원자 {} ({}) - {}",
        area.name, ctx.univ_name, ctx.track_name, ctx.student_name, ctx.student_code, msg,
    );

    let outcome = match area.calc_type {
        CalcType::Numeric => {
            let vs: Option<String> = sqlx::query_scalar(
                "SELECT value FROM base_data
                 WHERE student_id = ? AND area_id = ?
                   AND (track_id = ? OR (? IS NULL AND track_id IS NULL))",
            )
            .bind(student_id).bind(area.id).bind(lookup_track).bind(lookup_track)
            .fetch_optional(&mut *db).await.map_err(|e| e.to_string())?;

            let s = vs.ok_or_else(|| format!(
                "전형요소 '{}': {} {} 지원자 {} ({})의 NUMERIC base_data가 없습니다",
                area.name, ctx.univ_name, ctx.track_name, ctx.student_name, ctx.student_code
            ))?;
            let value: i64 = s.trim().parse::<i64>().map_err(|_| format!(
                "전형요소 '{}': {} {} 지원자 {} ({})의 base_data 값 '{}' 을 정수로 파싱할 수 없습니다",
                area.name, ctx.univ_name, ctx.track_name, ctx.student_name, ctx.student_code, s.trim()
            ))?;

            compute_area_score(db, &meta, AreaScoreInput::Numeric(value), lookup_track)
                .await
                .map_err(|e| wrap(e.to_string()))?
        }

        CalcType::Category => {
            let values: Vec<String> = sqlx::query_scalar(
                "SELECT value FROM base_data
                 WHERE student_id = ? AND area_id = ?
                   AND (track_id = ? OR (? IS NULL AND track_id IS NULL))",
            )
            .bind(student_id).bind(area.id).bind(lookup_track).bind(lookup_track)
            .fetch_all(&mut *db).await.map_err(|e| e.to_string())?;

            if values.is_empty() {
                return Err(format!(
                    "전형요소 '{}': {} {} 지원자 {} ({})의 CATEGORY base_data가 없습니다",
                    area.name, ctx.univ_name, ctx.track_name, ctx.student_name, ctx.student_code
                ));
            }

            // CategoryUnknown은 확정 계산 오류 문구를 별도로 유지(범주명 명시).
            compute_area_score(db, &meta, AreaScoreInput::Category(&values), lookup_track)
                .await
                .map_err(|e| match e {
                    AreaScoreError::CategoryUnknown(cat) => format!(
                        "전형요소 '{}': {} {} 지원자 {} ({})에 대해 범주 '{}' 에 해당하는 category_map 항목이 없습니다",
                        area.name, ctx.univ_name, ctx.track_name, ctx.student_name, ctx.student_code, cat
                    ),
                    other => wrap(other.to_string()),
                })?
        }

        CalcType::Manual => {
            let v: Option<String> = sqlx::query_scalar(
                "SELECT value FROM base_data
                 WHERE student_id = ? AND area_id = ?
                   AND (track_id = ? OR (? IS NULL AND track_id IS NULL))",
            )
            .bind(student_id).bind(area.id).bind(lookup_track).bind(lookup_track)
            .fetch_optional(&mut *db).await.map_err(|e| e.to_string())?;

            let s = v.ok_or_else(|| format!(
                "전형요소 '{}': {} {} 지원자 {} ({})의 MANUAL base_data가 없습니다",
                area.name, ctx.univ_name, ctx.track_name, ctx.student_name, ctx.student_code
            ))?;
            let value: i64 = s.trim().parse::<i64>().map_err(|_| format!(
                "전형요소 '{}': {} {} 지원자 {} ({})의 MANUAL base_data 값 '{}' 을 정수로 파싱할 수 없습니다",
                area.name, ctx.univ_name, ctx.track_name, ctx.student_name, ctx.student_code, s.trim()
            ))?;

            compute_area_score(db, &meta, AreaScoreInput::Manual(value), lookup_track)
                .await
                .map_err(|e| wrap(e.to_string()))?
        }
    };

    Ok(outcome.raw.min(area.max_score))
}

// ── Handlers ──────────────────────────────────────────────────────

/// 단일 커넥션 위에서 점수 계산·results 저장·순위 계산을 수행한다.
/// 트랜잭션 관리는 호출자 책임. close_round 와 calculate_scores 양쪽에서 재사용한다.
pub async fn run_calculate_scores_on_conn(
    conn: &mut sqlx::SqliteConnection,
    round_id: i64,
    now: &str,
) -> Result<usize, String> {
    let areas: Vec<AreaRow> = sqlx::query_as::<_, AreaRow>(
        "SELECT id, name, calc_type, max_score, match_mode, category_agg, lookup_scope FROM areas ORDER BY id",
    )
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| e.to_string())?;

    let applications: Vec<AppRef> = sqlx::query_as::<_, AppRef>(
        "SELECT a.student_id, a.track_id, s.student_code, s.name, u.univ_name, ut.track_name
         FROM applications a
         JOIN students s ON s.id = a.student_id
         JOIN univ_tracks ut ON ut.id = a.track_id
         JOIN universities u ON u.id = ut.univ_id
         WHERE a.round_id = ?",
    )
    .bind(round_id)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| e.to_string())?;

    let mut count = 0usize;

    for app in &applications {
        let ctx = StudentTrackCtx {
            student_code: app.student_code.clone(),
            student_name: app.name.clone(),
            univ_name: app.univ_name.clone(),
            track_name: app.track_name.clone(),
        };
        let mut detail: HashMap<String, i64> = HashMap::new();
        let mut total: i64 = 0;
        for area in &areas {
            let sc = calc_area_score(&mut *conn, app.student_id, area, app.track_id, &ctx)
                .await?;
            detail.insert(area.id.to_string(), sc);
            total = total.checked_add(sc).ok_or_else(|| {
                format!("점수 합산 오버플로우: 지원자 {} ({})", ctx.student_name, ctx.student_code)
            })?;
        }
        let detail_json = serde_json::to_string(&detail).map_err(|e| e.to_string())?;

        sqlx::query(
            "INSERT INTO results
               (student_id, track_id, round_id, score_detail, total_score, ranking, recommended, calculated_at)
             VALUES (?, ?, ?, ?, ?, NULL, 0, ?)
             ON CONFLICT (student_id, track_id, round_id)
             DO UPDATE SET score_detail   = excluded.score_detail,
                           total_score    = excluded.total_score,
                           ranking        = NULL,
                           calculated_at  = excluded.calculated_at",
        )
        .bind(app.student_id).bind(app.track_id).bind(round_id)
        .bind(&detail_json).bind(total).bind(now)
        .execute(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
        count += 1;
    }

    // 대학 전체 순위 재계산 — univ 파티션, universities.prioritize_enrolled만 사용
    let univ_ids: Vec<i64> = sqlx::query_scalar(
        "SELECT DISTINCT ut.univ_id FROM applications a
         JOIN univ_tracks ut ON ut.id = a.track_id
         WHERE a.round_id = ?",
    )
    .bind(round_id)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| e.to_string())?;

    for univ_id in univ_ids {
        let prioritize: bool = sqlx::query_scalar(
            "SELECT prioritize_enrolled = 1 FROM universities WHERE id = ?",
        )
        .bind(univ_id)
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;

        let rows = sqlx::query(
            "SELECT r.student_id, r.track_id, r.total_score, s.is_enrolled
             FROM results r
             JOIN students s ON r.student_id = s.id
             JOIN univ_tracks ut ON ut.id = r.track_id
             WHERE r.round_id = ? AND ut.univ_id = ?",
        )
        .bind(round_id).bind(univ_id)
        .fetch_all(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;

        // (student_id, track_id, score, is_enrolled)
        let mut ranked: Vec<(i64, i64, Score, bool)> = rows
            .into_iter()
            .map(|r| (
                r.get::<i64, _>("student_id"),
                r.get::<i64, _>("track_id"),
                Score::from_raw(r.get::<i64, _>("total_score")),
                r.get::<bool, _>("is_enrolled"),
            ))
            .collect();

        ranked.sort_by(|a, b| {
            if prioritize {
                b.3.cmp(&a.3).then_with(|| b.2.cmp(&a.2))
            } else {
                b.2.cmp(&a.2)
            }
        });

        // Standard competition ranking: ties share the same rank (1,2,2,4,...)
        let mut actual_rank: i64 = 0;
        for (i, (sid, tid, score, enrolled)) in ranked.iter().enumerate() {
            let is_tie = i > 0 && {
                let prev = &ranked[i - 1];
                if prioritize {
                    prev.2 == *score && prev.3 == *enrolled
                } else {
                    prev.2 == *score
                }
            };
            if !is_tie {
                actual_rank = (i + 1) as i64;
            }
            sqlx::query(
                "UPDATE results SET ranking = ? WHERE student_id = ? AND track_id = ? AND round_id = ?",
            )
            .bind(actual_rank).bind(sid).bind(tid).bind(round_id)
            .execute(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
        }
    }

    Ok(count)
}

/// CLOSED 라운드 점수 재계산 (관리자 엔드포인트).
/// BEGIN IMMEDIATE 로 계산 구간 동안 다른 커넥션의 쓰기(base_data 수정 등)를 차단한다.
/// 상태 확인도 같은 tx 안에서 수행 — tx 밖 확인은 reopen_round와의 race로
/// OPEN 라운드에 점수·순위가 기록될 수 있다.
pub async fn calculate_scores(
    State(state): State<AppState>,
    Path(round_id): Path<i64>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // sqlx 관리 트랜잭션: 오류 경로에서 tx drop 시 자동 ROLLBACK — 커넥션 오염 없음
    let mut tx = state
        .db
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let round_status: Option<RoundStatus> = sqlx::query_scalar(
        "SELECT status FROM rounds WHERE id = ?",
    )
    .bind(round_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match round_status {
        Some(RoundStatus::Closed) => {}
        Some(_) => return Err((StatusCode::BAD_REQUEST, "CLOSED 라운드에서만 점수 계산이 가능합니다".into())),
        None => return Err((StatusCode::NOT_FOUND, "라운드를 찾을 수 없습니다".into())),
    }

    let now = chrono::Utc::now().to_rfc3339();
    let count = run_calculate_scores_on_conn(&mut *tx, round_id, &now)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    crate::audit::log(
        &mut *tx,
        AuditEntry {
            actor: Actor::Admin,
            action: AuditAction::ScoresRecalculated,
            round_id: Some(round_id),
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

pub async fn get_results(
    State(state): State<AppState>,
    Path(round_id): Path<i64>,
    Query(q): Query<ResultQuery>,
) -> Result<Json<Vec<ResultRow>>, ApiError> {
    let track_rank = track_rank_window("r", "ut", "s", true);
    let sql = format!(
        "SELECT r.student_id, r.track_id, r.round_id,
                r.total_score, r.score_detail, r.ranking, r.recommended,
                COALESCE(a.abandoned, 0) AS abandoned,
                COALESCE(a.excluded, 0) AS excluded, a.excluded_reason,
                s.student_code, s.name, s.grade, s.class_no, s.seq_no, s.is_enrolled,
                u.univ_name, ut.track_name,
                COALESCE(a.department_name, '') AS department_name,
                {track_rank}
         FROM results r
         JOIN students s ON r.student_id = s.id
         JOIN univ_tracks ut ON r.track_id = ut.id
         JOIN universities u ON ut.univ_id = u.id
         LEFT JOIN applications a ON a.student_id = r.student_id
                                  AND a.track_id  = r.track_id
                                  AND a.round_id  = r.round_id
         WHERE r.round_id = ?
           AND (? IS NULL OR r.track_id = ?)
         ORDER BY r.track_id, r.ranking NULLS LAST, r.total_score DESC"
    );
    let rows = sqlx::query_as::<_, ResultRow>(&sql)
    .bind(round_id)
    .bind(q.track_id).bind(q.track_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(rows))
}

// ── Results Excel export ──────────────────────────────────────────

#[derive(FromRow)]
struct AreaName {
    id: i64,
    name: String,
}

/// 지원자 명단 한 행 — applications 기준(지원 전원), 결과는 LEFT JOIN 이라 Option.
#[derive(FromRow)]
struct RosterRow {
    round_id: i64,
    student_code: String,
    name: String,
    grade: Option<i64>,
    class_no: Option<i64>,
    seq_no: Option<i64>,
    is_enrolled: i64,
    univ_name: String,
    track_name: String,
    department_name: String,
    score_detail: Option<String>,
    total_score: Option<i64>,
    ranking: Option<i64>,
    track_rank: Option<i64>,
    recommended: Option<i64>,
    abandoned: i64,
    excluded: i64,
    excluded_reason: Option<String>,
}

/// 지원자 명단 시트를 채운다. **`applications` 기준**이라 점수 미계산 지원자도 포함된다
/// (누가 지원했는지가 기준). 결과(점수·순위·추천)는 `results` 를 LEFT JOIN 해 붙인다.
///
/// - `round_id = Some`: 그 라운드만. 라운드 열 생략. 전형요소 점수 누락은 오류로 승격
///   (그 라운드는 현재 전형요소로 재계산돼 있어야 하므로 — "재계산 필요" 가드 유지).
/// - `round_id = None`: 전체 라운드. 라운드 열 포함. 과거 라운드가 지금 없는 전형요소를
///   가질 수 있으므로 누락 점수는 빈 칸으로 둔다(재계산 강요 금지).
/// - `univ_id = Some`: 그 대학만.
pub(crate) async fn write_roster_sheet(
    ws: &mut rust_xlsxwriter::Worksheet,
    db: &sqlx::SqlitePool,
    round_id: Option<i64>,
    univ_id: Option<i64>,
) -> Result<(), ApiError> {
    let areas: Vec<AreaName> = sqlx::query_as::<_, AreaName>(
        "SELECT id, name FROM areas ORDER BY id",
    )
    .fetch_all(db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // tr CTE 는 전 라운드 결과를 걸치므로 항상 라운드까지 partition (단일 라운드에도 안전)
    let track_rank = track_rank_window("r", "ut", "s", true);
    let mut filters = String::new();
    if round_id.is_some() { filters.push_str(" AND a.round_id = ?"); }
    if univ_id.is_some()  { filters.push_str(" AND ut.univ_id = ?"); }
    let roster_sql = format!(
        "WITH tr AS (
             SELECT r.student_id, r.track_id, r.round_id, {track_rank}
             FROM results r
             JOIN students s      ON s.id  = r.student_id
             JOIN univ_tracks ut  ON ut.id = r.track_id
         )
         SELECT a.round_id,
                s.student_code, s.name, s.grade, s.class_no, s.seq_no, s.is_enrolled,
                u.univ_name, ut.track_name, COALESCE(a.department_name, '') AS department_name,
                r.score_detail, r.total_score, r.ranking, tr.track_rank, r.recommended,
                COALESCE(a.abandoned, 0) AS abandoned,
                COALESCE(a.excluded, 0)  AS excluded, a.excluded_reason
         FROM applications a
         JOIN students s      ON s.id  = a.student_id
         JOIN univ_tracks ut  ON ut.id = a.track_id
         JOIN universities u  ON u.id  = ut.univ_id
         LEFT JOIN results r  ON r.student_id = a.student_id
                             AND r.track_id  = a.track_id
                             AND r.round_id  = a.round_id
         LEFT JOIN tr         ON tr.student_id = a.student_id
                             AND tr.track_id  = a.track_id
                             AND tr.round_id  = a.round_id
         WHERE 1=1{filters}
         ORDER BY s.is_enrolled DESC, s.student_code, a.round_id, u.univ_name, ut.track_name"
    );
    let mut q = sqlx::query_as::<_, RosterRow>(&roster_sql);
    if let Some(rid) = round_id { q = q.bind(rid); }
    if let Some(uid) = univ_id  { q = q.bind(uid); }
    let rows = q
        .fetch_all(db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 헤더
    let include_round = round_id.is_none();
    let mut col = 0u16;
    if include_round {
        ws.write_string(0, col, "라운드").map_err(excel::xlsx_err)?; col += 1;
    }
    for h in ["대학 순위", "모집단위 순위", "대학", "모집단위", "지원학과", "학생명", "학생코드", "학년", "반", "번호", "재학구분"] {
        ws.write_string(0, col, h).map_err(excel::xlsx_err)?; col += 1;
    }
    for area in &areas {
        ws.write_string(0, col, &area.name).map_err(excel::xlsx_err)?; col += 1;
    }
    for h in ["총점", "추천", "포기", "미선발여부", "미선발사유"] {
        ws.write_string(0, col, h).map_err(excel::xlsx_err)?; col += 1;
    }

    // 데이터
    let strict_area = round_id.is_some();
    for (i, r) in rows.iter().enumerate() {
        let row = (i + 1) as u32;
        let mut col = 0u16;

        if include_round {
            ws.write_number(row, col, r.round_id as f64).map_err(excel::xlsx_err)?; col += 1;
        }
        if let Some(rk) = r.ranking { ws.write_number(row, col, rk as f64).map_err(excel::xlsx_err)?; }
        col += 1;
        if let Some(tr) = r.track_rank { ws.write_number(row, col, tr as f64).map_err(excel::xlsx_err)?; }
        col += 1;
        ws.write_string(row, col, &r.univ_name).map_err(excel::xlsx_err)?; col += 1;
        ws.write_string(row, col, &r.track_name).map_err(excel::xlsx_err)?; col += 1;
        ws.write_string(row, col, &r.department_name).map_err(excel::xlsx_err)?; col += 1;
        ws.write_string(row, col, &r.name).map_err(excel::xlsx_err)?; col += 1;
        ws.write_string(row, col, &r.student_code).map_err(excel::xlsx_err)?; col += 1;
        if let Some(g) = r.grade { ws.write_number(row, col, g as f64).map_err(excel::xlsx_err)?; }
        col += 1;
        if let Some(c) = r.class_no { ws.write_number(row, col, c as f64).map_err(excel::xlsx_err)?; }
        col += 1;
        if let Some(s) = r.seq_no { ws.write_number(row, col, s as f64).map_err(excel::xlsx_err)?; }
        col += 1;
        ws.write_string(row, col, if r.is_enrolled == 1 { "재학" } else { "졸업" }).map_err(excel::xlsx_err)?; col += 1;

        // 전형요소별 점수 — 결과가 없으면(미계산 지원자) 빈 칸
        let detail: Option<HashMap<String, i64>> = match &r.score_detail {
            Some(json) => Some(serde_json::from_str(json).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!(
                "학생코드 {} score_detail JSON 파싱 실패: {}", r.student_code, e
            )))?),
            None => None,
        };
        for area in &areas {
            match detail.as_ref().and_then(|d| d.get(&area.id.to_string()).copied()) {
                Some(sc) => { ws.write_number(row, col, sc as f64 / 100_000.0).map_err(excel::xlsx_err)?; }
                None => {
                    // 결과 자체가 없으면 빈 칸. 결과는 있는데 이 전형요소만 없으면:
                    // 단일 라운드(strict)는 "재계산 필요" 오류, 전체 라운드는 빈 칸(과거 라운드 허용).
                    if strict_area && detail.is_some() {
                        return Err((StatusCode::INTERNAL_SERVER_ERROR, format!(
                            "학생코드 {} 전형요소 id={}의 점수가 없습니다. 점수 재계산이 필요합니다",
                            r.student_code, area.id
                        )));
                    }
                }
            }
            col += 1;
        }

        match r.total_score {
            Some(ts) => { ws.write_number(row, col, ts as f64 / 100_000.0).map_err(excel::xlsx_err)?; }
            None     => { ws.write_string(row, col, "미계산").map_err(excel::xlsx_err)?; }
        }
        col += 1;
        ws.write_string(row, col, if r.recommended == Some(1) { "추천" } else { "" }).map_err(excel::xlsx_err)?; col += 1;
        ws.write_string(row, col, if r.abandoned == 1 { "포기" } else { "" }).map_err(excel::xlsx_err)?; col += 1;
        ws.write_string(row, col, if r.excluded == 1 { "미선발" } else { "" }).map_err(excel::xlsx_err)?; col += 1;
        ws.write_string(row, col, r.excluded_reason.as_deref().unwrap_or("")).map_err(excel::xlsx_err)?;
    }

    Ok(())
}

pub async fn export_results(
    State(state): State<AppState>,
    Path(round_id): Path<i64>,
) -> Result<Response, ApiError> {
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet()
        .set_name("지원자 명단")
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    write_roster_sheet(ws, &state.db, Some(round_id), None).await?;

    let buf = wb
        .save_to_buffer()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let filename = format!("round_{}_applicants_{}.xlsx", round_id, excel::now_tag());
    Ok(excel::xlsx_response(buf, &filename))
}

// ── Round summary Excel export ────────────────────────────────────

#[derive(FromRow)]
struct RoundSummaryRow {
    univ_name: String,
    track_name: String,
    unit_quota: Option<i64>,
    applied_count: i64,
    abandoned_count: i64,
    before_count: i64,
    this_count: i64,
    total_quota: Option<i64>,
    univ_before_count: i64,
    univ_this_count: i64,
}

pub async fn export_round_summary(
    State(state): State<AppState>,
    Path(round_id): Path<i64>,
) -> Result<Response, ApiError> {
    let rows: Vec<RoundSummaryRow> = sqlx::query_as::<_, RoundSummaryRow>(
        "SELECT u.univ_name, ut.track_name, ut.unit_quota,
                CAST((SELECT COUNT(*) FROM applications ap
                      WHERE ap.track_id = ut.id AND ap.round_id = ?) AS INTEGER) AS applied_count,
                CAST((SELECT COUNT(*) FROM applications ab
                      WHERE ab.track_id = ut.id AND ab.round_id = ? AND ab.abandoned = 1) AS INTEGER) AS abandoned_count,
                CAST((SELECT COUNT(*) FROM results r2
                      JOIN applications a2 ON a2.student_id = r2.student_id
                                          AND a2.track_id  = r2.track_id
                                          AND a2.round_id  = r2.round_id
                      WHERE r2.track_id  = ut.id
                        AND r2.recommended = 1
                        AND a2.abandoned   = 0
                        AND r2.round_id    < ?) AS INTEGER) AS before_count,
                CAST((SELECT COUNT(*) FROM results r3
                      JOIN applications a3 ON a3.student_id = r3.student_id
                                          AND a3.track_id  = r3.track_id
                                          AND a3.round_id  = r3.round_id
                      WHERE r3.track_id  = ut.id
                        AND r3.recommended = 1
                        AND a3.abandoned   = 0
                        AND r3.round_id    = ?) AS INTEGER) AS this_count,
                u.total_quota,
                CAST((SELECT COUNT(*) FROM results r4
                      JOIN applications a4 ON a4.student_id = r4.student_id
                                          AND a4.track_id  = r4.track_id
                                          AND a4.round_id  = r4.round_id
                      JOIN univ_tracks ut4 ON ut4.id = r4.track_id
                      WHERE ut4.univ_id   = u.id
                        AND r4.recommended = 1
                        AND a4.abandoned   = 0
                        AND r4.round_id    < ?) AS INTEGER) AS univ_before_count,
                CAST((SELECT COUNT(*) FROM results r5
                      JOIN applications a5 ON a5.student_id = r5.student_id
                                          AND a5.track_id  = r5.track_id
                                          AND a5.round_id  = r5.round_id
                      JOIN univ_tracks ut5 ON ut5.id = r5.track_id
                      WHERE ut5.univ_id   = u.id
                        AND r5.recommended = 1
                        AND a5.abandoned   = 0
                        AND r5.round_id    = ?) AS INTEGER) AS univ_this_count
         FROM univ_tracks ut
         JOIN universities u ON u.id = ut.univ_id
         ORDER BY u.univ_name, ut.track_name",
    )
    .bind(round_id) // applied_count
    .bind(round_id) // abandoned_count
    .bind(round_id) // before_count
    .bind(round_id) // this_count
    .bind(round_id) // univ_before_count
    .bind(round_id) // univ_this_count
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut wb = Workbook::new();
    let ws = wb
        .add_worksheet()
        .set_name("선발 현황")
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let headers = [
        "대학", "모집단위", "모집단위 정원",
        "지원 인원", "추천 인원", "포기 인원",
        "모집단위 라운드 전 잔여석", "모집단위 남은 잔여석",
        "대학 전체 정원", "대학 라운드 전 잔여석", "대학 남은 잔여석",
    ];
    for (col, h) in headers.iter().enumerate() {
        ws.write_string(0, col as u16, *h).map_err(excel::xlsx_err)?;
    }

    for (i, row) in rows.iter().enumerate() {
        let r = (i + 1) as u32;
        ws.write_string(r, 0, &row.univ_name).map_err(excel::xlsx_err)?;
        ws.write_string(r, 1, &row.track_name).map_err(excel::xlsx_err)?;
        // 지원·추천·포기 인원은 정원 유무와 무관하게 항상 숫자
        ws.write_number(r, 3, row.applied_count as f64).map_err(excel::xlsx_err)?;
        ws.write_number(r, 4, row.this_count as f64).map_err(excel::xlsx_err)?;
        ws.write_number(r, 5, row.abandoned_count as f64).map_err(excel::xlsx_err)?;
        match row.unit_quota {
            Some(q) => {
                let before_remaining = (q - row.before_count).max(0);
                let after_remaining = (q - row.before_count - row.this_count).max(0);
                ws.write_number(r, 2, q as f64).map_err(excel::xlsx_err)?;
                ws.write_number(r, 6, before_remaining as f64).map_err(excel::xlsx_err)?;
                ws.write_number(r, 7, after_remaining as f64).map_err(excel::xlsx_err)?;
            }
            None => {
                ws.write_string(r, 2, "무제한").map_err(excel::xlsx_err)?;
                ws.write_string(r, 6, "무제한").map_err(excel::xlsx_err)?;
                ws.write_string(r, 7, "무제한").map_err(excel::xlsx_err)?;
            }
        }
        match row.total_quota {
            Some(tq) => {
                let univ_before_remaining = (tq - row.univ_before_count).max(0);
                let univ_after_remaining =
                    (tq - row.univ_before_count - row.univ_this_count).max(0);
                ws.write_number(r, 8, tq as f64).map_err(excel::xlsx_err)?;
                ws.write_number(r, 9, univ_before_remaining as f64).map_err(excel::xlsx_err)?;
                ws.write_number(r, 10, univ_after_remaining as f64).map_err(excel::xlsx_err)?;
            }
            None => {
                ws.write_string(r, 8, "무제한").map_err(excel::xlsx_err)?;
                ws.write_string(r, 9, "무제한").map_err(excel::xlsx_err)?;
                ws.write_string(r, 10, "무제한").map_err(excel::xlsx_err)?;
            }
        }
    }

    let buf = wb
        .save_to_buffer()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let filename = format!("round_{}_summary_{}.xlsx", round_id, excel::now_tag());
    Ok(excel::xlsx_response(buf, &filename))
}

// ── Teacher results ───────────────────────────────────────────────

#[derive(Serialize, FromRow)]
pub struct RoundInfo {
    pub id: i64,
    pub status: String,
    pub opened_at: String,
    pub closed_at: Option<String>,
    pub finalized_at: Option<String>,
}

#[derive(Serialize)]
pub struct TeacherResultsResponse {
    pub rounds: Vec<RoundInfo>,
    pub results: Vec<ResultRow>,
}

pub async fn teacher_get_results(
    State(state): State<AppState>,
    Extension(claims): Extension<TeacherClaims>,
) -> Result<Json<TeacherResultsResponse>, ApiError> {
    let rounds = sqlx::query_as::<_, RoundInfo>(
        "SELECT id, status, opened_at, closed_at, finalized_at FROM rounds ORDER BY id",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // track_rank는 FINALIZED 전체 결과 기준으로 계산(grade/class 필터 전)
    let tr_cte = track_rank_window("r2", "ut2", "s2", true);
    let results = if claims.grade == 0 && claims.class_no == 0 {
        let sql = format!(
            "WITH tr AS (
                 SELECT r2.student_id, r2.track_id, r2.round_id,
                        {tr_cte}
                 FROM results r2
                 JOIN students s2    ON s2.id   = r2.student_id
                 JOIN univ_tracks ut2 ON ut2.id = r2.track_id
                 JOIN rounds rnd2    ON rnd2.id  = r2.round_id
                 WHERE rnd2.status = 'FINALIZED'
             )
             SELECT r.student_id, r.track_id, r.round_id,
                    r.total_score, r.score_detail, r.ranking, r.recommended,
                    COALESCE(a.abandoned, 0) AS abandoned,
                COALESCE(a.excluded, 0) AS excluded, a.excluded_reason,
                    s.student_code, s.name, s.grade, s.class_no, s.seq_no, s.is_enrolled,
                    u.univ_name, ut.track_name,
                    COALESCE(a.department_name, '') AS department_name,
                    tr.track_rank
             FROM results r
             JOIN students s ON r.student_id = s.id
             JOIN univ_tracks ut ON r.track_id = ut.id
             JOIN universities u ON ut.univ_id = u.id
             JOIN rounds rnd ON rnd.id = r.round_id
             LEFT JOIN applications a ON a.student_id = r.student_id
                                      AND a.track_id  = r.track_id
                                      AND a.round_id  = r.round_id
             JOIN tr ON tr.student_id = r.student_id
                     AND tr.track_id  = r.track_id
                     AND tr.round_id  = r.round_id
             WHERE rnd.status = 'FINALIZED'
               AND s.is_enrolled = 0
             ORDER BY r.round_id, s.student_code, r.track_id"
        );
        sqlx::query_as::<_, ResultRow>(&sql)
            .fetch_all(&state.db)
            .await
    } else {
        let sql = format!(
            "WITH tr AS (
                 SELECT r2.student_id, r2.track_id, r2.round_id,
                        {tr_cte}
                 FROM results r2
                 JOIN students s2    ON s2.id   = r2.student_id
                 JOIN univ_tracks ut2 ON ut2.id = r2.track_id
                 JOIN rounds rnd2    ON rnd2.id  = r2.round_id
                 WHERE rnd2.status = 'FINALIZED'
             )
             SELECT r.student_id, r.track_id, r.round_id,
                    r.total_score, r.score_detail, r.ranking, r.recommended,
                    COALESCE(a.abandoned, 0) AS abandoned,
                COALESCE(a.excluded, 0) AS excluded, a.excluded_reason,
                    s.student_code, s.name, s.grade, s.class_no, s.seq_no, s.is_enrolled,
                    u.univ_name, ut.track_name,
                    COALESCE(a.department_name, '') AS department_name,
                    tr.track_rank
             FROM results r
             JOIN students s ON r.student_id = s.id
             JOIN univ_tracks ut ON r.track_id = ut.id
             JOIN universities u ON ut.univ_id = u.id
             JOIN rounds rnd ON rnd.id = r.round_id
             LEFT JOIN applications a ON a.student_id = r.student_id
                                      AND a.track_id  = r.track_id
                                      AND a.round_id  = r.round_id
             JOIN tr ON tr.student_id = r.student_id
                     AND tr.track_id  = r.track_id
                     AND tr.round_id  = r.round_id
             WHERE rnd.status = 'FINALIZED'
               AND s.grade = ?
               AND s.class_no = ?
             ORDER BY r.round_id, s.seq_no, r.track_id"
        );
        sqlx::query_as::<_, ResultRow>(&sql)
            .bind(claims.grade)
            .bind(claims.class_no)
            .fetch_all(&state.db)
            .await
    }
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(TeacherResultsResponse { rounds, results }))
}

// ── Score preview ─────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ScorePreviewQuery {
    pub student_id: i64,
    pub track_id: i64,
}

#[derive(Serialize)]
pub struct AreaPreview {
    pub area_id: i64,
    pub area_name: String,
    pub score: Score,
}

#[derive(Serialize)]
pub struct ScorePreviewResponse {
    pub total: Score,
    pub detail: Vec<AreaPreview>,
}

pub async fn score_preview(
    State(state): State<AppState>,
    Query(q): Query<ScorePreviewQuery>,
) -> Result<Json<ScorePreviewResponse>, ApiError> {
    let area_rows: Vec<AreaRow> = sqlx::query_as::<_, AreaRow>(
        "SELECT id, name, calc_type, max_score, match_mode, category_agg, lookup_scope
         FROM areas ORDER BY id",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    #[derive(FromRow)]
    struct StudentTrackInfo {
        student_code: String,
        name: String,
        univ_name: String,
        track_name: String,
    }
    let info: StudentTrackInfo = sqlx::query_as::<_, StudentTrackInfo>(
        "SELECT s.student_code, s.name, u.univ_name, ut.track_name
         FROM students s, univ_tracks ut, universities u
         WHERE s.id = ? AND ut.id = ? AND u.id = ut.univ_id",
    )
    .bind(q.student_id)
    .bind(q.track_id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let ctx = StudentTrackCtx {
        student_code: info.student_code,
        student_name: info.name,
        univ_name: info.univ_name,
        track_name: info.track_name,
    };

    let mut conn = state.db.acquire().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mut detail: Vec<AreaPreview> = Vec::new();
    let mut total_raw: i64 = 0;

    for aw in &area_rows {
        let score_raw = calc_area_score(&mut *conn, q.student_id, aw, q.track_id, &ctx)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
        total_raw = total_raw.checked_add(score_raw).ok_or_else(|| {
            (StatusCode::INTERNAL_SERVER_ERROR, "점수 합산 오버플로우".to_string())
        })?;
        detail.push(AreaPreview {
            area_id: aw.id,
            area_name: aw.name.clone(),
            score: Score::from_raw(score_raw),
        });
    }

    Ok(Json(ScorePreviewResponse { total: Score::from_raw(total_raw), detail }))
}

// ─────────────────────────────────────────────────────────────────


// URL: /results/:sid/:tid/:rid/recommend  (sid=student_id, tid=track_id, rid=round_id)
pub async fn recommend_result(
    State(state): State<AppState>,
    Path((sid, tid, rid)): Path<(i64, i64, i64)>,
) -> Result<StatusCode, ApiError> {
    // BEGIN IMMEDIATE: 정원 체크(SELECT COUNT)와 추천 확정(UPDATE) 사이의 TOCTOU 방지.
    // DEFERRED로 시작하면 두 커넥션이 동시에 COUNT=0을 읽고 둘 다 통과해 정원 초과가 발생한다.
    // sqlx 관리 트랜잭션: 오류 경로에서 tx drop 시 자동 ROLLBACK — 커넥션 오염 없음
    let mut tx = state.db.begin_with("BEGIN IMMEDIATE").await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 1. Round status check
    let status: Option<RoundStatus> = sqlx::query_scalar(
        "SELECT status FROM rounds WHERE id = ?",
    )
    .bind(rid)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match status {
        Some(RoundStatus::Closed) => {}
        Some(_) => return Err((StatusCode::BAD_REQUEST, "CLOSED 라운드에서만 추천 확정이 가능합니다".into())),
        None => return Err((StatusCode::NOT_FOUND, "라운드를 찾을 수 없습니다".into())),
    }

    // 1a. 기초데이터가 계산 이후에 바뀌었으면 추천할 수 없다 (F-017).
    //     낡은 순위로 추천을 확정하면 정원 판정 자체가 잘못된 근거 위에 선다.
    if crate::handlers::rounds::needs_recalc(&mut *tx, rid).await? {
        return Err((StatusCode::CONFLICT, crate::handlers::rounds::NEEDS_RECALC_MSG.into()));
    }

    // 1b. 미선발 처리된 지원은 추천 불가 — 존재하지 않으면 이후 검증에서 자연히 404/409로 처리된다
    let excluded: Option<bool> = sqlx::query_scalar(
        "SELECT excluded = 1 FROM applications WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(sid)
    .bind(tid)
    .bind(rid)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if excluded == Some(true) {
        return Err((StatusCode::CONFLICT, "미선발 처리된 지원은 추천할 수 없습니다".into()));
    }

    // 2. 모집단위 정원 정보 조회
    #[derive(sqlx::FromRow)]
    struct TrackInfo { unit_quota: Option<i64>, univ_id: i64 }
    let track_info: TrackInfo = sqlx::query_as(
        "SELECT unit_quota, univ_id FROM univ_tracks WHERE id = ?",
    )
    .bind(tid)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 3. 해당 모집단위의 전체 라운드 추천 확정 수 (포기 제외)
    let track_used: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM results r
         JOIN applications a ON a.student_id = r.student_id
                             AND a.track_id  = r.track_id
                             AND a.round_id  = r.round_id
         WHERE r.track_id = ? AND r.recommended = 1 AND a.abandoned = 0",
    )
    .bind(tid)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if let Some(uq) = track_info.unit_quota {
        if track_used >= uq {
            return Err((
                StatusCode::CONFLICT,
                format!("모집단위 정원({}명)이 이미 찼습니다 (현재 추천 확정 {}명)", uq, track_used),
            ));
        }
    }

    // 4. 대학 전체 정원 조회
    let total_quota: Option<i64> = sqlx::query_scalar(
        "SELECT total_quota FROM universities WHERE id = ?",
    )
    .bind(track_info.univ_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if let Some(tq) = total_quota {
        // 5. 해당 대학 전체 모집단위의 전체 라운드 추천 확정 수 (포기 제외)
        let univ_used: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM results r
             JOIN applications a ON a.student_id = r.student_id
                                 AND a.track_id  = r.track_id
                                 AND a.round_id  = r.round_id
             JOIN univ_tracks ut ON ut.id = r.track_id
             WHERE ut.univ_id = ? AND r.recommended = 1 AND a.abandoned = 0",
        )
        .bind(track_info.univ_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        if univ_used >= tq {
            return Err((
                StatusCode::CONFLICT,
                format!("대학 전체 정원({}명)이 이미 찼습니다 (현재 추천 확정 {}명)", tq, univ_used),
            ));
        }
    }

    // 5b. 트랙 내부 순서 가드 — 자동 추천의 D 규칙("대학 컷은 같은 트랙에서 track_rank
    //     상위자를 건너뛰고 하위자를 선택할 수 없다")을 수동 경로에도 적용한다.
    //     동점(track_rank 동일)은 우열이 정해지지 않은 상태라 서로 막지 않는다 —
    //     관리자 선택의 여지를 남긴다. 상위자가 이미 추천됐거나 포기했으면 막지 않는다.
    //     track_rank 는 자동 추천 1단계(3c)와 같은 윈도우 함수로 파생 —
    //     두 경로가 다른 순위를 쓰면 이 가드의 의미가 없다.
    #[derive(sqlx::FromRow)]
    struct Blocker { blockers: i64, top_rank: Option<i64> }
    let blocker_rank = track_rank_window("r", "ut", "s", false);
    let blocker_sql = format!(
        "WITH ranked AS (
             SELECT r.student_id, r.recommended,
                    {blocker_rank}
             FROM results r
             JOIN students s ON s.id = r.student_id
             JOIN univ_tracks ut ON ut.id = r.track_id
             WHERE r.round_id = ? AND r.track_id = ?
         )
         SELECT COUNT(*) AS blockers, MIN(k.track_rank) AS top_rank
         FROM ranked k
         JOIN applications a ON a.student_id = k.student_id
                             AND a.track_id  = ?
                             AND a.round_id  = ?
         WHERE k.recommended = 0 AND a.abandoned = 0 AND a.excluded = 0
           AND k.track_rank < (SELECT track_rank FROM ranked WHERE student_id = ?)"
    );
    let blocker: Blocker = sqlx::query_as(&blocker_sql)
    .bind(rid).bind(tid).bind(tid).bind(rid).bind(sid)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if blocker.blockers > 0 {
        let top = blocker.top_rank
            .ok_or((StatusCode::INTERNAL_SERVER_ERROR,
                    "상위 미추천자 순위를 계산할 수 없습니다".to_string()))?;
        return Err((
            StatusCode::CONFLICT,
            format!(
                "같은 모집단위에 아직 추천되지 않은 상위 순위 지원자가 {}명 있습니다 \
                 (최상위 모집단위 순위 {}위). 모집단위 순위를 건너뛴 추천은 할 수 없습니다. \
                 상위 지원자를 먼저 추천하거나, 해당 지원자가 지원을 포기한 경우 포기 처리 후 다시 시도하세요.",
                blocker.blockers, top
            ),
        ));
    }

    // 5c. 크로스트랙 순서 가드 — 대학 정원이 유한할 때, 같은 대학의 다른 모집단위에
    //     더 높은 대학 순위(ranking)를 가진 미해결 지원자가 있으면서 그 지원자의
    //     모집단위에 빈자리가 남아 있으면 차단한다. 상위자의 트랙이 만석이면 추천
    //     자체가 불가하므로 블로커에서 제외한다.
    //     대학 정원이 무제한이면 squeeze-out이 불가능하므로 검사를 건너뛴다.
    if total_quota.is_some() {
        let cross_blocker: Blocker = sqlx::query_as(
            "SELECT COUNT(*) AS blockers, MIN(r.ranking) AS top_rank
             FROM results r
             JOIN applications a ON a.student_id = r.student_id
                                 AND a.track_id  = r.track_id
                                 AND a.round_id  = r.round_id
             JOIN univ_tracks ut ON ut.id = r.track_id
             WHERE r.round_id = ?
               AND ut.univ_id = ?
               AND r.track_id != ?
               AND r.recommended = 0
               AND a.abandoned = 0
               AND a.excluded = 0
               AND r.ranking < (SELECT ranking FROM results
                                WHERE student_id = ? AND track_id = ? AND round_id = ?)
               AND (
                   ut.unit_quota IS NULL
                   OR (
                       SELECT COUNT(*) FROM results r2
                       JOIN applications a2 ON a2.student_id = r2.student_id
                                            AND a2.track_id  = r2.track_id
                                            AND a2.round_id  = r2.round_id
                       WHERE r2.track_id = r.track_id
                         AND r2.recommended = 1
                         AND a2.abandoned = 0
                   ) < ut.unit_quota
               )",
        )
        .bind(rid).bind(track_info.univ_id).bind(tid)
        .bind(sid).bind(tid).bind(rid)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        if cross_blocker.blockers > 0 {
            let top = cross_blocker.top_rank
                .ok_or((StatusCode::INTERNAL_SERVER_ERROR,
                        "상위 미추천자 대학 순위를 계산할 수 없습니다".to_string()))?;
            return Err((
                StatusCode::CONFLICT,
                format!(
                    "같은 대학 내 다른 모집단위에 아직 추천되지 않은 상위 대학 순위 지원자가 \
                     {}명 있습니다 (최상위 대학 순위 {}위). 해당 모집단위에 빈자리가 있으므로, \
                     상위 지원자를 먼저 추천하거나 미선발 처리 후 다시 시도하세요.",
                    cross_blocker.blockers, top
                ),
            ));
        }
    }

    // 6. 추천 확정
    let affected = sqlx::query(
        "UPDATE results SET recommended = 1 WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(sid).bind(tid).bind(rid)
    .execute(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .rows_affected();

    if affected == 0 {
        return Err((StatusCode::NOT_FOUND, "결과 행을 찾을 수 없습니다 (점수 계산 후 시도하세요)".into()));
    }

    let detail = crate::audit::application_detail(&mut *tx, sid, tid).await?;
    crate::audit::log(
        &mut *tx,
        AuditEntry {
            actor: Actor::Admin,
            action: AuditAction::RecommendConfirmed,
            round_id: Some(rid),
            student_id: Some(sid),
            detail,
        },
    )
    .await?;

    tx.commit().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn unrecommend_result(
    State(state): State<AppState>,
    Path((sid, tid, rid)): Path<(i64, i64, i64)>,
) -> Result<StatusCode, ApiError> {
    let mut tx = state.db.begin().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 상태 체크와 UPDATE를 같은 트랜잭션 안에서 처리 — FINALIZE race condition 방지
    let status: Option<RoundStatus> = sqlx::query_scalar(
        "SELECT status FROM rounds WHERE id = ?",
    )
    .bind(rid)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match status {
        Some(RoundStatus::Closed) => {}
        Some(_) => return Err((StatusCode::BAD_REQUEST, "CLOSED 라운드에서만 추천 취소가 가능합니다".into())),
        None => return Err((StatusCode::NOT_FOUND, "라운드를 찾을 수 없습니다".into())),
    }

    // rows_affected 확인 — 존재하지 않는 (sid,tid,rid) 조합 호출 시 recommend_result와 대칭하게
    // 404를 반환한다. 대칭화 전에는 spurious RecommendCanceled 감사 로그가 쌓여 사고 대응 시
    // 잡음과 실제 취소를 구분하기 어려웠다 (2차 감사 C 발견 O-1 소유자 라운드 #7).
    let result = sqlx::query(
        "UPDATE results SET recommended = 0 WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(sid).bind(tid).bind(rid)
    .execute(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "해당 지원자·모집단위·라운드의 결과가 없습니다".into()));
    }

    let detail = crate::audit::application_detail(&mut *tx, sid, tid).await?;
    crate::audit::log(
        &mut *tx,
        AuditEntry {
            actor: Actor::Admin,
            action: AuditAction::RecommendCanceled,
            round_id: Some(rid),
            student_id: Some(sid),
            detail,
        },
    )
    .await?;

    tx.commit().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

// ── Auto-recommend ────────────────────────────────────────────────

#[derive(Serialize)]
pub struct AutoRecommendItem {
    pub track_id: i64,
    pub univ_name: String,
    pub track_name: String,
    pub count: i64,
}

/// 수동 확인 필요 항목. `track_id`/`track_name` 이 None 이면 대학 전체 정원 컷에서 발생한
/// 대학 단위 항목이다(특정 모집단위에 귀속되지 않음).
#[derive(Serialize)]
pub struct AutoRecommendManualItem {
    pub track_id: Option<i64>,
    pub univ_name: String,
    pub track_name: Option<String>,
    pub reason: String,
}

#[derive(Serialize)]
pub struct AutoRecommendResponse {
    pub confirmed: Vec<AutoRecommendItem>,
    pub manual: Vec<AutoRecommendManualItem>,
}

/// 동점 그룹이 정원 경계를 가른 지점의 정보.
#[derive(Debug, Clone, PartialEq)]
pub struct TieBoundary {
    /// 경계에 걸린 동점 그룹의 순위
    pub rank: i64,
    /// 그 시점의 잔여 정원 (항상 > 0. 0이면 깨끗한 경계이므로 TieBoundary 가 아님)
    pub free: i64,
    /// 그 잔여석을 두고 경합하는 동점 인원 수 (free < contenders)
    pub contenders: i64,
}

/// 동점 그룹 원자적 채움 결과.
#[derive(Debug)]
pub struct FillOutcome<T> {
    /// 자동 확정된 상위 그룹들의 항목 (동점 경계 그룹은 포함되지 않는다)
    pub confirmed: Vec<T>,
    /// 동점이 정원 경계를 가른 경우에만 Some — 관리자 수동 판단 필요
    pub tie: Option<TieBoundary>,
}

/// **동점 그룹 원자적 채움** — 트랙 정원 채움 phase 와 대학 정원 컷 phase 가 공유하는 단일 규칙.
///
/// `items` 는 (순위, 항목) 쌍이며 순위 오름차순으로 정렬되어 있어야 한다.
/// `remaining` 은 잔여 정원(None = 무제한).
///
/// 같은 순위 값을 가진 항목들을 하나의 그룹으로 묶어 위에서부터 처리한다.
/// - 그룹 전원이 잔여 정원 안에 들어오면 전원 확정하고 다음 그룹으로.
/// - 전원을 넣으면 초과하면 거기서 멈춘다:
///   - 남은자리 == 0: 정원이 그룹 사이에 정확히 떨어진 **깨끗한 경계**. 수동 불필요.
///   - 남은자리  > 0: **동점이 정원 경계를 가름**. 그 그룹은 아무도 확정하지 않고
///     `tie` 로 보고한다(시스템이 동점 중 일부만 고를 수 없음 — 관리자 결정).
///
/// 어느 경우든 동점 그룹보다 **엄격히 상위인 항목은 자동 확정된다** (동점 존재 ≠ 전체 차단).
pub fn fill_by_rank_groups<T: Clone>(
    items: &[(i64, T)],
    remaining: Option<i64>,
) -> FillOutcome<T> {
    let Some(rem) = remaining else {
        // 무제한 — 전원 확정
        return FillOutcome {
            confirmed: items.iter().map(|(_, v)| v.clone()).collect(),
            tie: None,
        };
    };

    let mut confirmed: Vec<T> = Vec::new();

    let mut i = 0usize;
    while i < items.len() {
        let rank = items[i].0;
        let mut j = i;
        while j < items.len() && items[j].0 == rank {
            j += 1;
        }
        let group_size = (j - i) as i64;

        match decide_group(confirmed.len() as i64, group_size, rem) {
            GroupStep::Take => {
                confirmed.extend(items[i..j].iter().map(|(_, v)| v.clone()));
                i = j;
            }
            GroupStep::StopClean => return FillOutcome { confirmed, tie: None },
            GroupStep::StopTie { free } => {
                return FillOutcome {
                    confirmed,
                    tie: Some(TieBoundary { rank, free, contenders: group_size }),
                }
            }
        }
    }

    FillOutcome { confirmed, tie: None }
}

/// 동점 그룹 하나를 만났을 때의 판정 결과.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GroupStep {
    /// 그룹 전원이 잔여 정원 안 — 전원 확정하고 다음 그룹으로
    Take,
    /// 잔여 0 — 정원이 그룹 사이에 정확히 떨어진 **깨끗한 경계**. 수동 불필요
    StopClean,
    /// 0 < free < 그룹 크기 — **동점이 정원 경계를 가름**. 그룹은 아무도 확정하지 않고 수동
    StopTie { free: i64 },
}

/// 동점 경계 4갈래 판정의 **단일 정의**.
///
/// `fill_by_rank_groups`(전체 정렬 목록)와 `merge_univ_cut`(트랙 선두 집합)이
/// **같은 함수**를 사용해 두 경로의 경계 의미가 구조적으로 일치함을 보장한다.
/// (무제한(None)은 컷 자체가 없으므로 각 호출자가 진입 전에 처리한다.)
pub fn decide_group(confirmed_len: i64, group_size: i64, rem: i64) -> GroupStep {
    if confirmed_len + group_size <= rem {
        return GroupStep::Take;
    }
    let free = rem - confirmed_len;
    if free <= 0 {
        GroupStep::StopClean
    } else {
        GroupStep::StopTie { free }
    }
}

/// 2단계(대학 정원 컷) 병합 대상 후보 — 1단계를 통과한 행 하나.
#[derive(Debug, Clone, PartialEq)]
pub struct MergeCand {
    pub student_id: i64,
    pub track_id: i64,
    /// 모집단위 순위 (트랙 prioritize_enrolled 기준) — 트랙 내부 순서를 결정
    pub track_rank: i64,
    /// 대학 전체 순위 (results.ranking, 대학 prioritize_enrolled 기준) — 트랙 간 우선을 결정
    pub univ_rank: i64,
}

/// **대학 정원 컷 — 트랙 내부 순서 보존 k-way 병합** (D단계).
///
/// `tracks` 는 트랙별 1단계 확정 후보 리스트이며, 각 리스트는
/// `(track_rank, univ_rank)` 오름차순으로 정렬되어 있어야 한다.
///
/// 규칙: **대학 컷은 같은 트랙 안에서 track_rank 상위자를 건너뛰고 하위자를 선택할 수 없다.**
/// - 트랙 내부 순서 = 트랙 플래그(track_rank) — 항상 보존.
/// - 트랙 간 우선   = 대학 플래그(univ_rank) — 트랙 설정이 타 트랙 학생에게 영향 없음.
///
/// 매 단계에서 각 트랙의 **선두**(아직 미선택인 첫 후보)만 경쟁한다. 자기 트랙 상위자에
/// 막힌 후보는 애초에 경쟁 대상이 아니므로 동점 판정에도 넣지 않는다 — 그 후보가 탈락하는 것은
/// 해당 모집단위의 재학생 우선 정책이 정상 작동한 결과이지 오류가 아니다(수동 사유로 올리지 않음).
///
/// 동점 그룹 G = 대학 순위가 최상위(r)인 선두들 + 그 선두와 **트랙 내부에서도 동점**
/// (track_rank 동일)인 연속 후보들. track_rank 가 다르면 트랙 순서가 이미 우열을 정한
/// 것이므로 동점이 아니다. 경계 처리는 `decide_group` 으로 `fill_by_rank_groups` 와 동일.
///
/// 대학 플래그와 모든 트랙 플래그가 일치하는 구성에서는 대학 순위와 트랙 순서가 같으므로
/// 이 병합 결과는 **기존 전체 정렬(`fill_by_rank_groups`) 결과와 동일**하다.
pub fn merge_univ_cut(tracks: &[Vec<MergeCand>], remaining: Option<i64>) -> FillOutcome<MergeCand> {
    let Some(rem) = remaining else {
        // 대학 정원 무제한 — 컷 자체가 없으므로 1단계 결과가 그대로 최종
        return FillOutcome {
            confirmed: tracks.iter().flatten().cloned().collect(),
            tie: None,
        };
    };

    // pos[i] = 트랙 i 에서 다음에 볼 후보의 인덱스 (그 앞은 모두 선택됨)
    let mut pos: Vec<usize> = vec![0; tracks.len()];
    let mut confirmed: Vec<MergeCand> = Vec::new();

    loop {
        // 선두들 중 대학 순위가 가장 좋은 값 r
        let mut best: Option<i64> = None;
        for (ti, list) in tracks.iter().enumerate() {
            if let Some(head) = list.get(pos[ti]) {
                if best.map_or(true, |b| head.univ_rank < b) {
                    best = Some(head.univ_rank);
                }
            }
        }
        // 선두 집합이 비면 종료
        let Some(r) = best else {
            return FillOutcome { confirmed, tie: None };
        };

        // 동점 그룹 G — 선두들끼리만 판정
        let mut group: Vec<(usize, usize)> = Vec::new(); // (트랙 인덱스, 인원)
        let mut group_size: i64 = 0;
        for (ti, list) in tracks.iter().enumerate() {
            let Some(head) = list.get(pos[ti]) else { continue };
            if head.univ_rank != r {
                continue;
            }
            // 선두와 트랙 내부 동점(track_rank 동일)인 연속 후보까지 하나의 덩어리
            let mut n = 0usize;
            while let Some(c) = list.get(pos[ti] + n) {
                if c.univ_rank == r && c.track_rank == head.track_rank {
                    n += 1;
                } else {
                    break;
                }
            }
            group.push((ti, n));
            group_size += n as i64;
        }

        match decide_group(confirmed.len() as i64, group_size, rem) {
            GroupStep::Take => {
                for (ti, n) in group {
                    confirmed.extend(tracks[ti][pos[ti]..pos[ti] + n].iter().cloned());
                    pos[ti] += n;
                }
            }
            GroupStep::StopClean => return FillOutcome { confirmed, tie: None },
            GroupStep::StopTie { free } => {
                return FillOutcome {
                    confirmed,
                    tie: Some(TieBoundary { rank: r, free, contenders: group_size }),
                }
            }
        }
    }
}

/// CLOSED 라운드의 자동 추천 확정 (전 대학).
pub async fn auto_recommend_results(
    State(state): State<AppState>,
    Path(round_id): Path<i64>,
) -> Result<Json<AutoRecommendResponse>, ApiError> {
    run_auto_recommend(state, round_id, None).await
}

/// CLOSED 라운드의 자동 추천 확정 (지정 대학만).
pub async fn auto_recommend_results_univ(
    State(state): State<AppState>,
    Path((round_id, univ_id)): Path<(i64, i64)>,
) -> Result<Json<AutoRecommendResponse>, ApiError> {
    run_auto_recommend(state, round_id, Some(univ_id)).await
}

/// 자동 추천 본체. `univ_filter` 가 Some 이면 그 대학의 모집단위만 처리한다.
///
/// 2-phase 구조 — 대학 순위를 위에서 훑되 트랙이 찬 학생만 건너뛰는 모델과 결과 동치:
///   1단계(트랙 채움): 각 모집단위를 **모집단위 순위**(트랙 prioritize_enrolled)로 정원까지 채움
///   2단계(대학 컷)  : 1단계 확정분을 **대학 전체 순위**(대학 prioritize_enrolled)로 다시 정렬해
///                     대학 잔여 정원까지 컷
/// 두 phase 모두 `fill_by_rank_groups` 를 사용해 동점 그룹을 원자적으로 처리한다.
/// 재학생 우선은 각 범위의 자기 플래그만 사용한다(OR 금지).
async fn run_auto_recommend(
    state: AppState,
    round_id: i64,
    univ_filter: Option<i64>,
) -> Result<Json<AutoRecommendResponse>, ApiError> {
    // BEGIN IMMEDIATE: 정원 카운트와 UPDATE 사이 TOCTOU 방지 — recommend_result와 동일 이유
    let mut tx = state.db.begin_with("BEGIN IMMEDIATE").await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 1. 라운드 상태 확인
    let round_status: Option<RoundStatus> = sqlx::query_scalar(
        "SELECT status FROM rounds WHERE id = ?",
    )
    .bind(round_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match round_status {
        Some(RoundStatus::Closed) => {}
        Some(_) => return Err((StatusCode::BAD_REQUEST, "CLOSED 라운드에서만 자동 추천 확정이 가능합니다".into())),
        None => return Err((StatusCode::NOT_FOUND, "라운드를 찾을 수 없습니다".into())),
    }

    // 1b. 대학 지정 시 존재 확인 (Fail-Fast — 없는 대학을 조용히 빈 결과로 처리하지 않는다)
    if let Some(uid) = univ_filter {
        let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM universities WHERE id = ?")
            .bind(uid)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        if exists.is_none() {
            return Err((StatusCode::NOT_FOUND, "대학을 찾을 수 없습니다".into()));
        }
    }

    // 2. 이 라운드에 results가 있는 모집단위 목록 로드
    #[derive(sqlx::FromRow)]
    struct TrackRow {
        track_id: i64,
        univ_id: i64,
        univ_name: String,
        track_name: String,
        unit_quota: Option<i64>,
        total_quota: Option<i64>,
    }

    let tracks: Vec<TrackRow> = sqlx::query_as(
        "SELECT ut.id AS track_id, ut.univ_id, u.univ_name, ut.track_name, ut.unit_quota, u.total_quota
         FROM univ_tracks ut
         JOIN universities u ON u.id = ut.univ_id
         WHERE ut.id IN (SELECT DISTINCT track_id FROM results WHERE round_id = ?)
           AND (? IS NULL OR ut.univ_id = ?)
         ORDER BY ut.id",
    )
    .bind(round_id)
    .bind(univ_filter).bind(univ_filter)
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    #[derive(sqlx::FromRow)]
    struct CandidateRow {
        student_id: i64,
        /// 대학 전체 순위 (results.ranking) — 2단계에서 사용
        ranking: Option<i64>,
        /// 모집단위 순위 (트랙 prioritize_enrolled 기준 파생) — 1단계에서 사용
        track_rank: i64,
        recommended: i64,
        excluded: i64,
    }

    let mut confirmed_items: Vec<AutoRecommendItem> = Vec::new();
    let mut manual_items: Vec<AutoRecommendManualItem> = Vec::new();

    // univ_id → (univ_name, total_quota)
    let mut univ_meta: HashMap<i64, (String, Option<i64>)> = HashMap::new();
    // univ_id → 트랙별 1단계 확정 후보 리스트 (각 리스트는 트랙 내부 순서 = (track_rank, univ_rank) 오름차순).
    // 2단계 병합이 트랙 내부 순서를 보존해야 하므로 **트랙 경계를 유지한 채** 넘긴다(평탄화 금지).
    let mut univ_pool: HashMap<i64, Vec<Vec<MergeCand>>> = HashMap::new();
    // track_id → (univ_id, univ_name, track_name)
    let mut track_meta: HashMap<i64, (i64, String, String)> = HashMap::new();

    // 3. 1단계 — 모집단위별 정원 채움
    for track in &tracks {
        univ_meta
            .entry(track.univ_id)
            .or_insert_with(|| (track.univ_name.clone(), track.total_quota));
        track_meta.insert(
            track.track_id,
            (track.univ_id, track.univ_name.clone(), track.track_name.clone()),
        );

        // 3a. used = 전 라운드 누적, recommended=1 AND abandoned=0 (recommend_result 동일 기준)
        let used: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM results r
             JOIN applications a ON a.student_id = r.student_id
                                 AND a.track_id  = r.track_id
                                 AND a.round_id  = r.round_id
             WHERE r.track_id = ? AND r.recommended = 1 AND a.abandoned = 0",
        )
        .bind(track.track_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        // 3b. remaining = unit_quota - used. unit_quota NULL = 무제한
        let remaining: Option<i64> = track.unit_quota.map(|q| q - used);

        // 3c. 이 모집단위 전체 결과 행 + 모집단위 순위 파생.
        //     RANK() 는 이미 추천 확정된 행까지 포함해 계산한다 — 화면(get_results)의
        //     모집단위 순위와 같은 값이어야 사유 메시지의 순위가 관리자에게 일치한다.
        let candidate_rank = track_rank_window("r", "ut", "s", false);
        let candidate_sql = format!(
            "SELECT r.student_id, r.ranking, r.recommended, a.excluded,
                    {candidate_rank}
             FROM results r
             JOIN students s ON s.id = r.student_id
             JOIN univ_tracks ut ON ut.id = r.track_id
             JOIN applications a ON a.student_id = r.student_id
                                 AND a.track_id  = r.track_id
                                 AND a.round_id  = r.round_id
             WHERE r.round_id = ? AND r.track_id = ?
             ORDER BY track_rank ASC"
        );
        let all_rows: Vec<CandidateRow> = sqlx::query_as(&candidate_sql)
        .bind(round_id)
        .bind(track.track_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        // 미선발 처리된 지원은 자동 추천 후보에서 빠진다 — RANK() 자체는 excluded 포함 전원으로
        // 계산해 화면(get_results)·수동 추천 가드와 같은 순위를 유지한다.
        let candidates: Vec<&CandidateRow> =
            all_rows.iter().filter(|c| c.recommended == 0 && c.excluded == 0).collect();

        // 3d. ranking IS NULL → manual (Fail-Fast: silent 스킵 금지)
        //     2단계 대학 컷이 대학 전체 순위를 필요로 하므로 누락 시 자동 판단 불가.
        if candidates.iter().any(|c| c.ranking.is_none()) {
            manual_items.push(AutoRecommendManualItem {
                track_id: Some(track.track_id),
                univ_name: track.univ_name.clone(),
                track_name: Some(track.track_name.clone()),
                reason: "순위 미계산 — 점수 재계산 필요".into(),
            });
            continue;
        }

        // 3e. 후보 0명 또는 유한 정원이고 remaining <= 0 → 조용히 스킵 (정원 소진·후보 없음은 오류 아님)
        if candidates.is_empty() || matches!(remaining, Some(r) if r <= 0) {
            continue;
        }

        // 3f. 동점 그룹 원자적 채움 — 모집단위 순위 기준
        //     정렬 키에 대학 순위를 2차로 넣는다: track_rank 가 같은(트랙 내부 동점) 후보들 사이에서
        //     대학 순위가 좋은 쪽이 앞에 오도록 — 2단계 병합의 선두가 그 덩어리의 최선이어야 한다.
        //     동점 그룹은 원자 처리(전원 확정 or 전원 보류)이므로 1단계 결과에는 영향이 없다.
        let mut items: Vec<(i64, MergeCand)> = candidates
            .iter()
            .map(|c| {
                // 대학 전체 순위 — 3d 에서 NULL 없음이 보장됨
                let univ_rank = c.ranking.ok_or_else(|| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "대학 전체 순위를 찾을 수 없습니다".to_string(),
                    )
                })?;
                Ok((
                    c.track_rank,
                    MergeCand {
                        student_id: c.student_id,
                        track_id: track.track_id,
                        track_rank: c.track_rank,
                        univ_rank,
                    },
                ))
            })
            .collect::<Result<Vec<_>, ApiError>>()?;
        items.sort_by_key(|(_, c)| (c.track_rank, c.univ_rank));

        let outcome = fill_by_rank_groups(&items, remaining);

        if let Some(tie) = &outcome.tie {
            manual_items.push(AutoRecommendManualItem {
                track_id: Some(track.track_id),
                univ_name: track.univ_name.clone(),
                track_name: Some(track.track_name.clone()),
                reason: format!(
                    "모집단위 {}위 동점 — 잔여 {}석에 {}명 경합 (관리자 선택 필요)",
                    tie.rank, tie.free, tie.contenders,
                ),
            });
        }

        // 1단계에서 동점으로 확정되지 않은 후보는 2단계 풀에 들어가지 않는다.
        // outcome.confirmed 는 items 순서 = (track_rank, univ_rank) 오름차순을 그대로 보존한다.
        if !outcome.confirmed.is_empty() {
            univ_pool.entry(track.univ_id).or_default().push(outcome.confirmed);
        }
    }

    // 4. 2단계 — 대학 전체 정원 컷 (트랙 내부 순서 보존 k-way 병합)
    let mut final_picks: Vec<MergeCand> = Vec::new();

    let mut univ_ids: Vec<i64> = univ_pool.keys().copied().collect();
    univ_ids.sort_unstable();

    for univ_id in univ_ids {
        let pool = univ_pool.remove(&univ_id).unwrap_or_default();
        let (univ_name, total_quota) = univ_meta
            .get(&univ_id)
            .cloned()
            .ok_or_else(|| (StatusCode::INTERNAL_SERVER_ERROR, "대학 정보 누락".to_string()))?;

        let Some(tq) = total_quota else {
            // 대학 정원 무제한 — 컷 미발동, 1단계 결과가 곧 최종
            final_picks.extend(merge_univ_cut(&pool, None).confirmed);
            continue;
        };

        let univ_used: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM results r
             JOIN applications a ON a.student_id = r.student_id
                                 AND a.track_id  = r.track_id
                                 AND a.round_id  = r.round_id
             JOIN univ_tracks ut ON ut.id = r.track_id
             WHERE ut.univ_id = ? AND r.recommended = 1 AND a.abandoned = 0",
        )
        .bind(univ_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let remaining_univ = tq - univ_used;

        // 트랙 내부 순서를 보존한 채 대학 순위로 병합 컷.
        // (전체 재정렬 금지 — 같은 트랙의 track_rank 상위자를 건너뛰면 안 된다.)
        let outcome = merge_univ_cut(&pool, Some(remaining_univ));

        if let Some(tie) = &outcome.tie {
            manual_items.push(AutoRecommendManualItem {
                track_id: None,
                univ_name: univ_name.clone(),
                track_name: None,
                reason: format!(
                    "대학 전체 {}위 동점 — 잔여 {}석에 {}명 경합 \
                     (경합 대상은 각 모집단위의 다음 차례 지원자에 한함 — \
                     같은 모집단위 상위 지원자에게 막힌 동순위자는 제외 / \
                     대학 정원 {}명, 확정 {}명, 잔여 {}석 / 관리자 선택 필요)",
                    tie.rank, tie.free, tie.contenders, tq, univ_used, remaining_univ,
                ),
            });
        }

        final_picks.extend(outcome.confirmed);
    }

    // 5. 확정된 선발 대상만 UPDATE results SET recommended = 1 (기존 recommended=1 행은 변경 안 함)
    let mut per_track: HashMap<i64, i64> = HashMap::new();
    for pick in &final_picks {
        sqlx::query(
            "UPDATE results SET recommended = 1
             WHERE student_id = ? AND track_id = ? AND round_id = ?",
        )
        .bind(pick.student_id).bind(pick.track_id).bind(round_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        *per_track.entry(pick.track_id).or_insert(0) += 1;
    }

    let mut confirmed_track_ids: Vec<i64> = per_track.keys().copied().collect();
    confirmed_track_ids.sort_unstable();
    for tid in confirmed_track_ids {
        let (_, univ_name, track_name) = track_meta
            .get(&tid)
            .cloned()
            .ok_or_else(|| (StatusCode::INTERNAL_SERVER_ERROR, "모집단위 정보 누락".to_string()))?;
        confirmed_items.push(AutoRecommendItem {
            track_id: tid,
            univ_name,
            track_name,
            count: per_track[&tid],
        });
    }

    // 6. 감사 로그 후 COMMIT
    let confirmed_tracks = confirmed_items.len() as i64;
    let confirmed_students: i64 = confirmed_items.iter().map(|i| i.count).sum();
    let manual_tracks = manual_items.len() as i64;
    crate::audit::log(
        &mut *tx,
        AuditEntry {
            actor: Actor::Admin,
            action: AuditAction::AutoRecommendRun,
            round_id: Some(round_id),
            student_id: None,
            detail: serde_json::json!({
                "confirmed_tracks": confirmed_tracks,
                "confirmed_students": confirmed_students,
                "manual_tracks": manual_tracks,
                "univ_id": univ_filter,
            }),
        },
    )
    .await?;

    tx.commit().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(AutoRecommendResponse { confirmed: confirmed_items, manual: manual_items }))
}

// ── 공용 점수 계산 헬퍼 ─────────────────────────────────────────────
//
// 확정 계산(`calc_area_score`)과 담임 미리보기(`teacher_area_score_preview`)가
// **반드시** 이 헬퍼를 통과하도록 하는 단일 진입점.
//
// 이전에는 두 경로가 CalcType 분기·점수표 폴백·집계 규칙을 각자 재구현하고 있어
// 시맨틱이 갈렸다(2차 감사 A 발견 2: CATEGORY 폴백이 whole-map vs per-category로 달랐음).
// 그런 부류의 버그를 재발생시키지 않기 위해 값→점수 변환 규칙을 이 헬퍼로 흡수한다.
//
// 호출자 책임:
// - base_data 또는 사용자 입력에서 값을 읽어 `AreaScoreInput`으로 정규화
// - `AreaScoreOutcome.raw`에 `min(area.max_score)` 캡핑 (raw 초과 여부는 미리보기 warning에 사용)
// - `AreaScoreError`를 자기 컨텍스트(학생/트랙 정보, HTTP 응답 형태)로 감싸 반환
//
// 이 헬퍼가 담당하는 것:
// - CalcType별 점수 계산 규칙 (NUMERIC 구간 lookup, CATEGORY per-category 폴백·집계, MANUAL 통과)
// - 점수표(numeric_table / category_map)의 track별 → 공통 폴백
// - 오버플로 감지, 미설정 메타데이터 감지

pub struct AreaMeta<'a> {
    pub id: i64,
    pub name: &'a str,
    pub max_score: i64,
    pub match_mode: Option<MatchMode>,
    pub category_agg: Option<CategoryAgg>,
}

pub enum AreaScoreInput<'a> {
    Numeric(i64),           // ×100000 정수
    Category(&'a [String]),
    Manual(i64),            // ×100000 정수
}

pub struct AreaScoreOutcome {
    /// 만점(max_score) 캡핑을 적용하기 전 raw 점수. 호출자가 캡핑 담당.
    pub raw: i64,
    /// NUMERIC일 때 매칭된 threshold (미리보기 하이라이팅용). CATEGORY/MANUAL은 None.
    pub matched_numeric_threshold: Option<i64>,
    /// CATEGORY일 때 매칭된 범주들. NUMERIC/MANUAL은 빈 배열.
    pub matched_categories: Vec<String>,
}

#[derive(Debug)]
pub enum AreaScoreError {
    /// NUMERIC 타입인데 area.match_mode가 설정되지 않음
    NumericMatchModeUnset,
    /// NUMERIC 점수표(numeric_table)가 track별·공통 모두 비어있음
    NumericTableEmpty,
    /// NUMERIC 매칭 실패 (하한 미만, 미등록 값 등). 인자는 lookup_range_score의 상세 오류
    NumericNoMatch(String),
    /// CATEGORY 입력 값 배열이 비어있음 (선택된 범주 없음)
    CategoryEmpty,
    /// CATEGORY 범주가 점수표에 없음. 인자는 해당 범주명
    CategoryUnknown(String),
    /// CATEGORY 타입인데 area.category_agg가 설정되지 않음
    CategoryAggUnset,
    /// CATEGORY SUM 집계 중 i64 오버플로
    CategorySumOverflow,
    /// DB 접근 오류
    Db(String),
}

impl std::fmt::Display for AreaScoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NumericMatchModeUnset => write!(f, "NUMERIC 타입에 match_mode가 설정되지 않았습니다"),
            Self::NumericTableEmpty => write!(f, "NUMERIC 점수 기준표가 설정되지 않았습니다"),
            Self::NumericNoMatch(s) => write!(f, "{}", s),
            Self::CategoryEmpty => write!(f, "선택된 범주가 없습니다"),
            Self::CategoryUnknown(cat) => write!(f, "점수표에 없는 범주: {}", cat),
            Self::CategoryAggUnset => write!(f, "CATEGORY 타입에 category_agg가 설정되지 않았습니다"),
            Self::CategorySumOverflow => write!(f, "CATEGORY SUM 점수 합산 오버플로우"),
            Self::Db(s) => write!(f, "DB 오류: {}", s),
        }
    }
}

/// 공용 점수 계산. base_data 조회 없이, 이미 정규화된 값(`AreaScoreInput`)만으로 raw 점수를 산출한다.
///
/// 확정 계산·미리보기 모두 이 함수를 통과해야 시맨틱이 갈리지 않는다.
/// max_score 캡핑은 호출자가 `outcome.raw.min(area.max_score)`로 수행한다.
pub async fn compute_area_score(
    conn: &mut sqlx::SqliteConnection,
    area: &AreaMeta<'_>,
    input: AreaScoreInput<'_>,
    lookup_track: Option<i64>,
) -> Result<AreaScoreOutcome, AreaScoreError> {
    match input {
        AreaScoreInput::Numeric(value) => {
            let mode = area.match_mode.ok_or(AreaScoreError::NumericMatchModeUnset)?;
            let mut rows: Vec<(i64, i64)> = sqlx::query(
                "SELECT threshold, score FROM numeric_table
                 WHERE area_id = ? AND (track_id = ? OR (? IS NULL AND track_id IS NULL))
                 ORDER BY threshold",
            )
            .bind(area.id).bind(lookup_track).bind(lookup_track)
            .fetch_all(&mut *conn).await.map_err(|e| AreaScoreError::Db(e.to_string()))?
            .into_iter()
            .map(|r| (r.get::<i64, _>("threshold"), r.get::<i64, _>("score")))
            .collect();

            // track별 점수표가 비었으면 공통(track_id IS NULL)으로 폴백
            if rows.is_empty() && lookup_track.is_some() {
                rows = sqlx::query(
                    "SELECT threshold, score FROM numeric_table
                     WHERE area_id = ? AND track_id IS NULL
                     ORDER BY threshold",
                )
                .bind(area.id)
                .fetch_all(&mut *conn).await.map_err(|e| AreaScoreError::Db(e.to_string()))?
                .into_iter()
                .map(|r| (r.get::<i64, _>("threshold"), r.get::<i64, _>("score")))
                .collect();
            }

            if rows.is_empty() {
                return Err(AreaScoreError::NumericTableEmpty);
            }

            let raw = lookup_range_score(value, &rows, mode)
                .map_err(AreaScoreError::NumericNoMatch)?;
            let matched_threshold = find_numeric_matched_threshold(value, &rows, mode);
            Ok(AreaScoreOutcome {
                raw,
                matched_numeric_threshold: matched_threshold,
                matched_categories: Vec::new(),
            })
        }

        AreaScoreInput::Category(values) => {
            if values.is_empty() {
                return Err(AreaScoreError::CategoryEmpty);
            }
            let agg = area.category_agg.ok_or(AreaScoreError::CategoryAggUnset)?;

            // per-category 폴백: track별 표에 해당 범주가 없으면 공통(track_id IS NULL) 표에서 찾는다.
            // 이전에는 확정 계산이 per-category, 미리보기가 whole-map으로 달랐음 (2차 감사 발견 2).
            let track_map: Vec<(String, i64)> = sqlx::query(
                "SELECT category, score FROM category_map
                 WHERE area_id = ? AND (track_id = ? OR (? IS NULL AND track_id IS NULL))",
            )
            .bind(area.id).bind(lookup_track).bind(lookup_track)
            .fetch_all(&mut *conn).await.map_err(|e| AreaScoreError::Db(e.to_string()))?
            .into_iter()
            .map(|r| (r.get::<String, _>("category"), r.get::<i64, _>("score")))
            .collect();

            let fallback_map: Vec<(String, i64)> = if lookup_track.is_some() {
                sqlx::query(
                    "SELECT category, score FROM category_map
                     WHERE area_id = ? AND track_id IS NULL",
                )
                .bind(area.id)
                .fetch_all(&mut *conn).await.map_err(|e| AreaScoreError::Db(e.to_string()))?
                .into_iter()
                .map(|r| (r.get::<String, _>("category"), r.get::<i64, _>("score")))
                .collect()
            } else {
                Vec::new()
            };

            let mut scores: Vec<i64> = Vec::with_capacity(values.len());
            let mut matched: Vec<String> = Vec::with_capacity(values.len());
            for cat in values {
                let found = track_map.iter().find(|(k, _)| k == cat)
                    .or_else(|| fallback_map.iter().find(|(k, _)| k == cat));
                match found {
                    Some((_, sc)) => {
                        scores.push(*sc);
                        matched.push(cat.clone());
                    }
                    None => return Err(AreaScoreError::CategoryUnknown(cat.clone())),
                }
            }

            let raw = match agg {
                CategoryAgg::Sum => scores.iter()
                    .try_fold(0i64, |acc, &s| acc.checked_add(s))
                    .ok_or(AreaScoreError::CategorySumOverflow)?,
                // scores.len() > 0가 위 loop 진입 조건(values 비어있지 않음)으로 보장됨
                CategoryAgg::Max => *scores.iter().max().expect("scores non-empty (values checked above)"),
            };
            Ok(AreaScoreOutcome {
                raw,
                matched_numeric_threshold: None,
                matched_categories: matched,
            })
        }

        AreaScoreInput::Manual(value) => Ok(AreaScoreOutcome {
            raw: value,
            matched_numeric_threshold: None,
            matched_categories: Vec::new(),
        }),
    }
}

/// NUMERIC 매칭 시 실제로 선택된 threshold를 반환한다 (미리보기 하이라이팅용).
/// `lookup_range_score`와 정확히 대칭 — 후자가 score를, 이 함수가 threshold를 낸다.
fn find_numeric_matched_threshold(value: i64, rows: &[(i64, i64)], mode: MatchMode) -> Option<i64> {
    match mode {
        MatchMode::Upper => rows.iter()
            .filter(|(th, _)| value >= *th)
            .max_by_key(|(th, _)| *th)
            .map(|(th, _)| *th),
        MatchMode::Lower => rows.iter()
            .filter(|(th, _)| value <= *th)
            .min_by_key(|(th, _)| *th)
            .map(|(th, _)| *th)
            .or_else(|| rows.iter().max_by_key(|(th, _)| *th).map(|(th, _)| *th)),
        MatchMode::Exact => rows.iter()
            .find(|(th, _)| *th == value)
            .map(|(th, _)| *th),
    }
}
