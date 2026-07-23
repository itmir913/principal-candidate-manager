//! 운영자가 절차를 어기거나 실수했을 때 **그 자리에서** 막히는가 (fail-fast 시나리오).
//!
//! 기존 스위트는 기능 단위로는 촘촘하지만, 실제 사용 순서를 따라가다 한 걸음
//! 어긋났을 때를 관통해 보는 테스트가 없었다. 여기서는 관리자·담임이 실제로 밟는
//! 순서를 그대로 재현하면서 흔한 무지·실수·권한 밖 시도를 끼워 넣고, 시스템이
//! **부분 저장 없이 즉시 거부하고 사용자가 고칠 위치를 알려 주는지**를 본다.
//!
//! 거부는 3종 세트로 단언한다:
//!   ① 상태 코드  ② DB 행 불변(All-or-Nothing)  ③ 오류 메시지의 행번호·원인
//! 상태 코드만 보면 "엉뚱한 이유로 거부"도 통과하고, 메시지만 보면 부분 저장을 놓친다.
//!
//! 특히 **행번호**는 기존 import 거부 테스트가 한 번도 단언하지 않던 지점이다.
//! (앞 세션 발견 T-4) 행번호가 항상 2행이거나 off-by-one이어도 전 스위트가 초록이었고,
//! 그 경우 사용자는 멀쩡한 줄을 고치며 헤맨다.

mod common;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use principal_candidate_manager::{
    enums::{CalcType, CategoryAgg, LookupScope, MatchMode},
    handlers::{
        applications::{teacher_create_application, CreateApplicationBody},
        area_data::{base_data_import, category_map_import, numeric_table_import, StudentTypeQuery},
        areas::{create_area, delete_area, update_area, CreateAreaBody, UpdateAreaBody},
        classes::import_classes,
        rounds::{close_round, open_round},
        students::{import_enrolled, import_graduated, import_students},
        universities::{create_track, create_university, CreateTrackBody, CreateUnivBody},
    },
    score::Score,
    state::AppState,
};
use sqlx::SqlitePool;

// ── 헬퍼 ─────────────────────────────────────────────────────────

fn st(pool: &SqlitePool) -> State<AppState> {
    State(common::make_state(pool.clone()))
}

fn enrolled_q() -> Query<StudentTypeQuery> {
    Query(StudentTypeQuery { student_type: "enrolled".into() })
}

async fn count(pool: &SqlitePool, table: &str) -> i64 {
    sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {}", table))
        .fetch_one(pool)
        .await
        .unwrap()
}

/// 오류 목록이 **정확히 지정한 행들만** 가리키는지 단언한다.
/// 행번호를 보지 않으면 "항상 2행"이나 off-by-one이 그대로 통과한다.
fn assert_error_rows(errors: &[String], expected_rows: &[usize]) {
    let mut got: Vec<String> = errors
        .iter()
        .map(|e| e.split('행').next().unwrap_or("").trim().to_string())
        .collect();
    got.sort();
    let mut want: Vec<String> = expected_rows.iter().map(|r| r.to_string()).collect();
    want.sort();
    assert_eq!(got, want, "오류가 가리키는 행번호가 다르다: {:?}", errors);
    for e in errors {
        let cause = e.splitn(2, ": ").nth(1).unwrap_or("");
        assert!(!cause.trim().is_empty(), "행번호만 있고 원인이 없다: {}", e);
    }
}

fn numeric_area(name: &str) -> CreateAreaBody {
    CreateAreaBody {
        name: name.into(),
        max_score: Score::from_raw(10_000_000),
        calc_type: CalcType::Numeric,
        teacher_editable: false,
        lookup_scope: LookupScope::Simple,
        match_mode: Some(MatchMode::Upper),
        category_agg: None,
        multi_value: false,
        unit: None,
    }
}

fn multi_category_area(name: &str) -> CreateAreaBody {
    CreateAreaBody {
        name: name.into(),
        max_score: Score::from_raw(1_000_000),
        calc_type: CalcType::Category,
        teacher_editable: false,
        lookup_scope: LookupScope::Simple,
        match_mode: None,
        category_agg: Some(CategoryAgg::Sum),
        multi_value: true,
        unit: None,
    }
}

async fn new_area(pool: &SqlitePool, body: CreateAreaBody) -> i64 {
    let (_, Json(v)) = create_area(st(pool), Json(body)).await.unwrap();
    v["id"].as_i64().unwrap()
}

