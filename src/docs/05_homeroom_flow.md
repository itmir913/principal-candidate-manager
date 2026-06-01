# 05. 담임 입력 흐름 명세

## area-context — 전형요소 목록 + 현재 데이터 조회

`GET /api/teacher/area-context?student_id=X&track_id=Y`

**학생 소속 검증 (요청 즉시)**
- 일반 담임: `students.grade = claims.grade AND students.class_no = claims.class_no`
- 졸업생 담당(`grade=0, class_no=0`): `students.is_enrolled = 0`
- 소속이 아닌 학생이면 403 Forbidden.

**반환 내용 (전형요소별)**
- `area_id`, `area_name`, `calc_type`, `max_score`, `teacher_editable`, `match_mode`, `category_agg`, `multi_value`
- `current_values`: DB에 저장된 기초데이터 값 목록. NUMERIC/MANUAL은 `×100000` 정수를 소수 문자열로 변환(`fmt_score`), CATEGORY는 원문 문자열. 데이터가 없으면 빈 배열.
- `table`: NUMERIC이면 구간표(`threshold`, `score`), CATEGORY이면 범주표(`category`, `score`), MANUAL이면 `null`.
- COMPOSITE 전형요소의 경우 `track_id` 기준 테이블을 먼저 조회하고, 없으면 공통(`NULL`) 테이블로 폴백.

**multi_value 결정 방식**: `category_agg == Some(CategoryAgg::Sum)`이면 `multi_value = true`. 즉, Sum 집계 전형요소만 복수 값 입력을 허용한다.

---

## area-score-preview — 미리보기 점수 계산

`POST /api/teacher/area-score-preview`

**입력**: `area_id`, `track_id`, `values` (문자열 배열)

**실제 저장과의 차이**
- DB에 아무것도 저장하지 않는다.
- 입력값만을 가지고 점수를 즉시 계산해 응답한다.
- 점수표에서 매칭된 행의 key(`matched_keys`)를 함께 반환해 프론트엔드가 해당 행을 하이라이팅할 수 있게 한다.
- 만점 초과 시 자동으로 만점 적용하고 `warning` 메시지를 반환한다.

**NUMERIC**: `parse_display_str`로 소수 문자열을 `×100000` 정수로 변환 → `lookup_range_score`로 구간 매칭 → `find_numeric_matched_key`로 매칭된 threshold 반환.

**CATEGORY**: 입력된 각 범주 문자열을 `category_map`에서 조회 → `category_agg`(Sum/Max)로 집계.

**MANUAL**: `parse_display_str`로 변환 후 직접 점수로 사용. 테이블 조회 없음.

---

## 지원 등록 (`teacher_create_application`)

`POST /api/teacher/applications`

**처리 순서**

### 1. 트랜잭션 진입 전 검증 및 준비

1. 라운드 상태 확인: `OPEN`이어야 한다. 그 외 상태이면 400.
2. 학생 소속 검증: 담임 JWT의 grade/class_no와 일치 여부.
3. `department_name` 필수 여부: 비어 있으면 400.
4. 전형요소 목록 로드: `teacher_editable` 플래그 확인. `teacher_editable=false`인 전형요소에 값을 전송하면 403.
5. 값 인코딩: NUMERIC/MANUAL은 소수 문자열 → `×100000` 정수로 변환, CATEGORY는 문자열 그대로. 복수값 허용 여부 검증 포함.

### 2. 트랜잭션 내 처리 순서

**⓪ 라운드 상태 재확인 (TOCTOU 방지)**
- 트랜잭션 밖에서 이미 OPEN을 확인했더라도, 트랜잭션 시작 직후 다시 `SELECT status FROM rounds WHERE id = ?`로 상태를 재확인한다.
- 이유: 트랜잭션 밖 확인과 트랜잭션 진입 사이에 `close_round`가 호출되어 CLOSED로 바뀌는 TOCTOU race condition을 방지하기 위함이다.
- OPEN이 아니면 400 Bad Request 반환.

**① 기초데이터 저장** (`base_data` 테이블)
- 복수값(`multi_value=1`) 전형요소: 기존 행 전체 삭제 후 새 값들 삽입.
- 단일값(`multi_value=0`) 전형요소: `INSERT OR REPLACE`로 기존 행 대체.
- base_data를 먼저 저장해야 뒤에 오는 점수 계산이 새 값을 읽을 수 있다.

