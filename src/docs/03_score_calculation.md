# 03. 점수 계산 전 과정 명세

## 진입점

점수 계산은 두 경로로 시작된다:

- **`close_round`**: 라운드 종료 시 자동 호출. `BEGIN IMMEDIATE` 트랜잭션 내부에서 `run_calculate_scores_on_conn(conn, round_id, now)`를 직접 호출. 실패 시 ROLLBACK으로 CLOSED 상태 변경까지 취소된다.
- **`calculate_scores` (`POST /rounds/:id/calculate`)**: 관리자가 수동으로 재계산 요청. CLOSED 상태인지 먼저 확인 후 `run_calculate_scores(db, round_id)` 호출. 이 래퍼 함수도 내부적으로 `BEGIN IMMEDIATE`를 사용한다.

핵심 로직은 `run_calculate_scores_on_conn(&mut SqliteConnection, round_id, now)`에 집중되어 있으며, `run_calculate_scores`는 단독 호출을 위한 풀 기반 래퍼다.

---

## 입력 데이터

`run_calculate_scores` 함수가 읽어오는 테이블:

1. **`areas`**: 전형요소 목록 전체. `id, name, calc_type, max_score, match_mode, category_agg, lookup_scope` 컬럼.
2. **`applications`**: 해당 라운드에서 `confirmed=1`인 지원 신청. 학생 정보(`students` JOIN), 모집단위 정보(`univ_tracks`, `universities` JOIN)를 함께 조회.
3. **`base_data`**: 각 학생·전형요소·(모집단위)별 원시 데이터. `calc_area_score` 함수 내부에서 개별 조회.
4. **`numeric_table`**: NUMERIC 전형요소의 구간별 점수 기준표. `calc_area_score` 내부에서 조회.
5. **`category_map`**: CATEGORY 전형요소의 범주별 점수 맵. `calc_area_score` 내부에서 조회.

---

## CalcType별 계산 로직

### NUMERIC (구간 점수)

1. `base_data`에서 해당 학생·전형요소·(모집단위)의 값을 문자열로 읽어 정수로 파싱한다.
   - 값이 없으면 오류(Fail-Fast 정책).
   - 정수 파싱 실패 시 오류.
2. `numeric_table`에서 해당 전형요소·(모집단위)의 구간표를 `threshold` 오름차순으로 조회한다.
   - COMPOSITE 전형요소에서 모집단위별 구간표가 없으면 공통(`track_id IS NULL`) 테이블로 폴백한다.
3. `lookup_range_score` 함수로 구간 매칭:
   - **UPPER**: 값 >= threshold인 행 중 threshold가 가장 큰 행의 점수. (값이 모든 threshold보다 작으면 오류)
   - **LOWER**: threshold가 허용 상한선 역할. 값 <= threshold인 행 중 threshold가 가장 작은 행의 점수. 값이 최대 threshold를 초과하면 최대 threshold 행의 점수 사용(오류 없음).
   - **EXACT**: threshold == 값인 행의 점수. 일치하는 행이 없으면 오류.
4. `max_score`로 상한을 적용한다: `raw.min(area.max_score)`.

### CATEGORY (범주 점수)

1. `base_data`에서 해당 학생·전형요소·(모집단위)의 값을 복수 행으로 조회한다 (multi_value 허용).
   - 값이 1건도 없으면 오류(Fail-Fast 정책) — 0점 강제가 아닌 오류 처리.
2. 각 범주 문자열마다 `category_map`에서 점수를 조회한다.
   - COMPOSITE 전형요소에서 모집단위별 범주표가 없으면 공통 테이블로 폴백.
   - 해당하는 범주 항목이 없으면 오류.
3. `category_agg`에 따라 집계:
   - **SUM**: 조회된 모든 점수의 합산.
   - **MAX**: 조회된 점수 중 최대값.
4. `max_score`로 상한 적용.

