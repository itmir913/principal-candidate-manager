-- ================================================================
-- APPLICATIONS
-- ================================================================
CREATE TABLE IF NOT EXISTS applications (
    student_id      INTEGER NOT NULL REFERENCES students(id),
    track_id        INTEGER NOT NULL REFERENCES univ_tracks(id),
    round_id        INTEGER NOT NULL REFERENCES rounds(id),
    abandoned       INTEGER NOT NULL DEFAULT 0 CHECK(abandoned IN (0, 1)),
    department_name TEXT    NOT NULL DEFAULT '',
    -- excluded: CLOSED 라운드에서 결격·서류미비 등으로 이번 라운드 추천 대상에서 제외.
    -- abandoned(포기, FINALIZED 전용)와 별개 — 정원 집계(recommended=1 AND abandoned=0)와
    -- 무관하므로 건드리면 이중 차감이 된다. 사유 없는 제외는 DB 레벨에서 차단(Fail-Fast).
    excluded         INTEGER NOT NULL DEFAULT 0 CHECK(excluded IN (0, 1)),
    excluded_reason  TEXT,
    PRIMARY KEY (student_id, track_id, round_id),
    CHECK (excluded = 0 OR (excluded_reason IS NOT NULL AND TRIM(excluded_reason) <> ''))
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

-- CLOSED : excluded/excluded_reason 변경만 허용(제외 처리·해제)
-- FINALIZED : abandoned 0→1 만 허용
CREATE TRIGGER IF NOT EXISTS trg_prevent_update_closed_application
BEFORE UPDATE ON applications
BEGIN
    SELECT RAISE(ABORT, 'Cannot update application: round is CLOSED. Only excluded/excluded_reason may change.')
    WHERE (SELECT status FROM rounds WHERE id = OLD.round_id) = 'CLOSED'
      AND (
          OLD.student_id      != NEW.student_id
          OR OLD.track_id         != NEW.track_id
          OR OLD.round_id         != NEW.round_id
          OR OLD.department_name  != NEW.department_name
          OR OLD.abandoned        != NEW.abandoned
      );
    SELECT RAISE(ABORT, 'Cannot update application: round is FINALIZED. Only abandoned 0->1 is permitted.')
    WHERE (SELECT status FROM rounds WHERE id = OLD.round_id) = 'FINALIZED'
      AND (
          OLD.student_id      != NEW.student_id
          OR OLD.track_id         != NEW.track_id
          OR OLD.round_id         != NEW.round_id
          OR OLD.department_name  != NEW.department_name
          OR (OLD.abandoned = 1 AND NEW.abandoned = 0)
          OR OLD.excluded         != NEW.excluded
          OR OLD.excluded_reason  IS NOT NEW.excluded_reason
      );
END;

-- CLOSED 라운드 지원자의 기초데이터 삭제 방지
-- INSERT OR REPLACE는 내부 DELETE에 대해 BEFORE DELETE 트리거를 발동시키지 않으므로
-- UPSERT(수정)는 자유롭게 허용되고 명시적 DELETE만 차단된다.
CREATE TRIGGER IF NOT EXISTS trg_prevent_base_data_delete_for_applied
BEFORE DELETE ON base_data
BEGIN
    SELECT RAISE(ABORT, 'Cannot delete base_data: student has application in CLOSED round')
    WHERE EXISTS (
        SELECT 1 FROM applications ap
        JOIN rounds r ON r.id = ap.round_id
        WHERE ap.student_id = OLD.student_id
          AND r.status = 'CLOSED'
    );
END;
