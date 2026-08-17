//! 감사 산출물 — 독립 오라클(E4) 대조용 실측값 덤프.
//!
//! `pcm-md/oracle/scenarios.json` 을 읽어 각 시나리오를 in-memory DB에 그대로 심고,
//! 실제 구현(`run_calculate_scores_on_conn` + `get_results` 핸들러)이 낸
//! total_score / ranking / track_rank 를 `pcm-md/oracle/actual.json` 으로 덤프한다.
//!
//! 오라클(파이썬)이 같은 입력으로 계산한 값과 `compare.py` 가 대조한다.
//!
//! 환경변수 `PCM_ORACLE_DIR` 가 없으면 **아무것도 하지 않고 통과**한다 —
//! 일반 `cargo test` 실행에 영향을 주지 않기 위함.
//!
//! 실행:
//!   PCM_ORACLE_DIR=/c/Users/user/Desktop/pcm-md/oracle cargo test --test audit_oracle_dump -- --nocapture

mod common;

use axum::extract::{Path, Query, State};
use common::{create_test_pool, make_state};
use principal_candidate_manager::handlers::scoring::{
    get_results, run_calculate_scores_on_conn, ResultQuery,
};
use serde_json::{json, Value};
use sqlx::SqlitePool;

fn oracle_dir() -> Option<std::path::PathBuf> {
    std::env::var("PCM_ORACLE_DIR").ok().map(std::path::PathBuf::from)
}

fn as_i64(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(|x| x.as_i64()).unwrap_or_else(|| panic!("{k} 누락: {v}"))
}

fn opt_i64(v: &Value, k: &str) -> Option<i64> {
    v.get(k).and_then(|x| x.as_i64())
}

fn as_str<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(|x| x.as_str()).unwrap_or_else(|| panic!("{k} 누락: {v}"))
}

fn opt_str<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
    v.get(k).and_then(|x| x.as_str())
}

