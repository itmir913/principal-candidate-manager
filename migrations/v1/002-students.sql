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
-- 재학생 위치(학년·반·번호) 유일성 — 위치 기반 학생 조회·upsert(기초데이터 import 등)의 무결성 전제
CREATE UNIQUE INDEX IF NOT EXISTS idx_students_position
    ON students(grade, class_no, seq_no)
    WHERE is_enrolled = 1;
