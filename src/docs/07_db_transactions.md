# 07. 트랜잭션 경계 명세

## 트랜잭션을 사용하는 핸들러 목록

| 핸들러 | 트랜잭션 범위 | 비고 |
|--------|---------------|------|
| `close_round` | 기초데이터 누락 검증 + rounds 상태 변경 + 점수 계산·results 저장 + 순위 계산 전체 | `BEGIN IMMEDIATE` 단일 커넥션·트랜잭션. 점수 계산 실패 시 ROLLBACK으로 상태 변경까지 취소됨 |
| `reopen_round` | rounds 상태 변경 + results 추천·순위 초기화 | 두 UPDATE를 원자적으로 처리 |
| `calculate_scores` | 라운드 상태 확인 + results 저장 전체 + 순위 계산 전체 | `BEGIN IMMEDIATE`. 상태 확인이 tx 안에 있어 reopen과의 TOCTOU 없음 |
| `recommend_result` | 상태 조회 + 정원 조회 + recommended 갱신 | `BEGIN IMMEDIATE`. 정원 체크(SELECT COUNT)~UPDATE 사이 TOCTOU 방지 |
| `unrecommend_result` | 상태 조회 + recommended 갱신 | 일반 트랜잭션. recommended 갱신 원자화 |
| `finalize_round` | 상태 확인 + 정원 검증 + rounds UPDATE | `BEGIN IMMEDIATE`. 상태 확인~FINALIZED 변경까지 원자적 처리. UPDATE에 `AND status='CLOSED'` 이중 가드 |
| `teacher_create_application` | 라운드 OPEN 재확인 + base_data 저장 + applications upsert + 점수 계산 + results 저장 | `BEGIN IMMEDIATE`. 시작 시점 쓰기 잠금으로 재확인이 확정적 — close_round와 race 시 깨끗한 400 |

**`BEGIN IMMEDIATE` 구현 방식**: `Pool::begin_with("BEGIN IMMEDIATE")` (sqlx 0.8) 사용. sqlx가 트랜잭션 상태를 추적하므로 오류 경로에서 tx drop 시 자동 ROLLBACK되고, COMMIT/ROLLBACK 실패 시에도 커넥션이 열린 tx 상태로 풀에 반환되지 않는다. 과거의 수동 `sqlx::query("BEGIN IMMEDIATE")` + `COMMIT`/`ROLLBACK` 문자열 실행 패턴은 sqlx가 tx를 인지하지 못해 커넥션 오염 위험이 있어 2026-07-15에 전면 교체했다. **신규 코드에서 수동 BEGIN 문자열 실행 금지.**
| `teacher_delete_application` | results 삭제 + applications 삭제 | FK 제약 순서(results 먼저 삭제) 보장. 졸업생 담당 분기 포함(is_enrolled=0 검증). |
| `numeric_table_import` | 전체 삭제 + 행 삽입 반복 | 오류 시 tx drop으로 자동 롤백 |
| `category_map_import` | 전체 삭제 + 행 삽입 반복 + 0점 검증 | 오류 시 tx drop으로 자동 롤백 |
| `base_data_import` | 단일값: INSERT OR REPLACE / 복수값: (student, track) 조합별 DELETE + INSERT | 오류 시 tx drop으로 자동 롤백. 파일에 없는 학생 데이터는 보존됨 |
| `import_students` | 전체 행 upsert 반복 | 오류 시 tx drop으로 자동 롤백 |
| `import_enrolled` | 재학생 위치 기반 upsert 반복 | 오류 시 tx drop으로 자동 롤백 |
| `import_graduated` | 졸업생 upsert 반복 | 오류 시 tx drop으로 자동 롤백 |
| `import_classes` | 학급 upsert 반복 | tx.begin() 후 루프 내에서 행마다 bcrypt 계산(tx 커넥션은 사용하지 않는 순수 CPU 단계). upsert_class와 달리 tx 시작 이후 계산 |
| `upsert_class` | classes INSERT 또는 UPDATE | 신규/기존 분기 처리 원자화 |
| `daegyo_import` / `univ_import` | INSERT OR REPLACE 반복 (파일에 없는 학생 보존) | `do_import` 함수 공통 사용 |

---

## bcrypt 위치 — DB 쓰기 전 계산

**확인된 모든 위치에서 bcrypt는 DB 쓰기(INSERT/UPDATE) 전에 계산한다.** `import_classes`만 예외적으로 `tx.begin()` 이후 루프 내에서 계산하나, tx 커넥션을 사용하지 않는 순수 CPU 단계이므로 DB 잠금에 영향이 없다.

- `change_admin_password` (handlers/auth.rs): `bcrypt::hash` → `UPDATE app_configs` (트랜잭션 없음, 단일 쿼리)
- `teacher_change_password` (handlers/applications.rs): `bcrypt::hash` → `UPDATE classes` (트랜잭션 없음, 단일 쿼리)
- `import_classes` (handlers/classes.rs): `tx.begin()` 이후 루프 내에서 행마다 `bcrypt::hash`를 계산한다. tx 커넥션을 사용하지 않는 순수 CPU 단계에서 계산하므로 DB 잠금 유지 시간에 영향이 없다. `upsert_class`와 달리 tx 시작 이전이 아님에 주의. (코드 내 주석 "트랜잭션 진입 전에 미리 계산"은 해당 행의 DB 접근 전을 의미한다.)
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
| `abandon_application` (관리자) | `applications` UPDATE 1건 | 단일 UPDATE는 원자적 |
| `teacher_abandon_application` | `applications` UPDATE 1건 | 단일 UPDATE는 원자적 |
| `add_enrolled_student` | `students` INSERT OR REPLACE 1건 | 위치(학년·반·번호) 기반 upsert. student_code 자동 생성 |
| `add_graduated_student` | `students` INSERT OR REPLACE 1건 | student_code 기반 upsert |
| `delete_student` | `students` DELETE 1건 | 단일 DELETE는 원자적 |
| `delete_class` | `classes` DELETE 1건 | 단일 DELETE는 원자적 |

