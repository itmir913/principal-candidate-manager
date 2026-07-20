use axum::{
    extract::{Query, State},
    http::StatusCode,
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::{
    auth::TeacherClaims,
    enums::{CalcType, CategoryAgg, LookupScope, MatchMode},
    handlers::{
        area_data::fmt_score,
        scoring::{compute_area_score, AreaMeta, AreaScoreInput},
    },
    score::Score,
    state::AppState,
};

type ApiError = (StatusCode, String);

// ── Area Context ──────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct AreaContextQuery {
    pub student_id: i64,
    pub track_id: i64,
}

/// 점수표의 한 행. key는 NUMERIC에서 threshold(f64), CATEGORY에서 범주명(String).
#[derive(Debug, Serialize)]
pub struct ScoreTableRow {
    pub key: serde_json::Value,
    pub score: Score,
}

#[derive(Debug, Serialize)]
pub struct AreaContextItem {
    pub area_id: i64,
    pub area_name: String,
    pub calc_type: CalcType,
    pub max_score: Score,
    pub teacher_editable: bool,
    pub match_mode: Option<MatchMode>,
    pub category_agg: Option<CategoryAgg>,
    pub multi_value: bool,
    pub unit: Option<String>,
    /// 기저장 기초데이터 값 목록. NUMERIC/MANUAL은 표시용 소수 문자열, CATEGORY는 원문.
    /// 데이터 없으면 빈 배열.
    pub current_values: Vec<String>,
    /// NUMERIC/CATEGORY는 점수표, MANUAL은 None.
    pub table: Option<Vec<ScoreTableRow>>,
}

#[derive(sqlx::FromRow)]
struct AreaRow {
    id: i64,
    name: String,
    max_score: i64,
    calc_type: CalcType,
    teacher_editable: bool,
    lookup_scope: LookupScope,
    match_mode: Option<MatchMode>,
    category_agg: Option<CategoryAgg>,
    unit: Option<String>,
}

/// GET /api/teacher/area-context?student_id=X&track_id=Y
///
/// 전형요소 목록, 점수표, 기저장 기초데이터를 반환한다.
/// COMPOSITE 전형요소는 track_id별 테이블을 우선 사용하고, 없으면 전역(NULL) 테이블로 폴백.
pub async fn teacher_area_context(
    State(state): State<AppState>,
    Extension(claims): Extension<TeacherClaims>,
    Query(q): Query<AreaContextQuery>,
) -> Result<Json<Vec<AreaContextItem>>, ApiError> {
    let is_grad = claims.grade == 0 && claims.class_no == 0;
    let belongs: bool = if is_grad {
        sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM students WHERE id = ? AND is_enrolled = 0)",
        )
        .bind(q.student_id)
        .fetch_one(&state.db)
        .await
    } else {
        sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM students WHERE id = ? AND grade = ? AND class_no = ?)",
        )
        .bind(q.student_id)
        .bind(claims.grade)
        .bind(claims.class_no)
        .fetch_one(&state.db)
        .await
    }
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if !belongs {
        return Err((StatusCode::FORBIDDEN, "해당 학생은 담당 학급이 아닙니다".into()));
    }

    let areas: Vec<AreaRow> = sqlx::query_as::<_, AreaRow>(
        "SELECT id, name, max_score, calc_type, teacher_editable, lookup_scope,
                match_mode, category_agg, unit
         FROM areas ORDER BY id",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut items: Vec<AreaContextItem> = Vec::with_capacity(areas.len());

    for area in &areas {
        let lookup_track: Option<i64> = if area.lookup_scope == LookupScope::Composite {
            Some(q.track_id)
        } else {
            None
        };

        let table: Option<Vec<ScoreTableRow>> = match area.calc_type {
            CalcType::Numeric => {
                let mut rows = load_numeric_table(&state.db, area.id, lookup_track).await?;
                if rows.is_empty() && lookup_track.is_some() {
                    rows = load_numeric_table(&state.db, area.id, None).await?;
                }
                Some(rows)
            }
            CalcType::Category => {
                let mut rows = load_category_table(&state.db, area.id, lookup_track).await?;
                if rows.is_empty() && lookup_track.is_some() {
                    rows = load_category_table(&state.db, area.id, None).await?;
                }
                Some(rows)
            }
            CalcType::Manual => None,
        };

        let current_values =
            load_base_data_display(&state.db, q.student_id, area.id, lookup_track, area.calc_type)
                .await?;

        items.push(AreaContextItem {
            area_id: area.id,
            area_name: area.name.clone(),
            calc_type: area.calc_type,
            max_score: Score::from_raw(area.max_score),
            teacher_editable: area.teacher_editable,
            match_mode: area.match_mode,
            category_agg: area.category_agg,
            multi_value: area.category_agg == Some(CategoryAgg::Sum),
            unit: area.unit.clone(),
            current_values,
            table,
        });
    }

    Ok(Json(items))
}

