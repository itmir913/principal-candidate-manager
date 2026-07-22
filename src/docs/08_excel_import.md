# Excel Import 명세

모든 import는 `excel::parse_file_rows_with_headers` → `col_map` → `require_cols` 순서로 진행한다.
열 인덱스 직접 참조 금지 — 반드시 헤더 이름 기반.

---

## 공통 규칙

| 규칙 | 설명 |
|---|---|
| All-or-Nothing | 오류 1건이라도 발생 시 rollback + 422, 부분 저장 없음. **유일한 예외는 외부 석차연명부의 석차 값 열 (§7-1)** — 값 없음/변환 실패는 error가 아니라 행 skip + warning |
| 중복 = error | warning/skip 처리 금지. **파일 내 동일 키 중복 행도 error** (마지막 행 silent win 금지) — 모든 import에 적용 |
| 헤더 중복 | 동일 열 이름이 2개 이상이면 즉시 400 |
| 빈 파일 | 헤더 없으면 필수 열 누락으로 400. 점수 기준(numeric/category)·외부 import는 **데이터 0행도 400** (빈 파일이 기존 데이터를 조용히 비우는 것 방지) |
| 인코딩 | UTF-8 BOM → UTF-8 → EUC-KR(CP949) 자동 감지 |
| .xls 차단 | `.xls` 업로드 시 즉시 400 (사용자에게 `.xlsx` 변환 안내) |
| 빈 행 | 모든 셀이 비어 있는 행은 무시 |

점수 저장: ×100000 정수. `parse_display_value` 함수가 `f64 → round → i64` 변환. 소수점 5자리 초과 시 오류.

---

## 엔티티별 import 규칙

### 1. 학반 (Classes) — `/api/classes/import`

- **파일 형식**: xlsx / CSV
- **필수 헤더**: `학년`, `반`, `비밀번호`
- **동작**: `INSERT OR REPLACE` (upsert) — DELETE 없음, 다른 반에 영향 없음
- **검증**:
  - 학년/반은 1 이상 숫자 (0·음수·파싱 실패 → 해당 행 오류; 특수계정 0/0 생성 불가). 단건 upsert(`PUT /classes/:g/:c`)도 동일하게 1 이상 강제
  - 파일 내 동일 (학년, 반) 중복 행 → 오류
  - 신규 학급 행에 비밀번호 누락 → 행 오류 (기존 학급은 비밀번호 없이 담임명만 갱신 가능)
  - 비밀번호 bcrypt 해싱 후 저장

---

### 2. 학생 (Students) — 3종류

#### 2a. 전체 학생 import — `/api/students/import`
- **필수 헤더**: `학생코드`, `이름`, `학년`, `반`, `번호`, `재학여부`
- **동작**: upsert (student_code 기준 SELECT-후-UPDATE/INSERT 분기, `students.rs:363-395`) — DELETE 없음. tx 안·단일 커넥션에서 실행되므로 SQL 네이티브 `ON CONFLICT DO UPDATE`와 결과 동등, TOCTOU 없음
- **재학여부**: `재학`/`재학생` → is_enrolled=1, `졸업`/`졸업생` → is_enrolled=0, 그 외(숫자 0/1·빈 값 포함) → 해당 행 오류 (silent default 금지 — 재학/졸업 분류는 우선순위·기초데이터 범위에 영향. 숫자 0/1은 의미가 모호해 배제)
- **위치 유일성**: 재학생 위치(학년+반+번호)는 학생코드가 달라도 유일해야 함. 파일 내 위치 중복 행 → 오류. DB에 이미 다른 학생코드가 점유한 위치로 upsert 시도 → 행 오류. DB 최후 방어선은 `idx_students_position` 부분 유니크 인덱스 (기초데이터 import의 위치 기반 학생 조회가 임의 학생에게 점수를 귀속시키는 것을 방지)

#### 2b. 재학생 import — `/api/students/enrolled/import`
- **필수 헤더**: `학년`, `반`, `번호`, `이름`
- **동작**: upsert (grade+class_no+seq_no 기준)
- **is_enrolled**: 항상 1로 고정

