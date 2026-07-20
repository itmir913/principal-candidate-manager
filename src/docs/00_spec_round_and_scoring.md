# 00. 라운드·성적·추천·마감 통합 명세

> **이 문서가 흡수한 파일**:  
> `02_round_lifecycle.md`, `03_score_calculation.md`, `04_ranking_sort.md`,  
> `06_candidate_confirm.md`, `11_state_matrix.md`  
> 흡수 이유: 주제가 완전히 겹치고, 2026-07-19 이후 용어·설계 변경이 각 파일에 불균일하게 반영되어 있어 단일 진실 원천이 필요하다.

---

## 용어 주의사항

UI·문서에서 사용하는 **"미선발"**은 DB 컬럼 `applications.excluded`, API 경로 `/exclude`에 대응한다.  
코드 식별자를 인용할 때는 실제 이름(`excluded`)을 그대로 쓴다.

**"포기"**(`abandoned`)는 FINALIZED 상태에서 학생/담임이 자발적으로 신청하는 것으로 미선발과 별개다.  
두 용어의 정원 집계 영향이 서로 다르다 — §6에서 상세 설명.

---

## 인용 규약

이 문서의 가치는 **코드와 대조 가능한 근거**에 있으므로, 인용이 깨지면 문서가 무의미해진다.

- **Rust 코드는 `파일.rs::심볼` 로 인용한다** (예: `scoring.rs::finalize_round`).  
  **행 번호를 쓰지 말 것.** 2026-07-19 `track_rank_window()` 추출 리팩터링이 파일 상단에 20줄을
  삽입하면서 이 문서의 행 번호 인용 30여 개가 한꺼번에 무효화된 적이 있다. 심볼 이름은 코드가
  위아래로 밀려도 유효하고 grep·IDE 점프로 바로 찾을 수 있다.
- **SQL 마이그레이션은 행 번호를 써도 된다.** 조각 파일이 append-only로만 자라고, 인용할 때
  트리거·인덱스 **이름을 함께 적어** 번호가 밀려도 대상을 특정할 수 있기 때문이다.
- `src/score.rs` 처럼 trait impl 위주라 심볼 이름이 오히려 모호한 파일은 행 번호를 유지한다.

---

## 1. 라운드 생명주기

### 1.1 상태 정의와 전이 경로

```
         ┌──────────────┐
         │     OPEN     │◄──────── reopen (CLOSED → OPEN)
         └──────┬───────┘
                │ close
                ▼
         ┌──────────────┐
         │    CLOSED    │
         └──────┬───────┘
                │ finalize
                ▼
         ┌──────────────┐
         │  FINALIZED   │  (비가역)
         └──────────────┘
```

FINALIZED는 비가역이다. `trg_prevent_update_finalized_result` 트리거(`migrations/v1/009-results.sql:27`)가  
FINALIZED 라운드의 `results` 행 수정을 DB 수준에서 차단해, 핸들러 우회(직접 SQL)로도 되돌릴 수 없다.

### 1.2 상태×행위 매트릭스

| 엔드포인트 | 라운드 없음 | OPEN | CLOSED | FINALIZED |
|---|---|---|---|---|
| `POST /rounds/open` | **201** | 409 | 409 | **201** |
| `PUT /rounds/:id/close` | 404 | **200** | 404 | 404 |
| `PUT /rounds/:id/reopen` | 404 | 404 | **204** | 404 |
| `PUT /rounds/:id/finalize` | 404 | 404 | **204**¹ | 404 |
| `POST /rounds/:id/calculate` | 404 | 400 | **200** | 400 |
| `POST /rounds/:id/auto-recommend` | 404 | 400 | **200** | 400 |
| `POST /rounds/:id/auto-recommend/univ/:univ_id` | 404 | 400 | **200** | 400 |
| `PUT /results/:sid/:tid/:rid/recommend` | 404 | 400 | **204**² | 400 |
| `PUT /results/:sid/:tid/:rid/unrecommend` | 404 | 400 | **204** | 400 |
| `PUT /applications/:sid/:tid/:rid/abandon` (관리자) | 404 | 400 | 400 | **204**³ |
| `PUT /applications/:sid/:tid/:rid/exclude` | 404 | 400 | **204**⁴ | 400 |
| `DELETE /applications/:sid/:tid/:rid/exclude` | 404 | 400 | **204** | 400 |
| `POST /teacher/applications` | 404 | **201** | 400 | 400 |
| `DELETE /teacher/applications/:sid/:tid/:rid` | 404 | **204** | 400 | 400 |
| `PUT /teacher/applications/:sid/:tid/:rid/abandon` | 404 | 400 | 400 | **204**³ |

¹ 전건 결정 완료(미결정 없음) + 정원 이내일 때 204. 미결정 있으면 422 + 전원 명단. 정원 초과 있으면 422 + 위반 목록. 미결정 검증이 정원 검증보다 먼저다.  
² 정원 찼으면 409. 미선발 처리됐으면 409. 같은 모집단위 상위 미결정자 있으면 409. results 없으면 404.  
³ 지원 내역 없으면 404. 담임은 담당 학급 아니면 403.  
⁴ 이미 추천 확정된 지원이면 409. 이미 미선발 상태이면 409. 사유 없으면 400.

**거부 셀 불변식**: 4xx 거부 시 `rounds`·`applications`·`results` 세 테이블은 단 한 행도 변하지 않는다.  
`tests/state_matrix.rs`가 이 표의 각 셀을 실행으로 고정한다.

**DB 방어선 6종**:

| 방어선 | 위치 | 차단 대상 |
|---|---|---|
| `idx_one_active_round` | `003-rounds.sql:19` | 비-FINALIZED 라운드 2개 이상 INSERT |
| `trg_require_all_decided_before_finalize` | `003-rounds.sql:26` | 미결정 지원 존재 시 CLOSED→FINALIZED 전환 |
| `trg_prevent_update_finalized_result` | `009-results.sql:27` | FINALIZED 라운드 results UPDATE |
| `trg_prevent_delete_closed_result` | `009-results.sql:36` | CLOSED/FINALIZED 라운드 results DELETE |
| `trg_prevent_exclude_recommended` | `008-applications.sql:76` | 추천 확정 행에 excluded=1 설정 |
| `trg_prevent_delete_closed_application` | `008-applications.sql:23` | CLOSED/FINALIZED 라운드 applications DELETE |
| `trg_prevent_update_closed_application` | `008-applications.sql:32` | CLOSED 라운드: excluded/excluded_reason 외 수정. FINALIZED: abandoned 0→1 외 수정 |

