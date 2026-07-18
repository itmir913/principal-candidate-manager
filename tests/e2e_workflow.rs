//! E2E 생명주기 시나리오 — "정해진 절차를 따르면 결과에 이상이 없다"를 실행으로 검증.
//!
//! 실제 핸들러만으로 전체 절차를 관통한다 (직접 SQL은 조회·단언에만 사용):
//! 학반 import → 학생 import(재학·졸업) → 대학/모집단위 생성(정원 설정) →
//! 전형요소 생성 → 점수 기준 import → 기초데이터 import →
//! 라운드1: open → 담임 지원 제출(점수계산) → close(순위) → 추천 → 정원 409 →
//!          finalize → 담임 포기 →
//! 라운드2: open → 재지원 → close → 추천(포기로 반환된 정원) → finalize →
//! 결과 export xlsx 파싱 검증.

mod common;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension, Json,
};
use principal_candidate_manager::{
    enums::{CalcType, LookupScope, MatchMode},
    excel,
    handlers::{
        applications::{
            teacher_abandon_application, teacher_create_application, BaseDataEntry,
            CreateApplicationBody,
        },
        area_data::{base_data_import, numeric_table_import, StudentTypeQuery},
        areas::{create_area, CreateAreaBody},
        classes::import_classes,
        rounds::{close_round, finalize_round, open_round},
        scoring::{export_results, recommend_result},
        students::{import_enrolled, import_graduated},
        universities::{create_track, create_university, CreateTrackBody, CreateUnivBody},
    },
    score::Score,
    state::AppState,
};
use sqlx::SqlitePool;

fn st(pool: &SqlitePool) -> State<AppState> {
    State(common::make_state(pool.clone()))
}

