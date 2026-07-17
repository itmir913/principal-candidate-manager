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