⚠️ [CATEGORY 0점 강제] memory의 "Score on submit" 가이드라인에는 "CATEGORY 0점 강제"가 언급되어 있으나, `run_calculate_scores`의 코드에서는 base_data가 없으면 오류로 처리한다. 0점 강제는 `category_map` 설계 단계에서 "해당 없음" 범주를 score=0으로 등록하는 방식으로 구현되며, `category_map_import` 시 양수 점수가 있는 그룹에 score=0 행이 없으면 import를 거부하는 검증으로 강제한다.

### MANUAL (수동 입력)

1. `base_data`에서 해당 학생·전형요소·(모집단위)의 값을 1개 조회한다.
   - 값이 없으면 오류.
   - 정수 파싱 실패 시 오류.
2. 조회된 정수값을 그대로 점수로 사용한다 (별도 테이블 조회 없음).
3. `max_score`로 상한 적용.

---

## 음수 값 허용 정책

`base_data`의 값(value) 필드는 **음수를 허용**한다. NUMERIC·MANUAL 전형요소 모두 해당된다.

- **설계 의도**: 특정 전형요소는 감점 방식으로 운영될 수 있다. 예를 들어 출결 불량 시 -5점 처리.
- `parse_display_value`는 소수점 5자리 초과 여부만 검증하고 부호(±)는 제한하지 않는다.
- 관리자 import(`base_data_import`)와 담임 입력(`teacher_create_application`) 양쪽 모두 음수 허용.
- MANUAL은 `max_score` 상한만 적용하며 하한(음수 제한)은 없다.
- 음수 값은 합산 점수(`total_score`)에 그대로 반영되므로 전체 합이 음수가 될 수도 있다.

---

## 구간표(`numeric_table`) import 시 검증

`numeric_table_import`는 저장 전에 두 가지 품질 검증을 수행한다.

### 단조성(Monotonicity) 검증 — 오류

threshold 오름차순으로 점수가 단조적이지 않으면 import를 거부한다(422).

| `match_mode` | 요구 조건 | 위반 예시 |
|---|---|---|
| `UPPER` | threshold 증가 시 점수 **비감소** (같거나 증가) | threshold 30→60인데 점수 90→80 |
| `LOWER` | threshold 증가 시 점수 **비증가** (같거나 감소) | threshold 0→3인데 점수 50→60 |
| `EXACT` | 해당 없음 (단조성 제약 없음) | — |

**이유**: UPPER(상한→유리)와 LOWER(하한→유리) 모드에서 점수가 역전되면 점수 계산 시 silent wrong 결과가 나온다. import 단계에서 차단해 데이터 품질을 강제한다.

### UPPER 기준값 0 누락 — 경고(warning)

UPPER 모드 구간표에 `threshold=0` 행이 없으면 경고를 반환한다. import는 허용하되, 실제 학생 데이터가 모든 threshold보다 낮을 경우 `close_round` 시 해당 학생의 점수 계산이 실패할 수 있음을 사전 안내한다.

---

## LookupScope 처리 (COMPOSITE vs SIMPLE)

- **SIMPLE**: `base_data`, `numeric_table`, `category_map` 조회 시 `track_id IS NULL` 조건.
- **COMPOSITE**: `track_id = 지원 모집단위 id` 조건. 해당 모집단위 데이터가 없으면 공통(`NULL`) 테이블로 폴백. 폴백은 `calc_area_score` 내부에서 자동 처리.

---

## score_detail 저장 방식

각 지원자에 대해 전형요소별 점수를 `HashMap<String, i64>` 형태로 구성한다.
- 키: 전형요소 id를 문자열로 변환한 값 (예: `"1"`, `"2"`)
- 값: `×100000` 정수 그대로 (Score newtype의 내부 표현값)
- `serde_json::to_string`으로 직렬화해 `results.score_detail` 컬럼에 TEXT로 저장.

JSON 응답 시에는 `score_detail_as_map` 커스텀 시리얼라이저가 이 TEXT를 파싱해 `Score::from_raw`를 통해 각 값을 f64로 변환해 응답한다.

---

## 결과 저장 패턴

