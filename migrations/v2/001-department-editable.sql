-- ================================================================
-- v2: 마감된 라운드에서도 학과명(department_name) 수정 허용 (이슈 #32)
--
-- 학과명은 점수 산출에 쓰이지 않는다 — results 에 박제되지도 않고
-- (009-results.sql: score_detail 은 {area_id: score} 뿐), 조회·내보내기가
-- applications 를 실시간 JOIN 하므로 값 하나만 바꾸면 재계산 없이 반영된다.
-- 반면 계열·모집단위(track_id)는 선발 단위 자체이므로 계속 동결한다.
--
-- v1 의 트리거를 그대로 옮기되 CLOSED/FINALIZED 두 분기에서
-- department_name 조건 한 줄씩만 뺐다. 나머지 조건은 손대지 않는다.
-- 이미 배포된 v1 DB 위에서 도는 조각이므로 DROP 후 재생성한다.
-- ================================================================

DROP TRIGGER IF EXISTS trg_prevent_update_closed_application;

-- CLOSED : excluded/excluded_reason/department_name 변경만 허용
-- FINALIZED : abandoned 0→1 과 department_name 변경만 허용
CREATE TRIGGER IF NOT EXISTS trg_prevent_update_closed_application
BEFORE UPDATE ON applications
BEGIN
    SELECT RAISE(ABORT, 'Cannot update application: round is CLOSED. Only excluded/excluded_reason/department_name may change.')
    WHERE (SELECT status FROM rounds WHERE id = OLD.round_id) = 'CLOSED'
      AND (
          OLD.student_id      != NEW.student_id
          OR OLD.track_id         != NEW.track_id
          OR OLD.round_id         != NEW.round_id
          OR OLD.abandoned        != NEW.abandoned
      );
    SELECT RAISE(ABORT, 'Cannot update application: round is FINALIZED. Only abandoned 0->1 and department_name are permitted.')
    WHERE (SELECT status FROM rounds WHERE id = OLD.round_id) = 'FINALIZED'
      AND (
          OLD.student_id      != NEW.student_id
          OR OLD.track_id         != NEW.track_id
          OR OLD.round_id         != NEW.round_id
          OR (OLD.abandoned = 1 AND NEW.abandoned = 0)
          OR OLD.excluded         != NEW.excluded
          OR OLD.excluded_reason  IS NOT NEW.excluded_reason
      );
END;