#### 2c. 졸업생 import — `/api/students/graduated/import`
- **필수 헤더**: `학생코드`, `이름`
- **동작**: upsert (student_code 기준)
- **is_enrolled**: 항상 0으로 고정

> students import는 모두 upsert — DELETE+INSERT 금지.
> 파일 내 동일 키(2a·2c: 학생코드, 2b: 학년+반+번호) 중복 행은 error — 전체 422.

---

### 3. 점수 기준 — numeric_table (RANGE 전형요소)

**엔드포인트**: `POST /api/areas/:id/numeric-table/import`

- **파일 형식**: xlsx / CSV
- **기본 필수 헤더**: `기준값`, `점수`
- **COMPOSITE 선택 헤더**: `대학명`, `모집단위명` (열은 선택 사항 — 있으면 트랙별 값, 없거나 셀만 비어있으면 공통(track_id NULL) 값으로 저장. 담당자가 "이 area는 트랙 무관 공통값" 의도로 두 열을 생략하는 워크플로가 지원됨. 한 쪽만 채우면 오류)
- **동작**: 해당 area의 numeric_table 전체 DELETE 후 INSERT (tx 안에서)

**검증 순서**:
1. area의 calc_type이 Numeric인지 확인 (아니면 400)
2. CLOSED 라운드 존재 시 import 차단 (`guard_no_closed_round`)
3. 헤더 파싱 + `require_cols` + 데이터 0행이면 400 (빈 파일이 기준표를 비우는 것 방지)
4. 각 행: 기준값·점수 숫자 변환
5. 점수 > max_score → 오류
6. (track_id, threshold) 중복 → 오류
7. COMPOSITE: 대학명/모집단위명 쌍 검증, `find_or_create_track` 호출
8. **오류 없으면**: 단조성 검사 (UPPER: threshold↑ → score 비감소, LOWER: threshold↑ → score 비증가)
9. 경고: UPPER 모드에서 기준값 0 행 없으면 warning (최저값 미만 학생 점수 산출 불가)
10. tx.commit()

**오류 시**: tx drop으로 자동 rollback (find_or_create_track으로 생성된 대학/모집단위도 함께 rollback)

---

### 4. 점수 기준 — category_map (CATEGORY 전형요소)

**엔드포인트**: `POST /api/areas/:id/category-map/import`

- **기본 필수 헤더**: `범주`, `점수`
- **COMPOSITE 선택 헤더**: `대학명`, `모집단위명` (numeric_table과 동일하게 **선택 사항**이다 —
  `require_cols`는 `범주`·`점수`만 강제한다(`area_data.rs:598`). 두 열을 생략하면
  공통(track_id NULL) 값으로 저장된다. 한 쪽만 채우면 오류)
- **동작**: 해당 area의 category_map 전체 DELETE 후 INSERT (tx 안에서)

**검증 순서**:
1. calc_type이 Category인지 확인
2. CLOSED 라운드 존재 시 차단
3. 데이터 0행이면 400. 각 행: 범주 비어 있으면 오류, 점수 변환
4. 점수 > max_score → 오류
5. (track_id, category) 중복 → 오류
6. **0점 항목 필수 검증**: (area_id, track_id) 그룹별로 양수 점수가 1개 이상이면 score=0인 범주 행 필수
   - 감점 전용 그룹 (양수 점수 없음) → 0점 행 없어도 허용
7. tx.commit()

---

### 5. 기초 데이터 (base_data) — 재학생/졸업생 분리

**엔드포인트**: `POST /api/areas/:id/base-data/import?student_type=enrolled|graduated`

**재학생 (enrolled)**:
- **필수 헤더**: `학년`, `반`, `번호`, `이름`, `값`
- COMPOSITE: 추가로 `대학명`, `모집단위명`
- 학생 조회: `grade + class_no + seq_no + is_enrolled=1`

