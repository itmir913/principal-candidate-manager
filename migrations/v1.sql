-- ================================================================
-- 학교장추천 선발 시스템 — Schema v10
-- universities(대학 마스터) / univ_tracks(모집단위) 2단계 분리
-- Float-Free Architecture (×100000 완전 정수화) + Abandon 박제 로직
-- ================================================================

-- ================================================================
-- APP CONFIGS
-- ================================================================
CREATE TABLE IF NOT EXISTS app_configs (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- ================================================================
-- CLASSES
-- ================================================================
CREATE TABLE IF NOT EXISTS classes (
    grade         INTEGER NOT NULL,
    class_no      INTEGER NOT NULL,
    teacher_name  TEXT,
    password_hash TEXT    NOT NULL,
    PRIMARY KEY (grade, class_no)
);

-- ================================================================
-- STUDENTS
-- CHECK: 재학생(is_enrolled=1) ↔ 졸업생(is_enrolled=0) 컬럼 상호배제
-- ================================================================
CREATE TABLE IF NOT EXISTS students (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    student_code TEXT    NOT NULL UNIQUE,
    name         TEXT    NOT NULL,
    grade        INTEGER,
    class_no     INTEGER,
    seq_no       INTEGER,
    is_enrolled  INTEGER NOT NULL DEFAULT 1 CHECK(is_enrolled IN (0, 1)),
    grad_year    INTEGER,
    FOREIGN KEY (grade, class_no) REFERENCES classes(grade, class_no),
    CHECK (
        (is_enrolled = 1
            AND grade    IS NOT NULL
            AND class_no IS NOT NULL
            AND seq_no   IS NOT NULL
            AND grad_year IS NULL)
        OR
        (is_enrolled = 0
            AND grade    IS NULL
            AND class_no IS NULL
            AND seq_no   IS NULL
            AND grad_year IS NOT NULL)
    )
);

-- ================================================================
-- ROUNDS
-- status 생명주기: OPEN ⟷ CLOSED → FINALIZED
--   OPEN      : 담임 지원 입력 기간
--   CLOSED    : 담임 입력 차단, 관리자 점수 확인·추천 확정/취소 기간
--   FINALIZED : 추천 확정 박제, 담임 포기 입력 가능, 결과 공개
-- ================================================================
CREATE TABLE IF NOT EXISTS rounds (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    status       TEXT NOT NULL CHECK(status IN ('OPEN', 'CLOSED', 'FINALIZED')),
    opened_at    TEXT NOT NULL,
    closed_at    TEXT,
    finalized_at TEXT
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_one_open_round
    ON rounds(status) WHERE status = 'OPEN';

-- ================================================================
-- AREAS
-- max_score: INTEGER (×100000)
-- lookup_scope:
--   SIMPLE    = univ_tracks와 무관한 전역 전형요소
--   COMPOSITE = 지원 모집단위별로 점수가 달라지는 전형요소
-- calc_type:
--   NUMERIC  = 숫자 측정값을 점수 테이블에서 조회 (match_mode 필수)
--   CATEGORY = 텍스트 범주 선택 후 매핑 (category_agg 필수)
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
    CHECK(calc_type = 'CATEGORY' OR multi_value = 0),
    CHECK(calc_type = 'NUMERIC' OR match_mode IS NULL),
    CHECK(calc_type = 'CATEGORY' OR category_agg IS NULL)
);

-- ================================================================
-- UNIVERSITIES (대학 마스터)
-- total_quota: NULL = 전체 모집단위 합산 제한 없음
-- prioritize_enrolled: 0=동일기준, 1=재학생우선
-- ================================================================
CREATE TABLE IF NOT EXISTS universities (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    univ_name           TEXT    NOT NULL UNIQUE,
    total_quota         INTEGER,
    prioritize_enrolled INTEGER NOT NULL DEFAULT 0
                                CHECK(prioritize_enrolled IN (0, 1))
);

-- ================================================================
-- UNIV_TRACKS (모집단위)
-- unit_quota: NULL = 해당 모집단위 제한 없음
-- prioritize_enrolled: 0=동일기준, 1=재학생우선 (대학 설정이 우선 적용됨)
-- ================================================================
CREATE TABLE IF NOT EXISTS univ_tracks (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    univ_id             INTEGER NOT NULL REFERENCES universities(id) ON DELETE CASCADE,
    track_name          TEXT    NOT NULL,
    unit_quota          INTEGER,
    prioritize_enrolled INTEGER NOT NULL DEFAULT 0
                                CHECK(prioritize_enrolled IN (0, 1)),
    UNIQUE (univ_id, track_name)
);

-- ================================================================
-- NUMERIC_TABLE  (NUMERIC calc_type 전용)
-- threshold: INTEGER (×100000, 원본 측정값)
-- score: INTEGER (×100000)
-- track_id: NULL → SIMPLE(전역), NOT NULL → COMPOSITE(모집단위별)
-- ================================================================
CREATE TABLE IF NOT EXISTS numeric_table (
    area_id   INTEGER NOT NULL REFERENCES areas(id) ON DELETE CASCADE,
    track_id  INTEGER REFERENCES univ_tracks(id) ON DELETE CASCADE,
    threshold INTEGER NOT NULL,   -- ×100000
    score     INTEGER NOT NULL    -- ×100000
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_numeric_table
    ON numeric_table(area_id, COALESCE(track_id, 0), threshold);

-- ================================================================
-- CATEGORY_MAP
-- score: INTEGER (×100000)
-- track_id: NULL → SIMPLE(전역), NOT NULL → COMPOSITE(모집단위별)
-- ================================================================
CREATE TABLE IF NOT EXISTS category_map (
    area_id  INTEGER NOT NULL REFERENCES areas(id) ON DELETE CASCADE,
    track_id INTEGER REFERENCES univ_tracks(id) ON DELETE CASCADE,
    category TEXT    NOT NULL,
    score    INTEGER NOT NULL     -- ×100000
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_category_map
    ON category_map(area_id, COALESCE(track_id, 0), category);

-- ================================================================
-- BASE_DATA
-- value 포맷:
--   NUMERIC  : "3050000" (원본 측정값 ×100000 정수 문자열)
--   CATEGORY : "회장"    (범주값)
--   MANUAL   : "850000"  (점수 ×100000 정수 문자열)
-- track_id: NULL → SIMPLE, NOT NULL → COMPOSITE
-- ================================================================
CREATE TABLE IF NOT EXISTS base_data (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    student_id  INTEGER NOT NULL REFERENCES students(id) ON DELETE CASCADE,
    area_id     INTEGER NOT NULL REFERENCES areas(id) ON DELETE CASCADE,
    track_id    INTEGER REFERENCES univ_tracks(id) ON DELETE CASCADE,
    value       TEXT    NOT NULL,
    multi_value INTEGER NOT NULL DEFAULT 0 CHECK(multi_value IN (0, 1))
);
-- 단일값 전형요소: (학생, 전형요소, 모집단위) 당 정확히 1행
CREATE UNIQUE INDEX IF NOT EXISTS idx_base_data_single
    ON base_data(student_id, area_id, COALESCE(track_id, 0))
    WHERE multi_value = 0;
-- 복수값 전형요소: 동일 (학생, 전형요소, 모집단위, 값) 중복 방지
CREATE UNIQUE INDEX IF NOT EXISTS idx_base_data_multi
    ON base_data(student_id, area_id, COALESCE(track_id, 0), value)
    WHERE multi_value = 1;
CREATE INDEX IF NOT EXISTS idx_base_data_student
    ON base_data(student_id);

-- ================================================================
-- APPLICATIONS
-- confirmed + abandoned: 독립 생명주기, 동시 1 허용
-- confirmed: 원래는 "담임이 자기 반 학생 전원 입력을 완료했음을 확정"하는 용도로 설계됐으나,
--            현재 시스템은 제출 행위 자체가 곧 확정이므로 항상 1로 삽입됨.
--            임시저장→확정 제출 흐름이 추가될 경우 이 필드를 활성화할 것.
-- ================================================================
CREATE TABLE IF NOT EXISTS applications (
    student_id      INTEGER NOT NULL REFERENCES students(id),
    track_id        INTEGER NOT NULL REFERENCES univ_tracks(id),
    round_id        INTEGER NOT NULL REFERENCES rounds(id),
    confirmed       INTEGER NOT NULL DEFAULT 0 CHECK(confirmed IN (0, 1)),
    abandoned       INTEGER NOT NULL DEFAULT 0 CHECK(abandoned IN (0, 1)),
    department_name TEXT    NOT NULL DEFAULT '',
    PRIMARY KEY (student_id, track_id, round_id)
);
CREATE INDEX IF NOT EXISTS idx_applications_round
    ON applications(round_id);

-- CLOSED / FINALIZED 라운드 행 삭제 방지
CREATE TRIGGER IF NOT EXISTS trg_prevent_delete_closed_application
BEFORE DELETE ON applications
BEGIN
    SELECT RAISE(ABORT, 'Cannot delete application: round is CLOSED or FINALIZED')
    WHERE (SELECT status FROM rounds WHERE id = OLD.round_id) IN ('CLOSED', 'FINALIZED');
END;

-- CLOSED : 모든 업데이트 차단
-- FINALIZED : abandoned 0→1 만 허용
CREATE TRIGGER IF NOT EXISTS trg_prevent_update_closed_application
BEFORE UPDATE ON applications
BEGIN
    SELECT RAISE(ABORT, 'Cannot update application: round is CLOSED')
    WHERE (SELECT status FROM rounds WHERE id = OLD.round_id) = 'CLOSED';
    SELECT RAISE(ABORT, 'Cannot update application: round is FINALIZED. Only abandoned 0->1 is permitted.')
    WHERE (SELECT status FROM rounds WHERE id = OLD.round_id) = 'FINALIZED'
      AND (
          OLD.student_id      != NEW.student_id
          OR OLD.track_id         != NEW.track_id
          OR OLD.round_id         != NEW.round_id
          OR OLD.confirmed        != NEW.confirmed
          OR OLD.department_name  != NEW.department_name
          OR (OLD.abandoned = 1 AND NEW.abandoned = 0)
      );
END;

-- ================================================================
-- RESULTS
-- total_score: INTEGER (×100000)
-- score_detail: JSON {"area_id": score_int, ...} (×100000)
-- FK CASCADE 미적용: 불변 이력 보존
-- Abandon 박제: 포기(abandoned=1) 발생 시 이 테이블을 수정하지 않는다.
--   recommended=1 행은 영구 불변 스냅샷(Immutable Snapshot).
--   잔여석 = 정원 - COUNT(이전 라운드 recommended=1) 로 실시간 계산.
-- ================================================================
CREATE TABLE IF NOT EXISTS results (
    student_id     INTEGER NOT NULL,
    track_id       INTEGER NOT NULL,
    round_id       INTEGER NOT NULL,
    score_detail   TEXT    NOT NULL DEFAULT '{}',
    total_score    INTEGER NOT NULL DEFAULT 0,
    ranking        INTEGER,
    recommended    INTEGER NOT NULL DEFAULT 0 CHECK(recommended IN (0, 1)),
    calculated_at  TEXT    NOT NULL,
    PRIMARY KEY (student_id, track_id, round_id),
    FOREIGN KEY (student_id, track_id, round_id)
        REFERENCES applications(student_id, track_id, round_id)
);
CREATE INDEX IF NOT EXISTS idx_results_round_track
    ON results(round_id, track_id);

-- FINALIZED 라운드의 results 행 수정 전면 차단 (recommended 박제 보호)
CREATE TRIGGER IF NOT EXISTS trg_prevent_update_finalized_result
BEFORE UPDATE ON results
BEGIN
    SELECT RAISE(ABORT, 'Cannot update result: round is FINALIZED')
    WHERE (SELECT status FROM rounds WHERE id = OLD.round_id) = 'FINALIZED';
END;
