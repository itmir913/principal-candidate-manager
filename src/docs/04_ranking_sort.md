# 04. 정렬·순위·동점 처리 명세

## 정렬 키 순서 — 순위 계산 시

순위 계산은 `run_calculate_scores` 함수의 트랜잭션 내부에서 모집단위별로 수행된다.

**재학생 우선 플래그가 있는 경우 (대학 또는 모집단위의 `prioritize_enrolled = 1`)**

1순위 → 재학생 여부 내림차순 (`is_enrolled DESC`): 재학생을 졸업생보다 앞에 배치
2순위 → 총점 내림차순 (`total_score DESC`): 같은 재학 구분 내에서 점수 높은 순

**재학생 우선 플래그가 없는 경우**

1순위 → 총점 내림차순 (`total_score DESC`)만 적용

**`prioritize_enrolled` 판단 기준**: 해당 모집단위(`univ_tracks`)의 `prioritize_enrolled` 또는 해당 대학(`universities`)의 `prioritize_enrolled` 중 하나라도 1이면 우선 적용된다 (`u.prioritize_enrolled = 1 OR ut.prioritize_enrolled = 1`).

---

## ranking 부여 방식

**확인된 내용**: 순서대로 `rank + 1`을 순차 부여한다 (`for (rank, ...) in ranked.iter().enumerate()`). 즉, **동점자에게 동일 순위를 부여하지 않는다** — 정렬 순서에 따라 1, 2, 3, … 순차 부여.

**이유**: 동점자가 있을 때 어느 학생을 먼저 배치할지는 정렬 안정성(stable sort 아닌 점, 비교 함수의 동점 처리 방식)에 달려 있다. Rust의 `sort_by`는 안정 정렬이므로 동점자 사이에서는 원래 순서가 유지된다. 그러나 동일 순위 번호를 부여하는 로직은 없다.

⚠️ [ranking 동점 처리] ranking 로직을 동점자에게 같은 순위를 부여하는 방식(standard competition ranking: 1,2,2,4...)으로 수정함. 동점자 정원 초과 시 관리자 확인이 필요함. 

- 동점 판정: `prioritize` 트랙이면 `is_enrolled` + `total_score` 둘 다 동일해야 동점, 아니면 `total_score`만 비교
- 동점이면 이전 학생과 같은 `actual_rank` 유지; 동점이 아니면 `i + 1`로 갱신
- 결과 예시: 점수가 100, 100, 90, 80이면 순위는 1, 1, 3, 4

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