**② 지원 등록** (`applications` 테이블)
- `INSERT INTO applications ... ON CONFLICT(student_id, track_id, round_id) DO UPDATE SET department_name = excluded.department_name`
- 즉, 동일 (student, track, round) 조합이 이미 있으면 `department_name`만 업데이트한다. **INSERT OR IGNORE가 아닌 ON CONFLICT DO UPDATE 패턴**.
- `confirmed=1, abandoned=0`으로 고정 삽입.

**③ 점수 계산** (트랜잭션 내에서 실행)
- 방금 저장한 기초데이터를 읽어 모든 전형요소에 대한 점수를 계산한다.
- `calc_area_score`를 트랜잭션(`&mut *tx`) 내에서 호출.

**④ results 저장**
- `INSERT INTO results ... ON CONFLICT DO UPDATE` — `close_round`에서와 동일한 패턴.
- 신규 삽입 시: `ranking=NULL, recommended=0`.
- 충돌(upsert) 시: `score_detail`, `total_score`, `ranking(NULL)`, `calculated_at`만 갱신하고 `recommended`는 갱신하지 않는다 — 기존 추천 상태가 보존된다. OPEN 라운드에서는 recommended=1이 실질적으로 발생하지 않으므로 동작에는 영향 없으나, 명시적으로 0으로 초기화하지 않는다는 점을 인지해야 한다.

**트랜잭션 커밋** 후 201 Created 반환.

---

## 저장 가능 조건 (canSave) 검증

**백엔드 검증 방식**:
- 백엔드에서는 `teacher_editable=true`인 전형요소에 값이 없어도 거부하지 않는다. 비어 있는 `values` 배열을 가진 `base_data_entries`는 조용히 건너뛴다.
- `teacher_editable=false` 전형요소는 값을 전송하면 거부(403)하지만, 전송하지 않는 것은 허용한다.

**설계 의도 추정**: canSave 조건(모든 전형요소 값 입력 완료 여부)은 **프론트엔드에서만 검증**하는 것으로 보인다. 백엔드는 값이 없는 teacher_editable 전형요소가 있어도 저장 자체는 허용한다. 다만, CLOSED 시점의 `run_calculate_scores`에서 해당 base_data가 없으면 오류가 발생하므로 실질적으로는 등록 전 전체 입력이 강제된다.

기존 코드는 `teacher_editable=false` 영역에 값이 전송되면 403으로 거부했지만, 반대로 `teacher_editable=true` 영역의 값이 누락된 경우는 조용히 통과시켰습니다.

추가된 검증([applications.rs:432](src/handlers/applications.rs))은 `all_areas` 중 `teacher_editable=true`인 모든 전형요소가 `submitted_area_ids`에 포함되어 있는지 확인하며, 하나라도 빠지면 422(UNPROCESSABLE_ENTITY)로 거부합니다. 이제 프론트엔드 `canSave` 가드 없이도 백엔드에서 완전성을 보장합니다.

---

## 포기 처리 (`teacher_abandon_application`)

`PUT /api/teacher/applications/:sid/:tid/:rid/abandon`

**허용 상태**: FINALIZED 라운드에서만 가능. OPEN/CLOSED이면 400.

**학생 소속 검증**: 일반 담임은 해당 학생이 자신의 grade/class_no 소속인지, 졸업생 담당은 `is_enrolled=0`인지 검증.

**처리**: `UPDATE applications SET abandoned = 1`

**포기 후 재지원 가능 여부**: FINALIZED 라운드에서는 지원 등록(`teacher_create_application`)이 라운드 상태 검증에서 400을 반환하므로 재지원 불가.

⚠️ [teacher_delete_application] 담임이 지원을 완전히 삭제하는 API(`DELETE /api/teacher/applications/:sid/:tid/:rid`)는 OPEN 상태에서만 가능하며, `results` 테이블도 함께 삭제한다. 이는 포기(abandoned=1)와 구별된다. 포기는 결과는 남기되 추천 대상에서 제외하는 것이고, 삭제는 지원 자체를 없애는 것이다.

**학생 소속 검증**: `teacher_create_application`·`teacher_abandon_application`과 동일하게, 졸업생 담당(`grade=0, class_no=0`)은 `is_enrolled=0` 조건으로, 일반 담임은 `grade/class_no` 조건으로 검증한다.