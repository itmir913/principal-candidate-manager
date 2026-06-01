# 03. 점수 계산 전 과정 명세

## 진입점

점수 계산은 두 경로로 시작된다:
- **`close_round`**: 라운드 종료 시 자동 호출 (상태 검증 없이 호출됨 — 호출 전 이미 CLOSED 검증 완료)
- **`calculate_scores` (`POST /rounds/:id/calculate`)**: 관리자가 수동으로 재계산 요청. CLOSED 상태인지 먼저 확인 후 `run_calculate_scores` 호출.

두 경로 모두 `run_calculate_scores(db, round_id)`를 최종적으로 호출한다.

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

점수 계산 전체 흐름의 트랜잭션은 두 구간으로 나뉜다:

1. **트랜잭션 밖 (읽기 전용 계산)**: 별도 connection(`db.acquire()`)으로 모든 학생의 점수를 계산해 `Vec<(student_id, track_id, detail_json, total)>`에 수집. Connection은 계산 완료 후 drop.
2. **트랜잭션 내 (쓰기)**: `db.begin()`으로 트랜잭션 시작 → results 저장 → 모집단위별 순위 계산·저장 → `tx.commit()`.

이렇게 읽기와 쓰기를 분리한 이유는 SQLite의 동시성 제한(쓰기 잠금) 시간을 최소화하기 위함으로 추정된다.

---

## Score newtype — 직렬화·역직렬화·DB 저장

**DB 저장**: `sqlx::Encode` 구현에서 `i64`와 동일하게 처리 — 내부 정수값(`self.0`) 그대로 SQLite INTEGER 컬럼에 저장.

**DB 조회**: `sqlx::Decode` 구현에서 `i64`를 읽어 `Score(i64)`로 래핑.

**JSON 직렬화**: `Serialize` 구현에서 `self.0 as f64 / 100_000.0`으로 나눠 소수로 직렬화. 예: 내부값 `3050000` → JSON `30.5`.

**JSON 역직렬화**: `Deserialize` 구현에서 `f64`를 읽어 `(f * 100_000.0).round() as i64`로 변환. 예: JSON `30.5` → 내부값 `3050000`.

---

## score_preview (`GET /api/teacher/score-preview`)

담임 화면에서 학생·모집단위 조합의 전체 점수를 미리 계산하는 엔드포인트.

- `run_calculate_scores`와 달리 DB에 아무것도 저장하지 않는다.
- 전 전형요소 점수 합산 시 `checked_add`를 사용해 `i64` 오버플로우를 방지한다.

---

## 수동 재계산 (`POST /rounds/:id/calculate`)

`close_round`와의 차이점:
- CLOSED 상태 확인을 직접 수행한다 (close_round는 상태 변경 후 호출하므로 이미 CLOSED가 보장됨).
- 기초데이터 누락 사전 검증을 하지 않는다. 누락이 있으면 `run_calculate_scores` 내부에서 오류가 발생한다.
- 상태를 변경하지 않는다. CLOSED 상태 그대로 재계산만 수행.
- 응답: `{ "calculated": N }`.