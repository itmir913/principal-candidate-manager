# Excel Import 명세

모든 import는 `excel::parse_file_rows_with_headers` → `col_map` → `require_cols` 순서로 진행한다.
열 인덱스 직접 참조 금지 — 반드시 헤더 이름 기반.

---

## 공통 규칙

| 규칙 | 설명 |
|---|---|
| All-or-Nothing | 오류 1건이라도 발생 시 rollback + 422, 부분 저장 없음 |
| 중복 = error | warning/skip 처리 금지 |
| 헤더 중복 | 동일 열 이름이 2개 이상이면 즉시 400 |
| 빈 파일 | 헤더 없으면 빈 결과 반환 (오류 아님) |
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
  - 학년/반 숫자 파싱 실패 → 해당 행 오류
  - 비밀번호 bcrypt 해싱 후 저장
  - grade=0, class_no=0 (특수계정) import 금지 여부: 코드상 차단 없음 — 관리자 주의 필요

---

### 2. 학생 (Students) — 3종류

#### 2a. 전체 학생 import — `/api/students/import`
- **필수 헤더**: `학생코드`, `이름`, `학년`, `반`, `번호`, `재학여부`
- **동작**: upsert (student_code 기준 `ON CONFLICT DO UPDATE`) — DELETE 없음
- **재학여부**: `1` 또는 `재학` → is_enrolled=1, 그 외 → 0

#### 2b. 재학생 import — `/api/students/enrolled/import`
- **필수 헤더**: `학년`, `반`, `번호`, `이름`
- **동작**: upsert (grade+class_no+seq_no 기준)
- **is_enrolled**: 항상 1로 고정

#### 2c. 졸업생 import — `/api/students/graduated/import`
- **필수 헤더**: `학생코드`, `이름`
- **동작**: upsert (student_code 기준)
- **is_enrolled**: 항상 0으로 고정

> students import는 모두 upsert — DELETE+INSERT 금지.

---

### 3. 점수 기준 — numeric_table (RANGE 전형요소)

**엔드포인트**: `POST /api/areas/:id/numeric-table/import`

- **파일 형식**: xlsx / CSV
- **기본 필수 헤더**: `기준값`, `점수`
- **COMPOSITE 추가 헤더**: `대학명`, `모집단위명` (둘 다 있거나 둘 다 비워야 함)
- **동작**: 해당 area의 numeric_table 전체 DELETE 후 INSERT (tx 안에서)

**검증 순서**:
1. area의 calc_type이 Numeric인지 확인 (아니면 400)
2. CLOSED 라운드 존재 시 import 차단 (`guard_no_closed_round`)
3. 헤더 파싱 + `require_cols`
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
- **COMPOSITE 추가 헤더**: `대학명`, `모집단위명`
- **동작**: 해당 area의 category_map 전체 DELETE 후 INSERT (tx 안에서)

**검증 순서**:
1. calc_type이 Category인지 확인
2. CLOSED 라운드 존재 시 차단
3. 각 행: 범주 비어 있으면 오류, 점수 변환
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
- 학생 조회: `student_code`

**동작 분기**:
- `multi_value=0` (단일값): (student_id, track_id) 중복 행 → 오류. 오류 없으면 `INSERT OR REPLACE`
- `multi_value=1` (복수값, CATEGORY SUM): 중복 행 허용. 오류 없으면 파일에 등장한 (student, track) 조합만 DELETE 후 INSERT

**student_type 필터 필수**: `enrolled` 업로드 → `is_enrolled=1` 학생만, `graduated` → `is_enrolled=0` 학생만. 반대편 데이터 건드리지 않음.

**값 변환**:
- NUMERIC / MANUAL: `parse_display_value` (×100000)
- CATEGORY: 문자열 그대로 저장
- MANUAL: 값 > max_score → 오류 (입력값이 곧 점수)

**COMPOSITE**: 대학명+모집단위명 모두 비면 track_id=NULL(공통), 하나만 있으면 오류

---

### 6. 외부 가져오기 — 대교협 석차연명부

**엔드포인트**: `POST /api/areas/:id/base-data/external/daegyo/import`

- **파일 형식**: xlsx 전용 (xls 차단)
- **area 제약**: lookup_scope=COMPOSITE인 전형요소만 허용
- **파싱 구조**:
  - 1행: `지역-대학명(캠퍼스)-전형유형-...` 형식에서 대학명 추출 (index 1)
  - 2행: 헤더 (`학년`, `반`, `번호`, `이름`, `일반등급`, `내점수(환산)`, `내등급(환산)` 필수)
  - 3행~: 데이터. `내점수(환산)` = "미제공" 이면 `일반등급` 사용, 아니면 `내등급(환산)` 사용
- **학생 조회**: grade+class_no+seq_no, is_enrolled=1 (재학생만)
- **이름 불일치**: warning (import 계속)
- **값 변환**: area.calc_type에 따라 (NUMERIC/MANUAL: ×100000, CATEGORY: 그대로)
- **동작**: `INSERT OR REPLACE` (student_type 필터 없이 track_id 기반으로 구분됨)
- **오류 있으면**: tx rollback, find_or_create_track으로 생성된 트랙도 rollback

#### 미리보기 (`/daegyo/preview`)
- 파일만 업로드, univ_name/track_name 불필요
- 파싱 결과 상위 5행 + 총 건수 반환

---

### 7. 외부 가져오기 — 유니브 석차연명부

**엔드포인트**: `POST /api/areas/:id/base-data/external/univ/import`

- **파일 형식**: xls 전용 (xlsx 차단)
- **파싱 구조**:
  - 1행 B열(index 1): 대학명
  - 6행(index 5): 헤더 (`학년`, `반`, `번호`, `이름`, `등급` 필수)
  - 7행(index 6)~: 데이터. 사용 값: `등급`
- 이후 로직은 대교협과 동일

---

## CLOSED 라운드 guard

`numeric_table_import`, `category_map_import`는 진입 시 `guard_no_closed_round` 호출:
- CLOSED 상태 라운드가 존재하면 **409 Conflict** 반환
- 이유: CLOSED 라운드의 점수 기준을 수정하면 저장된 results와 불일치 발생

`base_data_import`, 외부 import에는 이 guard 없음 (CLOSED 시 base_data 수정은 별도 trigger로 보호).

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
