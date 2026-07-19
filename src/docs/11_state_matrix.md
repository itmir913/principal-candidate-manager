# 11. 상태기계 × 쓰기 엔드포인트 매트릭스

라운드 상태 4가지 {라운드 없음, OPEN, CLOSED, FINALIZED} × 모든 쓰기 엔드포인트의
기대 상태코드 표. `tests/state_matrix.rs`가 이 표의 각 셀을 실행으로 고정한다
(한 테스트 케이스 = 표의 한 셀).

**"라운드 없음"의 의미**: `open_round`는 rounds 테이블이 비어 있는 상태,
나머지 엔드포인트는 존재하지 않는 라운드 id(9999)로 호출한 경우.

**거부 셀 불변식**: 4xx 거부 시 rounds·applications·results 세 테이블은
단 한 행도 변하지 않는다 (매 거부 셀마다 스냅샷 비교로 단언).

## 관리자 엔드포인트

| 엔드포인트 | 라운드 없음 | OPEN | CLOSED | FINALIZED |
|---|---|---|---|---|
| `POST /rounds/open` | **201** | 409 | 409 | **201** |
| `PUT /rounds/:id/close` | 404 | **200** | 404 | 404 |
| `PUT /rounds/:id/reopen` | 404 | 404 | **204** | 404 |
| `PUT /rounds/:id/finalize` | 404 | 404 | **204**¹ | 404 |
| `POST /rounds/:id/calculate` | 404 | 400 | **200** | 400 |
| `POST /rounds/:id/auto-recommend` | 404 | 400 | **200** | 400 |
| `POST /rounds/:id/auto-recommend/univ/:univ_id` | 404 | 400 | **200** | 400 |
| `PUT /results/:sid/:tid/:rid/recommend` | 404 | 400 | **204**³ | 400 |
| `PUT /results/:sid/:tid/:rid/unrecommend` | 404 | 400 | **204** | 400 |
| `PUT /applications/:sid/:tid/:rid/abandon` | 404 | 400 | 400 | **204**⁴ |

## 담임 엔드포인트

| 엔드포인트 | 라운드 없음 | OPEN | CLOSED | FINALIZED |
|---|---|---|---|---|
| `POST /teacher/applications` | 404 | **201** | 400 | 400 |
| `DELETE /teacher/applications/:sid/:tid/:rid` | 404 | **204** | 400 | 400 |
| `PUT /teacher/applications/:sid/:tid/:rid/abandon` | 404 | 400 | 400 | **204**⁴ |

¹ 전건 추천/제외 결정 완료 + 정원 이내일 때 204. 위반 시 422 + 위반 명단 JSON, 상태는 CLOSED 유지.
  미결정 지원자가 있으면 정원 초과보다 먼저 422를 반환한다 (`tests/handler_rounds.rs`에서 별도 커버).
³ 정원 찼으면 409, results 행 없으면 404 (`tests/handler_scoring.rs`에서 별도 커버).
⁴ 지원 내역 없으면 404, 담임은 담당 학급 아니면 403
  (`tests/handler_applications.rs`에서 별도 커버).

## 상태별 허용 행위 요약 (02_round_lifecycle.md와 일치)

- **OPEN**: 담임 지원 등록·취소만 가능. 점수 계산·추천·포기 전부 거부.
- **CLOSED**: 관리자 재계산·추천·추천취소·자동 추천 확정(auto-recommend)·reopen·finalize만 가능. 담임 쓰기 전부 거부.
- **FINALIZED**: 관리자·담임 포기 처리만 가능. 그 외 쓰기 전부 거부.
  FINALIZED 라운드 존재는 새 라운드 open을 막지 않는다.

## DB 방어선 4종 (핸들러 우회 직접 SQL 차단)

| 방어선 | 차단 대상 | 직접 SQL 테스트 위치 |
|---|---|---|
| `idx_one_active_round` | 비-FINALIZED 라운드 2개 이상 INSERT | `tests/handler_rounds.rs::db_rejects_second_active_round` |
| `trg_prevent_delete_closed_result` | CLOSED/FINALIZED 라운드 results DELETE | `tests/handler_rounds.rs::db_rejects_result_delete_in_closed_round` |
| `trg_prevent_update_finalized_result` | FINALIZED 라운드 results UPDATE | `tests/state_matrix.rs::db_rejects_result_update_in_finalized_round` |
| `trg_require_all_decided_before_finalize` | 미결정 지원 존재 시 CLOSED→FINALIZED 전환 | `tests/handler_rounds.rs::trigger_blocks_direct_sql_finalize_when_undecided` |

## 발견 사항 (문서-코드 불일치) — 해소됨

**[LOW → 수정 완료] 존재하지 않는 라운드의 상태코드 비일관성**
과거 `recommend`/`unrecommend`/`abandon`(관리자·담임)/`teacher delete`는 라운드 부재를
상태 불일치와 구분하지 않고 400을 반환했다. `09_api_endpoints.md` 공통 에러 표
(404 = 리소스 없음) 및 `close`/`reopen`/`finalize`/`calculate`/`teacher create`(404)와
일관되도록 5개 핸들러 전부 `match { 대상상태 => 진행, Some(_) => 400, None => 404 }`
패턴으로 통일했다. **모든 쓰기 엔드포인트에서 라운드 부재 = 404**가 명세다.
