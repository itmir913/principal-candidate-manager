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
