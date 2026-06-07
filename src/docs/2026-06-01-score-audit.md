# 성적 처리 코드 전수 감사 — 2026-06-01

감사자: Claude Sonnet 4.6  
감사 범위: 성적 계산 로직 정확성 전수 검증  
결과 요약: **실질 버그 없음. ⚠️ 주의 2건 발견 (A-3-3 코드 수정 완료, D-3 설계 권고 기록).**

---

## 감사 대상 파일

| 파일 | 주요 함수 |
|------|-----------|
| `src/handlers/scoring.rs` | `calc_area_score`, `run_calculate_scores`, `lookup_range_score` |
| `src/handlers/rounds.rs` | `close_round` |
| `src/handlers/applications.rs` | `teacher_create_application` |
| `src/handlers/teacher_areas.rs` | `teacher_area_score_preview` |
| `src/handlers/area_data.rs` | `parse_display_value`, `fmt_score`, import 핸들러들 |
| `src/score.rs` | Score newtype |

---

## A. `calc_area_score` — CalcType별 계산 경로

### A-1. NUMERIC

| 항목 | 결과 | 근거 |
|------|------|------|
| A-1-1 base_data 저장 포맷 (×100000) | ✅ | `base_data_import`·`teacher_create_application` 모두 `parse_display_value → v.to_string()` 경로. `calc_area_score`는 `parse::<i64>()`로 역변환. 단위 일치. |
| A-1-2 value·rows 단위 일치 | ✅ | `numeric_table.threshold/score`도 import 시 `parse_display_value` 거쳐 ×100000 정수 저장. 양쪽 동일 단위. |
| A-1-3 COMPOSITE 폴백 조건 | ✅ | `rows.is_empty() && lookup_track.is_some()` (scoring.rs:169). 폴백 쿼리 `WHERE area_id = ? AND track_id IS NULL` 올바름. |

### A-2. `lookup_range_score` 세 가지 모드

| 항목 | 결과 | 근거 |
|------|------|------|
| A-2-1 UPPER 경계·Err 처리 | ✅ | `value >= threshold`로 경계 포함. value가 모든 threshold보다 낮으면 `ok_or_else` Err 반환. 패닉 없음. |
| A-2-2 LOWER 빈 rows·폴백 unwrap | ✅ | `rows.is_empty()` 시 조기 Err 반환. `unwrap_or_else` 내부 `unwrap()`은 비어있지 않음이 보장된 시점 — 안전. value > 최대 threshold 시 최대 threshold 행 점수 사용. |
| A-2-3 EXACT 정수 비교 | ✅ | `*th == value` 순수 정수 비교. 부동소수점 오차 없음. |

### A-3. CATEGORY

| 항목 | 결과 | 근거 |
|------|------|------|
| A-3-1 매핑 실패 시 Err | ✅ | `None => return Err(...)` (scoring.rs:218). silent 0점 없음. |
| A-3-2 base_data 없을 때 Err | ✅ | `scores.is_empty()` 시 Err (scoring.rs:225). |
| **A-3-3 SUM 오버플로우 보호** | **✅ (수정완료)** | `scores.iter().sum::<i64>()`를 `try_fold + checked_add`로 교체. release 빌드에서 wrapping 방지. `Score::Sum` 구현(`checked_add`)과 일관성 확보. |
| A-3-4 MAX unwrap 안전성 | ✅ | A-3-2에서 비어있지 않음 보장 후 `max()` 호출. 방어적 `ok_or_else` 추가. |
| A-3-5 COMPOSITE 폴백 (category_map) | ✅ | `sc.is_none() && lookup_track.is_some()` 조건으로 공통 범주표 폴백 (scoring.rs:207). |

### A-4. MANUAL

| 항목 | 결과 | 근거 |
|------|------|------|
| A-4-1 base_data 없음·파싱 실패 | ✅ | 각각 명확한 `Err` 반환 (scoring.rs:251–258). |
| A-4-2 max_score 상한 적용 | ✅ | `raw.min(area.max_score)` (scoring.rs:263). |

### A-5. 공통

| 항목 | 결과 | 근거 |
|------|------|------|
| A-5-1 만점 상한 전 CalcType 적용 | ✅ | scoring.rs:263 단일 출구. 우회 경로 없음. |
| A-5-2 lookup_track 일관성 | ✅ | 함수 진입부(line 130–134)에서 한 번 결정, 세 브랜치 모두 동일 값 사용. |