### 1.3 라운드 열기 (`POST /rounds/open`)

`INSERT ... SELECT ... WHERE NOT EXISTS (...) RETURNING id` 패턴으로 "진행 중(OPEN 또는 CLOSED) 라운드 없음"  
확인과 삽입을 원자적으로 처리한다 (`src/handlers/rounds.rs::open_round`). RETURNING id가 None이면 409.  
FINALIZED 라운드만 있으면 허용 — FINALIZED 라운드는 "진행 중"이 아니다.

DB 방어선: `idx_one_active_round`(`003-rounds.sql:19`) — `status != 'FINALIZED'` 조건 부분 유니크 인덱스.  
핸들러 우회 경로에서도 OPEN·CLOSED 라운드 동시 2개를 차단한다.

### 1.4 라운드 종료 (`PUT /rounds/:id/close`)

**전체 흐름이 `BEGIN IMMEDIATE` 단일 트랜잭션** (`rounds.rs::close_round`).  
오류 경로에서 tx drop 시 자동 ROLLBACK — 라운드는 OPEN으로 복귀.  
"CLOSED + 결과 없음" 불일치 상태가 구조적으로 불가능하다.

1. **기초데이터 누락 사전 검증** (`rounds.rs::close_round`): 모든 지원자×전형요소 조합에 대해 base_data 존재 확인.  
   COMPOSITE 전형요소는 `track_id = ap.track_id`, SIMPLE은 `track_id IS NULL` 조건.  
   누락 1건이라도 있으면 422 + 최대 5건 안내, ROLLBACK.
2. **상태 변경** (`rounds.rs::close_round`):  
   `UPDATE rounds SET status='CLOSED', closed_at=? WHERE id=? AND status='OPEN'`.  
   rows_affected=0이면 404.
3. **점수 계산** (`rounds.rs::close_round`): `run_calculate_scores_on_conn` 호출.  
   실패 시 ROLLBACK → OPEN 복귀. 성공 시 COMMIT.

`BEGIN IMMEDIATE`를 쓰는 이유: 검증 구간 동안 다른 커넥션의 쓰기(base_data import 등)를 차단해  
검증 후 계산 직전에 데이터가 바뀌는 race condition을 방지한다.

### 1.5 라운드 재개 (`PUT /rounds/:id/reopen`)

단일 트랜잭션 (`rounds.rs::reopen_round`):

1. `UPDATE rounds SET status='OPEN', closed_at=NULL WHERE id=? AND status='CLOSED'` — CLOSED 라운드만 허용.
2. `UPDATE results SET recommended=0, ranking=NULL WHERE round_id=?` — 추천 플래그·순위 초기화.

초기화 이유: 재개 후 기초데이터 변경 → 재계산 시 순위가 달라진다. stale 추천·순위가 잔존하면  
관리자에게 잘못된 정보가 표시되므로 모두 지운다. 점수(`total_score`, `score_detail`)는 남겨두어  
재계산 시 덮어쓴다.

`excluded`(미선발) 상태도 동일하게 초기화한다: `UPDATE applications SET excluded=0, excluded_reason=NULL WHERE round_id=?`.  
rounds.status를 OPEN으로 먼저 변경한 후 이 UPDATE를 실행하므로 `trg_prevent_update_closed_application` 트리거가 비활성 상태다.

### 1.6 라운드 확정 (`PUT /rounds/:id/finalize`)

**`BEGIN IMMEDIATE` 단일 트랜잭션** (`rounds.rs::finalize_round`):

1. `SELECT status` → CLOSED가 아니면 404 (`rounds.rs::finalize_round`).
2. **미결정 검증** (`rounds.rs::finalize_round`):  
   `excluded=0 AND COALESCE(r.recommended, 0)=0`. results 행 없음(LEFT JOIN null)도 미결정에 포함.  
   미결정 1건이라도 있으면 422 + 전원 명단(LIMIT 없음). 정원 검증보다 먼저 수행.
3. **모집단위 정원 초과 검증** (`rounds.rs::finalize_round`):  
   `unit_quota IS NOT NULL`인 트랙 중 전 라운드 누적 `recommended=1 AND abandoned=0` > `unit_quota`.  
   최대 5건.
4. **대학 정원 초과 검증** (`rounds.rs::finalize_round`):  
   `total_quota IS NOT NULL`인 대학 중 전 라운드 누적 합산 > `total_quota`. 최대 5건.
5. 위반 있으면 422 + `{"error":..., "track_violations":[...], "univ_violations":[...]}` JSON.
6. `UPDATE rounds SET status='FINALIZED', finalized_at=? WHERE id=? AND status='CLOSED'` (`rounds.rs::finalize_round`).

DB 방어선: `trg_require_all_decided_before_finalize`(`003-rounds.sql:26`) —  
핸들러 우회 직접 SQL에서도 미결정이 있으면 ABORT.

---

## 2. 성적(총점) 계산

### 2.1 ×100000 정수 체계와 Score newtype

모든 점수는 DB에 **×100000 정수**로 저장한다. `Score(i64)` newtype이 이 불변식을 컴파일 타임에 강제한다 (`src/score.rs`):

- **DB 저장**: `sqlx::Encode` → `i64` 그대로 SQLite INTEGER (`score.rs:53`).
- **DB 조회**: `sqlx::Decode` → `i64`를 `Score(i64)`로 래핑 (`score.rs:45`).
- **JSON 직렬화**: `self.0 as f64 / 100_000.0` — 예: 내부값 `3050000` → JSON `30.5` (`score.rs:18`).
- **JSON 역직렬화**: `(f * 100_000.0).round() as i64` — 예: JSON `30.5` → `3050000` (`score.rs:35`).  
  비유한 값·±10억 초과는 즉시 오류 (`score.rs:29, 33`).

