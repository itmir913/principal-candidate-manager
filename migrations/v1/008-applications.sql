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
