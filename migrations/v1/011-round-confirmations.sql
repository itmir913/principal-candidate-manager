-- ================================================================
-- ROUND_CONFIRMATIONS (담임 입력 확정)
-- 담임이 "우리 반 지원 입력을 모두 마쳤다"고 명시적으로 확정한 기록.
-- round_id → rounds CASCADE DELETE: 라운드 삭제 시 함께 삭제
-- grade/class_no → classes CASCADE DELETE: 학급 삭제 시 함께 삭제
-- ================================================================
CREATE TABLE IF NOT EXISTS round_confirmations (
    round_id     INTEGER NOT NULL,
    grade        INTEGER NOT NULL,
    class_no     INTEGER NOT NULL,
    confirmed_at TEXT    NOT NULL,
    PRIMARY KEY (round_id, grade, class_no),
    FOREIGN KEY (round_id) REFERENCES rounds(id) ON DELETE CASCADE,
    FOREIGN KEY (grade, class_no) REFERENCES classes(grade, class_no) ON DELETE CASCADE
);