`Score: Ord`가 구현되어 정렬·비교 시 부동소수점 오차 없이 정수 비교한다 (`score.rs:3`).  
`Score: Add + Sum`은 내부적으로 `checked_add` — overflow 시 panic (Fail-Fast, `score.rs:65, 71`).

프론트엔드는 백엔드가 반환한 JSON 값을 그대로 표시한다. ÷100000 직접 계산 금지.

### 2.2 진입점

| 경로 | 진입 함수 | 트랜잭션 소유자 |
|------|-----------|----------------|
| `close_round` | `run_calculate_scores_on_conn` | close_round의 BEGIN IMMEDIATE (`rounds.rs::close_round`) |
| `POST /rounds/:id/calculate` | `calculate_scores` → `run_calculate_scores_on_conn` | calculate_scores 내부 BEGIN IMMEDIATE (`scoring.rs::calculate_scores`) |
| `POST /teacher/applications` | `calc_area_score` (전형요소별) | teacher_create_application의 BEGIN IMMEDIATE |

핵심 로직은 `run_calculate_scores_on_conn(&mut SqliteConnection, round_id, now)` (`scoring.rs::run_calculate_scores_on_conn`)에 집중.  
트랜잭션 관리는 호출자 책임.

### 2.3 CalcType별 계산 로직

> **공용 헬퍼 `scoring.rs::compute_area_score`**
> CalcType별 점수 계산 규칙·점수표 폴백·오버플로 감지는 이 헬퍼 하나에만 있다.
> 확정 계산(`calc_area_score`)과 담임 미리보기(`teacher_area_score_preview`)는
> 각자 값을 정규화(`AreaScoreInput::{Numeric,Category,Manual}`)한 뒤 반드시 이 헬퍼를 통과한다.
> 두 경로가 시맨틱이 갈리는 부류의 버그(예: 미리보기의 whole-map 폴백 vs 확정의 per-category 폴백)를
> 컴파일 타임에 차단하기 위한 구조.
> 등가성은 `tests/handler_teacher_area.rs::preview_and_confirmed_produce_identical_score_category_composite`로 고정.

#### NUMERIC (구간 점수) — `scoring.rs::compute_area_score` (AreaScoreInput::Numeric)

1. `base_data`에서 ×100000 정수 문자열 조회 → `parse::<i64>()`. 없거나 파싱 실패 시 오류.
2. `numeric_table`에서 `threshold` 오름차순 구간표 조회.  
   COMPOSITE이고 모집단위별 구간표가 없으면 공통(`track_id IS NULL`)으로 폴백 (`scoring.rs::calc_area_score`).
3. `lookup_range_score` (`scoring.rs::lookup_range_score`):
   - **UPPER**: `value >= threshold`인 행 중 최대 threshold 행의 점수. 모든 threshold보다 작으면 오류 (Fail-Fast).
   - **LOWER**: `value <= threshold`인 행 중 최소 threshold 행의 점수. value > 최대 threshold이면 최대 threshold 행 사용(오류 아님 — `scoring.rs::lookup_range_score`).
   - **EXACT**: `threshold == value`인 행. 없으면 오류.
4. `raw.min(area.max_score)` — 상한 적용 (`scoring.rs::calc_area_score`).

#### CATEGORY (범주 점수) — `scoring.rs::compute_area_score` (AreaScoreInput::Category)

1. `base_data`에서 범주 문자열 복수 행 조회. 0건이면 오류.
2. 각 범주를 `category_map`에서 조회. 없으면 오류 (0점 silent fallback 금지, `scoring.rs::calc_area_score`).  
   COMPOSITE 폴백: 모집단위별 없으면 공통 테이블 (`scoring.rs::calc_area_score`).
3. `category_agg`에 따라 집계:
   - **SUM**: `try_fold + checked_add` — overflow 시 Fail-Fast (`scoring.rs::calc_area_score`).
   - **MAX**: `scores.iter().max()`. 비어있지 않음이 1번에서 보장 (`scoring.rs::calc_area_score`).
4. `max_score` 상한 적용.

**CATEGORY 0점 처리**: `category_map` 설계 단계에서 "해당 없음" 범주를 score=0으로 등록하는 방식.  
`category_map_import` 시 양수 점수 있는 그룹에 score=0 행이 없으면 import 거부(`08_excel_import.md` §4).

#### MANUAL (수동 입력) — `scoring.rs::compute_area_score` (AreaScoreInput::Manual)

1. `base_data`에서 ×100000 정수 문자열 1개 조회. 없으면 오류.
2. `parse::<i64>()` 후 그대로 점수로 사용.
3. `max_score` 상한 적용. 하한 없음(음수 허용 — 감점 설계).

### 2.4 전체 점수 합산 및 results 저장

각 지원자에 대해 전 전형요소 순회 후 `checked_add`로 합산(`scoring.rs::run_calculate_scores_on_conn`). overflow 시 Fail-Fast.

results 저장 패턴 (`scoring.rs::run_calculate_scores_on_conn`):
```sql
INSERT INTO results (student_id, track_id, round_id, score_detail, total_score, ranking, recommended, calculated_at)
VALUES (?, ?, ?, ?, ?, NULL, 0, ?)
ON CONFLICT (student_id, track_id, round_id)
DO UPDATE SET score_detail=excluded.score_detail,
              total_score=excluded.total_score,
              ranking=NULL,
              calculated_at=excluded.calculated_at
```

- **ranking**: NULL로 초기화 후 순위 루프에서 채운다.
- **recommended**: 갱신하지 않음 — 재계산해도 기존 추천 상태 보존.

### 2.5 실패 처리

`run_calculate_scores_on_conn`이 `Err`를 반환하면 호출자의 tx가 drop되어 자동 ROLLBACK.

- `close_round`에서 호출 시: ROLLBACK → 라운드 OPEN 복귀.
- `calculate_scores`에서 호출 시: ROLLBACK → CLOSED 상태 유지, results 변경 없음.

어떤 입력이 실패를 유발하는가:

