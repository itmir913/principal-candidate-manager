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