**졸업생 (graduated)**:
- **필수 헤더**: `학생코드`, `이름`, `값`
- COMPOSITE: 추가로 `대학명`, `모집단위명`
- 학생 조회: `student_code + is_enrolled=0` — 재학생 코드가 섞이면 행 오류 (재학생 데이터 침범 금지)

**동작 분기**:
- `multi_value=0` (단일값): (student_id, track_id) 중복 행 → 오류. 오류 없으면 `INSERT OR REPLACE`
- `multi_value=1` (복수값, CATEGORY SUM): 중복 행 허용. 오류 없으면 파일에 등장한 (student, track) 조합만 DELETE 후 INSERT
  - DELETE가 CLOSED 라운드 지원자 보호 트리거에 걸리면 500이 아닌 **422 + 학생코드 안내**로 번역 (보호 로직은 트리거가 단일 진실 원천, 핸들러는 오류 매핑만)

**student_type 필터 필수**: `enrolled` 업로드 → `is_enrolled=1` 학생만, `graduated` → `is_enrolled=0` 학생만. 반대편 데이터 건드리지 않음. `enrolled`/`graduated` 외의 값은 silent fallback 없이 **400** (list·template·import 공통, `parse_student_type`).

**값 변환**:
- NUMERIC / MANUAL: `parse_display_value` (×100000)
- CATEGORY: 문자열 그대로 저장
- MANUAL: 값 > max_score → 오류 (입력값이 곧 점수)

**COMPOSITE**: 대학명+모집단위명 모두 비면 track_id=NULL(공통), 하나만 있으면 오류

---

### 6. 외부 가져오기 — 대교협 석차연명부

**엔드포인트**: `POST /api/areas/:id/base-data/external/daegyo/import`

- **파일 형식**: xlsx 전용 (xls 차단)
- **area 제약**: lookup_scope=COMPOSITE인 전형요소만 허용. **multi_value=1(CATEGORY SUM) 전형요소는 400 거부** — 석차연명부는 학생당 단일 값이고, 값 변경 재업로드 시 유니크 인덱스(value 포함) 때문에 기존 행이 남아 SUM 이중 합산이 발생하기 때문. 복수값 데이터는 기초 데이터 업로드 사용
- **파싱 구조**:
  - 1행: `지역-대학명(캠퍼스)-전형유형-...` 형식에서 대학명 추출 (index 1)
  - 2행: 헤더 (`학년`, `반`, `번호`, `이름`, `일반등급`, `내점수(환산)`, `내등급(환산)` 필수)
  - 3행~: 데이터. `내점수(환산)` = "미제공" 이면 `일반등급` 사용, 아니면 `내등급(환산)` 사용
- **학생 조회**: grade+class_no+seq_no, is_enrolled=1 (재학생만)
- **파일 내 동일 학생 중복 행**: 오류 (전체 422). 오류 행 번호는 원본 엑셀 기준(1-based)
- **데이터 0행**: 400 (트랙만 생성되는 no-op 방지)
- **이름 불일치**: warning (import 계속)
- **석차 값 없음/변환 실패**: **해당 행만 건너뛰고 warning** (전체 거부 아님 — 아래 참조)
- **저장 행 0건**: 422 (모든 행이 건너뛰어진 경우. 값 열을 잘못 고른 파일이 "완료 — 0건"으로 통과하는 것 방지). 이 경우에도 건너뛴 사유 warning은 응답에 포함
- **값 변환**: area.calc_type에 따라 (NUMERIC/MANUAL: ×100000, CATEGORY: 그대로)
- **동작**: `INSERT OR REPLACE` (student_type 필터 없이 track_id 기반으로 구분됨)
- **오류 있으면**: tx rollback, find_or_create_track으로 생성된 트랙도 rollback

#### 미리보기 (`/daegyo/preview`)
- 파일만 업로드, univ_name/track_name 불필요
- 파싱 결과 상위 5행 + 총 건수 반환
- `header_info`: 1행 A열 원문(`지역-대학명(캠퍼스)-전형유형-...`) 그대로. 올린 파일이 맞는지 사용자가 눈으로 확인하는 용도

---

### 7. 외부 가져오기 — 유니브 석차연명부