async fn new_univ_track(pool: &SqlitePool) -> i64 {
    let (_, Json(u)) = create_university(
        st(pool),
        Json(CreateUnivBody {
            univ_name: "한국대".into(),
            total_quota: Some(Some(5)),
            prioritize_enrolled: false,
        }),
    )
    .await
    .unwrap();
    let (_, Json(t)) = create_track(
        st(pool),
        Path(u["id"].as_i64().unwrap()),
        Json(CreateTrackBody {
            track_name: "컴퓨터공학".into(),
            unit_quota: Some(Some(5)),
            prioritize_enrolled: false,
        }),
    )
    .await
    .unwrap();
    t["id"].as_i64().unwrap()
}

async fn student_by_position(pool: &SqlitePool, g: i64, c: i64, s: i64) -> i64 {
    sqlx::query_scalar(
        "SELECT id FROM students WHERE grade=? AND class_no=? AND seq_no=? AND is_enrolled=1",
    )
    .bind(g).bind(c).bind(s)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn app(sid: i64, tid: i64, rid: i64) -> CreateApplicationBody {
    CreateApplicationBody {
        student_id: sid,
        track_id: tid,
        round_id: rid,
        department_name: "컴퓨터공학과".into(),
        ..Default::default()
    }
}

// ── 시나리오 1: 준비 순서를 건너뛰면 그 단계에서 막힌다 ──────────

#[tokio::test]
async fn setup_steps_out_of_order_are_blocked_then_succeed_in_order() {
    let pool = common::create_test_pool_shared().await;
    let roster = "이름,학년,반,번호\n홍길동,3,1,1\n이순신,3,1,2\n";

    // (1) 학급을 만들기 전에 재학생 명단부터 올린다 — 가장 흔한 첫 실수
    let (status, Json(res)) =
        import_enrolled(st(&pool), common::csv_multipart(roster).await).await.unwrap();
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "학급 없이 통과하면 안 된다");
    assert_error_rows(&res.errors, &[2, 3]);
    assert!(
        res.errors.iter().all(|e| e.contains("학급 목록에 없습니다")),
        "원인이 학급 누락이어야 한다: {:?}",
        res.errors
    );
    assert_eq!(count(&pool, "students").await, 0, "한 행도 저장되면 안 된다");

    // (2) 학급을 만든다
    let (status, _) = import_classes(
        st(&pool),
        common::csv_multipart("학년,반,담임명,비밀번호\n3,1,담임A,pass1234\n").await,
    )
    .await
    .unwrap();
    assert_eq!(status, StatusCode::OK);

    // (3) 학생을 넣기 전에 기초데이터부터 올린다
    let a_grade = new_area(&pool, numeric_area("내신")).await;
    let base_csv = "학년,반,번호,이름,값\n3,1,1,홍길동,4.2\n3,1,2,이순신,3.5\n";
    let (status, Json(res)) = base_data_import(
        st(&pool), Path(a_grade), enrolled_q(),
        common::csv_multipart(base_csv).await,
    )
    .await
    .unwrap();
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "학생 없이 통과하면 안 된다");
    assert_error_rows(&res.errors, &[2, 3]);
    assert!(
        res.errors.iter().all(|e| e.contains("등록된 재학생을 찾을 수 없습니다")),
        "{:?}",
        res.errors
    );
    assert_eq!(count(&pool, "base_data").await, 0);

    // (4) 학생을 넣는다 — 이제 (1)이 통과한다
    let (status, Json(res)) =
        import_enrolled(st(&pool), common::csv_multipart(roster).await).await.unwrap();
    assert_eq!(status, StatusCode::OK, "{:?}", res.errors);
    assert_eq!(res.inserted, 2);

    // (5) 기초데이터도 이제 통과한다
    let (status, Json(res)) = base_data_import(
        st(&pool), Path(a_grade), enrolled_q(),
        common::csv_multipart(base_csv).await,
    )
    .await
    .unwrap();
    assert_eq!(status, StatusCode::OK, "{:?}", res.errors);
    assert_eq!(count(&pool, "base_data").await, 2);

    // (6) 점수 기준(구간표)을 올리지 않은 채 라운드를 열고 담임이 제출한다.
    //     기준표가 없으면 점수를 낼 수 없다 — 조용히 0점으로 저장되면 안 된다.
    let tid = new_univ_track(&pool).await;
    let (_, Json(r)) = open_round(st(&pool)).await.unwrap();
    let rid = r["id"].as_i64().unwrap();
    let s1 = student_by_position(&pool, 3, 1, 1).await;

    let submit = teacher_create_application(
        st(&pool),
        Extension(common::teacher_claims(3, 1)),
        Json(app(s1, tid, rid)),
    )
    .await;
    assert!(
        submit.is_err(),
        "점수 기준이 없는데 제출이 성공하면 0점이 조용히 확정된다"
    );
    assert_eq!(count(&pool, "applications").await, 0, "실패한 제출이 남으면 안 된다");

    // (7) 기준표를 올린 뒤에야 제출이 통과한다
    let (status, Json(res)) = numeric_table_import(
        st(&pool),
        Path(a_grade),
        common::csv_multipart("기준값,점수\n0,0\n3,80\n4,100\n").await,
    )
    .await
    .unwrap();
    assert_eq!(status, StatusCode::OK, "{:?}", res.errors);

    let status = teacher_create_application(
        st(&pool),
        Extension(common::teacher_claims(3, 1)),
        Json(app(s1, tid, rid)),
    )
    .await
    .unwrap();
    assert_eq!(status, StatusCode::CREATED, "올바른 순서로는 통과해야 한다");

    // 절차를 지켰을 때의 점수가 실제로 맞는지까지 확인 (내신 4.2 → 100점)
    let Json(body) = close_round(st(&pool), Path(rid)).await.unwrap();
    assert_eq!(body["calculated"], 1);
    let total: i64 = sqlx::query_scalar(
        "SELECT total_score FROM results WHERE student_id = ? AND round_id = ?",
    )
    .bind(s1).bind(rid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(total, 10_000_000, "내신 4.2 → 100점");
}