`results` 테이블에 저장 시 `ON CONFLICT (student_id, track_id, round_id) DO UPDATE SET ...` 패턴을 사용한다.
- 동일 (student_id, track_id, round_id) 조합이 이미 있으면 `score_detail`, `total_score`, `ranking`, `calculated_at`을 갱신한다.
- **`ranking`은 NULL로 초기화**한 다음 트랜잭션 내에서 다시 계산해 채운다.
- `recommended` 필드는 갱신하지 않는다 — 재계산해도 기존 추천 상태가 보존된다.

---

## 트랜잭션 경계

`run_calculate_scores_on_conn`은 단일 `&mut SqliteConnection` 위에서 읽기와 쓰기를 순차적으로 수행한다. 트랜잭션 관리는 호출자가 담당한다.

- **`close_round`에서 호출 시**: 호출자가 `BEGIN IMMEDIATE`를 선점. 검증·상태 변경·점수 계산·results 저장·순위 계산 전체가 하나의 커넥션·트랜잭션으로 처리된다.
- **`run_calculate_scores`(수동 재계산 래퍼)에서 호출 시**: 래퍼가 `BEGIN IMMEDIATE`를 선점한 뒤 `run_calculate_scores_on_conn`을 호출하고 COMMIT / ROLLBACK한다.

`BEGIN IMMEDIATE`를 사용하므로, 계산 구간 동안 다른 커넥션의 쓰기(base_data import 등)가 차단된다. SQLite WAL 모드에서 읽기는 스냅샷 격리로 계속 허용된다.

점수 계산과 results INSERT가 같은 커넥션에서 이루어지므로, 순위 계산 시 방금 저장한 results를 같은 트랜잭션에서 바로 읽을 수 있다.

---

## Score newtype — 직렬화·역직렬화·DB 저장

**DB 저장**: `sqlx::Encode` 구현에서 `i64`와 동일하게 처리 — 내부 정수값(`self.0`) 그대로 SQLite INTEGER 컬럼에 저장.

**DB 조회**: `sqlx::Decode` 구현에서 `i64`를 읽어 `Score(i64)`로 래핑.

**JSON 직렬화**: `Serialize` 구현에서 `self.0 as f64 / 100_000.0`으로 나눠 소수로 직렬화. 예: 내부값 `3050000` → JSON `30.5`.

**JSON 역직렬화**: `Deserialize` 구현에서 `f64`를 읽어 `(f * 100_000.0).round() as i64`로 변환. 예: JSON `30.5` → 내부값 `3050000`.

---

## score_preview (`GET /api/score-preview`) — 관리자 전용

관리자 화면에서 특정 학생·모집단위 조합의 전체 점수를 미리 계산하는 엔드포인트.

- 라우트: `GET /api/score-preview` (`require_admin` 미들웨어 적용, 관리자만 호출 가능).
- `run_calculate_scores`와 달리 DB에 아무것도 저장하지 않는다.
- 전 전형요소 점수 합산 시 `checked_add`를 사용해 `i64` 오버플로우를 방지한다.

담임용 미리보기는 별개 엔드포인트이다: `POST /api/teacher/area-score-preview` (`05_homeroom_flow.md` 참조). 전형요소 단위로 계산하며 `matched_keys`(하이라이팅)와 `warning`을 추가로 반환한다는 점에서 다르다.

---

## 수동 재계산 (`POST /rounds/:id/calculate`)

`close_round`와의 차이점:
- CLOSED 상태 확인을 직접 수행한다.
- 기초데이터 누락 사전 검증을 하지 않는다. 누락이 있으면 `run_calculate_scores_on_conn` 내부에서 422 오류가 발생하고 ROLLBACK된다.
- 상태를 변경하지 않는다. CLOSED 상태 그대로 재계산만 수행.
- `run_calculate_scores(db, round_id)` 래퍼를 호출하며, 이 래퍼도 `BEGIN IMMEDIATE`로 계산 구간 동안 다른 커넥션의 쓰기를 차단한다.
- 응답: `{ "calculated": N }`.