**엔드포인트**: `POST /api/areas/:id/base-data/external/univ/import`

- **파일 형식**: xls 전용 (xlsx 차단)
- **파싱 구조**:
  - 1행 B열(index 1): 대학명
  - 6행(index 5): 헤더 (`학년`, `반`, `번호`, `이름`, `등급` 필수)
  - 7행(index 6)~: 데이터. 사용 값: `등급`
- 이후 로직은 대교협과 동일
- 미리보기의 `header_info`: 1~3행 A·B열(대학/학과/전형)을 `라벨: 값 | 라벨: 값 | 라벨: 값` 한 줄로 직렬화

---

### 7-1. 외부 가져오기 예외 — 석차 값 없음/변환 실패는 행 건너뛰기

**적용 범위**: 대교협·유니브 외부 가져오기(`external_import.rs::do_import`)의 **석차 값 열 한 곳만**.
기초 데이터 업로드(§5)와 점수 기준 import(§3·§4)는 종전대로 값 누락·변환 실패 시 전체 422.

**규칙**:

| 상황 | 처리 |
|---|---|
| 석차 값 셀이 비어 있음 (trim 후 빈 문자열) | 해당 행 skip + warning `"{행}행: {학년}학년 {반}반 {번호}번 {이름} — 석차 값이 비어 있어 건너뜀"` |
| NUMERIC/MANUAL에서 `parse_display_value` 실패 | 해당 행 skip + warning `"{행}행: {학년}학년 {반}반 {번호}번 {이름} — 석차 값 '{원본}' 숫자 변환 실패 → 건너뜀"` |
| 학년·반·번호 변환 실패 / 미등록 학생 / 파일 내 중복 | **종전대로 error → 전체 422** (학생 식별 실패는 건너뛰면 안 됨) |
| 건너뛴 결과 저장 행이 0건 | 422 + `"저장된 행이 없습니다 — 모든 행의 석차 값이 비어 있거나 숫자로 변환할 수 없습니다"` |

**이유**: 전출·자퇴 학생은 대교협/유니브 프로그램이 등급 열을 값 없음 표시로 내보낸다.
이를 error로 처리하면 관리자가 업로드할 때마다 석차연명부 원본에서 해당 학생 행을
직접 지워야 하고, 실수로 성적이 있는 행을 지울 위험이 생긴다.

**실제 대교협 파일의 전출 학생 행** (2026-07-21 확인):

| 학년 | 반 | 번호 | 이름 | 일반점수 | 일반등급 | 내점수(환산) | 내등급(환산) | 석차 |
|---|---|---|---|---|---|---|---|---|
| 3 | 6 | 20 | 홍길동 | `'-` | `'-` | `'-` | `'-` | 335 |

- 점수·등급 4개 열이 **모두** 값 없음이고 `석차` 열에만 숫자가 남는다.
- 셀 값에 **엑셀 텍스트 접두 아포스트로피가 포함**되어 있다 — calamine이 읽는 문자열은
  `-`가 아니라 `'-`다. 따라서 빈 셀 검사에 걸리지 않고 `parse_display_value` 실패 경로로 간다.
  경고 메시지에도 원본 그대로 `'-`로 표시되어야 관리자가 파일에서 해당 셀을 찾을 수 있다.
- `내점수(환산)`이 `'-`이지 `"미제공"`이 아니므로 §6의 일반등급 분기는 타지 않고
  `내등급(환산)`(`'-`)을 그대로 쓴다 — 결과적으로 skip.
- 회귀 테스트: `tests/handler_external_import.rs::daegyo_import_real_transfer_row_skips_only_that_student`
  (이 행을 아포스트로피까지 그대로 재현)

**건너뛴 행의 안전성**: skip된 학생은 해당 area의 base_data가 아예 없는 상태로 남는다.
잘못된 점수가 저장되는 것이 아니라 데이터 부재이므로, 그 학생이 실제로 지원하면
라운드 마감 전 기초데이터 누락 검증(`close_round` 사전 검증)에서 다시 드러난다.
즉 이 예외는 **오류를 은폐하지 않고 시점만 뒤로 미룬다**.

