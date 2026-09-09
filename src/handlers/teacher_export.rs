//! 담임용 라운드 결과 CSV 내보내기 (이슈 #24).
//!
//! 담임이 결과를 **문자로 일괄 발송**할 때 쓰는 파일이다. 발송 사이트는 대개 "한 행 = 한
//! 수신자"를 전제하므로, 학생이 여러 대학에 지원했어도 **학생 한 명이 한 행**이어야 한다.
//! 그래서 지원 건들은 한 칸에 쉼표로 이어 붙인다.
//!
//! 열은 발송에 필요한 것만 둔다 — 학생코드·학년·반·번호·이름·선발결과. 총점과 순위는 담지
//! 않는다. 문자로 나갈 파일에 굳이 넣을 정보가 아니고, 실수로 치환 문구에 섞이면 그대로
//! 학생에게 전송된다.

use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::Response,
    Extension,
};

use crate::{
    auth::TeacherClaims,
    excel,
    handlers::scoring::{fetch_teacher_results, ResultRow},
    state::AppState,
};

type ApiError = (StatusCode, String);

/// 지원 한 건의 결과 문구. 화면(ResultsTab)의 상태 표기와 같은 말을 쓴다 —
/// 담임이 화면에서 "추천 확정"으로 본 것이 파일에서 다른 낱말이면 대조할 수 없다.
fn status_label(r: &ResultRow) -> &'static str {
    if r.abandoned {
        "포기됨"
    } else if r.recommended {
        "추천 확정"
    } else {
        "미선발"
    }
}

/// 한 지원 건을 "대학 모집단위(상태)" 로 적는다. 라운드 번호는 전체 내보내기에서만 붙인다 —
/// 라운드별 파일에는 이미 파일명과 헤더에 라운드가 있어 매 칸마다 반복할 이유가 없다.
fn entry_text(r: &ResultRow, with_round: bool) -> String {
    if with_round {
        format!("{}라운드 {} {}({})", r.round_id, r.univ_name, r.track_name, status_label(r))
    } else {
        format!("{} {}({})", r.univ_name, r.track_name, status_label(r))
    }
}

/// 결과 행들을 학생 단위로 접어 CSV 로 만든다.
///
/// 입력은 `fetch_teacher_results` 의 정렬 순서(라운드 → 번호/학생코드 → 모집단위)를 그대로
/// 따른다고 전제한다. 학생의 등장 순서를 그 순서로 보존하려고 별도 정렬을 하지 않는다.
fn build_csv(rows: &[ResultRow], with_round: bool) -> Result<Vec<u8>, ApiError> {
    // 학생별로 묶는다. 입력은 라운드 → 번호 → 모집단위 순이라, 그대로 접으면 전 라운드
    // 내보내기에서 "1라운드에 지원한 학생들 다음에 2라운드에만 지원한 학생" 순서가 된다
    // (3번, 7번, 5번). 담임은 이 파일을 Excel 로 열어 출석부와 대조하므로 번호순이어야 한다.
    let mut order: Vec<i64> = Vec::new();
    let mut grouped: std::collections::HashMap<i64, Vec<&ResultRow>> = std::collections::HashMap::new();
    for r in rows {
        if !grouped.contains_key(&r.student_id) {
            order.push(r.student_id);
        }
        grouped.entry(r.student_id).or_default().push(r);
    }

    // 재학생은 번호, 졸업생은 학생코드 순 — 화면(ResultsTab 의 studentsByRound)과 같은 기준이다.
    // 졸업생은 seq_no 가 모두 None 이라 학생코드 비교로 넘어간다(Option 은 None < Some).
    order.sort_by(|a, b| {
        let key = |sid: &i64| {
            grouped
                .get(sid)
                .and_then(|e| e.first())
                .map(|r| (r.seq_no, r.student_code.clone()))
        };
        key(a).cmp(&key(b))
    });

    let mut wtr = csv::Writer::from_writer(Vec::new());
    // "학생코드"는 저장소 표준 용어다 — 스키마 컬럼명(student_code)이자 학생 명단·기초데이터
    // 엑셀 헤더가 모두 이 낱말을 쓴다(students.rs, area_data.rs, scoring.rs). 담임이 다른
    // 파일과 대조할 때 같은 열이 다른 이름으로 보이면 안 된다.
    wtr.write_record(["학생코드", "학년", "반", "번호", "이름", "선발결과"])
        .map_err(csv_err)?;

    for sid in order {
        // order 는 grouped 의 키에서 만들었으므로 항상 존재한다
        let entries = grouped.get(&sid).ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            "CSV 생성 실패: 학생 묶음을 찾을 수 없습니다".to_string(),
        ))?;
        let first = entries[0];

        // 졸업생은 학년·반·번호가 없다(스키마 CHECK) — 빈 칸으로 둔다
        let cell = |v: Option<i64>| v.map(|n| n.to_string()).unwrap_or_default();
        let summary = entries
            .iter()
            .map(|r| entry_text(r, with_round))
            .collect::<Vec<_>>()
            .join(", ");

        wtr.write_record([
            first.student_code.as_str(),
            &cell(first.grade),
            &cell(first.class_no),
            &cell(first.seq_no),
            first.name.as_str(),
            &summary,
        ])
        .map_err(csv_err)?;
    }

    let body = wtr.into_inner().map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, format!("CSV 생성 실패: {e}"))
    })?;

    // UTF-8 BOM — 없으면 Excel 이 한글을 깨진 글자로 연다. 문자 발송 사이트에 올리기 전에
    // 담임이 Excel 로 열어 확인하는 것이 보통이라 BOM 이 실질적으로 필요하다.
    let mut out = Vec::with_capacity(body.len() + 3);
    out.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
    out.extend_from_slice(&body);
    Ok(out)
}

fn csv_err(e: csv::Error) -> ApiError {
    (StatusCode::INTERNAL_SERVER_ERROR, format!("CSV 생성 실패: {e}"))
}

fn csv_response(body: Vec<u8>, filename: &str) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/csv; charset=utf-8")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename),
        )
        .body(axum::body::Body::from(body))
        // 헤더 값이 모두 정적이거나 ASCII 파일명이라 build 가 실패할 수 없다
        .expect("CSV 응답 생성")
}

/// 라운드 하나의 결과 CSV. 마감(FINALIZED)된 라운드만 행이 나온다 —
/// `fetch_teacher_results` 가 FINALIZED 로 한정하므로 진행 중 라운드는 헤더만 나간다.
pub async fn teacher_round_results_csv(
    State(state): State<AppState>,
    Extension(claims): Extension<TeacherClaims>,
    Path(round_id): Path<i64>,
) -> Result<Response, ApiError> {
    let rows = fetch_teacher_results(&state.db, &claims, Some(round_id))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let body = build_csv(&rows, false)?;
    Ok(csv_response(
        body,
        &format!("round{}_results_{}.csv", round_id, excel::now_tag()),
    ))
}

/// 마감된 전 라운드를 합친 CSV. 학생 한 명이 한 행이므로, 여러 라운드에 지원했다면
/// 한 칸에 "N라운드 …" 가 이어 붙는다.
pub async fn teacher_all_results_csv(
    State(state): State<AppState>,
    Extension(claims): Extension<TeacherClaims>,
) -> Result<Response, ApiError> {
    let rows = fetch_teacher_results(&state.db, &claims, None)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let body = build_csv(&rows, true)?;
    Ok(csv_response(
        body,
        &format!("all_results_{}.csv", excel::now_tag()),
    ))
}
