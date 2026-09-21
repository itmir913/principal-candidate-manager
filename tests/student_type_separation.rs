//! 규칙 7(기초데이터 import 의 student_type 분리)이 **무엇에 기대고 있는지** 고정한다.
//!
//! 전수 확인(2026-09-21): `base_data` 에 대한 일괄 DELETE 는 없다. 분리는 **학생을 찾는
//! 시점**에 걸린다 — 세 곳이다.
//!   ① `area_data.rs` 졸업생 경로: `WHERE student_code = ? AND is_enrolled = 0`
//!   ② `area_data.rs` 재학생 경로: `WHERE grade=? AND class_no=? AND seq_no=? AND is_enrolled = 1`
//!   ③ `external_import.rs` 재학생 경로: ②와 같은 형태
//!
//! 변이 검사로 세 필터를 각각 지워 보니 **①만 테스트가 잡았다**
//! (`base_data_import_graduated_rejects_enrolled_student_code`). ②·③ 은 전 스위트가
//! 초록이었다 — 그런데 이것은 방어 공백이 아니라 **등가 변이**다:
//! `students` 의 CHECK 가 `is_enrolled = 0 ⟹ grade/class_no/seq_no IS NULL` 을 강제하므로,
//! 위치(학년·반·번호)로 조회하면 졸업생은 **애초에 걸릴 수 없다**(SQL 에서 `NULL = ?` 는
//! 참이 될 수 없다). ②·③ 의 `is_enrolled = 1` 은 이중 방어일 뿐이다.
//!
//! **문제는 그 등가성이 스키마 CHECK 에 달려 있다는 것이다.** `schema_freeze.rs` 가 v1·v2
//! 지문을 동결하지만, 새 버전(v3…)에서 CHECK 를 완화하면 새 스냅샷이 정당하게 만들어지고
//! 세 쿼리의 전제는 **조용히** 무너진다. 그 순간을 여기서 잡는다.

mod common;
use common::create_test_pool;

/// 졸업생을 하나 만든다. CHECK 상 grade/class_no/seq_no 는 NULL 이어야 한다.
async fn insert_graduate(pool: &sqlx::SqlitePool, code: &str) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO students (student_code, name, is_enrolled, grad_year)
         VALUES (?, '김졸업', 0, 2024) RETURNING id",
    )
    .bind(code)
    .fetch_one(pool)
    .await
    .expect("졸업생 삽입 실패")
}

#[tokio::test]
async fn graduate_cannot_hold_a_class_position() {
    // 이것이 ②·③ 의 `is_enrolled = 1` 을 **이중 방어로 만드는** 전제다.
    // 이 단언이 깨지면(= CHECK 가 완화되면) 위치 조회가 졸업생을 집을 수 있게 되고,
    // 그때부터 두 필터는 **필수**가 된다 — 지우면 안 된다.
    let pool = create_test_pool().await;

    // FK(grade, class_no) → classes 를 **먼저** 만족시킨다.
    // 이걸 빼면 INSERT 가 CHECK 에 닿기 전에 FK 위반으로 거부돼, 이 테스트가
    // **엉뚱한 이유로** 통과한다. 2026-09-21 실제로 그러서, CHECK 에서 위치 조항을
    // 지우는 변이를 이 테스트가 **못 잡았다**.
    sqlx::query("INSERT INTO classes (grade, class_no, password_hash) VALUES (3, 1, 'x')")
        .execute(&pool)
        .await
        .expect("학급 삽입 실패");

    let err = sqlx::query(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled, grad_year)
         VALUES ('G999', '김졸업', 3, 1, 5, 0, 2024)",
    )
    .execute(&pool)
    .await
    .expect_err(
        "졸업생에게 학년·반·번호가 붙었다. students CHECK 가 완화됐다면 \n         area_data.rs / external_import.rs 의 위치 조회에 붙은 `is_enrolled = 1` 은 \n         이제 **이중 방어가 아니라 유일한 방어**다 — 절대 지우지 마라.",
    );

    // **왜** 거부됐는지까지 본다. FK·UNIQUE 로 막힌 것이라면 이 테스트는 CHECK 를
    // 전혀 지키지 못하면서 초록이기만 한다.
    let msg = err.to_string();
    assert!(
        msg.contains("CHECK"),
        "INSERT 가 CHECK 가 아닌 다른 제약으로 거부됐다 — 이 테스트는 스키마 CHECK 를 \n         지키지 못하고 있다: {msg}"
    );
}

#[tokio::test]
async fn position_lookup_never_finds_a_graduate() {
    // 위 CHECK 를 행동으로 다시 확인한다. 세 쿼리가 실제로 쓰는 형태 그대로 물어본다.
    let pool = create_test_pool().await;
    insert_graduate(&pool, "G001").await;

    let found: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM students WHERE grade = ? AND class_no = ? AND seq_no = ?",
    )
    .bind(3_i64)
    .bind(1_i64)
    .bind(5_i64)
    .fetch_optional(&pool)
    .await
    .expect("조회 실패");

    assert!(
        found.is_none(),
        "위치 조회가 졸업생을 찾았다 — 재학생 기초데이터 업로드가 졸업생 행을 덮어쓸 수 있다"
    );
}

#[tokio::test]
async fn student_code_is_shared_across_types_so_the_graduate_filter_is_load_bearing() {
    // ① 의 `is_enrolled = 0` 은 **이중 방어가 아니다.** `student_code` 는 재학생·졸업생을
    // 가리지 않고 UNIQUE 한 하나의 공간이라, 필터가 없으면 졸업생 파일의 코드가
    // 재학생 행을 집어 그 학생의 기초데이터를 덮어쓴다.
    // 그래서 ①에는 전용 테스트가 있다(handler_area_data 쪽). 여기서는 **왜 필요한지**를
    // 남긴다 — 다음 사람이 "②·③ 처럼 이중 방어겠지" 하고 지우지 않도록.
    let pool = create_test_pool().await;

    sqlx::query(
        "INSERT INTO classes (grade, class_no, password_hash) VALUES (3, 1, 'x')",
    )
    .execute(&pool)
    .await
    .expect("학급 삽입 실패");
    sqlx::query(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled)
         VALUES ('S001', '김재학', 3, 1, 1, 1)",
    )
    .execute(&pool)
    .await
    .expect("재학생 삽입 실패");

    // 필터 없이 학생코드로만 찾으면 재학생이 잡힌다 — 이것이 ①이 막는 상황이다.
    let without_filter: Option<i64> =
        sqlx::query_scalar("SELECT id FROM students WHERE student_code = ?")
            .bind("S001")
            .fetch_optional(&pool)
            .await
            .expect("조회 실패");
    assert!(without_filter.is_some(), "픽스처가 잘못됐다");

    // 필터를 붙이면 졸업생이 아니므로 잡히지 않는다.
    let with_filter: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM students WHERE student_code = ? AND is_enrolled = 0",
    )
    .bind("S001")
    .fetch_optional(&pool)
    .await
    .expect("조회 실패");
    assert!(
        with_filter.is_none(),
        "`is_enrolled = 0` 필터가 재학생을 걸러 내지 못한다 — 졸업생 업로드가 \
         재학생 기초데이터를 덮어쓴다(규칙 7)"
    );
}