**warning 전달**: skip 사유는 `warnings`와 분리된 별도 벡터에 모았다가 응답 직전에 합친다.
전체 422로 rollback되는 경우에도 skip 사유는 응답에 실어야 관리자가 원인을 알 수 있고,
반대로 rollback된 `'대학/모집단위 자동 추가됨'` warning은 사실이 아니므로 제외된다.

관련: `silent_fallback_allowed.md` #29

---

## CLOSED 라운드 guard

`numeric_table_import`, `category_map_import`는 진입 시 `guard_no_closed_round` 호출:
- **CLOSED 또는 FINALIZED** 상태 라운드가 존재하면 **409 Conflict** 반환 (`areas.rs:67-82`)
- 이유: CLOSED/FINALIZED 라운드의 점수 기준을 수정하면 저장된 results와 불일치 발생

`base_data_import`, 외부 import에는 이 guard 없음 (CLOSED 시 base_data 수정은 별도 trigger로 보호).

---

## 파싱 Fail-Fast 정책

### 지수 표기 거부 (`area_data.rs::parse_display_value`)

표시 문자열 → ×100000 정수 파싱 시 **지수 표기(`1e-6`, `2E5` 등)를 파싱 전 명시적으로
거부**한다. Rust f64 파서는 지수 표기를 수용하지만, 뒤이은 소수 자릿수 검사가 원본
문자열의 `'.'` 위치에 의존하므로 지수 표기 입력은 검사를 조용히 우회한다.

- `"0.000001"` → `소수점 5자리 초과` Err (거부)
- `"1e-6"` → 표기에 따른 갈림 없이 `지수 표기는 지원되지 않습니다` Err (거부)

학교 성적·점수 도메인에 지수 표기가 필요한 정당한 이유가 없으므로 표기 대칭성과
Fail-Fast를 위해 파싱 진입 시점에 차단한다.

### DataType variant 명시 Err (`excel.rs::cell_to_str`)

calamine `DataType`의 10개 variant 중 처리 가능한 4개(String/Float/Int/Bool)와 정당한
빈 셀(Empty)만 `Ok`, 나머지 **5개는 명시적 `Err`로 승격**한다.

| variant | 처리 |
|---|---|
| `String` / `Float` / `Int` / `Bool` | `Ok(문자열 변환)` |
| `Empty` | `Ok("")` — 빈 셀은 정당한 값 없음 |
| `DateTime` / `DateTimeIso` | Err "날짜 서식 셀은 지원되지 않습니다..." |
| `Duration` / `DurationIso` | Err "시간 서식 셀은 지원되지 않습니다..." |
| `Error(CellErrorType)` | Err "셀에 수식 오류(...)가 있습니다..." |

**wildcard `_ => String::new()`로 복원 금지.** 이전 wildcard 처리에서 학번·점수 열에
실수로 날짜 서식이 적용되거나 `#REF!` 같은 수식 오류 셀이 있으면 조용히 빈 문자열이
되어 downstream `is_empty()` 체크에 우연히 걸리는 fail-safe에 의존했다. 특히
`resolve_track`의 `(true, true) => Some(None)` 경로에서는 COMPOSITE 트랙 값이 공통
테이블로 조용히 강등 저장되는 실질 사고 경로였다.

새 variant 추가 시 명시적 `Err`로 처리하거나 `Ok` 처리 근거를 밝힐 것. 회귀 테스트
`src/excel.rs::cell_to_str_tests`에서 전 variant를 검증한다.

---

## 오류 응답 형식

```json
{
  "rows": 0,
  "errors": ["2행: 점수 '3.5'가 전형요소 만점(3)을 초과합니다", ...],
  "warnings": []
}
```

HTTP 상태: 422 (오류 있을 때) / 200 (성공)

성공 응답:
```json
{
  "rows": 42,
  "errors": [],
  "warnings": ["'서울대/컴퓨터공학부' 모집단위 자동 추가됨"]
}
```
