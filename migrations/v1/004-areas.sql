-- ================================================================
-- AREAS
-- max_score: INTEGER (×100000)
-- lookup_scope:
--   SIMPLE    = univ_tracks와 무관한 전역 전형요소
--   COMPOSITE = 지원 모집단위별로 점수가 달라지는 전형요소
-- calc_type:
--   NUMERIC  = 숫자 측정값을 점수 테이블에서 조회 (match_mode 필수)
--   CATEGORY = 텍스트 범주 중 하나 선택 후 매핑 (category_agg 필수)
--   MANUAL   = 담임교사 직접 입력
-- match_mode (NUMERIC 전용):
--   UPPER = threshold가 하한선 (값이 클수록 유리, 봉사시간 등)
--   LOWER = threshold가 상한선 (값이 작을수록 유리, 결석일수 등)
--   EXACT = 정확한 일치
-- ================================================================
CREATE TABLE IF NOT EXISTS areas (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    name             TEXT    NOT NULL UNIQUE,
    max_score        INTEGER NOT NULL CHECK(max_score >= 0),  -- ×100000 정수 (순수 감점 전형요소는 0 허용)
    calc_type        TEXT    NOT NULL CHECK(calc_type IN ('NUMERIC', 'CATEGORY', 'MANUAL')),
    teacher_editable INTEGER NOT NULL DEFAULT 1 CHECK(teacher_editable IN (0, 1)),
    lookup_scope     TEXT    NOT NULL DEFAULT 'SIMPLE'
                             CHECK(lookup_scope IN ('SIMPLE', 'COMPOSITE')),
    match_mode       TEXT    CHECK(match_mode IN ('UPPER', 'LOWER', 'EXACT')),
    category_agg     TEXT    CHECK(category_agg IN ('SUM', 'MAX')),
    -- 0=단일값(NUMERIC·MANUAL·단일선택CATEGORY), 1=복수값(복수선택CATEGORY 전용)
    multi_value      INTEGER NOT NULL DEFAULT 0 CHECK(multi_value IN (0, 1)),
    unit             TEXT,    -- 기준값 표시 단위 (예: '시간', '등급'). NUMERIC·MANUAL 전용, 표시용
    CHECK(calc_type = 'CATEGORY' OR multi_value = 0),
    CHECK(calc_type = 'NUMERIC' OR match_mode IS NULL),
    CHECK(calc_type = 'CATEGORY' OR category_agg IS NULL),
    CHECK(calc_type != 'CATEGORY' OR unit IS NULL)
);
