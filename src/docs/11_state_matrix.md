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
| `PUT /results/:sid/:tid/:rid/recommend` | 400² | 400 | **204**³ | 400 |
| `PUT /results/:sid/:tid/:rid/unrecommend` | 400² | 400 | **204** | 400 |
| `PUT /applications/:sid/:tid/:rid/abandon` | 400² | 400 | 400 | **204**⁴ |

## 담임 엔드포인트

| 엔드포인트 | 라운드 없음 | OPEN | CLOSED | FINALIZED |
|---|---|---|---|---|
| `POST /teacher/applications` | 404 | **201** | 400 | 400 |
| `DELETE /teacher/applications/:sid/:tid/:rid` | 400² | **204** | 400 | 400 |
| `PUT /teacher/applications/:sid/:tid/:rid/abandon` | 400² | 400 | 400 | **204**⁴ |

¹ 정원(unit_quota/total_quota) 초과 시 422 + 위반 목록 JSON, 상태는 CLOSED 유지
  (`tests/handler_rounds.rs`에서 별도 커버).
² **문서-코드 불일치 (감사 발견)**: 존재하지 않는 라운드에 대해 404가 아닌 400을
  반환한다 — 아래 [발견 사항] 참조.
³ 정원 찼으면 409, results 행 없으면 404 (`tests/handler_scoring.rs`에서 별도 커버).
⁴ 지원 내역 없으면 404, 담임은 담당 학급 아니면 403
  (`tests/handler_applications.rs`에서 별도 커버).

## 상태별 허용 행위 요약 (02_round_lifecycle.md와 일치)

- **OPEN**: 담임 지원 등록·취소만 가능. 점수 계산·추천·포기 전부 거부.
- **CLOSED**: 관리자 재계산·추천·추천취소·reopen·finalize만 가능. 담임 쓰기 전부 거부.
- **FINALIZED**: 관리자·담임 포기 처리만 가능. 그 외 쓰기 전부 거부.
  FINALIZED 라운드 존재는 새 라운드 open을 막지 않는다.

## DB 방어선 3종 (핸들러 우회 직접 SQL 차단)

| 방어선 | 차단 대상 | 직접 SQL 테스트 위치 |
|---|---|---|
| `idx_one_active_round` | 비-FINALIZED 라운드 2개 이상 INSERT | `tests/handler_rounds.rs::db_rejects_second_active_round` |
| `trg_prevent_delete_closed_result` | CLOSED/FINALIZED 라운드 results DELETE | `tests/handler_rounds.rs::db_rejects_result_delete_in_closed_round` |
| `trg_prevent_update_finalized_result` | FINALIZED 라운드 results UPDATE | `tests/state_matrix.rs::db_rejects_result_update_in_finalized_round` |

## 발견 사항 (문서-코드 불일치)

**[LOW] 존재하지 않는 라운드의 상태코드 비일관성**
`09_api_endpoints.md` 공통 에러 표는 404를 "리소스 없음"으로 정의하고, 실제로
`close`(404)·`reopen`(404)·`finalize`(404)·`calculate`(404)·`teacher create`(404)는
라운드 부재 시 404를 반환한다. 반면 `recommend`/`unrecommend`/`abandon`(관리자·담임)/
`teacher delete`는 라운드 부재를 상태 불일치와 구분하지 않고 400을 반환한다
(예: "CLOSED 라운드에서만 추천 확정이 가능합니다"). 실질적 위험은 없으나
(어느 쪽이든 거부 + DB 불변), API 일관성 관점의 개선 여지가 있다.
이 매트릭스는 **현재 구현된 동작(400)** 을 명세로 고정한다.
