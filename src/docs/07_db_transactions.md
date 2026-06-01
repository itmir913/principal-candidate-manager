# 07. 트랜잭션 경계 명세

## 트랜잭션을 사용하는 핸들러 목록

| 핸들러 | 트랜잭션 범위 | 비고 |
|--------|---------------|------|
| `close_round` | 기초데이터 누락 검증 + rounds 상태 변경 | 검증과 상태 변경을 원자적으로 처리. run_calculate_scores는 이 트랜잭션 커밋 후 별도 트랜잭션으로 실행 |
| `reopen_round` | rounds 상태 변경 + results 추천·순위 초기화 | 두 UPDATE를 원자적으로 처리 |
| `run_calculate_scores` | results 저장 전체 + 순위 계산 전체 | 읽기(점수 계산)는 트랜잭션 밖, 쓰기만 트랜잭션 내 |
| `recommend_result` | 상태 조회 + 정원 조회 + recommended 갱신 | race condition 방지 목적 |
| `unrecommend_result` | 상태 조회 + recommended 갱신 | race condition 방지 목적 |
| `teacher_create_application` | base_data 저장 + applications upsert + 점수 계산 + results 저장 | 4단계 원자적 처리 |
| `teacher_delete_application` | results 삭제 + applications 삭제 | FK 제약 순서(results 먼저 삭제) 보장. 졸업생 담당 분기 포함(is_enrolled=0 검증). |
| `numeric_table_import` | 전체 삭제 + 행 삽입 반복 | 오류 시 tx drop으로 자동 롤백 |
| `category_map_import` | 전체 삭제 + 행 삽입 반복 + 0점 검증 | 오류 시 tx drop으로 자동 롤백 |
| `base_data_import` | student_type별 삭제 + 행 삽입 반복 | 오류 시 tx drop으로 자동 롤백 |
| `import_students` | 전체 행 upsert 반복 | 오류 시 tx drop으로 자동 롤백 |
| `import_enrolled` | 재학생 위치 기반 upsert 반복 | 오류 시 tx drop으로 자동 롤백 |
| `import_graduated` | 졸업생 upsert 반복 | 오류 시 tx drop으로 자동 롤백 |
| `import_classes` | 학급 upsert 반복 | bcrypt 계산은 행 단위로 트랜잭션 진입 전에 수행 |
| `upsert_class` | classes INSERT 또는 UPDATE | 신규/기존 분기 처리 원자화 |
| `daegyo_import` / `univ_import` | 모집단위별 삭제 + 행 삽입 반복 | `do_import` 함수 공통 사용 |

---

## bcrypt 위치 — 트랜잭션 진입 전 계산

**확인된 모든 위치에서 bcrypt는 트랜잭션 진입 전에 계산한다.**

- `change_admin_password` (handlers/auth.rs): `bcrypt::hash` → `UPDATE app_configs` (트랜잭션 없음, 단일 쿼리)
- `teacher_change_password` (handlers/applications.rs): `bcrypt::hash` → `UPDATE classes` (트랜잭션 없음, 단일 쿼리)
- `import_classes` (handlers/classes.rs): 루프 내에서 행마다 `bcrypt::hash`를 트랜잭션 밖에서 계산 후 → 트랜잭션 내 INSERT/UPDATE. 주석에 "bcrypt는 CPU 작업이므로 트랜잭션 진입 전에 미리 계산"이라고 명시되어 있음.
- `upsert_class` (handlers/classes.rs): 동일하게 `bcrypt::hash` 먼저 계산 후 트랜잭션 시작.

**이유**: bcrypt는 CPU 집약 연산으로, 트랜잭션 내에서 실행하면 DB 잠금 유지 시간이 늘어난다. SQLite는 동시 쓰기가 제한적이므로 이를 방지하기 위한 설계.

---

## 트랜잭션 없이 단일 쿼리로 처리하는 쓰기 작업

