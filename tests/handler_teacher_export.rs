//! 담임용 결과 CSV (이슈 #24).
//!
//! 이 파일은 담임이 **문자 일괄발송 사이트에 그대로 올리는** 파일이라, 모양이 어긋나면
//! 잘못된 학생에게 잘못된 문구가 나간다. 학생 한 명이 한 행이라는 것, 쉼표가 든 칸이
//! 제대로 따옴표로 감싸진다는 것, 한글이 Excel 에서 깨지지 않는다는 것을 고정한다.

mod common;

use axum::{
    extract::{Path, State},
    Extension,
};
use principal_candidate_manager::{
    handlers::teacher_export::{teacher_all_results_csv, teacher_round_results_csv},
    state::AppState,
};

fn app_state(pool: sqlx::SqlitePool) -> State<AppState> {
    State(common::make_state(pool))
}

async fn body_text(res: axum::response::Response) -> String {
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap().to_vec();
    String::from_utf8(bytes).unwrap()
}

fn parse(text: &str) -> Vec<csv::StringRecord> {
    let mut rdr = csv::Reader::from_reader(text.trim_start_matches('\u{feff}').as_bytes());
    rdr.records().map(|r| r.expect("CSV 파싱")).collect()
}

/// 재학생 2명(3학년 1반) + 대학 2곳 + 마감 라운드 1개.
/// 1번 학생은 두 곳 지원(한 곳 추천, 한 곳 미선발), 2번 학생은 한 곳 지원(미선발).
async fn setup(pool: &sqlx::SqlitePool) -> i64 {
    sqlx::query("INSERT INTO classes (grade, class_no, teacher_name, password_hash) VALUES (3, 1, '김담임', 'x')")
        .execute(pool)
        .await
        .unwrap();

    for (id, univ) in [(1i64, "한국대학교"), (2, "민족대학교")] {
        sqlx::query("INSERT INTO universities (id, univ_name, total_quota) VALUES (?, ?, 5)")
            .bind(id)
            .bind(univ)
            .execute(pool)
            .await
            .unwrap();
        // 모집단위 이름에 쉼표를 넣어 CSV 인용을 강제로 검사한다
        let track = if id == 1 { "컴퓨터공학, 전기공학" } else { "국어교육" };
        sqlx::query("INSERT INTO univ_tracks (id, univ_id, track_name, unit_quota) VALUES (?, ?, ?, 3)")
            .bind(id)
            .bind(id)
            .bind(track)
            .execute(pool)
            .await
            .unwrap();
    }

    let rid: i64 = sqlx::query_scalar(
        "INSERT INTO rounds (status, opened_at, closed_at, finalized_at) \
         VALUES ('FINALIZED', '2026-01-01T00:00:00Z', '2026-01-02T00:00:00Z', '2026-01-03T00:00:00Z') \
         RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();

    for (seq, code, name) in [(1i64, "S001", "홍길동"), (2, "S002", "김철수")] {
        let sid: i64 = sqlx::query_scalar(
            "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
             VALUES (?, ?, 3, 1, ?, 1) RETURNING id",
        )
        .bind(code)
        .bind(name)
        .bind(seq)
        .fetch_one(pool)
        .await
        .unwrap();

        let tracks: &[i64] = if seq == 1 { &[1, 2] } else { &[2] };
        for &tid in tracks {
            sqlx::query("INSERT INTO applications (student_id, track_id, round_id) VALUES (?, ?, ?)")
                .bind(sid)
                .bind(tid)
                .bind(rid)
                .execute(pool)
                .await
                .unwrap();
            let recommended = i64::from(seq == 1 && tid == 1);
            sqlx::query(
                "INSERT INTO results (student_id, track_id, round_id, score_detail, total_score, ranking, recommended, calculated_at) \
                 VALUES (?, ?, ?, '{}', 9000000, 1, ?, '2026-01-02T01:00:00Z')",
            )
            .bind(sid)
            .bind(tid)
            .bind(rid)
            .bind(recommended)
            .execute(pool)
            .await
            .unwrap();
        }
    }
    rid
}

async fn round_csv(pool: &sqlx::SqlitePool, grade: i64, class_no: i64, rid: i64) -> String {
    let res = teacher_round_results_csv(
        app_state(pool.clone()),
        Extension(common::teacher_claims(grade, class_no)),
        Path(rid),
    )
    .await
    .unwrap();
    body_text(res).await
}

// ── 행 구성 ───────────────────────────────────────────────────────

/// 핵심 계약: 학생이 여러 곳에 지원해도 **한 행**이다. 발송 사이트가 한 행을 한 수신자로
/// 읽기 때문에, 여기서 두 행이 되면 같은 학생에게 문자가 두 번 간다.
#[tokio::test]
async fn one_row_per_student_even_with_multiple_applications() {
    let pool = common::create_test_pool().await;
    let rid = setup(&pool).await;
    let text = round_csv(&pool, 3, 1, rid).await;

    let rows = parse(&text);
    assert_eq!(rows.len(), 2, "학생 2명 = 2행이어야 한다:\n{text}");
    assert_eq!(rows.iter().filter(|r| &r[4] == "홍길동").count(), 1, "홍길동은 한 행");
}

/// 선발된 곳과 미선발된 곳을 **모두** 담는다. 미선발 통보도 함께 보내기 때문이다.
#[tokio::test]
async fn summary_lists_both_recommended_and_not() {
    let pool = common::create_test_pool().await;
    let rid = setup(&pool).await;
    let text = round_csv(&pool, 3, 1, rid).await;

    let rows = parse(&text);
    let hong = rows.iter().find(|r| &r[4] == "홍길동").expect("홍길동 행");
    let summary = &hong[5];
    assert!(summary.contains("한국대학교"), "추천된 대학: {summary}");
    assert!(summary.contains("추천 확정"), "추천 상태 표기: {summary}");
    assert!(summary.contains("민족대학교"), "미선발 대학도 담는다: {summary}");
    assert!(summary.contains("미선발"), "미선발 상태 표기: {summary}");
}

/// 모집단위 이름에 쉼표가 들어가도 칸이 갈라지면 안 된다 — 갈라지면 이후 열이 통째로
/// 밀려 이름 칸에 대학명이 들어가는 식으로 어긋난다.
#[tokio::test]
async fn commas_inside_a_field_do_not_split_columns() {
    let pool = common::create_test_pool().await;
    let rid = setup(&pool).await;
    let text = round_csv(&pool, 3, 1, rid).await;

    for rec in parse(&text) {
        assert_eq!(rec.len(), 6, "모든 행이 6열이어야 한다: {rec:?}");
    }
    let rows = parse(&text);
    let hong = rows.iter().find(|r| &r[4] == "홍길동").unwrap();
    assert!(
        hong[5].contains("컴퓨터공학, 전기공학"),
        "쉼표가 든 모집단위명이 온전해야 한다: {}",
        &hong[5]
    );
}

// ── 열 구성 ───────────────────────────────────────────────────────

/// 문자로 나갈 파일에 점수·순위를 담지 않는다 — 치환 문구에 섞이면 그대로 전송된다.
#[tokio::test]
async fn columns_are_limited_to_delivery_needs() {
    let pool = common::create_test_pool().await;
    let rid = setup(&pool).await;
    let text = round_csv(&pool, 3, 1, rid).await;

    let header = text.lines().next().unwrap().trim_start_matches('\u{feff}');
    assert_eq!(header, "학번,학년,반,번호,이름,선발결과");
    assert!(!text.contains("총점"), "총점 열이 있으면 안 된다");
    assert!(!text.contains("순위"), "순위 열이 있으면 안 된다");
}

/// Excel 이 한글을 깨뜨리지 않으려면 BOM 이 필요하다. 담임은 발송 사이트에 올리기 전에
/// Excel 로 열어 확인한다.
#[tokio::test]
async fn csv_starts_with_utf8_bom() {
    let pool = common::create_test_pool().await;
    let rid = setup(&pool).await;

    let res = teacher_round_results_csv(
        app_state(pool.clone()),
        Extension(common::teacher_claims(3, 1)),
        Path(rid),
    )
    .await
    .unwrap();
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap().to_vec();

    assert_eq!(&bytes[..3], &[0xEF, 0xBB, 0xBF], "UTF-8 BOM 으로 시작해야 한다");
}

// ── 범위 ─────────────────────────────────────────────────────────

/// 담임은 자기 반만 받는다. 다른 반 학생이 섞이면 개인정보가 새는 것이다.
#[tokio::test]
async fn other_classes_are_not_included() {
    let pool = common::create_test_pool().await;
    let rid = setup(&pool).await;

    sqlx::query("INSERT INTO classes (grade, class_no, teacher_name, password_hash) VALUES (3, 2, '박담임', 'x')")
        .execute(&pool)
        .await
        .unwrap();
    let other: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled) \
         VALUES ('S100', '옆반학생', 3, 2, 1, 1) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO applications (student_id, track_id, round_id) VALUES (?, 1, ?)")
        .bind(other)
        .bind(rid)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO results (student_id, track_id, round_id, score_detail, total_score, ranking, recommended, calculated_at) \
         VALUES (?, 1, ?, '{}', 9000000, 2, 0, '2026-01-02T01:00:00Z')",
    )
    .bind(other)
    .bind(rid)
    .execute(&pool)
    .await
    .unwrap();

    let text = round_csv(&pool, 3, 1, rid).await;
    assert!(!text.contains("옆반학생"), "다른 반 학생이 섞였다:\n{text}");
}