async fn seed(pool: &SqlitePool, scn: &Value) {
    // 라운드 — CLOSED 로 바로 만든다(지원 INSERT 는 트리거 대상이 아니다).
    sqlx::query("INSERT INTO rounds (id, status, opened_at, closed_at) VALUES (1, 'CLOSED', ?, ?)")
        .bind("2026-08-17T00:00:00Z")
        .bind("2026-08-17T01:00:00Z")
        .execute(pool)
        .await
        .unwrap();

    // 전형요소
    for a in scn["areas"].as_array().unwrap() {
        sqlx::query(
            "INSERT INTO areas (id, name, max_score, calc_type, teacher_editable, lookup_scope,
                                match_mode, category_agg, multi_value, unit)
             VALUES (?, ?, ?, ?, 1, ?, ?, ?, ?, NULL)",
        )
        .bind(as_i64(a, "id"))
        .bind(as_str(a, "name"))
        .bind(as_i64(a, "max_score"))
        .bind(as_str(a, "calc_type"))
        .bind(as_str(a, "lookup_scope"))
        .bind(opt_str(a, "match_mode"))
        .bind(opt_str(a, "category_agg"))
        .bind(as_i64(a, "multi_value"))
        .execute(pool)
        .await
        .unwrap();
    }

    // 대학 / 모집단위
    for u in scn["universities"].as_array().unwrap() {
        sqlx::query(
            "INSERT INTO universities (id, univ_name, total_quota, prioritize_enrolled)
             VALUES (?, ?, ?, ?)",
        )
        .bind(as_i64(u, "id"))
        .bind(as_str(u, "univ_name"))
        .bind(opt_i64(u, "total_quota"))
        .bind(as_i64(u, "prioritize_enrolled"))
        .execute(pool)
        .await
        .unwrap();
    }
    for t in scn["tracks"].as_array().unwrap() {
        sqlx::query(
            "INSERT INTO univ_tracks (id, univ_id, track_name, unit_quota, prioritize_enrolled)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(as_i64(t, "id"))
        .bind(as_i64(t, "univ_id"))
        .bind(as_str(t, "track_name"))
        .bind(opt_i64(t, "unit_quota"))
        .bind(as_i64(t, "prioritize_enrolled"))
        .execute(pool)
        .await
        .unwrap();
    }

    // 학급 (students FK)
    let mut classes: Vec<(i64, i64)> = scn["students"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| Some((opt_i64(s, "grade")?, opt_i64(s, "class_no")?)))
        .collect();
    classes.sort_unstable();
    classes.dedup();
    for (g, c) in classes {
        sqlx::query("INSERT INTO classes (grade, class_no, teacher_name, password_hash) VALUES (?, ?, '담임', 'x')")
            .bind(g)
            .bind(c)
            .execute(pool)
            .await
            .unwrap();
    }

    // 학생
    for s in scn["students"].as_array().unwrap() {
        sqlx::query(
            "INSERT INTO students (id, student_code, name, grade, class_no, seq_no, is_enrolled, grad_year)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(as_i64(s, "id"))
        .bind(as_str(s, "student_code"))
        .bind(as_str(s, "name"))
        .bind(opt_i64(s, "grade"))
        .bind(opt_i64(s, "class_no"))
        .bind(opt_i64(s, "seq_no"))
        .bind(as_i64(s, "is_enrolled"))
        .bind(opt_i64(s, "grad_year"))
        .execute(pool)
        .await
        .unwrap();
    }

    // 점수표
    for r in scn["numeric_table"].as_array().unwrap() {
        sqlx::query("INSERT INTO numeric_table (area_id, track_id, threshold, score) VALUES (?, ?, ?, ?)")
            .bind(as_i64(r, "area_id"))
            .bind(opt_i64(r, "track_id"))
            .bind(as_i64(r, "threshold"))
            .bind(as_i64(r, "score"))
            .execute(pool)
            .await
            .unwrap();
    }
    for r in scn["category_map"].as_array().unwrap() {
        sqlx::query("INSERT INTO category_map (area_id, track_id, category, score) VALUES (?, ?, ?, ?)")
            .bind(as_i64(r, "area_id"))
            .bind(opt_i64(r, "track_id"))
            .bind(as_str(r, "category"))
            .bind(as_i64(r, "score"))
            .execute(pool)
            .await
            .unwrap();
    }

    // 기초 데이터
    for b in scn["base_data"].as_array().unwrap() {
        sqlx::query(
            "INSERT INTO base_data (student_id, area_id, track_id, value, multi_value)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(as_i64(b, "student_id"))
        .bind(as_i64(b, "area_id"))
        .bind(opt_i64(b, "track_id"))
        .bind(as_str(b, "value"))
        .bind(as_i64(b, "multi_value"))
        .execute(pool)
        .await
        .unwrap();
    }

    // 지원
    for a in scn["applications"].as_array().unwrap() {
        sqlx::query("INSERT INTO applications (student_id, track_id, round_id) VALUES (?, ?, ?)")
            .bind(as_i64(a, "student_id"))
            .bind(as_i64(a, "track_id"))
            .bind(as_i64(a, "round_id"))
            .execute(pool)
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn dump_actual_for_oracle_comparison() {
    let Some(dir) = oracle_dir() else {
        eprintln!("PCM_ORACLE_DIR 미설정 — 덤프 생략");
        return;
    };
    let raw = std::fs::read_to_string(dir.join("scenarios.json")).expect("scenarios.json 읽기 실패");
    let scenarios: Vec<Value> = serde_json::from_str(&raw).unwrap();

    let mut out: Vec<Value> = Vec::new();
    for scn in &scenarios {
        let pool = create_test_pool().await;
        seed(&pool, scn).await;

        let mut conn = pool.acquire().await.unwrap();
        let calc = run_calculate_scores_on_conn(&mut conn, 1, "2026-08-17T02:00:00Z").await;
        drop(conn);

        let calc_err = match calc {
            Ok(_) => None,
            Err(e) => Some(e),
        };

        // DB 원시값 (정수 그대로)
        let raw_rows: Vec<(i64, i64, i64, Option<i64>, String)> = sqlx::query_as(
            "SELECT student_id, track_id, total_score, ranking, score_detail
             FROM results WHERE round_id = 1 ORDER BY student_id, track_id",
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        // 핸들러(JSON 계약) 값 — track_rank 포함
        let state = make_state(pool.clone());
        let handler = get_results(
            State(state),
            Path(1i64),
            Query(ResultQuery { track_id: None }),
        )
        .await;
        let handler_rows: Vec<Value> = match handler {
            Ok(axum::Json(rows)) => rows
                .iter()
                .map(|r| serde_json::to_value(r).unwrap())
                .collect(),
            Err((_, e)) => {
                out.push(json!({ "name": scn["name"], "handler_error": e }));
                continue;
            }
        };

        let mut rows: Vec<Value> = Vec::new();
        for (sid, tid, total, ranking, detail) in &raw_rows {
            let h = handler_rows
                .iter()
                .find(|h| h["student_id"].as_i64() == Some(*sid) && h["track_id"].as_i64() == Some(*tid));
            rows.push(json!({
                "student_id": sid,
                "track_id": tid,
                "round_id": 1,
                "total_score_raw": total,
                "total_score_json": h.and_then(|h| h["total_score"].as_f64()),
                "ranking": ranking,
                "track_rank": h.and_then(|h| h["track_rank"].as_i64()),
                "score_detail_raw": serde_json::from_str::<Value>(detail).unwrap(),
                "score_detail_json": h.map(|h| h["score_detail"].clone()),
            }));
        }
        out.push(json!({
            "name": scn["name"],
            "calc_error": calc_err,
            "rows": rows,
        }));
        pool.close().await;
    }

    let path = dir.join("actual.json");
    std::fs::write(&path, serde_json::to_string_pretty(&out).unwrap()).unwrap();
    eprintln!("덤프 완료: {} (시나리오 {}개)", path.display(), out.len());
}