// ── 시나리오 2: 엑셀 오타는 정확한 행을 가리키며 전부 거부된다 ───

#[tokio::test]
async fn classes_import_errors_point_at_the_real_rows_and_save_nothing() {
    let pool = common::create_test_pool().await;
    // 3행: 담임명 누락, 5행: 비밀번호가 4자 미만 — 정상 행 사이에 끼워 넣는다
    let csv = "학년,반,담임명,비밀번호\n\
               1,1,담임A,pass1234\n\
               1,2,,pass1234\n\
               1,3,담임C,pass1234\n\
               1,4,담임D,ab\n";
    let (status, Json(res)) =
        import_classes(st(&pool), common::csv_multipart(csv).await).await.unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let errors: Vec<String> =
        res["errors"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
    assert_error_rows(&errors, &[3, 5]);
    assert_eq!(count(&pool, "classes").await, 0, "정상 행도 저장되면 안 된다");
}

#[tokio::test]
async fn students_import_errors_point_at_the_real_rows_and_save_nothing() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 1, 1).await;
    // 3행: 앞 행과 같은 자리(1학년 1반 1번), 5행: 졸업인데 졸업연도 없음
    let csv = "학생코드,이름,재학여부,학년,반,번호,졸업연도\n\
               S001,홍길동,재학,1,1,1,\n\
               S002,이순신,재학,1,1,1,\n\
               S003,김유신,재학,1,1,3,\n\
               S004,강감찬,졸업,,,,\n";
    let (status, Json(res)) =
        import_students(st(&pool), common::csv_multipart(csv).await).await.unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_error_rows(&res.errors, &[3, 5]);
    assert_eq!(count(&pool, "students").await, 0);
}

#[tokio::test]
async fn enrolled_import_errors_point_at_the_real_rows_and_save_nothing() {
    let pool = common::create_test_pool().await;
    common::insert_class(&pool, 3, 1).await;
    // 3행: 이름 누락, 5행: 없는 학급(9반)
    let csv = "이름,학년,반,번호\n\
               홍길동,3,1,1\n\
               ,3,1,2\n\
               김유신,3,1,3\n\
               이순신,3,9,4\n";
    let (status, Json(res)) =
        import_enrolled(st(&pool), common::csv_multipart(csv).await).await.unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_error_rows(&res.errors, &[3, 5]);
    assert!(
        res.errors.iter().any(|e| e.contains("9반")),
        "없는 학급은 학년·반을 알려 줘야 한다: {:?}",
        res.errors
    );
    assert_eq!(count(&pool, "students").await, 0);
}