`close_round`는 `BEGIN IMMEDIATE` **단일 커넥션·트랜잭션**으로 아래 전 과정을 처리한다.

1. 기초데이터 누락 검증
2. `UPDATE rounds SET status = 'CLOSED'`
3. `run_calculate_scores_on_conn` — 점수 계산·results 저장·순위 계산

세 단계 중 어느 하나라도 실패하면 ROLLBACK — status 변경까지 취소되어 라운드는 OPEN으로 복귀한다.

**`BEGIN IMMEDIATE`의 격리 효과**: 트랜잭션 보유 시간 동안 다른 커넥션의 쓰기(base_data import 등)가 SQLite 수준에서 차단된다. WAL 모드에서 읽기는 스냅샷 격리로 계속 허용된다.

---

## 실패 시 롤백 조건

**명시적 `.rollback()` 호출**: 코드 전반에 걸쳐 명시적 `tx.rollback()` 호출은 없다.

**자동 롤백 방식**: Rust의 소유권 시스템을 활용한다. `tx` 변수가 scope를 벗어나거나 함수에서 early return 할 때 `Transaction` 타입의 `Drop` 구현이 자동으로 `rollback`을 실행한다. 즉, `tx.commit()`을 호출하지 않고 함수가 종료되면 자동 롤백된다.

**Import 핸들러의 패턴**: `if !errors.is_empty() { return Ok((StatusCode::UNPROCESSABLE_ENTITY, ...)); }` 형태로 오류 발생 시 commit 없이 return한다. 이 시점에 `tx`가 drop되어 자동 롤백된다. 주석에 "tx이 drop되면 자동 rollback — 부분 삽입 없음"이라고 명시된 위치도 있다.

---

## Import 핸들러의 트랜잭션 경계와 오류 정책

### `base_data_import`
- begin → 행별 UPSERT 반복 → 오류 시 422 + 자동 롤백 → 성공 시 commit
- **단일값(`multi_value=0`)**: `INSERT OR REPLACE`로 기존 행 덮어쓰기.
- **복수값(`multi_value=1`)**: 파일에 등장하는 `(student_id, track_id)` 조합의 기존 행을 DELETE 후 INSERT. 파일에 없는 다른 학생의 행은 건드리지 않는다.
- **파일에 없는 학생 데이터는 보존된다.** 이전 전체 DELETE+INSERT 방식과 달리, 기초데이터는 학생별 측정 사실값이므로 교체 대상이 아닌 데이터는 유지한다.
- 재학생/졸업생 독립 업로드 구조는 유지된다 (`student_type` 파라미터로 대상 학생 필터링).

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

---

## DB 방어선 (트리거)

핸들러 트랜잭션과 별도로, DB 수준에서도 불변식을 강제하는 트리거가 존재한다.  
핸들러 우회(직접 SQL, 외부 DB 클라이언트)에서도 동작한다.

| 트리거 | 파일 | 차단 대상 |
|--------|------|----------|
| `idx_one_active_round` (UNIQUE 인덱스) | `003-rounds.sql:19` | 비-FINALIZED 라운드 2개 이상 INSERT |
| `trg_require_all_decided_before_finalize` | `003-rounds.sql:26` | 미결정 지원(`excluded=0 AND COALESCE(recommended,0)=0`) 존재 시 CLOSED→FINALIZED 전환 |
| `trg_prevent_update_finalized_result` | `009-results.sql:28` | FINALIZED 라운드 `results` 행 UPDATE |
| `trg_prevent_delete_closed_result` | `009-results.sql:36` | CLOSED/FINALIZED 라운드 `results` 행 DELETE |
| `trg_prevent_delete_closed_application` | `008-applications.sql:23` | CLOSED/FINALIZED 라운드 `applications` 행 DELETE |
| `trg_prevent_update_closed_application` | `008-applications.sql:32` | CLOSED 라운드: `excluded`/`excluded_reason` 외 수정. FINALIZED: `abandoned` 0→1 외 수정 |
| `trg_prevent_exclude_recommended` | `008-applications.sql:76` | `recommended=1`인 지원에 대해 `excluded` 0→1 설정 |
| `trg_prevent_base_data_delete_for_applied` | `008-applications.sql:60` | CLOSED 라운드 지원자의 `base_data` 삭제 (UPSERT는 허용) |

**앱 레벨 가드와 이중 방어하는 이유**:  
앱 레벨에서는 오류 명단(누가 미결정인지, 어느 항목이 충돌인지)을 JSON으로 반환할 수 있다.  
트리거는 `RAISE(ABORT, '문자열')` 만 가능해 상세 정보를 반환할 수 없다.  
앱 레벨은 UX, 트리거는 무결성 최후 방어선으로 역할을 분리한다.