| 시나리오 | 발생 조건 |
|---------|-----------|
| base_data 없음 | NUMERIC/MANUAL/CATEGORY 모두 |
| NUMERIC UPPER 매칭 실패 | 학생 값이 모든 threshold보다 낮음 |
| NUMERIC EXACT 매칭 실패 | 일치하는 threshold 없음 |
| CATEGORY 범주 매핑 실패 | category_map에 없는 범주값 |

---

## 3. 순위 산출

### 3.1 대학 전체 순위 (`results.ranking`)

`results.ranking`은 **대학(university) 단위 파티션**으로 계산된다 (`scoring.rs::run_calculate_scores_on_conn`).

정렬 키:
- `universities.prioritize_enrolled=1`이면: `is_enrolled DESC → total_score DESC`
- `=0`이면: `total_score DESC`

**대학 단위 순위는 `universities.prioritize_enrolled`만 참조한다** (`scoring.rs::run_calculate_scores_on_conn`).  
모집단위의 `univ_tracks.prioritize_enrolled`는 여기서 사용되지 않는다.

### 3.2 모집단위 순위 (`track_rank`)

`track_rank`는 `results` 테이블에 저장되지 않고 **조회 시 파생**된다. 파생 식의 정의는
`track_rank_window()` 헬퍼 한 곳에만 존재한다 — SQL 본문은 §3.3 참조 (여기에 다시 옮겨
적지 않는다. 두 곳에 쓰면 갈라진다).

**모집단위 순위는 `univ_tracks.prioritize_enrolled`만 참조한다**.  
대학의 `universities.prioritize_enrolled`는 여기서 사용되지 않는다.

저장하지 않는 이유: 순위는 동적으로 파생되는 값으로 관리자 수동 조작·재계산 시 자동으로 최신화되어야 한다. 별도 컬럼에 저장하면 점수 변경 시 순위를 함께 갱신하는 동기화 부담이 생긴다.

### 3.3 모집단위 순위 윈도우 함수 — 단일 출처

**모집단위 순위 파생 식은 `track_rank_window()` 헬퍼 하나로 통일되어 있다** (`scoring.rs`).  
이전에는 7곳에 동일한 `CAST(RANK() OVER (...) AS INTEGER) AS track_rank` 리터럴이 중복 존재했다.  
헬퍼가 없으면 한 곳만 수정했을 때 나머지가 달라져 가드·화면·자동 추천이 서로 다른 순위를 쓰는 상태가 된다.

```rust
fn track_rank_window(r: &'static str, ut: &'static str, s: &'static str,
                     partition_by_round: bool) -> String
```

생성되는 SQL:

```sql
CAST(RANK() OVER (
    PARTITION BY {r}.track_id[, {r}.round_id]
    ORDER BY
        CASE WHEN {ut}.prioritize_enrolled = 1 THEN {s}.is_enrolled ELSE NULL END DESC NULLS LAST,
        {r}.total_score DESC
) AS INTEGER) AS track_rank
```

| 파라미터 | 의미 |
|---------|------|
| `r`, `ut`, `s` | SQL 테이블 별칭 (results, univ_tracks, students). 감싸는 쿼리의 FROM·JOIN과 일치해야 하며, 어긋나면 런타임 `no such column` 으로 깨진다. `&'static str` 이므로 호출부는 리터럴만 넘길 수 있다(SQL 조립이라 런타임 값은 인젝션 경로) |
| `partition_by_round=true` | `PARTITION BY {r}.track_id, {r}.round_id` — **여러 라운드를 걸치는 쿼리** |
| `partition_by_round=false` | `PARTITION BY {r}.track_id` — `WHERE round_id = ?` 로 **단일 라운드가 고정된 쿼리** |

판단 기준은 **여러 라운드를 걸치는가** 하나뿐이다. CTE인지 인라인인지와는 무관하다 —
아래 표에서 보듯 두 형태가 `true`/`false` 양쪽에 모두 존재한다.

**사용 위치 7곳**:

| 핸들러 | 용도 | 쿼리 형태 | 라운드 범위 | partition_by_round |
|--------|------|----------|-----------|-------------------|
| `get_results` | 화면 표시 | 인라인 | 단일(`WHERE r.round_id = ?`) | true¹ |
| `export_results` | 엑셀 내보내기 | 인라인 | 단일(`WHERE r.round_id = ?`) | true¹ |
| `export_round_summary` | 요약 내보내기 | CTE | **다중**(CTE에 라운드 필터 없음) | true |
| `teacher_get_results` (졸업생 분기) | 담임 결과 조회 | CTE | **다중**(FINALIZED 전체) | true |
| `teacher_get_results` (재학생 분기) | 담임 결과 조회 | CTE | **다중**(FINALIZED 전체) | true |
| `recommend_result` blocker 쿼리 | 수동 추천 트랙 순서 가드 | CTE | 단일 | false |
| `run_auto_recommend` 3c 단계 | 자동 추천 1단계 후보 순위 | 인라인 | 단일 | false |

¹ 단일 라운드이므로 `round_id` 파티션은 결과에 영향이 없다(무해한 잉여). 리팩터링 이전
리터럴이 그러했고, 동작 변경을 피하기 위해 그대로 보존했다.

어긋나면: 관리자가 화면에서 보는 순위와 가드·자동 추천이 비교하는 순위가 달라져  
가드가 잘못된 후보를 막거나 허용하고, 자동 추천이 다른 결과를 낸다.

### 3.4 동점 처리 — Standard Competition Ranking

같은 순위 값을 가진 학생은 같은 순위를 부여하고 다음 순위를 건너뛴다 (1, 1, 3, 4, …) (`scoring.rs::run_calculate_scores_on_conn`).

동점 판정 조건 (`scoring.rs::run_calculate_scores_on_conn`):
- `prioritize=true`: `(total_score, is_enrolled)` 둘 다 동일해야 동점.
- `prioritize=false`: `total_score`만 비교.

예: 점수가 100, 100, 90, 80이면 순위는 1, 1, 3, 4.

### 3.5 `prioritize_enrolled` — 각 범위는 자기 플래그만 쓴다

