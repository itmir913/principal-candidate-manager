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
    auth::TeacherClaims,
    enums::{CalcType, CategoryAgg, LookupScope, MatchMode, RoundStatus},
    excel, score::Score, state::AppState,
};

type ApiError = (StatusCode, String);

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
    pub recommended: bool,
    pub abandoned: bool,
    pub student_code: String,
    pub name: String,
    pub grade: Option<i64>,
    pub class_no: Option<i64>,
    pub seq_no: Option<i64>,
    pub is_enrolled: bool,
    pub univ_name: String,
    pub track_name: String,
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
                format!("UPPER 매칭 실패: 값 {}에 해당하는 구간 항목이 없습니다 (모든 하한치보다 낮습니다)", value)
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
            .ok_or_else(|| format!("EXACT 매칭 실패: 값 {}에 해당하는 구간 항목이 없습니다", value)),
    }
}

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

    let raw: i64 = match area.calc_type {
        CalcType::Numeric => {
            let value_str: Option<String> = sqlx::query_scalar(
                "SELECT value FROM base_data
                 WHERE student_id = ? AND area_id = ?
                   AND (track_id = ? OR (? IS NULL AND track_id IS NULL))",
            )
            .bind(student_id).bind(area.id).bind(lookup_track).bind(lookup_track)
            .fetch_optional(&mut *db).await.map_err(|e| e.to_string())?;

            let vs = value_str.ok_or_else(|| {
                format!("전형요소 '{}': {} {} 지원자 {} ({})의 NUMERIC base_data가 없습니다",
                    area.name, ctx.univ_name, ctx.track_name, ctx.student_name, ctx.student_code)
            })?;
            let value: i64 = vs.trim().parse::<i64>().map_err(|_| {
                format!("전형요소 '{}': {} {} 지원자 {} ({})의 base_data 값 '{}' 을 정수로 파싱할 수 없습니다",
                    area.name, ctx.univ_name, ctx.track_name, ctx.student_name, ctx.student_code, vs.trim())
            })?;
            let mode = area.match_mode
                .ok_or_else(|| format!("전형요소 '{}': NUMERIC 타입에 match_mode가 설정되지 않았습니다", area.name))?;

            let mut rows: Vec<(i64, i64)> = sqlx::query(
                "SELECT threshold, score FROM numeric_table
                 WHERE area_id = ? AND (track_id = ? OR (? IS NULL AND track_id IS NULL))
                 ORDER BY threshold",
            )
            .bind(area.id).bind(lookup_track).bind(lookup_track)
            .fetch_all(&mut *db).await.map_err(|e| e.to_string())?
            .into_iter()
            .map(|r| (r.get::<i64, _>("threshold"), r.get::<i64, _>("score")))
            .collect();

            // 모집단위별 점수 기준이 없으면 공통(track_id IS NULL) 테이블로 폴백
            if rows.is_empty() && lookup_track.is_some() {
                rows = sqlx::query(
                    "SELECT threshold, score FROM numeric_table
                     WHERE area_id = ? AND track_id IS NULL
                     ORDER BY threshold",
                )
                .bind(area.id)
                .fetch_all(&mut *db).await.map_err(|e| e.to_string())?
                .into_iter()
                .map(|r| (r.get::<i64, _>("threshold"), r.get::<i64, _>("score")))
                .collect();
            }

            lookup_range_score(value, &rows, mode)
                .map_err(|e| format!("전형요소 '{}': {} {} 지원자 {} ({}) - {}",
                    area.name, ctx.univ_name, ctx.track_name, ctx.student_name, ctx.student_code, e))?
        }

        CalcType::Category => {
            let values: Vec<String> = sqlx::query_scalar(
                "SELECT value FROM base_data
                 WHERE student_id = ? AND area_id = ?
                   AND (track_id = ? OR (? IS NULL AND track_id IS NULL))",
            )
            .bind(student_id).bind(area.id).bind(lookup_track).bind(lookup_track)
            .fetch_all(&mut *db).await.map_err(|e| e.to_string())?;

            let mut scores: Vec<i64> = Vec::new();
            for cat in &values {
                let mut sc: Option<i64> = sqlx::query_scalar(
                    "SELECT score FROM category_map
                     WHERE area_id = ? AND category = ?
                       AND (track_id = ? OR (? IS NULL AND track_id IS NULL))",
                )
                .bind(area.id).bind(cat.as_str()).bind(lookup_track).bind(lookup_track)
                .fetch_optional(&mut *db).await.map_err(|e| e.to_string())?;

                // 모집단위별 범주 기준이 없으면 공통(track_id IS NULL) 범주표로 폴백
                if sc.is_none() && lookup_track.is_some() {
                    sc = sqlx::query_scalar(
                        "SELECT score FROM category_map
                         WHERE area_id = ? AND category = ? AND track_id IS NULL",
                    )
                    .bind(area.id).bind(cat.as_str())
                    .fetch_optional(&mut *db).await.map_err(|e| e.to_string())?;
                }

                match sc {
                    Some(s) => scores.push(s),
                    None => return Err(format!(
                        "전형요소 '{}': {} {} 지원자 {} ({})에 대해 범주 '{}' 에 해당하는 category_map 항목이 없습니다",
                        area.name, ctx.univ_name, ctx.track_name, ctx.student_name, ctx.student_code, cat
                    )),
                }
            }

            if scores.is_empty() {
                return Err(format!(
                    "전형요소 '{}': {} {} 지원자 {} ({})의 CATEGORY base_data가 없습니다",
                    area.name, ctx.univ_name, ctx.track_name, ctx.student_name, ctx.student_code
                ));
            }
            match area.category_agg {
                Some(CategoryAgg::Sum) => scores.iter().sum::<i64>(),
                Some(CategoryAgg::Max) => *scores.iter().max()
                    .ok_or_else(|| format!("전형요소 '{}': MAX 집계이지만 점수 목록이 비어 있습니다", area.name))?,
                None => return Err(format!(
                    "전형요소 '{}': CATEGORY 타입에 category_agg가 설정되지 않았습니다", area.name
                )),
            }
        }

        CalcType::Manual => {
            let v: Option<String> = sqlx::query_scalar(
                "SELECT value FROM base_data
                 WHERE student_id = ? AND area_id = ?
                   AND (track_id = ? OR (? IS NULL AND track_id IS NULL))",
            )
            .bind(student_id).bind(area.id).bind(lookup_track).bind(lookup_track)
            .fetch_optional(&mut *db).await.map_err(|e| e.to_string())?;

            match v {
                None => return Err(format!(
                    "전형요소 '{}': {} {} 지원자 {} ({})의 MANUAL base_data가 없습니다",
                    area.name, ctx.univ_name, ctx.track_name, ctx.student_name, ctx.student_code
                )),
                Some(s) => s.trim().parse::<i64>().map_err(|_| {
                    format!("전형요소 '{}': {} {} 지원자 {} ({})의 MANUAL base_data 값 '{}' 을 정수로 파싱할 수 없습니다",
                        area.name, ctx.univ_name, ctx.track_name, ctx.student_name, ctx.student_code, s.trim())
                })?,
            }
        }
    };

    Ok(raw.min(area.max_score))
}

