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

-- CLOSED/FINALIZED 라운드의 results 행 삭제 차단 (박제·집계 보호, UPDATE 차단과 대칭)
-- OPEN 라운드는 담임 지원 취소 시 results 동반 삭제를 위해 허용
CREATE TRIGGER IF NOT EXISTS trg_prevent_delete_closed_result
BEFORE DELETE ON results
BEGIN
    SELECT RAISE(ABORT, 'Cannot delete result: round is CLOSED or FINALIZED')
    WHERE (SELECT status FROM rounds WHERE id = OLD.round_id) IN ('CLOSED', 'FINALIZED');
END;