| 순위 종류 | 참조 플래그 | OR 금지 이유 |
|---------|------------|------------|
| `results.ranking` (대학 전체) | `universities.prioritize_enrolled` | 대학 단위 순위이므로 대학 설정 |
| `track_rank` (모집단위, 파생) | `univ_tracks.prioritize_enrolled` | 모집단위 단위 순위이므로 모집단위 설정 |

OR(`u.prioritize_enrolled=1 OR ut.prioritize_enrolled=1`) 사용 금지.  
불변식 "대학=1이면 그 대학의 모든 트랙=1"은 트리거가 강제하므로, 구분해서 써도 대학=1 구성에서  
두 순위가 일치한다. 단, "대학=0, 특정 트랙=1"(그 모집단위만 재학생 우선) 구성도 유효하다.

---

## 4. 추천 확정 (수동)

### 4.1 `recommend_result` — 전체 검증 절차

`PUT /results/:sid/:tid/:rid/recommend`, `BEGIN IMMEDIATE` (`scoring.rs::recommend_result`).

| 단계 | 검사 내용 | 실패 응답 |
|------|----------|----------|
| 1. 라운드 상태 | `status == CLOSED` | 400 (OPEN/FINALIZED), 404 (없음) |
| 1b. 미선발 체크 | `excluded != 1` | 409 "미선발 처리된 지원" (`scoring.rs::recommend_result`) |
| 2. 모집단위 정원 | 전 라운드 `recommended=1 AND abandoned=0` 합산 < `unit_quota` | 409 "정원 찼음" (`scoring.rs::recommend_result`) |
| 3. 대학 정원 | 전 라운드 동일 집계 < `total_quota` | 409 "대학 전체 정원 찼음" (`scoring.rs::recommend_result`) |
| 4. 트랙 순서 가드 | 같은 모집단위 내 `track_rank` 상위이면서 미결정(미추천·미포기·미선발)인 학생 없음 | 409 "상위 순위 지원자 미결정" (`scoring.rs::recommend_result`) |
| 5. results 갱신 | `UPDATE results SET recommended=1` | rows_affected=0이면 404 (결과 행 없음) |

**정원 집계 기준**: `recommended=1 AND abandoned=0`, 전 라운드 누적 (`scoring.rs::recommend_result`).  
`excluded` 상태는 집계에 영향 없다. 이전 라운드에서 추천된 인원도 포함된다.

**unit_quota / total_quota가 NULL이면 무제한** — 정원 체크를 건너뜀 (`scoring.rs::recommend_result`).

### 4.2 트랙 순서 가드 상세 (`scoring.rs::recommend_result`)

blocker 쿼리 조건:
```sql
WHERE k.recommended = 0 AND a.abandoned = 0 AND a.excluded = 0
  AND k.track_rank < (SELECT track_rank FROM ranked WHERE student_id = ?)
```

- **`<` 조건**: 동점자(track_rank 동일)는 서로 막지 않는다.  
  관리자가 동점자 중 먼저 추천할 대상을 선택할 여지를 남긴다.
- abandoned=0, excluded=0: 포기·미선발은 "상위자"로 세지 않는다.

### 4.3 추천 취소 (`unrecommend_result`)

`PUT /results/:sid/:tid/:rid/unrecommend`. 일반 트랜잭션 (`scoring.rs::unrecommend_result`).

- CLOSED 상태만 허용. FINALIZED이면 400.
- `UPDATE results SET recommended=0`. ranking 변경 없음.
- **역방향 순위 가드 없음**: 1위가 취소되어도 2위가 추천 상태인 채로 남는다 — §알려진 미결 사항 1 참조.

### 4.4 TOCTOU 방지

`recommend_result`가 `BEGIN IMMEDIATE`를 쓰는 이유: 정원 COUNT(SELECT)와 `recommended=1` 설정(UPDATE)  
사이에 다른 커넥션이 끼어들어 "COUNT=0 읽고 둘 다 통과 → 정원 초과"가 발생하는 race를 차단한다.

`exclude_application`도 `BEGIN IMMEDIATE`: `recommended` 상태 조회 후 `excluded=1` 설정까지  
원자적으로 처리해 `recommend_result`와 race 시 모순 상태(recommended=1 AND excluded=1)를 방지한다.

DB 방어선 `trg_prevent_exclude_recommended`(`applications.sql:76`): 추천 확정 후 동일 지원을  
미선발 처리하는 경로를 DB 수준에서도 차단한다.

---

## 5. 자동 추천

### 5.1 개요

`POST /rounds/:id/auto-recommend` — 모든 대학.  
`POST /rounds/:id/auto-recommend/univ/:id` — 지정 대학만.  
모두 `run_auto_recommend` 호출 (`scoring.rs::run_auto_recommend`). CLOSED 상태 필수.

**2-phase 구조**:

1. **1단계 (트랙 채움)**: 각 모집단위를 **모집단위 순위**(`univ_tracks.prioritize_enrolled` 기준)로 정원까지 채움.
2. **2단계 (대학 컷)**: 1단계 확정분을 **대학 전체 순위**(`results.ranking`, `universities.prioritize_enrolled` 기준)로 대학 잔여 정원까지 컷.

### 5.2 동점 그룹 원자적 채움 — `fill_by_rank_groups` / `decide_group`

`fill_by_rank_groups(items: &[(rank, T)], remaining: Option<i64>)` (`scoring.rs::fill_by_rank_groups`).  
`decide_group(confirmed_len, group_size, rem)` (`scoring.rs::decide_group`) — 4갈래 판정:

| 조건 | 판정 | 의미 |
|------|------|------|
| `confirmed_len + group_size ≤ rem` | `Take` | 그룹 전원 확정 후 다음 그룹 |
| `rem - confirmed_len == 0` (free=0) | `StopClean` | 정원이 그룹 사이에 딱 떨어짐. 수동 불필요 |
| `0 < rem - confirmed_len < group_size` | `StopTie{free}` | 동점이 정원을 가름. 그룹 전원 보류, manual |

같은 함수를 1단계(`fill_by_rank_groups`)와 2단계(`merge_univ_cut`)가 공유 (`scoring.rs::decide_group`) —  
두 경로의 경계 의미가 구조적으로 일치함을 보장한다.

### 5.3 후보 선별 기준 (`scoring.rs::run_auto_recommend`)