async fn student_id_by_code(pool: &SqlitePool, code: &str) -> i64 {
    sqlx::query_scalar("SELECT id FROM students WHERE student_code = ?")
        .bind(code)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn student_id_by_position(pool: &SqlitePool, grade: i64, class_no: i64, seq_no: i64) -> i64 {
    sqlx::query_scalar(
        "SELECT id FROM students WHERE grade = ? AND class_no = ? AND seq_no = ? AND is_enrolled = 1",
    )
    .bind(grade)
    .bind(class_no)
    .bind(seq_no)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn result_row(pool: &SqlitePool, sid: i64, tid: i64, rid: i64) -> (i64, Option<i64>, i64) {
    sqlx::query_as(
        "SELECT total_score, ranking, recommended FROM results \
         WHERE student_id = ? AND track_id = ? AND round_id = ?",
    )
    .bind(sid)
    .bind(tid)
    .bind(rid)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// 담임 지원 제출 바디: 담임 입력 전형요소(면접)에 값 1개
fn app_body(sid: i64, tid: i64, rid: i64, interview_area: i64, score: &str) -> CreateApplicationBody {
    CreateApplicationBody {
        student_id: sid,
        track_id: tid,
        round_id: rid,
        department_name: "지원학과".into(),
        base_data_entries: vec![BaseDataEntry {
            area_id: interview_area,
            values: vec![score.to_string()],
        }],
        ..Default::default()
    }
}

#[tokio::test]
async fn full_two_round_lifecycle() {
    let pool = common::create_test_pool().await;

    // ── 1. 학반·학생 등록 (관리자 import) ─────────────────────────
    let (status, _) = import_classes(
        st(&pool),
        common::csv_multipart("학년,반,담임명,비밀번호\n3,1,담임A,pass1234\n").await,
    )
    .await
    .unwrap();
    assert_eq!(status, StatusCode::OK, "학반 import");

    let (status, _) = import_enrolled(
        st(&pool),
        common::csv_multipart("이름,학년,반,번호\n홍길동,3,1,1\n이순신,3,1,2\n").await,
    )
    .await
    .unwrap();
    assert_eq!(status, StatusCode::OK, "재학생 import");

    let (status, _) = import_graduated(
        st(&pool),
        common::csv_multipart("학생코드,이름,졸업연도\nG001,김졸업,2024\n").await,
    )
    .await
    .unwrap();
    assert_eq!(status, StatusCode::OK, "졸업생 import");

    let s1 = student_id_by_position(&pool, 3, 1, 1).await; // 홍길동
    let s2 = student_id_by_position(&pool, 3, 1, 2).await; // 이순신
    let g1 = student_id_by_code(&pool, "G001").await;

    // ── 2. 대학·모집단위 (정원: 대학 전체 2, 컴공 1, 기계 1) ──────
    let (_, Json(body)) = create_university(
        st(&pool),
        Json(CreateUnivBody {
            univ_name: "한국대".into(),
            total_quota: Some(Some(2)),
            prioritize_enrolled: false,
        }),
    )
    .await
    .unwrap();
    let univ_id = body["id"].as_i64().unwrap();

    let mut track_ids = Vec::new();
    for name in ["컴공", "기계"] {
        let (_, Json(body)) = create_track(
            st(&pool),
            Path(univ_id),
            Json(CreateTrackBody {
                track_name: name.into(),
                unit_quota: Some(Some(1)),
                prioritize_enrolled: false,
            }),
        )
        .await
        .unwrap();
        track_ids.push(body["id"].as_i64().unwrap());
    }
    let (t_comp, t_mech) = (track_ids[0], track_ids[1]);

    // ── 3. 전형요소: 내신(관리자 입력) + 면접(담임 입력) ──────────
    let (_, Json(body)) = create_area(
        st(&pool),
        Json(CreateAreaBody {
            name: "내신".into(),
            max_score: Score::from_raw(10_000_000), // 100점
            calc_type: CalcType::Numeric,
            teacher_editable: false,
            lookup_scope: LookupScope::Simple,
            match_mode: Some(MatchMode::Upper),
            category_agg: None,
            multi_value: false,
            unit: None,
        }),
    )
    .await
    .unwrap();
    let a_grade = body["id"].as_i64().unwrap();

    let (_, Json(body)) = create_area(
        st(&pool),
        Json(CreateAreaBody {
            name: "면접".into(),
            max_score: Score::from_raw(5_000_000), // 50점
            calc_type: CalcType::Manual,
            teacher_editable: true,
            lookup_scope: LookupScope::Simple,
            match_mode: None,
            category_agg: None,
            multi_value: false,
            unit: None,
        }),
    )
    .await
    .unwrap();
    let a_interview = body["id"].as_i64().unwrap();

    // ── 4. 점수 기준 + 기초데이터 (관리자 import) ─────────────────
    let (status, _) = numeric_table_import(
        st(&pool),
        Path(a_grade),
        common::csv_multipart("기준값,점수\n0,0\n3,80\n4,100\n").await,
    )
    .await
    .unwrap();
    assert_eq!(status, StatusCode::OK, "내신 기준표 import");

    let (status, _) = base_data_import(
        st(&pool),
        Path(a_grade),
        Query(StudentTypeQuery { student_type: "enrolled".into() }),
        common::csv_multipart("학년,반,번호,이름,값\n3,1,1,홍길동,4.2\n3,1,2,이순신,3.5\n").await,
    )
    .await
    .unwrap();
    assert_eq!(status, StatusCode::OK, "재학생 내신 기초데이터 import");

    let (status, _) = base_data_import(
        st(&pool),
        Path(a_grade),
        Query(StudentTypeQuery { student_type: "graduated".into() }),
        common::csv_multipart("학생코드,이름,값\nG001,김졸업,0.5\n").await,
    )
    .await
    .unwrap();
    assert_eq!(status, StatusCode::OK, "졸업생 내신 기초데이터 import");

    // ── 5. 라운드 1: open → 담임 제출 → close ─────────────────────
    let (_, Json(body)) = open_round(st(&pool)).await.unwrap();
    let r1 = body["id"].as_i64().unwrap();

    let teacher = common::teacher_claims(3, 1);
    let grad_teacher = common::teacher_claims(0, 0);

    // 홍길동·이순신 → 컴공 경쟁, 김졸업 → 기계
    let status = teacher_create_application(
        st(&pool), Extension(teacher.clone()), Json(app_body(s1, t_comp, r1, a_interview, "45")),
    )
    .await
    .unwrap();
    assert_eq!(status, StatusCode::CREATED);
    let status = teacher_create_application(
        st(&pool), Extension(teacher.clone()), Json(app_body(s2, t_comp, r1, a_interview, "40")),
    )
    .await
    .unwrap();
    assert_eq!(status, StatusCode::CREATED);
    let status = teacher_create_application(
        st(&pool), Extension(grad_teacher.clone()), Json(app_body(g1, t_mech, r1, a_interview, "30")),
    )
    .await
    .unwrap();
    assert_eq!(status, StatusCode::CREATED);

    let Json(body) = close_round(st(&pool), Path(r1)).await.unwrap();
    assert_eq!(body["calculated"], 3, "지원자 3명 전원 점수 계산");

    // 점수·순위 검증: 홍길동 내신 4.2→100 + 면접 45 = 145 (1위),
    //                이순신 내신 3.5→80 + 면접 40 = 120 (2위), 김졸업 0 + 30 = 30
    let (total, rank, _) = result_row(&pool, s1, t_comp, r1).await;
    assert_eq!((total, rank), (14_500_000, Some(1)), "홍길동 145점 1위");
    let (total, rank, _) = result_row(&pool, s2, t_comp, r1).await;
    assert_eq!((total, rank), (12_000_000, Some(2)), "이순신 120점 2위");
    let (total, rank, _) = result_row(&pool, g1, t_mech, r1).await;
    assert_eq!((total, rank), (3_000_000, Some(3)), "김졸업 30점 기계 — 대학 전체 3위");

    // ── 6. 추천: 1위 확정, 정원(컴공 1명) 초과는 409 ──────────────
    let status = recommend_result(st(&pool), Path((s1, t_comp, r1))).await.unwrap();
    assert_eq!(status, StatusCode::NO_CONTENT);

    let res = recommend_result(st(&pool), Path((s2, t_comp, r1))).await;
    assert_eq!(res.unwrap_err().0, StatusCode::CONFLICT, "컴공 정원 1명 초과 → 409");

    // ── 7. finalize → 담임 포기 처리 ──────────────────────────────
    let status = finalize_round(st(&pool), Path(r1)).await.unwrap();
    assert_eq!(status, StatusCode::NO_CONTENT);

    let status = teacher_abandon_application(
        st(&pool), Extension(teacher.clone()), Path((s1, t_comp, r1)),
    )
    .await
    .unwrap();
    assert_eq!(status, StatusCode::NO_CONTENT, "홍길동 포기");

    // 포기는 applications만 변경 — FINALIZED results는 박제 유지
    let (_, _, recommended) = result_row(&pool, s1, t_comp, r1).await;
    assert_eq!(recommended, 1, "포기해도 results.recommended 박제 유지");

    // ── 8. 라운드 2: 이순신 재지원 → 포기로 반환된 정원에 추천 ────
    let (status, Json(body)) = open_round(st(&pool)).await.unwrap();
    assert_eq!(status, StatusCode::CREATED, "FINALIZED 후 새 라운드 open 가능");
    let r2 = body["id"].as_i64().unwrap();

    let status = teacher_create_application(
        st(&pool), Extension(teacher.clone()), Json(app_body(s2, t_comp, r2, a_interview, "40")),
    )
    .await
    .unwrap();
    assert_eq!(status, StatusCode::CREATED);

    let Json(body) = close_round(st(&pool), Path(r2)).await.unwrap();
    assert_eq!(body["calculated"], 1);

    // 라운드1 추천자(홍길동)가 포기했으므로 컴공 정원 1석이 반환되어 추천 가능해야 함
    let status = recommend_result(st(&pool), Path((s2, t_comp, r2))).await.unwrap();
    assert_eq!(status, StatusCode::NO_CONTENT, "포기로 반환된 정원에 라운드2 추천 성공");

    let status = finalize_round(st(&pool), Path(r2)).await.unwrap();
    assert_eq!(status, StatusCode::NO_CONTENT);

    // ── 9. 결과 export xlsx 검증 (라운드 1) ───────────────────────
    let resp = export_results(st(&pool), Path(r1)).await.unwrap();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    assert!(excel::is_xlsx(&bytes), "결과 export는 xlsx여야 함");

    let rows = excel::parse_xlsx_all_rows_raw(&bytes).unwrap();
    assert_eq!(rows.len(), 4, "헤더 + 지원자 3행");
    let header = &rows[0];
    for h in ["대학 순위", "모집단위 순위", "대학", "모집단위", "학생명", "내신", "면접", "총점", "추천", "포기"] {
        assert!(header.iter().any(|c| c == h), "export 헤더에 '{}' 누락: {:?}", h, header);
    }
    // 홍길동 행: 총점 145, 추천 + 포기 표기
    let hong = rows.iter().find(|r| r.iter().any(|c| c == "홍길동")).expect("홍길동 행");
    assert!(hong.iter().any(|c| c == "145"), "총점 145 표기: {:?}", hong);
    assert!(hong.iter().any(|c| c == "추천"), "추천 표기: {:?}", hong);
    assert!(hong.iter().any(|c| c == "포기"), "포기 표기: {:?}", hong);
}