/// 마감되지 않은 라운드는 결과를 내보내지 않는다 — 화면과 같은 기준이다.
#[tokio::test]
async fn unfinalized_round_exports_header_only() {
    let pool = common::create_test_pool().await;
    let rid = setup(&pool).await;
    sqlx::query("UPDATE rounds SET status = 'CLOSED' WHERE id = ?")
        .bind(rid)
        .execute(&pool)
        .await
        .unwrap();

    let text = round_csv(&pool, 3, 1, rid).await;
    assert!(parse(&text).is_empty(), "헤더만 나와야 한다:\n{text}");
}

// ── 전체 내보내기 ─────────────────────────────────────────────────

/// 전 라운드를 합칠 때는 어느 라운드 결과인지 칸 안에 남아야 한다.
#[tokio::test]
async fn all_rounds_export_labels_each_round() {
    let pool = common::create_test_pool().await;
    let _ = setup(&pool).await;

    let res = teacher_all_results_csv(
        app_state(pool.clone()),
        Extension(common::teacher_claims(3, 1)),
    )
    .await
    .unwrap();
    let text = body_text(res).await;

    let rows = parse(&text);
    let hong = rows.iter().find(|r| &r[4] == "홍길동").expect("홍길동 행");
    assert!(hong[5].contains("라운드"), "라운드 표기가 있어야 한다: {}", &hong[5]);
    assert_eq!(rows.iter().filter(|r| &r[4] == "홍길동").count(), 1, "전체에서도 한 행");
}

