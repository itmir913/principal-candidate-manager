//! 감사 산출물 — 독립 오라클(E4) 대조용 실측값 덤프.
//!
//! `tools/oracle/scenarios.json` 을 읽어 각 시나리오를 in-memory DB에 그대로 심고,
//! 실제 구현(`run_calculate_scores_on_conn` + `get_results` 핸들러)이 낸
//! total_score / ranking / track_rank 를 `tools/oracle/actual.json` 으로 덤프한다.
//!
//! 오라클(파이썬)이 같은 입력으로 계산한 값과 `compare.py` 가 대조한다.
//!
//! 라운드 3 확장: 시나리오가 `excluded`/`abandoned`/`recommended` 플래그와
//! `round_status` 를 담을 수 있다. 이 플래그들은 **실제 생명주기 순서대로** 적용한다 —
//! CLOSED 에서 excluded·recommended 를 UPDATE 하고, FINALIZED 로 전이한 뒤 abandoned 를 UPDATE.
//! 트리거(`trg_prevent_update_closed_application`, `trg_require_all_decided_before_finalize`,
//! `trg_prevent_update_finalized_result`)를 우회하지 않으므로, 여기서 만들어지는 상태는
//! 전부 API 로 도달 가능한 상태다.
//!
//! 환경변수 `PCM_ORACLE_DIR` 가 없으면 **아무것도 하지 않고 통과**한다 —
//! 일반 `cargo test` 실행에 영향을 주지 않기 위함.
//!
//! 실행:
//!   npm run test:oracle    # 또는 PCM_ORACLE_DIR=tools/oracle cargo test --test audit_oracle_dump

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
    // 라운드 — 항상 CLOSED 로 만든다. FINALIZED 시나리오는 플래그 적용 후 전이한다.
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

/// 시나리오의 excluded / recommended / abandoned 플래그를 **실제 생명주기 순서대로** 적용한다.
/// 트리거를 우회하지 않으므로, 실패하면 그 상태가 API 로 도달 불가하다는 뜻이다.
async fn apply_lifecycle_flags(pool: &SqlitePool, scn: &Value) -> Result<(), String> {
    // ① CLOSED 에서 미선발 처리 (trg_prevent_update_closed_application 이 허용하는 유일한 변경)
    for a in scn["applications"].as_array().unwrap() {
        if opt_i64(a, "excluded") == Some(1) {
            sqlx::query(
                "UPDATE applications SET excluded = 1, excluded_reason = ?
                 WHERE student_id = ? AND track_id = ? AND round_id = ?",
            )
            .bind(opt_str(a, "excluded_reason").unwrap_or("감사 시나리오"))
            .bind(as_i64(a, "student_id"))
            .bind(as_i64(a, "track_id"))
            .bind(as_i64(a, "round_id"))
            .execute(pool)
            .await
            .map_err(|e| format!("excluded UPDATE 실패: {e}"))?;
        }
    }

    // ② CLOSED 에서 추천 확정 (results 는 FINALIZED 전에만 UPDATE 가능)
    for a in scn["applications"].as_array().unwrap() {
        if opt_i64(a, "recommended") == Some(1) {
            sqlx::query(
                "UPDATE results SET recommended = 1
                 WHERE student_id = ? AND track_id = ? AND round_id = ?",
            )
            .bind(as_i64(a, "student_id"))
            .bind(as_i64(a, "track_id"))
            .bind(as_i64(a, "round_id"))
            .execute(pool)
            .await
            .map_err(|e| format!("recommended UPDATE 실패: {e}"))?;
        }
    }

    if opt_str(scn, "round_status") != Some("FINALIZED") {
        return Ok(());
    }

    // ③ FINALIZED 전이 — trg_require_all_decided_before_finalize 가
    //    "모든 지원이 excluded=1 또는 recommended=1" 을 강제한다.
    sqlx::query("UPDATE rounds SET status = 'FINALIZED', finalized_at = ? WHERE id = 1")
        .bind("2026-08-17T03:00:00Z")
        .execute(pool)
        .await
        .map_err(|e| format!("FINALIZED 전이 실패: {e}"))?;

    // ④ FINALIZED 에서만 포기 처리 (abandoned 0→1 만 허용)
    for a in scn["applications"].as_array().unwrap() {
        if opt_i64(a, "abandoned") == Some(1) {
            sqlx::query(
                "UPDATE applications SET abandoned = 1
                 WHERE student_id = ? AND track_id = ? AND round_id = ?",
            )
            .bind(as_i64(a, "student_id"))
            .bind(as_i64(a, "track_id"))
            .bind(as_i64(a, "round_id"))
            .execute(pool)
            .await
            .map_err(|e| format!("abandoned UPDATE 실패: {e}"))?;
        }
    }
    Ok(())
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

        // 프로덕션과 같은 트랜잭션 경계로 감싼다 — `calculate_scores`(scoring.rs:420-460)와
        // `close_round` 는 둘 다 BEGIN IMMEDIATE 안에서 호출하고, Err 면 commit 하지 않아
        // sqlx 가 drop 시 ROLLBACK 한다. 감싸지 않으면 오류 시나리오에서 **오류 직전까지의
        // 지원자 행이 남아** 있어 "부분 저장 없음"이 깨진 것처럼 보인다(하네스 인공물).
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
        let calc = run_calculate_scores_on_conn(&mut tx, 1, "2026-08-17T02:00:00Z").await;
        match &calc {
            Ok(_) => tx.commit().await.unwrap(),
            Err(_) => tx.rollback().await.unwrap(),
        }

        let calc_err = match calc {
            Ok(_) => None,
            Err(e) => Some(e),
        };

        // 계산이 성공한 시나리오에만 생명주기 플래그를 적용한다
        // (오류 경로 시나리오는 results 행 자체가 없다).
        let lifecycle_err = if calc_err.is_none() {
            apply_lifecycle_flags(&pool, scn).await.err()
        } else {
            None
        };
        if let Some(e) = lifecycle_err {
            out.push(json!({ "name": scn["name"], "lifecycle_error": e }));
            pool.close().await;
            continue;
        }

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
                // 라운드 3 — 화면이 내보내는 상태 플래그 (A-2 대조용)
                "excluded": h.and_then(|h| h["excluded"].as_bool()),
                "abandoned": h.and_then(|h| h["abandoned"].as_bool()),
                "recommended": h.and_then(|h| h["recommended"].as_bool()),
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
