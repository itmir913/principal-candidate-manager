//! 감사 라운드 3 — 과제 1 (A-1): ×100000 진입점 두 개의 거부 규칙 비대칭.
//!
//! `Score` 를 JSON 으로 받는 필드는 전 코드베이스에 **하나뿐**이다:
//! `CreateAreaBody::max_score` (areas.rs:35) — `POST /api/areas`.
//! 검색식은 03-round2-findings.md §1.1 참조.
//!
//! 이 테스트는 **현재 동작을 고정한다**(E2). 실패하도록 만든 재현 테스트가 아니다.
//! 판정이 "명세 공백(S-07)"이므로 위반 재현이 아니라 비대칭의 사실을 못박는 것이 목적.

mod common;

use axum::body::Body;
use axum::extract::{FromRequest, State};
use axum::http::{Request, StatusCode};
use axum::Json;
use principal_candidate_manager::handlers::area_data::{fmt_score, parse_display_value};
use principal_candidate_manager::handlers::areas::{create_area, list_areas, CreateAreaBody};

/// 실제 `POST /api/areas` 와 같은 경로로 body 를 역직렬화한다
/// (axum `Json` 추출기 → serde_json → `Deserialize for Score`).
async fn extract_body(json: &str) -> Result<CreateAreaBody, StatusCode> {
    let req = Request::builder()
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(json.to_string()))
        .unwrap();
    Json::<CreateAreaBody>::from_request(req, &())
        .await
        .map(|Json(b)| b)
        .map_err(|e| e.status())
}

fn body_json(name: &str, max_score_literal: &str) -> String {
    format!(
        r#"{{"name":"{name}","max_score":{max_score_literal},"calc_type":"MANUAL",
            "teacher_editable":true,"lookup_scope":"SIMPLE",
            "match_mode":null,"category_agg":null}}"#
    )
}

/// E3 — `f → f*1e5 → round → raw → 응답 JSON` 전 구간 수기 추적.
/// 같은 리터럴을 Excel·담임 경로(`parse_display_value`)에 넣었을 때와 나란히 비교한다.
#[tokio::test]
async fn json_vs_excel_entrypoint_trace() {
    let cases = [
        "10",          // 정수
        "10.00001",    // 소수 5자리 — 양쪽 다 허용
        "10.000004",   // 6자리, 내림쪽
        "10.000006",   // 6자리, 올림쪽
        "10.000005",   // 6자리, 정확히 절반
        "0.000001",    // 6자리, raw 0 으로 붕괴
        "1e-6",        // 지수 표기 (JSON 숫자로는 합법)
        "1e5",         // 지수 표기 (큰 값)
    ];

    println!("\n{:<12} | {:>18} | {:>16} | {:>10} | {:>12} | {}", 
             "입력 리터럴", "f (f64)", "f*100000.0", "round→raw", "응답 JSON", "parse_display_value(같은 문자열)");
    println!("{}", "-".repeat(120));

    for (i, lit) in cases.iter().enumerate() {
        let pool = common::create_test_pool().await;
        let state = common::make_state(pool.clone());

        // ── Excel·담임 경로 (문자열) ──
        let excel = match parse_display_value(lit) {
            Ok(raw) => format!("Ok(raw={raw} → 표시 {})", fmt_score(raw)),
            Err(e) => format!("Err({e})"),
        };

        // ── JSON 경로 ──
        let name = format!("A{i}");
        let extracted = extract_body(&body_json(&name, lit)).await;
        let (f_str, mul_str, raw_str, resp_str) = match extracted {
            Err(status) => (
                format!("(추출 실패 {status})"), String::from("-"), String::from("-"), String::from("-"),
            ),
            Ok(body) => {
                let f: f64 = lit.parse().unwrap();
                let raw = body.max_score.raw();
                let created = create_area(State(state.clone()), Json(body)).await;
                let resp = match created {
                    Err((s, msg)) => format!("핸들러 거부 {s}: {msg}"),
                    Ok(_) => {
                        let rows = list_areas(State(state.clone())).await.unwrap().0;
                        let row = rows.iter().find(|r| r.name == name).unwrap();
                        serde_json::to_value(row).unwrap()["max_score"].to_string()
                    }
                };
                (format!("{f:?}"), format!("{:?}", f * 100_000.0), raw.to_string(), resp)
            }
        };

        println!("{lit:<12} | {f_str:>18} | {mul_str:>16} | {raw_str:>10} | {resp_str:>12} | {excel}");
    }
    println!();
}

/// E2 — 비대칭 자체를 고정한다. 같은 값이 경로에 따라 거부되거나 조용히 반올림된다.
#[tokio::test]
async fn six_decimal_rejected_by_excel_but_accepted_by_json() {
    for lit in ["10.000004", "10.000006", "0.000001"] {
        assert!(
            parse_display_value(lit).is_err(),
            "Excel·담임 경로는 소수 6자리를 거부해야 한다: {lit}"
        );
        let body = extract_body(&body_json("X", lit))
            .await
            .unwrap_or_else(|s| panic!("JSON 경로는 {lit} 을 받아들인다고 알려져 있다. 실제 상태: {s}"));
        // 조용히 반올림된 값이 들어온다 — 오류가 아니다
        let _ = body.max_score.raw();
    }
}

/// E2 — 두 경로가 **공유하는** 관문(유한성·±10억)은 실제로 양쪽에 다 있는가. (과제 1-4 부수 확인)
#[tokio::test]
async fn finiteness_and_billion_bound_exist_on_both_paths() {
    // 유한성: JSON 은 문법상 NaN/Infinity 리터럴을 못 쓰므로 f64 로 직접 검사한다.
    assert!(parse_display_value("nan").is_err(), "Excel: nan 거부");
    assert!(parse_display_value("inf").is_err(), "Excel: inf 거부");
    assert!(
        serde_json::from_str::<principal_candidate_manager::score::Score>("1e400").is_err(),
        "JSON: 오버플로로 inf 가 되는 리터럴은 Score 역직렬화가 거부해야 한다"
    );

    // ±10억 경계: 양쪽 동일 상수(1_000_000_000.0)
    assert!(parse_display_value("1000000001").is_err(), "Excel: 10억 초과 거부");
    assert!(parse_display_value("-1000000001").is_err(), "Excel: -10억 미만 거부");
    assert!(
        extract_body(&body_json("Y", "1000000001")).await.is_err(),
        "JSON: 10억 초과 거부"
    );
    assert!(
        extract_body(&body_json("Y", "-1000000001")).await.is_err(),
        "JSON: -10억 미만 거부"
    );
    // 경계값 자체(=10억)는 양쪽 다 통과
    assert!(parse_display_value("1000000000").is_ok(), "Excel: 정확히 10억은 통과");
    assert!(
        extract_body(&body_json("Z", "1000000000")).await.is_ok(),
        "JSON: 정확히 10억은 통과"
    );
}