#[tokio::test]
async fn graduated_import_errors_point_at_the_real_rows_and_save_nothing() {
    let pool = common::create_test_pool().await;
    // 3행: 이름 누락, 5행: 졸업연도가 숫자가 아님
    let csv = "학생코드,이름,졸업연도\n\
               G001,김졸업,2024\n\
               G002,,2024\n\
               G003,박졸업,2023\n\
               G004,최졸업,작년\n";
    let (status, Json(res)) =
        import_graduated(st(&pool), common::csv_multipart(csv).await).await.unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_error_rows(&res.errors, &[3, 5]);
    assert_eq!(count(&pool, "students").await, 0);
}

// ── 시나리오 3: 라운드를 마감하면 점수 기준이 얼어붙는다 ─────────

/// 학급·학생·대학·전형요소·기준표·기초데이터·지원까지 끝내고 라운드를 마감한 상태.
/// 반환: (내신 area_id, 복수값 CATEGORY area_id, student_id, round_id)
async fn closed_round_fixture(pool: &SqlitePool) -> (i64, i64, i64, i64) {
    let _ = import_classes(
        st(pool),
        common::csv_multipart("학년,반,담임명,비밀번호\n3,1,담임A,pass1234\n").await,
    )
    .await
    .unwrap();
    let _ = import_enrolled(st(pool), common::csv_multipart("이름,학년,반,번호\n홍길동,3,1,1\n").await)
        .await
        .unwrap();

    let a_grade = new_area(pool, numeric_area("내신")).await;
    let _ = numeric_table_import(
        st(pool),
        Path(a_grade),
        common::csv_multipart("기준값,점수\n0,0\n3,80\n4,100\n").await,
    )
    .await
    .unwrap();

    let a_vol = new_area(pool, multi_category_area("봉사목록")).await;
    let _ = category_map_import(
        st(pool),
        Path(a_vol),
        common::csv_multipart("범주,점수\n해당없음,0\n교내봉사,5\n교외봉사,3\n").await,
    )
    .await
    .unwrap();

    let (status, Json(res)) = base_data_import(
        st(pool),
        Path(a_grade),
        enrolled_q(),
        common::csv_multipart("학년,반,번호,이름,값\n3,1,1,홍길동,4.2\n").await,
    )
    .await
    .unwrap();
    assert_eq!(status, StatusCode::OK, "{:?}", res.errors);

    let (status, Json(res)) = base_data_import(
        st(pool),
        Path(a_vol),
        enrolled_q(),
        common::csv_multipart("학년,반,번호,이름,값\n3,1,1,홍길동,교내봉사\n").await,
    )
    .await
    .unwrap();
    assert_eq!(status, StatusCode::OK, "{:?}", res.errors);

    let tid = new_univ_track(pool).await;
    let (_, Json(r)) = open_round(st(pool)).await.unwrap();
    let rid = r["id"].as_i64().unwrap();
    let sid = student_by_position(pool, 3, 1, 1).await;
    teacher_create_application(
        st(pool),
        Extension(common::teacher_claims(3, 1)),
        Json(app(sid, tid, rid)),
    )
    .await
    .unwrap();
    let _ = close_round(st(pool), Path(rid)).await.unwrap();

    (a_grade, a_vol, sid, rid)
}

#[tokio::test]
async fn closed_round_freezes_score_criteria() {
    let pool = common::create_test_pool_shared().await;
    let (a_grade, a_vol, _, _) = closed_round_fixture(&pool).await;

    let before: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT threshold, score FROM numeric_table WHERE area_id = ? ORDER BY threshold",
    )
    .bind(a_grade)
    .fetch_all(&pool)
    .await
    .unwrap();

    // 기준표 재업로드 — 저장된 순위와 어긋나므로 409
    let err = numeric_table_import(
        st(&pool),
        Path(a_grade),
        common::csv_multipart("기준값,점수\n0,0\n3,90\n4,100\n").await,
    )
    .await
    .err()
    .expect("마감 후 기준표 수정은 거부되어야 한다");
    assert_eq!(err.0, StatusCode::CONFLICT, "{}", err.1);

    let after: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT threshold, score FROM numeric_table WHERE area_id = ? ORDER BY threshold",
    )
    .bind(a_grade)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(before, after, "거부됐는데 기준표가 바뀌었다 (DELETE만 실행됨)");

    // 범주표도 마찬가지
    let err = category_map_import(
        st(&pool),
        Path(a_vol),
        common::csv_multipart("범주,점수\n해당없음,0\n교내봉사,9\n").await,
    )
    .await
    .err()
    .expect("마감 후 범주표 수정은 거부되어야 한다");
    assert_eq!(err.0, StatusCode::CONFLICT);

    // 전형요소 이름 수정·삭제도 막힌다 (산출물의 감사 추적이 끊긴다)
    let err = update_area(
        st(&pool),
        Path(a_grade),
        Json(UpdateAreaBody { name: Some("내신(수정)".into()), teacher_editable: None, unit: None }),
    )
    .await
    .err()
    .expect("마감 후 전형요소 수정은 거부되어야 한다");
    assert_eq!(err.0, StatusCode::CONFLICT);

    let err = delete_area(st(&pool), Path(a_vol))
        .await
        .err()
        .expect("마감 후 전형요소 삭제는 거부되어야 한다");
    assert_eq!(err.0, StatusCode::CONFLICT);

    let name: String = sqlx::query_scalar("SELECT name FROM areas WHERE id = ?")
        .bind(a_grade)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(name, "내신", "거부됐는데 이름이 바뀌었다");
    assert_eq!(count(&pool, "areas").await, 2, "거부됐는데 전형요소가 사라졌다");
}

