-- ================================================================
-- AUDIT_LOG (감사 기록)
-- 모든 선발 관련 쓰기 행위를 본 작업과 같은 트랜잭션에서 기록한다.
-- FK 없음: 대상(학생·대학 등)이 삭제된 뒤에도 로그는 보존되어야 한다.
--   사람이 읽을 대상 정보는 detail JSON에 스냅샷으로 저장한다.
-- action: Rust AuditAction enum(SCREAMING_SNAKE_CASE)이 유일한 소스.
--   DB CHECK를 걸지 않는 이유: 릴리즈 후 액션 추가마다 스키마 변경 방지.
-- ================================================================
CREATE TABLE IF NOT EXISTS audit_log (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    at             TEXT    NOT NULL,   -- RFC3339 UTC
    actor_type     TEXT    NOT NULL CHECK(actor_type IN ('ADMIN', 'TEACHER')),
    actor_grade    INTEGER,            -- TEACHER일 때만
    actor_class_no INTEGER,            -- TEACHER일 때만
    actor_name     TEXT,               -- 행위 시점 담임명 스냅샷 (ADMIN은 NULL)
    action         TEXT    NOT NULL,
    round_id       INTEGER,            -- 해당되는 경우만
    student_id     INTEGER,            -- 해당되는 경우만
    detail         TEXT    NOT NULL DEFAULT '{}',  -- JSON 스냅샷
    CHECK (
        (actor_type = 'ADMIN'   AND actor_grade IS NULL     AND actor_class_no IS NULL)
        OR
        (actor_type = 'TEACHER' AND actor_grade IS NOT NULL AND actor_class_no IS NOT NULL)
    )
);
CREATE INDEX IF NOT EXISTS idx_audit_log_at    ON audit_log(at);
CREATE INDEX IF NOT EXISTS idx_audit_log_round ON audit_log(round_id);

-- 감사 로그는 전면 불변 — 수정·삭제를 DB 레벨에서 차단
CREATE TRIGGER IF NOT EXISTS trg_prevent_update_audit_log
BEFORE UPDATE ON audit_log
BEGIN
    SELECT RAISE(ABORT, 'audit_log is immutable');
END;

CREATE TRIGGER IF NOT EXISTS trg_prevent_delete_audit_log
BEFORE DELETE ON audit_log
BEGIN
    SELECT RAISE(ABORT, 'audit_log is immutable');
END;
