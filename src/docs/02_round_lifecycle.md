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

**전체 흐름이 단일 `BEGIN IMMEDIATE` 트랜잭션으로 원자적으로 처리된다.**

`BEGIN IMMEDIATE`를 사용하는 이유:
- 이 시점부터 다른 커넥션의 **쓰기(base_data 수정 포함)를 차단**한다.
- WAL 모드에서 다른 커넥션의 **읽기는 그대로 허용**된다.
- 점수 계산이 실패하면 ROLLBACK으로 status 변경까지 함께 취소되어 **라운드는 OPEN 상태로 복귀**한다. CLOSED 상태에서 점수 없는 불일치 상태가 원천적으로 불가능하다.

### ① 기초데이터 누락 사전 검증

검증 쿼리는 다음 조건으로 누락을 탐지한다:
- 해당 라운드의 모든 지원 신청을 기준으로,
- 모든 전형요소(`areas`)에 대해 해당 학생의 `base_data`가 존재하는지 확인한다.
- COMPOSITE 전형요소는 `base_data.track_id = ap.track_id`를 추가 조건으로, SIMPLE 전형요소는 `track_id IS NULL`을 조건으로 한다.
- `LIMIT 5`를 사용하는 이유: 누락이 있는 경우 전체 목록을 반환하면 응답이 너무 크고 사용자에게는 처음 몇 건만 표시해도 충분하기 때문이다.
- 누락이 1건이라도 있으면 422 Unprocessable Entity를 반환하고 ROLLBACK. 라운드 상태는 OPEN 그대로 유지된다.

**사전 검증의 한계**: base_data 존재 여부만 확인한다. `numeric_table` / `category_map`의 내용 불일치(예: UPPER 모드에서 학생 값이 모든 threshold보다 낮음, CATEGORY 범주가 맵에 없음)는 ②점수 계산 단계에서 422 오류로 감지된다. 이 경우에도 ROLLBACK으로 OPEN 상태가 유지된다.

### ② status 변경

`UPDATE rounds SET status = 'CLOSED', closed_at = ? WHERE id = ? AND status = 'OPEN'`을 실행한다.
- `AND status = 'OPEN'` 조건으로 이미 CLOSED인 라운드를 이중으로 닫는 것을 방지한다.
- `rows_affected == 0`이면 ROLLBACK 후 404 반환.

### ③ 점수 계산 (같은 트랜잭션·커넥션 안에서)

`run_calculate_scores_on_conn`을 호출해 모든 지원자의 점수를 계산하고 `results` 테이블에 저장한다.
- 실패 시 ROLLBACK → status 변경(②)도 취소되어 라운드는 OPEN으로 복귀.
- 성공 시 COMMIT → status가 CLOSED로 확정되고 results가 박제된다.
- 응답에 계산된 지원자 수(`{ "calculated": N }`)를 포함한다.

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

## `base_data` 무결성 보호 — DB 트리거

### 보호 대상

CLOSED 상태 라운드에 지원서가 있는 학생의 `base_data` 행에 대한 **명시적 DELETE**를 DB 레벨에서 차단한다.

### 트리거: `trg_prevent_base_data_delete_for_applied`

```sql
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
```

### 논리 근거

`close_round`는 CLOSED 전이 전에 모든 지원자의 `base_data` 완전성을 사전 검증한다 (①번 단계).  
따라서 **CLOSED 상태 = base_data 완전성 보장**이 DB 불변식이 된다.  
이 불변식을 깨는 경로는 명시적 DELETE뿐이므로, 트리거 하나로 완전한 보호가 가능하다.

### UPSERT는 항상 허용 — SQLite INSERT OR REPLACE 동작

`INSERT OR REPLACE`는 내부적으로 DELETE + INSERT를 수행하지만, BEFORE DELETE 트리거를 발동시키지 **않는다**.  
따라서 기초데이터 수정(UPSERT) — 담임교사 재입력, 관리자 Excel 재업로드 — 은 CLOSED 상태에서도 자유롭게 허용된다.

### CLOSED-only 조건 선택 이유

FINALIZED 라운드는 새 OPEN 라운드와 공존 가능하다.  
FINALIZED 조건을 추가하면 이전 라운드에서 FINALIZED된 학생이 새 라운드에서 기초데이터를 제출할 때 차단된다.  
CLOSED만을 조건으로 함으로써 다중 라운드 시나리오에서도 적법한 요청을 막지 않는다.

### 명시적 DELETE 경로 (전체 2곳)

| 위치 | 경로 | 트리거 발동 여부 |
|------|------|-----------------|
| `area_data.rs:982` | 관리자 Excel 복수값 import — `DELETE FROM base_data WHERE student_id=? AND area_id=?` | CLOSED 지원자면 ABORT |
| `applications.rs:559` | 담임 복수값 재제출 DELETE | OPEN 라운드 활성 = CLOSED 없음 = 미발동 (항상 안전) |

**핸들러 레벨 체크를 별도로 추가하지 말 것.** 이 트리거가 단일 진실 원천이다.

---

## `get_current_round` — 현재 라운드 조회 (`GET /api/rounds/current`, 공개 엔드포인트)

- `SELECT ... FROM rounds WHERE status = 'OPEN' LIMIT 1`
- OPEN 상태 라운드를 최대 1개만 반환한다.
- 결과는 `Option<RoundRow>` — OPEN 라운드가 없으면 `null` 반환.
- `LIMIT 1`을 사용하지만, `open_round`에서 OPEN 또는 CLOSED 라운드가 1개 이상이면 신규 생성을 막으므로 정상 상태에서는 OPEN 라운드가 최대 1개만 존재한다.
- 즉, 동시에 여러 OPEN 라운드가 존재할 수 없는 설계이다.