각 모집단위 전체 results 행 중 **`recommended=0 AND excluded=0`**인 지원만 후보다:

- 이미 추천된(`recommended=1`) 지원 제외 — 재추천 방지.
- 미선발된(`excluded=1`) 지원 제외 — 미선발 후보 자동 추천 방지.
- **순위 계산(`RANK()`)은 excluded 포함 전원으로 계산** (`scoring.rs::run_auto_recommend`):  
  화면(`get_results`)·수동 추천 가드와 동일한 순위를 유지하기 위함.

### 5.4 2단계 — 대학 정원 컷: 전체 재정렬이 아닌 이유 (`merge_univ_cut`, `scoring.rs::merge_univ_cut`)

단순히 1단계 결과를 평탄화해 대학 전체 순위로 재정렬하면 **트랙 정원이 의미를 잃는다**.

예: 트랙 A 정원=1, A1(90점)·A2(85점). 트랙 B에 B1(88점).  
1단계 결과: 트랙 A → [A1], 트랙 B → [B1].  
전체 재정렬: [A1(90), B1(88), A2(85)] — A2가 부활해 대학 전체 2위가 된다.  
이는 트랙 A 정원 초과로 탈락한 A2를 대학 컷에서 살리는 모순이다.

`merge_univ_cut`은 트랙별 1단계 결과를 평탄화하지 않고 유지한다.  
각 반복에서 **각 트랙의 선두(아직 미선택인 첫 후보)들만** 대학 순위로 경쟁한다.  
자기 트랙의 상위자에게 막힌 후보는 선두가 될 수 없어 경쟁 대상에서 제외된다.

### 5.5 숫자 예시

**설정**:
- 대학 정원(`total_quota`) = 3명, 이미 추천된 인원 없음
- **트랙 A** (`unit_quota`=2, `prioritize_enrolled`=1)
  - A1: total_score=90, is_enrolled=1 → track_rank=1, univ_rank=1
  - A2: total_score=90, is_enrolled=1 → track_rank=1, univ_rank=1 (A1과 동점)
  - A3: total_score=80, is_enrolled=0 → track_rank=3, univ_rank=5
- **트랙 B** (`unit_quota`=2, `prioritize_enrolled`=1)
  - B1: total_score=88, is_enrolled=1 → track_rank=1, univ_rank=2
  - B2: total_score=75, is_enrolled=1 → track_rank=2, univ_rank=4

**1단계 — 모집단위 정원 채움**

트랙 A (`remaining`=2):

| 그룹 | 크기 | confirmed + 크기 vs 정원 | 판정 |
|------|------|--------------------------|------|
| rank=1 (A1, A2) | 2 | 0 + 2 ≤ 2 | Take |
| rank=3 (A3) | 1 | 2 + 1 > 2, free=0 | StopClean |

`pool_A` = [A1(tr=1, ur=1), A2(tr=1, ur=1)]. A3 탈락 — 정원 소진 (수동 사유 아님).

트랙 B (`remaining`=2):

| 그룹 | 크기 | confirmed + 크기 vs 정원 | 판정 |
|------|------|--------------------------|------|
| rank=1 (B1) | 1 | 0 + 1 ≤ 2 | Take |
| rank=2 (B2) | 1 | 1 + 1 ≤ 2 | Take |

`pool_B` = [B1(tr=1, ur=2), B2(tr=2, ur=4)].

**2단계 — 대학 전체 정원 컷** (`remaining_univ` = 3 − 0 = 3)

`merge_univ_cut([[A1,A2], [B1,B2]], 3)`:

| 라운드 | 각 트랙 선두 | 최선 ur | 그룹 G | 크기 | 판정 |
|--------|------------|---------|-------|------|------|
| 1 | A1(ur=1), B1(ur=2) | 1 | A트랙: A1+A2 (track_rank=1 동점) | 2 | 0+2 ≤ 3 → Take |
| 2 | A 소진, B1(ur=2) | 2 | B트랙: B1 (B2는 track_rank=2 ≠ 1) | 1 | 2+1 ≤ 3 → Take |
| 3 | A 소진, B2(ur=4) | 4 | B2 | 1 | 3+1 > 3, free=0 → StopClean |

최종 확정: **[A1, A2, B1]** (3명 = 대학 정원). B2 탈락 — 정원 소진.

**만약 total_quota=1이었다면** (동점이 정원 경계를 가르는 경우):

- 라운드 1: A1+A2 (group_size=2), confirmed_len=0, 0+2=2 > 1, free=1 > 0  
  → **StopTie{rank=1, free=1, contenders=2}**
- A1, A2 모두 보류. `manual` 보고 — 관리자가 둘 중 1명을 선택해야 함. 최종 확정: [].

---

## 6. 미선발 처리

### 6.1 "미선발"과 "포기"의 차이

| 항목 | 미선발 (`excluded`) | 포기 (`abandoned`) |
|------|--------------------|--------------------|
| DB 컬럼 | `applications.excluded` | `applications.abandoned` |
| API 경로 | `PUT /applications/:sid/:tid/:rid/exclude` | `PUT /applications/:sid/:tid/:rid/abandon` |
| 허용 라운드 상태 | **CLOSED** | **FINALIZED** |
| 사유 필수 | 예 (DB CHECK 강제) | 아니오 |
| 정원 집계 영향 | **없음** — `recommended=1 AND abandoned=0` 기준이므로 excluded는 무관 | **있음** — `abandoned=0` 조건에서 제외 |
| 추천과 관계 | **상호배타** | 독립 — 추천된 학생도 포기 가능 |

### 6.2 사유 필수 — DB CHECK 강제

`migrations/v1/008-applications.sql:17`:
```sql
CHECK (excluded = 0 OR (excluded_reason IS NOT NULL AND TRIM(excluded_reason) <> ''))
```
사유 없이 `excluded=1`을 직접 INSERT/UPDATE하면 DB 레벨에서 ABORT.

앱 레벨에서도 `exclude_application` (`applications.rs::exclude_application`): `body.reason.trim().is_empty()` 시 400.

### 6.3 추천과 상호배타 — 양방향 차단

