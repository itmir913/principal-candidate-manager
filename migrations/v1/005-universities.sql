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