async fn load_numeric_table(
    db: &sqlx::SqlitePool,
    area_id: i64,
    track_id: Option<i64>,
) -> Result<Vec<ScoreTableRow>, ApiError> {
    sqlx::query(
        "SELECT threshold, score FROM numeric_table
         WHERE area_id = ? AND (track_id = ? OR (? IS NULL AND track_id IS NULL))
         ORDER BY threshold",
    )
    .bind(area_id)
    .bind(track_id)
    .bind(track_id)
    .fetch_all(db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
    .map(|rows| {
        rows.into_iter()
            .map(|r| ScoreTableRow {
                key: serde_json::Value::from(
                    r.get::<i64, _>("threshold") as f64 / 100_000.0,
                ),
                score: Score::from_raw(r.get::<i64, _>("score")),
            })
            .collect()
    })
}

async fn load_category_table(
    db: &sqlx::SqlitePool,
    area_id: i64,
    track_id: Option<i64>,
) -> Result<Vec<ScoreTableRow>, ApiError> {
    sqlx::query(
        "SELECT category, score FROM category_map
         WHERE area_id = ? AND (track_id = ? OR (? IS NULL AND track_id IS NULL))
         ORDER BY category, score",
    )
    .bind(area_id)
    .bind(track_id)
    .bind(track_id)
    .fetch_all(db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
    .map(|rows| {
        rows.into_iter()
            .map(|r| ScoreTableRow {
                key: serde_json::Value::from(r.get::<String, _>("category")),
                score: Score::from_raw(r.get::<i64, _>("score")),
            })
            .collect()
    })
}

async fn load_base_data_display(
    db: &sqlx::SqlitePool,
    student_id: i64,
    area_id: i64,
    track_id: Option<i64>,
    calc_type: CalcType,
) -> Result<Vec<String>, ApiError> {
    let raw: Vec<String> = sqlx::query_scalar(
        "SELECT value FROM base_data
         WHERE student_id = ? AND area_id = ?
           AND (track_id = ? OR (? IS NULL AND track_id IS NULL))",
    )
    .bind(student_id)
    .bind(area_id)
    .bind(track_id)
    .bind(track_id)
    .fetch_all(db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let values = raw
        .into_iter()
        .map(|v| match calc_type {
            CalcType::Numeric | CalcType::Manual => v
                .trim()
                .parse::<i64>()
                .map(|n| fmt_score(n))
                .unwrap_or(v),
            CalcType::Category => v,
        })
        .collect();

    Ok(values)
}

// ── Area Score Preview ────────────────────────────────────────────

#[derive(Deserialize)]
pub struct AreaScorePreviewBody {
    pub area_id: i64,
    pub track_id: i64,
    pub values: Vec<String>,
}

#[derive(Serialize)]
pub struct AreaScorePreviewResponse {
    pub score: Option<Score>,
    /// 점수표에서 하이라이팅할 행의 key 목록.
    /// NUMERIC: threshold f64, CATEGORY: 범주명 String.
    pub matched_keys: Vec<serde_json::Value>,
    pub warning: Option<String>,
    pub error: Option<String>,
}

#[derive(sqlx::FromRow)]
struct AreaProps {
    calc_type: CalcType,
    max_score: i64,
    lookup_scope: LookupScope,
    match_mode: Option<MatchMode>,
    category_agg: Option<CategoryAgg>,
}

/// POST /api/teacher/area-score-preview, POST /api/area-score-preview (관리자 데모에서도 재사용)
///
/// 입력값을 DB에 저장하지 않고 점수를 즉시 계산한다.
/// 프론트엔드 실시간 점수표 하이라이팅 전용.
pub async fn teacher_area_score_preview(
    State(state): State<AppState>,
    Json(body): Json<AreaScorePreviewBody>,
) -> Result<Json<AreaScorePreviewResponse>, ApiError> {
    let area: AreaProps = sqlx::query_as::<_, AreaProps>(
        "SELECT calc_type, max_score, lookup_scope, match_mode, category_agg
         FROM areas WHERE id = ?",
    )
    .bind(body.area_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or_else(|| (StatusCode::NOT_FOUND, format!("전형요소 id={} 없음", body.area_id)))?;

    if body.values.is_empty() {
        return Ok(Json(AreaScorePreviewResponse {
            score: None,
            matched_keys: vec![],
            warning: None,
            error: Some("값이 입력되지 않았습니다".into()),
        }));
    }

    let lookup_track: Option<i64> = if area.lookup_scope == LookupScope::Composite {
        Some(body.track_id)
    } else {
        None
    };

    // 사용자 입력을 CalcType에 맞는 AreaScoreInput으로 정규화한다.
    // 값 파싱(표시 문자열 → i64) 오류는 preview 응답 형태로 즉시 반환한다.
    let input: AreaScoreInput = match area.calc_type {
        CalcType::Numeric => match parse_display_str(&body.values[0]) {
            Ok(v) => AreaScoreInput::Numeric(v),
            Err(e) => return Ok(Json(preview_error(e))),
        },
        CalcType::Category => AreaScoreInput::Category(&body.values),
        CalcType::Manual => match parse_display_str(&body.values[0]) {
            Ok(v) => AreaScoreInput::Manual(v),
            Err(e) => return Ok(Json(preview_error(e))),
        },
    };

    let meta = AreaMeta {
        id: body.area_id,
        name: "",  // 미리보기 오류 메시지는 area 이름을 쓰지 않음
        max_score: area.max_score,
        match_mode: area.match_mode,
        category_agg: area.category_agg,
    };

    let mut conn = state.db.acquire().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let outcome = match compute_area_score(&mut conn, &meta, input, lookup_track).await {
        Ok(o) => o,
        Err(e) => return Ok(Json(preview_error(e.to_string()))),
    };

    let capped = outcome.raw.min(area.max_score);
    let matched_keys: Vec<serde_json::Value> = if !outcome.matched_categories.is_empty() {
        outcome.matched_categories.iter()
            .map(|c| serde_json::Value::from(c.as_str()))
            .collect()
    } else if let Some(th) = outcome.matched_numeric_threshold {
        vec![serde_json::Value::from(th as f64 / 100_000.0)]
    } else {
        vec![]
    };

    let warning = if outcome.raw > area.max_score {
        let msg = match area.calc_type {
            CalcType::Manual => "입력값이 만점을 초과하여 만점으로 처리됩니다",
            _ => "계산된 점수가 만점을 초과하여 만점으로 처리됩니다",
        };
        Some(msg.to_string())
    } else {
        None
    };
    Ok(Json(AreaScorePreviewResponse {
        score: Some(Score::from_raw(capped)),
        matched_keys,
        warning,
        error: None,
    }))
}

fn preview_error(msg: String) -> AreaScorePreviewResponse {
    AreaScorePreviewResponse {
        score: None,
        matched_keys: vec![],
        warning: None,
        error: Some(msg),
    }
}

// ── Helpers ───────────────────────────────────────────────────────

/// 표시값 문자열("30.5") → 내부 정수(3050000). area_data::parse_display_value 동일 로직.
fn parse_display_str(s: &str) -> Result<i64, String> {
    crate::handlers::area_data::parse_display_value(s)
}
