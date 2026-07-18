# 04. 정렬·순위·동점 처리 명세

## 정렬 키 순서 — 순위 계산 시

순위 계산은 `run_calculate_scores` 함수의 트랜잭션 내부에서 모집단위별로 수행된다.

**재학생 우선 플래그가 있는 경우 (대학 또는 모집단위의 `prioritize_enrolled = 1`)**

1순위 → 재학생 여부 내림차순 (`is_enrolled DESC`): 재학생을 졸업생보다 앞에 배치
2순위 → 총점 내림차순 (`total_score DESC`): 같은 재학 구분 내에서 점수 높은 순

**재학생 우선 플래그가 없는 경우**

1순위 → 총점 내림차순 (`total_score DESC`)만 적용

**`prioritize_enrolled` 판단 기준 — 각 범위는 자기 플래그만 사용한다 (OR 금지, D2)**

- **대학 전체 순위**(`results.ranking`, 대학 파티션) = `universities.prioritize_enrolled` 만.
- **모집단위 순위**(`track_rank`, 트랙 파티션 파생) = `univ_tracks.prioritize_enrolled` 만.

불변식 "대학=1 ⇒ 그 대학 모든 트랙=1"을 트리거가 강제하므로, 대학=1이면 트랙도 실제 1이라
OR 없이도 두 순위가 일치한다. 대학=0·트랙=1(그 모집단위만 재학생 우선)은 허용되는 정상 구성이다.
대학 값이 바뀌면 트랙에 **양방향 cascade** 된다(1→0 시 트랙의 1은 cascade 강제값이므로 되돌림).

**자동 추천 2단계의 대학 정원 컷 (D)**: 대학 컷은 같은 모집단위 안에서 `track_rank` 상위자를
건너뛰고 하위자를 선택할 수 없다. 1단계 확정분을 전체 재정렬하지 않고, 각 트랙의 **선두**(아직
미선택인 첫 후보)들만 대학 순위로 겨루는 k-way 병합(`merge_univ_cut`)으로 컷한다.
동점 판정도 선두들끼리만 하며, 자기 트랙 상위자에 막힌 후보는 경쟁 대상이 아니므로
탈락해도 수동 사유가 아니다(그 모집단위 정책의 정상 결과 — "이번 라운드 미추천").
경계 4갈래 판정은 `decide_group` 하나를 `fill_by_rank_groups` 와 공유한다.

---

## ranking 부여 방식

**동점 처리 방식**: Standard competition ranking(1,1,3,4,...) — 동점자에게 동일 순위를 부여한다.

- 동점 판정: `prioritize` 트랙이면 `is_enrolled` + `total_score` 둘 다 동일해야 동점, 아니면 `total_score`만 비교
- 동점이면 이전 학생과 같은 `actual_rank` 유지; 동점이 아니면 `i + 1`로 갱신
- 결과 예시: 점수가 100, 100, 90, 80이면 순위는 1, 1, 3, 4
- 동점자 정원 초과 시 `finalize_round`에서 422가 발생하므로 관리자 확인 필요

**ranking이 채워지는 시점**: `run_calculate_scores` 내부 트랜잭션 커밋 시점. 즉, `close_round` 완료 후 또는 수동 재계산(`calculate_scores`) 완료 후.

---

## 결과 조회 시 정렬 (`get_results`)

`GET /rounds/:id/results`의 SQL 정렬 기준:
1순위 → `r.track_id` 오름차순 (모집단위별 그룹)
2순위 → `r.ranking` 오름차순 (`NULLS LAST` — 순위 없는 행은 맨 뒤)
3순위 → `r.total_score` 내림차순 (동일 순위 내 점수 높은 순, ranking이 NULL인 경우를 위한 보조 정렬)

---

## 추천 가능 조건 (`recommend_result`)

`PUT /results/:sid/:tid/:rid/recommend` 호출 시 트랜잭션 내에서 다음을 순서대로 검증한다:

1. **라운드 상태 검증**: `rounds.status == CLOSED`여야 한다. OPEN 또는 FINALIZED 상태에서는 추천 불가 → 400 Bad Request.

2. **모집단위 정원 체크**: `univ_tracks.unit_quota`가 설정된 경우, 해당 모집단위의 전체 라운드에 걸쳐 `recommended=1 AND abandoned=0`인 건수(`track_used`)를 센다. `track_used >= unit_quota`이면 409 Conflict.

3. **대학 전체 정원 체크**: `universities.total_quota`가 설정된 경우, 해당 대학의 모든 모집단위에 걸쳐 `recommended=1 AND abandoned=0`인 건수(`univ_used`)를 센다. `univ_used >= total_quota`이면 409 Conflict.

4. **results 행 갱신**: `UPDATE results SET recommended = 1 WHERE ...`. 해당 행이 없으면(점수 계산 전) 404.

**추천 후 ranking 변경 여부**: 추천(`recommend_result`)은 `results.recommended`만 1로 변경한다. 순위 재계산은 없다.

---

## unrecommend (추천 취소)

`PUT /results/:sid/:tid/:rid/unrecommend`:
- CLOSED 상태에서만 가능 → 400.
- `UPDATE results SET recommended = 0 WHERE ...`
- **ranking 초기화 없음** — `recommended`만 0으로 변경. ranking은 기존값 유지.

---

## FINALIZED 라운드에서의 추천·취소

`recommend_result`와 `unrecommend_result` 모두 `status == CLOSED`를 필수 조건으로 검증한다.
FINALIZED 상태에서는 400 Bad Request가 반환된다.
즉, **FINALIZED 라운드에서는 추천과 추천 취소가 모두 불가**하다.