/// 졸업생 담당(0/0)은 졸업생만 받고, 학년·반·번호 칸은 비어 있다.
#[tokio::test]
async fn graduate_rows_have_empty_class_columns() {
    let pool = common::create_test_pool().await;
    let rid = setup(&pool).await;

    let gid: i64 = sqlx::query_scalar(
        "INSERT INTO students (student_code, name, is_enrolled, grad_year) \
         VALUES ('G001', '졸업생김', 0, 2025) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO applications (student_id, track_id, round_id) VALUES (?, 1, ?)")
        .bind(gid)
        .bind(rid)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO results (student_id, track_id, round_id, score_detail, total_score, ranking, recommended, calculated_at) \
         VALUES (?, 1, ?, '{}', 9500000, 1, 1, '2026-01-02T01:00:00Z')",
    )
    .bind(gid)
    .bind(rid)
    .execute(&pool)
    .await
    .unwrap();

    let text = round_csv(&pool, 0, 0, rid).await;
    let rows = parse(&text);
    assert_eq!(rows.len(), 1, "졸업생 1명만:\n{text}");
    assert_eq!(&rows[0][0], "G001");
    assert_eq!(&rows[0][1], "", "졸업생은 학년이 비어 있다");
    assert_eq!(&rows[0][2], "", "반이 비어 있다");
    assert_eq!(&rows[0][3], "", "번호가 비어 있다");
    assert_eq!(&rows[0][4], "졸업생김");
}