| 경로 | 앱 레벨 가드 | DB 레벨 가드 |
|------|-------------|-------------|
| 추천된 지원을 미선발하려는 경우 | `recommended=1`이면 409 (`applications.rs::exclude_application`) | `trg_prevent_exclude_recommended` (`applications.sql:76`) |
| 미선발된 지원을 추천하려는 경우 | `excluded=1`이면 409 (`scoring.rs::recommend_result`) | — |

### 6.4 미선발 처리 (`applications.rs::exclude_application`)

1. 사유 trim + 빈 문자열 거부 (400).
2. `BEGIN IMMEDIATE` 획득.
3. 라운드 CLOSED 확인 (`check_round_closed_for_exclusion`).
4. `excluded` 현재 상태 확인: 없으면 404, 이미 미선발이면 409.
5. 추천 확정 여부 확인: `recommended=1`이면 409.
6. `UPDATE applications SET excluded=1, excluded_reason=?`.

### 6.5 미선발의 의미 확장 (2026-07-19)

원래 "결격"만을 위한 기능이었으나, **정원 미달·행정적 판단 등 관리자의 명시적 "이번 라운드 미추천" 결정**  
전반에 사용하도록 의미가 확장됐다. 코드 식별자(`excluded`)는 유지.

---

## 7. 라운드 마감

### 7.1 사전 검증 순서

`finalize_round` (`rounds.rs::finalize_round`):

```
① 미결정 지원 확인 → ② 모집단위 정원 초과 확인 → ③ 대학 정원 초과 확인
```

①이 먼저인 이유: 미결정과 정원 초과가 동시에 존재할 때 미결정 명단을 먼저 보여줘  
관리자가 명단을 정리하도록 유도한다.

### 7.2 "미결정"의 정의

`excluded=0 AND COALESCE(r.recommended, 0)=0` (`rounds.rs::finalize_round`):

- `excluded=0`: 아직 "이번 라운드 미추천" 결정이 없는 지원
- `COALESCE(r.recommended, 0)=0`: results 행이 없거나(LEFT JOIN NULL) recommended=0

**results 행이 아예 없는 경우도 미결정**이다.  
이는 점수 계산(`close_round` 또는 `calculate_scores`) 전 상태를 의미한다.  
이 코드는 silent fallback이 아닌 "results 없음 = 미결정"이라는 의도된 3상태(추천/미선발/미결정) 판정이다.

### 7.3 422 응답 형태

미결정이 있으면:
```json
{
  "error": "추천 또는 제외가 결정되지 않은 지원자가 있어 라운드를 마감할 수 없습니다",
  "undecided": [
    {"student_code":"...", "student_name":"...", "grade":3, "class_no":2, "univ_name":"...", "track_name":"..."},
    ...
  ]
}
```
**LIMIT 없음** — 미결정 전원 명단 반환 (`rounds.rs::finalize_round`, LIMIT 없이 `fetch_all`).

정원 초과이면:
```json
{
  "error": "정원 초과로 라운드를 확정할 수 없습니다",
  "track_violations": [...],
  "univ_violations": [...]
}
```
정원 초과는 최대 5건씩 (`rounds.rs::finalize_round, 349` LIMIT 5).

### 7.4 DB 트리거 이중 방어와 그 이유

| 레이어 | 구현 위치 | 역할 |
|--------|----------|------|
| 앱 레벨 | `finalize_round` handler | 미결정 명단을 JSON으로 반환 — 관리자에게 누가 미결정인지 알림 |
| DB 레벨 | `trg_require_all_decided_before_finalize` (`003-rounds.sql:26`) | 직접 SQL 등 핸들러 우회 경로 차단 |

트리거만으로는 부족한 이유: 트리거는 `RAISE(ABORT, '문자열')` 만 반환할 수 있어  
어떤 지원자가 미결정인지 명단을 반환할 수 없다. 앱 레벨에서 명단을 구성해 반환해야 관리자가 조치를 취할 수 있다.

### 7.5 마감 후 허용/차단 행위

| 행위 | 가능 여부 | 근거 |
|------|---------|------|
| 추천 확정 / 취소 | **불가** | CLOSED 상태 필수 조건 |
| 미선발 처리 / 해제 | **불가** | CLOSED 상태 필수 조건 |
| 관리자 포기 처리 | **가능** | FINALIZED 상태 전용 |
| 담임 포기 처리 | **가능** | FINALIZED 상태 전용 |

---

## 8. 전체 흐름 (시간순)

```
[새 라운드 열기]
  POST /rounds/open
  └─ 진행 중 라운드 없음 원자적 확인 → OPEN 상태 생성

[담임 지원 접수 기간]
  POST /teacher/applications (담임)
  └─ OPEN 확인 → 학생 소속 확인 → base_data 저장 → applications upsert
     → 점수 계산(BEGIN IMMEDIATE) → results upsert

  DELETE /teacher/applications (담임)
  └─ OPEN 확인 → results 삭제 → applications 삭제

[기초데이터 / 점수 기준 준비]
  POST /areas/:id/base-data/import
  POST /areas/:id/numeric-table/import  ← CLOSED 라운드 없을 때만

[라운드 종료]
  PUT /rounds/:id/close
  └─ BEGIN IMMEDIATE
     ├─ base_data 누락 검증 → (실패) 422, OPEN 유지
     ├─ rounds.status='CLOSED'
     └─ run_calculate_scores_on_conn → (실패) ROLLBACK, OPEN 복귀
                                     → (성공) COMMIT

  ↕ [점수가 잘못됐으면]
  PUT /rounds/:id/reopen → 데이터 수정 → close 반복

[추천 확정 기간 (CLOSED)]
  PUT /results/:sid/:tid/:rid/recommend (수동 추천)
  └─ BEGIN IMMEDIATE → 라운드/미선발/정원/순서 가드 → recommended=1

  POST /rounds/:id/auto-recommend (자동 추천)
  └─ BEGIN IMMEDIATE → 2-phase fill → recommended=1 일괄

  PUT /applications/:sid/:tid/:rid/exclude (미선발)
  └─ BEGIN IMMEDIATE → 추천 여부 확인 → excluded=1

  PUT /results/:sid/:tid/:rid/unrecommend (추천 취소)
  DELETE /applications/:sid/:tid/:rid/exclude (미선발 해제)

[라운드 마감]
  PUT /rounds/:id/finalize
  └─ BEGIN IMMEDIATE
     ├─ ① 미결정 검증 → (있으면) 422 + 전원 명단
     ├─ ② 모집단위 정원 초과 → (있으면) 422 + 위반 목록
     ├─ ③ 대학 정원 초과 → (있으면) 422 + 위반 목록
     └─ rounds.status='FINALIZED' → COMMIT (비가역)

[마감 후 포기 처리]
  PUT /applications/:sid/:tid/:rid/abandon (관리자/담임)
  └─ FINALIZED 확인 → abandoned=1
     → 정원 계산에서 자동 제외 (abandoned=0 필터)
```