| 핸들러 | 쓰기 내용 | 단일 쿼리여도 안전한 이유 |
|--------|-----------|--------------------------|
| `admin_login` (초기화 시) | `app_configs` 단일 행 INSERT OR UPDATE | 행 하나를 원자적으로 upsert |
| `change_admin_password` | `app_configs` UPDATE 1건 | 단일 UPDATE는 원자적 |
| `teacher_change_password` | `classes` UPDATE 1건 | 단일 UPDATE는 원자적 |
| `open_round` | `rounds` INSERT 1건 | 단일 INSERT는 원자적 |
| `finalize_round` | `rounds` UPDATE 1건 | 단일 UPDATE는 원자적 |
| `abandon_application` (관리자) | `applications` UPDATE 1건 | 단일 UPDATE는 원자적 |
| `teacher_abandon_application` | `applications` UPDATE 1건 | 단일 UPDATE는 원자적 |
| `delete_student` | `students` DELETE 1건 | 단일 DELETE는 원자적 |
| `delete_class` | `classes` DELETE 1건 | 단일 DELETE는 원자적 |

`close_round`는 내부에서 두 개의 분리된 트랜잭션을 사용한다.

1. **첫 번째 트랜잭션**: 기초데이터 누락 검증과 `UPDATE rounds SET status = 'CLOSED'`를 하나의 트랜잭션으로 묶는다. 검증 실패 시 커밋 없이 자동 롤백되어 상태가 OPEN으로 유지된다.
2. **두 번째 트랜잭션** (`run_calculate_scores` 내부): 첫 번째 트랜잭션 커밋 후 별도로 시작되며 점수 계산·저장을 처리한다.

⚠️ [close_round 트랜잭션 분리] 두 트랜잭션이 원자적으로 묶이지 않으므로, 이론상 rounds는 CLOSED로 바뀌었으나 점수 계산이 실패하는 상황이 가능하다. 이 경우 수동 재계산(`/rounds/:id/calculate`)으로 복구할 수 있다.

---

## 실패 시 롤백 조건

**명시적 `.rollback()` 호출**: 코드 전반에 걸쳐 명시적 `tx.rollback()` 호출은 없다.

**자동 롤백 방식**: Rust의 소유권 시스템을 활용한다. `tx` 변수가 scope를 벗어나거나 함수에서 early return 할 때 `Transaction` 타입의 `Drop` 구현이 자동으로 `rollback`을 실행한다. 즉, `tx.commit()`을 호출하지 않고 함수가 종료되면 자동 롤백된다.

**Import 핸들러의 패턴**: `if !errors.is_empty() { return Ok((StatusCode::UNPROCESSABLE_ENTITY, ...)); }` 형태로 오류 발생 시 commit 없이 return한다. 이 시점에 `tx`가 drop되어 자동 롤백된다. 주석에 "tx이 drop되면 자동 rollback — 부분 삽입 없음"이라고 명시된 위치도 있다.

---

## Import 핸들러의 트랜잭션 경계와 오류 정책

### `base_data_import`
- begin → student_type 기반 DELETE → 행별 INSERT 반복 → 오류 시 422 + 자동 롤백 → 성공 시 commit
- 재학생(`enrolled`)과 졸업생(`graduated`) 데이터를 독립적으로 교체한다: `DELETE ... WHERE student_id IN (SELECT id FROM students WHERE is_enrolled = ?)`로 해당 student_type의 데이터만 삭제.

### `import_students`
- begin → 행별 `upsert_student` 반복 → 오류 시 422 + 자동 롤백 → 성공 시 commit
- upsert 방식(INSERT 또는 UPDATE)이므로 기존 데이터를 보존하면서 업데이트 가능.

### `import_enrolled`
- begin → 행별 `upsert_enrolled_by_position` 반복 → 오류 시 422 + 자동 롤백 → 성공 시 commit
- student_code를 자동 생성(연도+학년+반+번호 조합). 학급 존재 여부 검증 포함.

### `import_graduated`
- begin → 행별 `upsert_student` (is_enrolled=0) 반복 → 오류 시 422 + 자동 롤백 → 성공 시 commit

### `import_classes`
- begin → 행별 upsert 반복. bcrypt 계산은 각 행의 루프 내에서 트랜잭션 시작 후에 하는 것처럼 보이지만, 코드를 보면 행 단위 bcrypt 계산은 tx를 사용하지 않는 단계에서 수행된다(tx가 시작된 후이지만 bcrypt는 tx가 불필요한 순수 CPU 연산). 오류 시 422 + 자동 롤백 → 성공 시 commit.
- `import_classes`는 기존 학급 데이터를 전체 삭제하지 않고 upsert(교체 안내 예외 사항)하므로 기존 비밀번호가 보존된다.

**모든 Import 핸들러의 공통 정책**: 단 1건이라도 오류가 있으면 전체 거부(rollback + 422). 부분 import 없음.