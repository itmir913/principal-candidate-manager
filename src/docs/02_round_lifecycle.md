# 02. 라운드 상태 전이 명세

## 상태 정의

| 상태 | 의미 | 허용 행위 |
|------|------|-----------|
| `OPEN` | 지원 접수 진행 중 | 담임이 지원 등록·삭제 가능. 기초데이터 입력 가능. |
| `CLOSED` | 접수 마감. 점수 계산 완료 상태 | 관리자가 결과 조회, 추천/추천 취소 가능. 담임 지원 등록 불가. |
| `FINALIZED` | 확정 완료 | 관리자·담임이 포기 처리 가능. 추천/추천 취소 불가. |

---

## `open_round` — 새 라운드 열기

**조건 확인**
- `rounds` 테이블에서 `status IN ('OPEN', 'CLOSED')`인 행이 1건이라도 존재하면 409 Conflict를 반환한다.
- 즉, 진행 중(OPEN 또는 CLOSED) 라운드가 없을 때만 새 라운드를 열 수 있다.

**원자적 처리**
- `INSERT ... SELECT ... WHERE NOT EXISTS (SELECT 1 FROM rounds WHERE status IN ('OPEN', 'CLOSED')) RETURNING id` 패턴으로 존재 확인과 삽입을 단일 쿼리 안에서 원자적으로 처리한다.
- `RETURNING id`가 `None`이면 이미 진행 중 라운드가 있다는 뜻이므로 409 반환.
- 별도 SELECT → INSERT 분리 시 발생하는 TOCTOU race condition을 방지하기 위한 설계이다.

**생성**
- 위 조건을 통과하면 상태 `OPEN`, `opened_at = 현재 시각`으로 삽입.
- 201 Created + `{ "id": <새 라운드 id> }` 반환.

---

## `close_round` — 라운드 종료 (`PUT /rounds/:id/close`)

처리 순서는 세 단계다.

### ① 기초데이터 누락 사전 검증

상태 변경 전에 실행한다. 이 순서인 이유는 CLOSED 상태로 진입한 뒤 점수 계산이 실패하는 상황을 미리 막기 위함이다.

검증 쿼리는 다음 조건으로 누락을 탐지한다:
- 해당 라운드의 모든 지원 신청을 기준으로,
- 모든 전형요소(`areas`)에 대해 해당 학생의 `base_data`가 존재하는지 확인한다.
- COMPOSITE 전형요소는 `base_data.track_id = ap.track_id`를 추가 조건으로, SIMPLE 전형요소는 `track_id IS NULL`을 조건으로 한다.
- `LIMIT 5`를 사용하는 이유: 누락이 있는 경우 전체 목록을 반환하면 응답이 너무 크고 사용자에게는 처음 몇 건만 표시해도 충분하기 때문이다.
- 누락이 1건이라도 있으면 422 Unprocessable Entity를 반환하고 라운드 상태는 OPEN 그대로 유지한다.

### ② status 변경

`UPDATE rounds SET status = 'CLOSED', closed_at = ? WHERE id = ? AND status = 'OPEN'`을 실행한다.
- `AND status = 'OPEN'` 조건으로 이미 CLOSED인 라운드를 이중으로 닫는 것을 방지한다.
- `rows_affected == 0`이면 404 반환.

### ③ 자동 점수 계산

`run_calculate_scores`를 호출해 모든 지원자의 점수를 계산하고 `results` 테이블에 저장한다.
응답에 계산된 지원자 수(`{ "calculated": N }`)를 포함한다.

---

## `reopen_round` — 라운드 재개 (`PUT /rounds/:id/reopen`)

**목적**: CLOSED → OPEN 전환. 기초데이터를 수정하거나 지원을 추가·제거한 뒤 재계산하고 싶을 때 사용한다.

**처리 (트랜잭션 내)**

1. `UPDATE rounds SET status = 'OPEN', closed_at = NULL WHERE id = ? AND status = 'CLOSED'`
   - `closed_at`을 NULL로 초기화한다.
   - `AND status = 'CLOSED'` 조건으로 OPEN이나 FINALIZED 라운드를 잘못 되돌리는 것을 방지한다.
   - `rows_affected == 0`이면 404 반환.

