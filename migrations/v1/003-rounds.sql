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