---

## 처분 표 (src/docs 13개 파일)

| 파일 | 이번 주제 겹침 | 코드 일치 | 처리 결과 |
|------|-------------|----------|----------|
| `01_auth.md` | 아니오 | 예 | **유지** |
| `02_round_lifecycle.md` | **예** | 부분 불일치 (용어·미선발 엔드포인트 누락) | **통합 후 삭제** |
| `03_score_calculation.md` | **예** | 예 | **통합 후 삭제** |
| `04_ranking_sort.md` | **예** | 부분 불일치 (recommend 검증이 구버전) | **통합 후 삭제** |
| `05_homeroom_flow.md` | 아니오 | 예 | **유지** |
| `06_candidate_confirm.md` | **예** | **불일치** (1b·4 단계 누락) | **통합 후 삭제** |
| `07_db_transactions.md` | 아니오 | 부분 불일치 (신규 트리거 3종 미언급) | **유지 + 트리거 추가** |
| `08_excel_import.md` | 아니오 | 예 | **유지** |
| `09_api_endpoints.md` | 아니오 | **불일치** (finalize 미결정 단계 누락, /exclude 엔드포인트 누락) | **유지 + 수정** |
| `10_frontend_backend_contract.md` | 아니오 | 부분 불일치 (ResultRow에 excluded 필드 누락) | **유지 + 수정** |
| `11_state_matrix.md` | **예** | 부분 불일치 (recommend 거부 조건 불완전) | **통합 후 삭제 (행렬 §1.2에 보존)** |
| `silent_fallback_allowed.md` | 아니오 | 예 | **유지** |
| `2026-06-01-score-audit.md` | 아니오 | 구버전 SQL 참조 (C-1이 현재 코드와 불일치) | **삭제** |

---

## 확인 필요

1. **`trg_prevent_delete_closed_application` FINALIZED 라운드 포함** (`applications.sql:23`):  
   트리거 조건이 `IN ('CLOSED', 'FINALIZED')`로 FINALIZED 라운드 applications 삭제도 차단한다.  
   현재 FINALIZED 라운드에서 applications를 삭제하는 정상 경로가 없다고 가정한 것으로 보인다.  
   향후 "과거 라운드 데이터 정리" 기능을 추가할 경우 이 트리거가 막는다. 의도하신 것이 맞습니까?

2. **정원 초과 검증의 LIMIT 5** (`rounds.rs::finalize_round, 349`):  
   미결정 검증은 LIMIT 없이 전원을 반환하지만, 정원 초과 검증은 LIMIT 5를 사용한다.  
   정원 초과 건이 6건 이상 있을 경우 앞 5건만 표시된다. 의도된 것입니까?

4. **자동 추천 재실행 시 이전 `manual` 항목 처리**:  
   동점을 수동으로 처리한 뒤 자동 추천을 재실행하면 `confirmed` 목록에 이전 실행분이 포함되지 않아  
   "이번 실행에서 추가로 확정된 것"만 보인다. 이 동작이 관리자에게 명확히 안내되고 있습니까?

---

## 문서-코드 불일치 (이번 작업에서 발견, 아래 처리됨)

| 파일 | 불일치 내용 | 처리 |
|------|-----------|------|
| `09_api_endpoints.md` | `finalize_round` 상세에 미결정 사전 검증 단계(①) 누락 | 수정 완료 |
| `09_api_endpoints.md` | `지원 관리` 섹션에 `/applications/:sid/:tid/:rid/exclude` 엔드포인트(PUT/DELETE) 없음 | 추가 완료 |
| `10_frontend_backend_contract.md` | `ResultRow` 정의에 `excluded`, `excluded_reason` 필드 없음 | 수정 완료 |
| `07_db_transactions.md` | 신규 트리거 3종(`trg_prevent_delete_closed_application`, `trg_prevent_update_closed_application`, `trg_prevent_exclude_recommended`) 미언급 | 추가 완료 |

---

## 설계 결정 사항 (미결 아님)

1. **`unrecommend_result` 역방향 순위 가드 없음** (`scoring.rs::unrecommend_result`):  
   의도된 동작이다. 1위가 추천 취소된 후 2위만 추천 상태로 남더라도, 1위가 미선발 처리되지 않으면 미결정으로 남아 `finalize_round`에서 차단된다. 1위를 미선발 처리하면 2위만 추천된 상태로 마감이 허용된다. 즉 관리자가 명시적으로 1위를 미선발 처리해야 2위 단독 추천이 적법해지며, 이 책임을 시스템이 강제한다.

2. **재개(`reopen`) 후 excluded 초기화**:  
   `reopen_round`는 `recommended=0`, `ranking=NULL`과 함께 `excluded=0`, `excluded_reason=NULL`도 초기화한다. 추천 상태와 미선발 상태는 같은 라운드 맥락에 종속되므로 재개 시 함께 리셋한다 (`rounds.rs::reopen_round-226`).

3. **CLOSED/FINALIZED 라운드 존재 시 전형요소 이름 수정 차단**:  
   `guard_no_closed_round`가 `update_area`에 적용되어 이름 오탈자 수정도 막힌다. 이는 의도된 동작이다. 과거 라운드의 결과를 내보낼 때 점수 기준 명칭이 바뀌면 감사 추적이 불가능해지므로, 이름 포함 모든 전형요소 속성 변경을 차단한다. 다음 라운드에서 수정된 이름이 반영된다.
