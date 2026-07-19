-- ================================================================
-- ROUNDS
-- status 생명주기: OPEN ⟷ CLOSED → FINALIZED
--   OPEN      : 담임 지원 입력 기간
--   CLOSED    : 담임 입력 차단, 관리자 점수 확인·추천 확정/취소 기간
--   FINALIZED : 추천 확정 박제, 담임 포기 입력 가능, 결과 공개
-- ================================================================
CREATE TABLE IF NOT EXISTS rounds (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    status       TEXT NOT NULL CHECK(status IN ('OPEN', 'CLOSED', 'FINALIZED')),
    opened_at    TEXT NOT NULL,
    closed_at    TEXT,
    finalized_at TEXT
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_one_open_round
    ON rounds(status) WHERE status = 'OPEN';
-- 진행 중(비-FINALIZED) 라운드는 전체에서 최대 1개 — open_round의
-- 앱 레벨 원자적 검사(INSERT ... WHERE NOT EXISTS)에 대한 DB 방어선
CREATE UNIQUE INDEX IF NOT EXISTS idx_one_active_round
    ON rounds((1)) WHERE status != 'FINALIZED';

-- 미결정 지원이 남아 있으면 FINALIZED 전환 차단.
-- 앱 레벨 검증(finalize_round)에 대한 DB 방어선.
-- 미결정 = excluded=0 이면서 추천 확정되지 않음(results 행 없음 포함).
-- FINALIZED 는 results 를 영구 잠그므로 되돌릴 수 없다 — DB 에서도 막는다.
CREATE TRIGGER IF NOT EXISTS trg_require_all_decided_before_finalize
BEFORE UPDATE ON rounds
WHEN OLD.status = 'CLOSED' AND NEW.status = 'FINALIZED'
BEGIN
    SELECT RAISE(ABORT, 'Cannot finalize round: undecided applications remain')
    WHERE EXISTS (
        SELECT 1
        FROM applications a
        LEFT JOIN results r ON r.student_id = a.student_id
                           AND r.track_id   = a.track_id
                           AND r.round_id   = a.round_id
        WHERE a.round_id = OLD.id
          AND a.excluded = 0
          AND COALESCE(r.recommended, 0) = 0
    );
END;
