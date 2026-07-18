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
-- prioritize_enrolled: 0=동일기준, 1=재학생우선
-- 불변식: universities.prioritize_enrolled=1 이면 그 대학의 모든
--         univ_tracks.prioritize_enrolled 도 1 이어야 한다.
--         아래 트리거 3종이 DB 레벨에서 이 불변식을 강제한다.
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

-- 불변식: universities.prioritize_enrolled=1 ⇒ 그 대학 모든 트랙 prioritize_enrolled=1
-- 대학을 0→1로 전환 시 해당 대학 모든 트랙 cascade
CREATE TRIGGER IF NOT EXISTS trg_univ_prioritize_cascade
AFTER UPDATE OF prioritize_enrolled ON universities
WHEN NEW.prioritize_enrolled = 1 AND OLD.prioritize_enrolled = 0
BEGIN
    UPDATE univ_tracks SET prioritize_enrolled = 1 WHERE univ_id = NEW.id;
END;

-- 대학=1 상태에서 트랙 prioritize=0으로 INSERT 차단
CREATE TRIGGER IF NOT EXISTS trg_track_prioritize_insert_guard
BEFORE INSERT ON univ_tracks
WHEN NEW.prioritize_enrolled = 0
     AND (SELECT prioritize_enrolled FROM universities WHERE id = NEW.univ_id) = 1
BEGIN
    SELECT RAISE(ABORT, 'univ prioritize=1 requires track prioritize=1');
END;

-- 대학=1 상태에서 트랙 prioritize=0으로 UPDATE 차단
CREATE TRIGGER IF NOT EXISTS trg_track_prioritize_update_guard
BEFORE UPDATE OF prioritize_enrolled ON univ_tracks
WHEN NEW.prioritize_enrolled = 0
     AND (SELECT prioritize_enrolled FROM universities WHERE id = NEW.univ_id) = 1
BEGIN
    SELECT RAISE(ABORT, 'univ prioritize=1 requires track prioritize=1');
END;