// ── Handlers ──────────────────────────────────────────────────────

pub async fn calculate_scores(
    State(state): State<AppState>,
    Path(round_id): Path<i64>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let round_status: Option<RoundStatus> = sqlx::query_scalar(
        "SELECT status FROM rounds WHERE id = ?",
    )
    .bind(round_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match round_status {
        Some(RoundStatus::Closed) => {}
        Some(_) => return Err((StatusCode::BAD_REQUEST, "CLOSED 라운드에서만 점수 계산이 가능합니다".into())),
        None => return Err((StatusCode::NOT_FOUND, "라운드를 찾을 수 없습니다".into())),
    }

    let areas: Vec<AreaRow> = sqlx::query_as::<_, AreaRow>(
        "SELECT id, name, calc_type, max_score, match_mode, category_agg, lookup_scope FROM areas ORDER BY id",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let applications: Vec<AppRef> = sqlx::query_as::<_, AppRef>(
        "SELECT a.student_id, a.track_id, s.student_code, s.name, u.univ_name, ut.track_name
         FROM applications a
         JOIN students s ON s.id = a.student_id
         JOIN univ_tracks ut ON ut.id = a.track_id
         JOIN universities u ON u.id = ut.univ_id
         WHERE a.round_id = ? AND a.confirmed = 1",
    )
    .bind(round_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let now = chrono::Utc::now().to_rfc3339();
    let mut count = 0usize;

    // 점수 계산(읽기 전용)은 트랜잭션 밖에서 수행
    let mut conn = state.db.acquire().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let mut score_rows: Vec<(i64, i64, String, i64)> = Vec::new(); // (student_id, track_id, detail_json, total)
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
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
            detail.insert(area.id.to_string(), sc);
            total += sc;
        }
        let detail_json = serde_json::to_string(&detail)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        score_rows.push((app.student_id, app.track_id, detail_json, total));
    }

    // results 쓰기 + 순위 계산 전체를 하나의 트랜잭션으로
    let mut tx = state.db.begin().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    for (student_id, track_id, detail_json, total) in &score_rows {
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
        .bind(student_id).bind(track_id).bind(round_id)
        .bind(detail_json).bind(total).bind(&now)
        .execute(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        count += 1;
    }

    // 대학별 순위 재계산 (트랜잭션 내에서 읽어야 방금 쓴 점수가 보임)
    let mut track_ids: Vec<i64> = applications.iter().map(|a| a.track_id).collect();
    track_ids.sort_unstable();
    track_ids.dedup();

    for tid in track_ids {
        let prioritize: bool = sqlx::query_scalar(
            "SELECT u.prioritize_enrolled
             FROM univ_tracks ut JOIN universities u ON ut.univ_id = u.id
             WHERE ut.id = ?",
        )
        .bind(tid)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let rows = sqlx::query(
            "SELECT r.student_id, r.total_score, s.is_enrolled
             FROM results r
             JOIN students s ON r.student_id = s.id
             WHERE r.round_id = ? AND r.track_id = ?",
        )
        .bind(round_id).bind(tid)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let mut ranked: Vec<(i64, Score, bool)> = rows
            .into_iter()
            .map(|r| (
                r.get::<i64, _>("student_id"),
                Score::from_raw(r.get::<i64, _>("total_score")),
                r.get::<bool, _>("is_enrolled"),
            ))
            .collect();

        ranked.sort_by(|a, b| {
            b.1.cmp(&a.1).then_with(|| {
                if prioritize { b.2.cmp(&a.2) } else { std::cmp::Ordering::Equal }
            })
        });

        for (rank, (sid, _, _)) in ranked.iter().enumerate() {
            sqlx::query(
                "UPDATE results SET ranking = ? WHERE student_id = ? AND track_id = ? AND round_id = ?",
            )
            .bind((rank + 1) as i64).bind(sid).bind(tid).bind(round_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        }
    }

    tx.commit().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::json!({ "calculated": count })))
}

pub async fn get_results(
    State(state): State<AppState>,
    Path(round_id): Path<i64>,
    Query(q): Query<ResultQuery>,
) -> Result<Json<Vec<ResultRow>>, ApiError> {
    let rows = sqlx::query_as::<_, ResultRow>(
        "SELECT r.student_id, r.track_id, r.round_id,
                r.total_score, r.score_detail, r.ranking, r.recommended,
                COALESCE(a.abandoned, 0) AS abandoned,
                s.student_code, s.name, s.grade, s.class_no, s.seq_no, s.is_enrolled,
                u.univ_name, ut.track_name
         FROM results r
         JOIN students s ON r.student_id = s.id
         JOIN univ_tracks ut ON r.track_id = ut.id
         JOIN universities u ON ut.univ_id = u.id
         LEFT JOIN applications a ON a.student_id = r.student_id
                                  AND a.track_id  = r.track_id
                                  AND a.round_id  = r.round_id
         WHERE r.round_id = ?
           AND (? IS NULL OR r.track_id = ?)
         ORDER BY r.track_id, r.ranking NULLS LAST, r.total_score DESC",
    )
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

#[derive(FromRow)]
struct UnivRef {
    id: i64,
    univ_name: String,
    track_name: String,
}

pub async fn export_results(
    State(state): State<AppState>,
    Path(round_id): Path<i64>,
) -> Result<Response, ApiError> {
    let areas: Vec<AreaName> = sqlx::query_as::<_, AreaName>(
        "SELECT id, name FROM areas ORDER BY id",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let univs: Vec<UnivRef> = sqlx::query_as::<_, UnivRef>(
        "SELECT DISTINCT ut.id, u.univ_name, ut.track_name
         FROM results r
         JOIN univ_tracks ut ON r.track_id = ut.id
         JOIN universities u ON ut.univ_id = u.id
         WHERE r.round_id = ?
         ORDER BY u.univ_name, ut.track_name",
    )
    .bind(round_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let all_results = sqlx::query_as::<_, ResultRow>(
        "SELECT r.student_id, r.track_id, r.round_id,
                r.total_score, r.score_detail, r.ranking, r.recommended,
                COALESCE(a.abandoned, 0) AS abandoned,
                s.student_code, s.name, s.grade, s.class_no, s.seq_no, s.is_enrolled,
                u.univ_name, ut.track_name
         FROM results r
         JOIN students s ON r.student_id = s.id
         JOIN univ_tracks ut ON r.track_id = ut.id
         JOIN universities u ON ut.univ_id = u.id
         LEFT JOIN applications a ON a.student_id = r.student_id
                                  AND a.track_id  = r.track_id
                                  AND a.round_id  = r.round_id
         WHERE r.round_id = ?
         ORDER BY r.track_id, r.ranking NULLS LAST, r.total_score DESC",
    )
    .bind(round_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut wb = Workbook::new();

    for univ in &univs {
        let sheet_name: String = format!("{} {}", univ.univ_name, univ.track_name)
            .chars()
            .take(31)
            .collect();
        let ws = wb
            .add_worksheet()
            .set_name(&sheet_name)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        // 헤더 행
        let fixed_headers = ["순위", "학생명", "학생코드", "학년", "반", "번호", "재학구분"];
        let mut col = 0u16;
        for h in &fixed_headers {
            ws.write_string(0, col, *h).ok();
            col += 1;
        }
        for area in &areas {
            ws.write_string(0, col, &area.name).ok();
            col += 1;
        }
        ws.write_string(0, col, "총점").ok(); col += 1;
        ws.write_string(0, col, "추천").ok(); col += 1;
        ws.write_string(0, col, "포기").ok();

        // 데이터 행
        let univ_results: Vec<&ResultRow> =
            all_results.iter().filter(|r| r.track_id == univ.id).collect();

        for (i, r) in univ_results.iter().enumerate() {
            let row = (i + 1) as u32;
            let mut col = 0u16;

            if let Some(rank) = r.ranking {
                ws.write_number(row, col, rank as f64).ok();
            }
            col += 1;

            ws.write_string(row, col, &r.name).ok(); col += 1;
            ws.write_string(row, col, &r.student_code).ok(); col += 1;

            if let Some(g) = r.grade { ws.write_number(row, col, g as f64).ok(); }
            col += 1;
            if let Some(c) = r.class_no { ws.write_number(row, col, c as f64).ok(); }
            col += 1;
            if let Some(s) = r.seq_no { ws.write_number(row, col, s as f64).ok(); }
            col += 1;

            ws.write_string(row, col, if r.is_enrolled { "재학" } else { "졸업" }).ok();
            col += 1;

            let detail: HashMap<String, i64> = serde_json::from_str(&r.score_detail)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!(
                    "학생 id={} score_detail JSON 파싱 실패: {}", r.student_id, e
                )))?;
            for area in &areas {
                let sc = detail.get(&area.id.to_string()).copied()
                    .ok_or_else(|| (StatusCode::INTERNAL_SERVER_ERROR, format!(
                        "학생 id={} 전형요소 id={}의 점수가 없습니다. 점수 재계산이 필요합니다",
                        r.student_id, area.id
                    )))?;
                ws.write_number(row, col, sc as f64 / 100_000.0).ok();
                col += 1;
            }

            ws.write_number(row, col, r.total_score.raw() as f64 / 100_000.0).ok(); col += 1;
            ws.write_string(row, col, if r.recommended { "추천" } else { "" }).ok(); col += 1;
            ws.write_string(row, col, if r.abandoned { "포기" } else { "" }).ok();
        }
    }

    let buf = wb
        .save_to_buffer()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let filename = format!("results_round_{}_{}.xlsx", round_id, excel::now_tag());
    Ok(excel::xlsx_response(buf, &filename))
}

// ── Teacher results ───────────────────────────────────────────────

#[derive(Deserialize)]
pub struct TeacherResultQuery {
    pub round_id: Option<i64>,
}

pub async fn teacher_get_results(
    State(state): State<AppState>,
    Extension(claims): Extension<TeacherClaims>,
    Query(q): Query<TeacherResultQuery>,
) -> Result<Json<Vec<ResultRow>>, ApiError> {
    let round_id = match q.round_id {
        Some(rid) => rid,
        None => {
            let rid: Option<i64> = sqlx::query_scalar(
                "SELECT id FROM rounds ORDER BY id DESC LIMIT 1",
            )
            .fetch_optional(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            match rid {
                Some(id) => id,
                None => return Ok(Json(vec![])),
            }
        }
    };

    // FINALIZED 라운드에서만 결과 공개
    let status: Option<RoundStatus> = sqlx::query_scalar(
        "SELECT status FROM rounds WHERE id = ?",
    )
    .bind(round_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if status != Some(RoundStatus::Finalized) {
        return Ok(Json(vec![]));
    }

    let rows = sqlx::query_as::<_, ResultRow>(
        "SELECT r.student_id, r.track_id, r.round_id,
                r.total_score, r.score_detail, r.ranking, r.recommended,
                COALESCE(a.abandoned, 0) AS abandoned,
                s.student_code, s.name, s.grade, s.class_no, s.seq_no, s.is_enrolled,
                u.univ_name, ut.track_name
         FROM results r
         JOIN students s ON r.student_id = s.id
         JOIN univ_tracks ut ON r.track_id = ut.id
         JOIN universities u ON ut.univ_id = u.id
         LEFT JOIN applications a ON a.student_id = r.student_id
                                  AND a.track_id  = r.track_id
                                  AND a.round_id  = r.round_id
         WHERE r.round_id = ?
           AND s.grade = ?
           AND s.class_no = ?
         ORDER BY s.seq_no, r.track_id",
    )
    .bind(round_id)
    .bind(claims.grade)
    .bind(claims.class_no)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(rows))
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
        total_raw += score_raw;
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
    let status: Option<RoundStatus> = sqlx::query_scalar(
        "SELECT status FROM rounds WHERE id = ?",
    )
    .bind(rid)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if status != Some(RoundStatus::Closed) {
        return Err((StatusCode::BAD_REQUEST, "CLOSED 라운드에서만 추천 확정이 가능합니다".into()));
    }

    sqlx::query(
        "UPDATE results SET recommended = 1 WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(sid).bind(tid).bind(rid)
    .execute(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn unrecommend_result(
    State(state): State<AppState>,
    Path((sid, tid, rid)): Path<(i64, i64, i64)>,
) -> Result<StatusCode, ApiError> {
    let status: Option<RoundStatus> = sqlx::query_scalar(
        "SELECT status FROM rounds WHERE id = ?",
    )
    .bind(rid)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if status != Some(RoundStatus::Closed) {
        return Err((StatusCode::BAD_REQUEST, "CLOSED 라운드에서만 추천 취소가 가능합니다".into()));
    }

    sqlx::query(
        "UPDATE results SET recommended = 0 WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(sid).bind(tid).bind(rid)
    .execute(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}