2. `UPDATE results SET recommended = 0, ranking = NULL WHERE round_id = ?`
   - `results` 테이블의 추천 플래그와 순위를 모두 초기화한다.
   - **이유**: 재개 후 기초데이터가 바뀌어 재계산을 하면 순위가 달라질 수 있으므로, 과거의 추천·순위 데이터가 남아 있으면 stale 데이터가 표시되기 때문이다.
   - 점수 자체(`score_detail`, `total_score`)는 삭제하지 않는다 — 재계산 시 덮어쓴다.

---

## `finalize_round` — 라운드 확정 (`PUT /rounds/:id/finalize`)

**허용 상태**: CLOSED 상태에서만 가능.

**처리**
- 먼저 `SELECT status FROM rounds WHERE id = ?`로 상태를 조회한다.
- `status != 'CLOSED'`이거나 라운드가 없으면 404 반환.
- 이후 아래 정원 위반 검증을 수행하고, 통과하면 `UPDATE rounds SET status = 'FINALIZED', finalized_at = ? WHERE id = ?` 실행.
- UPDATE에는 `AND status = 'CLOSED'` 조건이 없으므로, SELECT 직후 `reopen_round` 가 동시에 호출되면 OPEN 상태에서 FINALIZED로 전환될 수 있는 이론적 TOCTOU가 존재한다 (단일 관리자 환경에서 실질적 위험은 낮음).

⚠️ [finalize_round] 코드 내부에서 아직 자동으로 추천자를 결정하지 않으므로 관리자가 직접 추천이 완료됐는지 확인 후 확정해야 하는 운영 규칙이 있다.

- `finalize_round`에 두 단계 검증 추가:
  1. **모집단위 검증** — `unit_quota IS NOT NULL`인 트랙 중, 전체 라운드 누적 `recommended=1 AND abandoned=0` 수가 `unit_quota`를 초과하는 항목 탐지
  2. **대학 검증** — `total_quota IS NOT NULL`인 대학 중, 소속 트랙 전체 추천 합산이 `total_quota`를 초과하는 항목 탐지
- 위반 항목이 있으면 422 + 위반 목록 JSON 반환, 상태는 `CLOSED` 유지
- 정원이 `NULL`이면 무제한으로 처리

---

## 전형요소 생성·삭제 제한 (`POST /api/areas`, `DELETE /api/areas/:id`)

마감된 라운드가 존재하는 경우 전형요소(`areas`)의 생성과 삭제가 차단된다.

**판단 기준**: `SELECT COUNT(*) FROM rounds WHERE status IN ('CLOSED', 'FINALIZED')` 결과가 1 이상이면 차단.

**반환**: 409 Conflict + 메시지 `"마감된 라운드가 존재하므로 전형요소를 생성하거나 삭제할 수 없습니다"`.

**이유**: 라운드 마감 시 각 전형요소별 학생 점수를 계산해 `results.score_detail`에 박제한다. 이후 전형요소를 추가하면 기존 score_detail에 해당 항목이 없어 내보내기가 실패하고, 삭제하면 과거 점수 기록에서 해당 전형요소 데이터가 유실된다.

**수정 가능한 작업**: 전형요소 `이름` 및 `teacher_editable` 변경(`PUT /api/areas/:id`)은 제한 없이 허용된다.

---

## `get_current_round` — 현재 라운드 조회 (`GET /api/rounds/current`, 공개 엔드포인트)

- `SELECT ... FROM rounds WHERE status = 'OPEN' LIMIT 1`
- OPEN 상태 라운드를 최대 1개만 반환한다.
- 결과는 `Option<RoundRow>` — OPEN 라운드가 없으면 `null` 반환.
- `LIMIT 1`을 사용하지만, `open_round`에서 OPEN 또는 CLOSED 라운드가 1개 이상이면 신규 생성을 막으므로 정상 상태에서는 OPEN 라운드가 최대 1개만 존재한다.
- 즉, 동시에 여러 OPEN 라운드가 존재할 수 없는 설계이다.
