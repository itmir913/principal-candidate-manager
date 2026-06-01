# 06. 추천자 확정 로직 명세

## applications.confirmed 필드

**현재 동작 (확인된 내용)**: `teacher_create_application`에서 지원 등록 시 `confirmed=1`을 하드코딩으로 삽입한다. 별도의 확정 단계가 없다.

**설계 부채**: 담임이 지원을 등록하면 즉시 confirmed=1이 된다. "담임이 제출(확정)한다"는 별도 행위가 없으며, 저장 = 확정이다. 추후 확정 로직을 별도로 구현할 경우 이 필드를 활용할 수 있도록 컬럼을 남겨둔 것으로 보인다. `project_confirmed_field.md` 메모리에 "추천 자동확정 로직 미구현, 추후 수정 필수"로 기록되어 있다.

`confirmed`는 "담임이 해당 라운드에서 자기 반 학생 입력을 모두 완료했음을 확정했는가"에 대한 플래그였다. 그런데, `임시저장→확정 흐름 필요성 낮아 구현 불필요로 판단`하여 구현을 건너뛴 것으로 보임.

**score 계산 시 확인**: `run_calculate_scores`에서 `confirmed=1`인 지원만 처리한다. 현재는 모든 지원이 confirmed=1이므로 사실상 전수 계산.

---

## recommend_result — 추천 확정 검증 절차

`PUT /results/:sid/:tid/:rid/recommend` (관리자 전용)

트랜잭션 내에서 다음 6단계를 순서대로 실행한다:

**1단계: 라운드 상태 검증**
- `rounds.status == CLOSED`여야 한다.
- OPEN, FINALIZED, 없음: 400 Bad Request.

**2단계: 모집단위 정원 조회**
- `univ_tracks.unit_quota`와 `univ_id`를 조회.

**3단계: 모집단위 정원 초과 여부 확인**
- 해당 모집단위에서 **전체 라운드**에 걸쳐 `recommended=1 AND abandoned=0`인 건수를 집계.
- `track_used >= unit_quota`이면 409 Conflict.
- `unit_quota`가 NULL이면 무제한으로 취급 (체크 건너뜀).

**4단계: 대학 전체 정원 조회**
- `universities.total_quota` 조회.

**5단계: 대학 전체 정원 초과 여부 확인**
- 해당 대학의 모든 모집단위에 걸쳐 **전체 라운드**의 `recommended=1 AND abandoned=0` 합산.
- `univ_used >= total_quota`이면 409 Conflict.
- `total_quota`가 NULL이면 무제한으로 취급 (체크 건너뜀).

**6단계: 추천 확정**
- `UPDATE results SET recommended = 1 WHERE student_id = ? AND track_id = ? AND round_id = ?`
- 해당 행이 없으면 404 (점수 계산 전에 호출한 경우).

**abandoned 상태와 추천의 관계**: 추천 시 해당 학생의 `abandoned` 여부는 검증하지 않는다. abandoned=1인 학생도 추천이 가능하다. 단, 정원 계산에서는 `abandoned=0` 조건으로 포기한 학생을 제외한다.

---

## abandon_application (관리자) — 포기 처리

`PUT /applications/:sid/:tid/:rid/abandon` (관리자 전용)

**허용 상태**: FINALIZED 라운드에서만 가능.
- OPEN, CLOSED 상태이면 400 Bad Request.

**처리**: `UPDATE applications SET abandoned = 1`

**이미 추천된 학생의 포기 처리 가능 여부**: 가능하다. `recommended` 상태를 확인하거나 변경하지 않는다. 이후 정원 계산에서는 `abandoned=0` 조건으로 포기한 추천이 제외되므로, 포기한 학생은 자동으로 정원에서 빠진다.

**담임 포기 처리 (`teacher_abandon_application`)와 동일한 로직**이지만, 관리자 버전은 학생 소속 검증이 없다.

---

## FINALIZED 상태에서의 허용 행위

| 행위 | 허용 여부 | 근거 |
|------|-----------|------|
| 추천 (`recommend_result`) | 불가 | CLOSED 상태 필수 조건, FINALIZED이면 400 |
| 추천 취소 (`unrecommend_result`) | 불가 | CLOSED 상태 필수 조건, FINALIZED이면 400 |
| 관리자 포기 처리 (`abandon_application`) | 가능 | FINALIZED 상태 필수 조건 |
| 담임 포기 처리 (`teacher_abandon_application`) | 가능 | FINALIZED 상태 필수 조건 |

---

## 확정 confirm 팝업 여부

**코드상 별도 confirm 단계 없음**: `recommend_result` 핸들러는 단일 PUT 요청으로 즉시 처리된다. 서버 측에 "확인 요청 → 확인 응답 → 확정 처리"의 2단계 흐름이 없다. 프론트엔드에서 confirm 팝업을 제거하는 것이 정책(`feedback_admin_confirm_policy.md` 참조)이며, 코드에도 별도 confirm 단계는 없다.

반면, 포기 처리(`abandon_application`)나 추천 취소(`unrecommend_result`)는 파괴적 행위로서 프론트엔드에서 confirm 팝업을 유지하도록 되어 있다 (이는 프론트엔드 정책이며 백엔드에는 별도 로직 없음).