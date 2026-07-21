//! 모집단위(`univ_tracks`) 삭제 시 딸린 데이터가 어디까지 함께 지워지는지 실증한다.
//!
//! `AreasTab.vue`의 석차연명부 가져오기 안내가 "모집단위를 지우면 딸린 데이터도
//! 함께 지워진다"고 서술한다. 스키마의 `ON DELETE CASCADE`만 읽고 단정하면
//! `foreign_keys` 설정·트리거 때문에 실제 동작이 다를 수 있으므로, 화면 안내의
//! 근거를 테스트로 고정한다.

mod common;

use sqlx::SqlitePool;

async fn count(pool: &SqlitePool, sql: &str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

/// (area_id, track_id, student_id)를 만들고 세 테이블에 트랙 소속 행을 넣는다.
/// 비교군으로 track_id NULL(공통) 행도 함께 넣는다.
async fn fixture(pool: &SqlitePool) -> (i64, i64) {
    sqlx::query("INSERT INTO classes (grade, class_no, teacher_name, password_hash) VALUES (3, 1, '담임', 'x')")
        .execute(pool).await.unwrap();
    sqlx::query(
        "INSERT INTO students (student_code, name, grade, class_no, seq_no, is_enrolled)
         VALUES ('S1', '학생', 3, 1, 1, 1)",
    ).execute(pool).await.unwrap();
    sqlx::query(
        "INSERT INTO areas (name, max_score, calc_type, lookup_scope, match_mode)
         VALUES ('교과내신', 10000000, 'NUMERIC', 'COMPOSITE', 'UPPER')",
    ).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO areas (name, max_score, calc_type, lookup_scope, category_agg)
                 VALUES ('활동', 5000000, 'CATEGORY', 'COMPOSITE', 'SUM')")
        .execute(pool).await.unwrap();
    sqlx::query("INSERT INTO universities (univ_name) VALUES ('가대학')")
        .execute(pool).await.unwrap();
    sqlx::query("INSERT INTO univ_tracks (univ_id, track_name) VALUES (1, '자연계열')")
        .execute(pool).await.unwrap();

    let student_id: i64 = 1;
    let track_id: i64 = 1;

    // 트랙 소속 행
    sqlx::query("INSERT INTO numeric_table (area_id, track_id, threshold, score) VALUES (1, 1, 100000, 900000)")
        .execute(pool).await.unwrap();
    sqlx::query("INSERT INTO category_map (area_id, track_id, category, score) VALUES (2, 1, '회장', 300000)")
        .execute(pool).await.unwrap();
    sqlx::query("INSERT INTO base_data (student_id, area_id, track_id, value) VALUES (1, 1, 1, '250000')")
        .execute(pool).await.unwrap();

    // 비교군 — 공통(track_id NULL) 행은 살아남아야 한다
    sqlx::query("INSERT INTO numeric_table (area_id, track_id, threshold, score) VALUES (1, NULL, 200000, 800000)")
        .execute(pool).await.unwrap();
    sqlx::query("INSERT INTO category_map (area_id, track_id, category, score) VALUES (2, NULL, '부회장', 200000)")
        .execute(pool).await.unwrap();
    sqlx::query("INSERT INTO base_data (student_id, area_id, track_id, value) VALUES (1, 2, NULL, '회장')")
        .execute(pool).await.unwrap();

    (student_id, track_id)
}

/// 모집단위를 지우면 그 모집단위에 딸린 기초 데이터와 점수 기준이 함께 지워진다.
/// 공통(track_id NULL) 행은 남는다.
#[tokio::test]
async fn deleting_track_cascades_base_data_and_score_tables() {
    let pool = common::create_test_pool().await;
    fixture(&pool).await;

    assert_eq!(count(&pool, "SELECT COUNT(*) FROM numeric_table").await, 2);
    assert_eq!(count(&pool, "SELECT COUNT(*) FROM category_map").await, 2);
    assert_eq!(count(&pool, "SELECT COUNT(*) FROM base_data").await, 2);

    sqlx::query("DELETE FROM univ_tracks WHERE id = 1").execute(&pool).await.unwrap();

    assert_eq!(
        count(&pool, "SELECT COUNT(*) FROM numeric_table WHERE track_id IS NOT NULL").await, 0,
        "모집단위 소속 점수 기준(numeric_table)이 남았다"
    );
    assert_eq!(
        count(&pool, "SELECT COUNT(*) FROM category_map WHERE track_id IS NOT NULL").await, 0,
        "모집단위 소속 범주 기준(category_map)이 남았다"
    );
    assert_eq!(
        count(&pool, "SELECT COUNT(*) FROM base_data WHERE track_id IS NOT NULL").await, 0,
        "모집단위 소속 기초 데이터가 남았다"
    );

    // 공통 행은 모집단위와 무관하므로 살아 있어야 한다.
    assert_eq!(count(&pool, "SELECT COUNT(*) FROM numeric_table").await, 1);
    assert_eq!(count(&pool, "SELECT COUNT(*) FROM category_map").await, 1);
    assert_eq!(count(&pool, "SELECT COUNT(*) FROM base_data").await, 1);
}

/// 대학을 지우면 그 대학의 모집단위를 거쳐 2단 cascade로 함께 지워진다.
#[tokio::test]
async fn deleting_university_cascades_through_tracks() {
    let pool = common::create_test_pool().await;
    fixture(&pool).await;

    sqlx::query("DELETE FROM universities WHERE id = 1").execute(&pool).await.unwrap();

    assert_eq!(count(&pool, "SELECT COUNT(*) FROM univ_tracks").await, 0);
    assert_eq!(count(&pool, "SELECT COUNT(*) FROM numeric_table WHERE track_id IS NOT NULL").await, 0);
    assert_eq!(count(&pool, "SELECT COUNT(*) FROM category_map WHERE track_id IS NOT NULL").await, 0);
    assert_eq!(count(&pool, "SELECT COUNT(*) FROM base_data WHERE track_id IS NOT NULL").await, 0);
}