#[tokio::test]
async fn closed_round_applicant_multi_value_base_data_cannot_be_replaced() {
    // 복수값 전형요소의 재업로드는 (학생, 모집단위) 조합을 DELETE 후 INSERT 한다.
    // 마감 라운드 지원자의 기초데이터가 지워지면 저장된 점수의 근거가 사라지므로
    // DB 트리거가 막고, 핸들러는 이를 500이 아니라 사람이 읽을 422로 옮겨야 한다.
    let pool = common::create_test_pool_shared().await;
    let (_, a_vol, _, _) = closed_round_fixture(&pool).await;

    let before: Vec<String> =
        sqlx::query_scalar("SELECT value FROM base_data WHERE area_id = ? ORDER BY value")
            .bind(a_vol)
            .fetch_all(&pool)
            .await
            .unwrap();

    let res = base_data_import(
        st(&pool),
        Path(a_vol),
        enrolled_q(),
        common::csv_multipart("학년,반,번호,이름,값\n3,1,1,홍길동,교외봉사\n").await,
    )
    .await;

    match res {
        Ok((status, Json(r))) => {
            assert_eq!(
                status,
                StatusCode::UNPROCESSABLE_ENTITY,
                "마감 지원자의 복수값 기초데이터가 교체됐다: {:?}",
                r.errors
            );
            assert!(
                r.errors.iter().any(|e| e.contains("홍길동") || e.contains("20")),
                "어느 학생 때문인지 알려 줘야 한다: {:?}",
                r.errors
            );
        }
        Err((code, msg)) => panic!("500이 아니라 422로 번역되어야 한다: {} {}", code, msg),
    }

    let after: Vec<String> =
        sqlx::query_scalar("SELECT value FROM base_data WHERE area_id = ? ORDER BY value")
            .bind(a_vol)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(before, after, "거부됐는데 기초데이터가 바뀌었다");
}

#[tokio::test]
async fn closed_round_applicant_cannot_be_flipped_between_enrolled_and_graduated() {
    // 재학/졸업 구분은 재학생 우선 정렬의 키다. 마감 시점 순위는 그 구분으로
    // 계산돼 저장돼 있으므로, 명단 재업로드로 구분만 바뀌면 순위와 어긋난다.
    let pool = common::create_test_pool_shared().await;
    let (_, _, sid, _) = closed_round_fixture(&pool).await;
    let code: String = sqlx::query_scalar("SELECT student_code FROM students WHERE id = ?")
        .bind(sid)
        .fetch_one(&pool)
        .await
        .unwrap();

    let csv = format!("학생코드,이름,재학여부,학년,반,번호,졸업연도\n{},홍길동,졸업,,,,2025\n", code);
    let (status, Json(res)) =
        import_students(st(&pool), common::csv_multipart(&csv).await).await.unwrap();

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_error_rows(&res.errors, &[2]);
    assert!(
        res.errors[0].contains("마감된 라운드"),
        "왜 막혔는지 알려 줘야 한다: {:?}",
        res.errors
    );

    let still_enrolled: bool =
        sqlx::query_scalar("SELECT is_enrolled = 1 FROM students WHERE id = ?")
            .bind(sid)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(still_enrolled, "거부됐는데 구분이 바뀌었다");
}