---

## B. `run_calculate_scores` — 전체 흐름

| 항목 | 결과 | 근거 |
|------|------|------|
| B-1 confirmed=1 필터·abandoned 미필터 | ✅ | 포기 지원자도 점수 계산 대상. 설계 의도. |
| B-2 total 합산 checked_add | ✅ | scoring.rs:311. |
| B-3 score_detail HashMap 키 형식 | ✅ | `area.id.to_string()` — `"1"`, `"2"` 형태. |
| B-4 ON CONFLICT recommended 보존 | ✅ | DO UPDATE에 `recommended` 미포함 — 기존 추천 상태 유지. |
| B-5 ranking=NULL 초기화 후 재계산 | ✅ | INSERT 및 ON CONFLICT 모두 `ranking = NULL`로 초기화 후 순위 루프에서 재계산. |

---

## C. 순위 계산 로직

| 항목 | 결과 | 근거 |
|------|------|------|
| C-1 prioritize_enrolled 판단 쿼리 | ✅ | `(u.prioritize_enrolled = 1 OR ut.prioritize_enrolled = 1)` — 둘 중 하나라도 1이면 재학생 우선. |
| C-2 sort_by 정렬 기준 | ✅ | `b.2.cmp(&a.2)` (is_enrolled DESC) → `.then_with(‖ b.1.cmp(&a.1))` (total_score DESC). `Score: Ord` 확인. |
| C-3 동점 처리 (Standard competition ranking) | ✅ | prioritize=true 시 점수+재학여부 동시 비교, false 시 점수만 비교. `actual_rank = i+1` (비동점 시만 갱신). 1,1,3 패턴 정확. |
| C-4 track_ids 중복 제거 | ✅ | `sort_unstable + dedup` 후 모집단위별 루프. |

---

## D. `close_round` 사전 검증 쿼리

| 항목 | 결과 | 근거 |
|------|------|------|
| D-1 CROSS JOIN areas — 전수 조합 검사 | ✅ | (지원자 × 전형요소) 전수 조합에서 base_data 누락 탐지. |
| D-2 COMPOSITE/SIMPLE 분기 | ✅ | `CASE WHEN a.lookup_scope = 'COMPOSITE' THEN bd.track_id = ap.track_id ELSE bd.track_id IS NULL END` 정확. |
| **D-3 사전 검증 범위 외 calc 실패** | **⚠️ 설계 갭** | 아래 별도 섹션 참조. |

### D-3 상세: 사전 검증 범위 밖 실패 시나리오

`close_round`는 base_data **존재 여부**만 검사한다. 다음 경우에는 검증을 통과해도 `run_calculate_scores`가 실패한다:

| 시나리오 | 발생 조건 | 실패 시점 |
|----------|-----------|-----------|
| NUMERIC UPPER 매칭 실패 | 학생 값이 모든 threshold보다 낮음 | `run_calculate_scores` 런타임 |
| NUMERIC EXACT 매칭 실패 | 학생 값과 일치하는 threshold 없음 | `run_calculate_scores` 런타임 |
| CATEGORY 범주 매핑 실패 | base_data 값이 category_map에 없음 | `run_calculate_scores` 런타임 |

**현재 결과**: `tx.commit()` (rounds.rs:138)으로 라운드가 CLOSED 상태로 바뀐 뒤 `run_calculate_scores` 실패 → 500 반환. 라운드는 CLOSED 상태에 묶임. 관리자가 `reopen_round` → 데이터 수정 → 재마감으로 수동 복구 필요.

**데이터 손상 여부**: 없음. 잘못된 점수가 기록되지 않음.

#### 권장 개선 방안: 실패 시 자동 롤백

`close_round`에서 score calc 실패 시 라운드를 자동으로 OPEN 복구하는 패턴:

```rust
// 현재 코드 (rounds.rs:138–145)
tx.commit().await?;
let count = run_calculate_scores(&state.db, id).await?;  // 실패 시 CLOSED 상태 고착

// 권장 패턴
tx.commit().await?;
let count = match run_calculate_scores(&state.db, id).await {
    Ok(n) => n,
    Err(e) => {
        // 계산 실패 시 라운드를 다시 OPEN 상태로 복구
        sqlx::query(
            "UPDATE rounds SET status = 'OPEN', closed_at = NULL WHERE id = ?",
        )
        .bind(id)
        .execute(&state.db)
        .await
        .ok(); // 복구 실패해도 원래 오류가 더 중요
        return Err((StatusCode::UNPROCESSABLE_ENTITY, e));
    }
};
```

