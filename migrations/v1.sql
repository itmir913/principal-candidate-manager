-- ================================================================
-- 학교장추천 선발 시스템 — Schema v7  (migration v0 → v1)
-- Float-Free Architecture (×10000 완전 정수화) + Abandon 박제 로직
-- ================================================================

PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

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
-- ================================================================
CREATE TABLE IF NOT EXISTS rounds (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    status      TEXT NOT NULL CHECK(status IN ('OPEN', 'CLOSED')),
    opened_at   TEXT NOT NULL,
    closed_at   TEXT
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_one_open_round
    ON rounds(status) WHERE status = 'OPEN';

-- ================================================================
-- AREAS
-- max_score: INTEGER (×10000)
-- lookup_scope:
--   SIMPLE    = univ_id와 무관한 전역 전형요소
--   COMPOSITE = 지원 대학별로 점수가 달라지는 전형요소
-- ================================================================
CREATE TABLE IF NOT EXISTS areas (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    name             TEXT    NOT NULL UNIQUE,
    max_score        INTEGER NOT NULL,
    calc_type        TEXT    NOT NULL CHECK(calc_type IN ('RANGE', 'CATEGORY', 'MANUAL')),
    teacher_editable INTEGER NOT NULL DEFAULT 1 CHECK(teacher_editable IN (0, 1)),
    lookup_scope     TEXT    NOT NULL DEFAULT 'SIMPLE'
                             CHECK(lookup_scope IN ('SIMPLE', 'COMPOSITE')),
    range_direction  TEXT    CHECK(range_direction IN ('UPPER', 'LOWER')),
    category_agg     TEXT    CHECK(category_agg IN ('SUM', 'MAX'))
);

-- ================================================================
-- RANGE_TABLE
-- threshold: INTEGER (×10000, 원본 측정값)
--   예) 내신 1.25등급 → 12500 / 봉사 30.5시간 → 305000
-- score: INTEGER (×10000)
-- univ_id: NULL → SIMPLE(전역), NOT NULL → COMPOSITE(대학별)
-- Out-of-bounds → 0점 (백엔드 의무 구현)
-- 구간 비교는 정수 대소비교만 사용 (Float-Free Zone)
-- ================================================================
CREATE TABLE IF NOT EXISTS range_table (
    area_id   INTEGER NOT NULL REFERENCES areas(id) ON DELETE CASCADE,
    univ_id   INTEGER REFERENCES universities(id),  -- NULL=SIMPLE, id=COMPOSITE
    threshold INTEGER NOT NULL,   -- ×10000
    score     INTEGER NOT NULL    -- ×10000
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_range_table
    ON range_table(area_id, COALESCE(univ_id, 0), threshold);

-- ================================================================
-- CATEGORY_MAP
-- score: INTEGER (×10000)
-- univ_id: NULL → SIMPLE(전역), NOT NULL → COMPOSITE(대학별)
-- ================================================================
CREATE TABLE IF NOT EXISTS category_map (
    area_id  INTEGER NOT NULL REFERENCES areas(id) ON DELETE CASCADE,
    univ_id  INTEGER REFERENCES universities(id),  -- NULL=SIMPLE, id=COMPOSITE
    category TEXT    NOT NULL,
    score    INTEGER NOT NULL     -- ×10000
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_category_map
    ON category_map(area_id, COALESCE(univ_id, 0), category);

-- ================================================================
-- UNIVERSITIES
-- prioritize_enrolled: 0=동일기준, 1=재학생우선
-- ================================================================
CREATE TABLE IF NOT EXISTS universities (
    id                   INTEGER PRIMARY KEY AUTOINCREMENT,
    univ_name            TEXT    NOT NULL,
    track_name           TEXT    NOT NULL,
    capacity             INTEGER NOT NULL,
    prioritize_enrolled  INTEGER NOT NULL DEFAULT 0
                                 CHECK(prioritize_enrolled IN (0, 1)),
    UNIQUE (univ_name, track_name)
);

-- ================================================================
-- BASE_DATA
-- value 포맷:
--   RANGE    : "305000"  (원본 측정값 ×10000 정수 문자열)
--   CATEGORY : "회장"    (범주값)
--   MANUAL   : "8500"    (점수 ×10000 정수 문자열)
-- 투명화 계층: 교사 입력 "30.5" → 백엔드 즉시 ×10000 → DB "305000"
--             DB "305000" → API 응답 시 /10000 → Vue 표시 "30.5"
-- ================================================================
CREATE TABLE IF NOT EXISTS base_data (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    student_id INTEGER NOT NULL REFERENCES students(id),
    area_id    INTEGER NOT NULL REFERENCES areas(id),
    univ_id    INTEGER REFERENCES universities(id),
    value      TEXT    NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_base_data
    ON base_data(student_id, area_id, COALESCE(univ_id, 0), value);
CREATE INDEX IF NOT EXISTS idx_base_data_student
    ON base_data(student_id);

-- ================================================================
-- APPLICATIONS
-- confirmed + abandoned: 독립 생명주기, 동시 1 허용
-- ================================================================
CREATE TABLE IF NOT EXISTS applications (
    student_id INTEGER NOT NULL REFERENCES students(id),
    univ_id    INTEGER NOT NULL REFERENCES universities(id),
    round_id   INTEGER NOT NULL REFERENCES rounds(id),
    confirmed  INTEGER NOT NULL DEFAULT 0 CHECK(confirmed IN (0, 1)),
    abandoned  INTEGER NOT NULL DEFAULT 0 CHECK(abandoned IN (0, 1)),
    PRIMARY KEY (student_id, univ_id, round_id)
);
CREATE INDEX IF NOT EXISTS idx_applications_round
    ON applications(round_id);

-- CLOSED 라운드 행 삭제 방지
CREATE TRIGGER IF NOT EXISTS trg_prevent_delete_closed_application
BEFORE DELETE ON applications
BEGIN
    SELECT RAISE(ABORT, 'Cannot delete application: round is CLOSED')
    WHERE (SELECT status FROM rounds WHERE id = OLD.round_id) = 'CLOSED';
END;

-- CLOSED 라운드 무단 수정 방지 (abandoned 0→1 만 허용)
CREATE TRIGGER IF NOT EXISTS trg_prevent_update_closed_application
BEFORE UPDATE ON applications
BEGIN
    SELECT RAISE(ABORT, 'Cannot update application: round is CLOSED. Only abandoned 0->1 is permitted.')
    WHERE (SELECT status FROM rounds WHERE id = OLD.round_id) = 'CLOSED'
      AND (
          OLD.student_id != NEW.student_id
          OR OLD.univ_id    != NEW.univ_id
          OR OLD.round_id   != NEW.round_id
          OR OLD.confirmed  != NEW.confirmed
          OR (OLD.abandoned = 1 AND NEW.abandoned = 0)
      );
END;

-- ================================================================
-- RESULTS
-- total_score: INTEGER (×10000)
-- score_detail: JSON {"area_id": score_int, ...} (×10000)
-- FK CASCADE 미적용: 불변 이력 보존
-- Abandon 박제: 포기(abandoned=1) 발생 시 이 테이블을 수정하지 않는다.
--   recommended=1 행은 영구 불변 스냅샷(Immutable Snapshot).
--   잔여석 = 정원 - COUNT(이전 라운드 recommended=1) 로 실시간 계산.
-- ================================================================
CREATE TABLE IF NOT EXISTS results (
    student_id     INTEGER NOT NULL,
    univ_id        INTEGER NOT NULL,
    round_id       INTEGER NOT NULL,
    score_detail   TEXT    NOT NULL DEFAULT '{}',
    total_score    INTEGER NOT NULL DEFAULT 0,
    ranking        INTEGER,
    recommended    INTEGER NOT NULL DEFAULT 0 CHECK(recommended IN (0, 1)),
    calculated_at  TEXT    NOT NULL,
    PRIMARY KEY (student_id, univ_id, round_id),
    FOREIGN KEY (student_id, univ_id, round_id)
        REFERENCES applications(student_id, univ_id, round_id)
);
CREATE INDEX IF NOT EXISTS idx_results_round_univ
    ON results(round_id, univ_id);
