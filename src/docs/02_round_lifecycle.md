# 02. 라운드 상태 전이 명세

## 상태 정의

| 상태 | 의미 | 허용 행위 |
|------|------|-----------|
| `OPEN` | 지원 접수 진행 중 | 담임이 지원 등록·삭제 가능. 기초데이터 입력 가능. |
| `CLOSED` | 접수 마감. 점수 계산 완료 상태 | 관리자가 결과 조회, 추천/추천 취소, 자동 추천 확정(auto-recommend) 가능. 담임 지원 등록 불가. |
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

**DB 방어선**
- `idx_one_active_round` 부분 유니크 인덱스(`ON rounds((1)) WHERE status != 'FINALIZED'`)가 비-FINALIZED 라운드 다중 존재를 DB 레벨에서도 차단한다. 앱 레벨 원자적 검사가 우회되더라도(직접 SQL 등) OPEN·CLOSED 라운드는 전체에서 최대 1개만 존재할 수 있다.
- `idx_one_open_round`(OPEN 단일성)는 그 부분집합으로 계속 유지된다.

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

**처리 (`BEGIN IMMEDIATE` 단일 트랜잭션)**

전체 흐름이 `BEGIN IMMEDIATE` 트랜잭션 안에서 원자적으로 처리된다.

1. `SELECT status FROM rounds WHERE id = ?` 로 현재 상태 확인.
   - `status != 'CLOSED'`이거나 라운드가 없으면 404 반환, ROLLBACK.
2. **미결정 지원 검증** — 추천도 제외도 되지 않은 지원이 있으면 422 반환, ROLLBACK. (아래 참조)
3. 아래 정원 위반 검증 수행. 위반이 있으면 422 반환, ROLLBACK.
4. `UPDATE rounds SET status = 'FINALIZED', finalized_at = ? WHERE id = ? AND status = 'CLOSED'` 실행.
   - `AND status = 'CLOSED'` 가드: 트랜잭션 보유 중 외부 변경 방어.

`BEGIN IMMEDIATE` 덕분에 상태 확인부터 UPDATE까지 다른 커넥션이 끼어들 수 없어, 과거에 존재하던 TOCTOU 위험이 해소됐다.

### 전건 결정 완료 사전 검증

마감 전 모든 지원에 대해 "추천 확정" 또는 "제외(결격)" 중 하나가 반드시 결정되어 있어야 한다.

**미결정의 정의**: `excluded = 0` AND (추천 확정되지 않음)
- (a) 대응하는 `results` 행이 있고 `recommended = 0`
- (b) 대응하는 `results` 행이 아예 없음(점수 미계산)

미결정이 1건이라도 있으면 422 + 미결정 지원자 전체 명단 JSON (`"undecided"` 키) 반환, 상태는 `CLOSED` 유지.
이 검증은 정원 검증보다 먼저 수행된다 — 두 위반이 동시에 존재할 때 미결정 안내를 먼저 표시한다.

**DB 방어선**: `trg_require_all_decided_before_finalize` 트리거가 핸들러 우회 경로에서도 동일 조건을 차단한다.

### 정원 초과 검증

- **모집단위 검증** — `unit_quota IS NOT NULL`인 트랙 중, 전체 라운드 누적 `recommended=1 AND abandoned=0` 수가 `unit_quota`를 초과하는 항목 탐지
- **대학 검증** — `total_quota IS NOT NULL`인 대학 중, 소속 트랙 전체 추천 합산이 `total_quota`를 초과하는 항목 탐지
- 위반 항목이 있으면 422 + 위반 목록 JSON 반환, 상태는 `CLOSED` 유지
- 정원이 `NULL`이면 무제한으로 처리

---

## 전형요소 설정 변경 제한 (`guard_no_closed_round`)

마감된 라운드(CLOSED 또는 FINALIZED)가 하나라도 존재하면 전형요소 관련 쓰기 작업이 전면 차단된다.

**판단 기준**: `SELECT COUNT(*) FROM rounds WHERE status IN ('CLOSED', 'FINALIZED')` 결과가 1 이상이면 차단.

**반환**: 409 Conflict

**차단 대상 — `guard_no_closed_round` 적용 핸들러:**

| 핸들러 | 엔드포인트 | 차단 이유 |
|--------|-----------|-----------|
| `create_area` | `POST /api/areas` | 추가된 전형요소가 기존 `score_detail`에 없어 내보내기 실패 가능 |
| `delete_area` | `DELETE /api/areas/:id` | 과거 score_detail에서 해당 전형요소 데이터 유실 |
| `update_area` | `PUT /api/areas/:id` | 이름·설정 변경이 기존 박제 결과와 불일치 유발 가능 |
| `numeric_table_import` | `POST /api/areas/:id/numeric-table/import` | 점수 기준 변경은 재계산을 강제해야 하므로 CLOSED 상태에서 금지 |
| `category_map_import` | `POST /api/areas/:id/category-map/import` | 동일 이유 |

**허용되는 수정**: 전형요소 이름·`teacher_editable` 변경 자체는 차단 대상이나, 실질적으로 `update_area`가 차단되므로 CLOSED/FINALIZED 라운드 존재 시에는 어떠한 수정도 불가.

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
| `area_data.rs` (base_data_import 복수값 분기) | 관리자 Excel 복수값 import — `DELETE FROM base_data WHERE student_id=? AND area_id=?` | CLOSED 지원자면 ABORT → 핸들러가 422 + 학생코드 안내로 번역 |
| `applications.rs` (teacher_create_application) | 담임 복수값 재제출 DELETE | OPEN 라운드 활성 = CLOSED 없음 = 미발동 (항상 안전) |

**핸들러 레벨 체크를 별도로 추가하지 말 것.** 이 트리거가 단일 진실 원천이다. 핸들러에서 허용되는 것은 트리거 ABORT 오류를 사용자 친화적 응답(500 → 422)으로 **번역**하는 것뿐이며, 보호 여부 판단 로직을 중복 구현해서는 안 된다.

### results 삭제 보호 (대칭 트리거)

`trg_prevent_delete_closed_result`: CLOSED/FINALIZED 라운드의 `results` 행 DELETE를 차단한다.
- 기존 `trg_prevent_update_finalized_result`(FINALIZED UPDATE 차단)와 대칭을 이뤄 박제 보호를 완성한다.
- OPEN 라운드는 담임 지원 취소(`teacher_delete_application`)가 results를 동반 삭제해야 하므로 허용한다.

---

## `get_current_round` — 현재 라운드 조회 (`GET /api/rounds/current`, 공개 엔드포인트)

- `SELECT ... FROM rounds WHERE status = 'OPEN' LIMIT 1`
- OPEN 상태 라운드를 최대 1개만 반환한다.
- 결과는 `Option<RoundRow>` — OPEN 라운드가 없으면 `null` 반환.
- `LIMIT 1`을 사용하지만, `open_round`에서 OPEN 또는 CLOSED 라운드가 1개 이상이면 신규 생성을 막으므로 정상 상태에서는 OPEN 라운드가 최대 1개만 존재한다.
- 즉, 동시에 여러 OPEN 라운드가 존재할 수 없는 설계이다.