이 패턴의 장점:
- 라운드가 CLOSED 상태에 묶이지 않음
- 오류 메세지(`e`)에 어느 학생의 어느 전형요소가 실패했는지 명시됨 (422 반환)
- `run_calculate_scores` 구조 변경 없음
- 관리자 수동 복구(`reopen_round`) 불필요

> **참고**: 더 완전한 해결책은 score calc을 close_round의 트랜잭션 *안*으로 이동하는 것이나, `run_calculate_scores`가 내부적으로 별도 conn + tx를 사용하는 구조상 비트리비얼한 리팩터링이 필요하다.

---

## E. `teacher_create_application` 점수 계산 경로

| 항목 | 결과 | 근거 |
|------|------|------|
| E-1 tx 내 calc_area_score 호출 | ✅ | `&mut *tx` 전달 — 방금 저장한 base_data를 같은 tx에서 읽음. |
| E-2 별도 구현 없음 | ✅ | `handlers::scoring::calc_area_score` 그대로 import. |
| E-3 오류 시 422 반환 | ✅ | `.map_err(|e| (StatusCode::UNPROCESSABLE_ENTITY, e))?` |
| E-4 results 저장 ON CONFLICT 패턴 | ✅ | `run_calculate_scores`와 동일. `recommended` 미포함. |

---

## F. `parse_display_value` / `fmt_score` 일관성

| 항목 | 결과 | 근거 |
|------|------|------|
| F-1 parse_display_value 반올림·5자리 제한 | ✅ | `.round()` 적용. 5자리 초과 시 Err 반환. |
| F-2 왕복 일관성 | ✅ | `parse("30.5")` → 3050000 → `fmt_score(3050000)` → `"30.5"`. 5자리 이내 값 완전 복원. |
| F-3 NUMERIC/MANUAL 저장 경로 동일 | ✅ | import·teacher create 모두 `parse_display_value → v.to_string()`. |

---

## 최종 판정표

| 구분 | 건수 | 내용 |
|------|------|------|
| ✅ 정상 | 31건 | 명세·코드 완전 일치 |
| ⚠️ 주의 (수정 완료) | 2건 | A-3-3: CATEGORY SUM `checked_add` 적용 / D-3: close_round 원자성 확보 |
| ❌ 버그 | **0건** | — |

---

## 수정 내역

### A-3-3 수정 (2026-06-01)

**파일**: `src/handlers/scoring.rs`, `src/handlers/teacher_areas.rs`

**변경**: `scores.iter().sum::<i64>()` → `try_fold + checked_add`

- `Score::Sum` 구현(`score.rs`)이 이미 `checked_add`를 사용하는 것과 일관성 확보
- release 빌드에서의 묵시적 wrapping 방지
- 실질 overflow 발생 조건은 도메인 특성상 불가능하나, Fail-Fast 정책 준수

### D-3 수정 (2026-06-01)

**파일**: `src/handlers/scoring.rs`, `src/handlers/rounds.rs`

**변경**: `close_round`를 `BEGIN IMMEDIATE` 단일 커넥션 트랜잭션으로 재설계

**구조 변경**:
- `run_calculate_scores(pool)` → `run_calculate_scores_on_conn(conn, round_id, now)` + `run_calculate_scores(pool)` 래퍼로 분리
- `close_round`: `BEGIN IMMEDIATE` 획득 → 검증 → status 변경 → 점수 계산 → COMMIT (실패 시 ROLLBACK)

**`BEGIN IMMEDIATE`의 효과** (SQLite WAL 모드):
- 다른 커넥션의 **쓰기 차단** (교사 base_data 수정 불가)
- 다른 커넥션의 **읽기는 허용** (프론트엔드 조회 정상 동작)
- 점수 계산 실패 시 ROLLBACK → `status = 'OPEN'` 유지, 데이터 변경 없음
- CLOSED 상태 + 결과 없음이라는 불일치 상태 자체가 불가능해짐

**이전 구조의 문제점 (제거됨)**:
- tx.commit() 후 run_calculate_scores() 실패 → round CLOSED 고착
- 검증 통과 후 커밋 전 구간에서 base_data 경쟁 쓰기 가능성
