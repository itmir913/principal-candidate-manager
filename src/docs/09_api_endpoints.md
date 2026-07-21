# API 엔드포인트 전체 명세

기본 prefix: `/api`

인가 레벨:
- **공개**: 미들웨어 없음
- **관리자**: `require_admin` (JWT, role=admin)
- **담임**: `require_teacher` (JWT, role=teacher), `/api/teacher/` prefix

---

## 공개 엔드포인트

| 메서드 | 경로 | 설명 |
|---|---|---|
| GET | `/health` | 헬스체크. 응답: `"ok"` |
| GET | `/version` | 앱 버전. 응답: `{"version": "0.1.x"}` |
| GET | `/rounds/current` | 현재 OPEN 라운드. 응답: `RoundRow | null` |
| GET | `/classes` | 반 목록 (로그인 폼용). 응답: `[{grade, class_no, ...}]` |

---

## 인증 (Auth)

| 메서드 | 경로 | 인가 | 설명 |
|---|---|---|---|
| GET | `/auth/admin/status` | 공개 | 관리자 계정 존재 여부 및 초기화 필요 여부 |
| POST | `/auth/admin` | 공개 | 관리자 로그인. Body: `{password}`. 응답: `{token}` |
| POST | `/auth/teacher` | 공개 | 담임 로그인. Body: `{grade, class_no, password}`. 응답: `{token, grade, class_no}` |
| PUT | `/auth/admin/password` | 관리자 | 관리자 비밀번호 변경. Body: `{current_password, new_password}` |
| GET | `/db-backup` | 관리자 | 백업 zip 다운로드 (blob). `pcm_backup_<YYYYMMDD_HHMMSS>.zip`, 내부 구조는 `pcm/data.db`(VACUUM INTO 스냅샷) + `pcm/config.json`(있을 때) |

---

## 학반 관리 (Classes) — 관리자

| 메서드 | 경로 | 설명 | 응답 |
|---|---|---|---|
| GET | `/classes/template` | xlsx 양식 다운로드 | blob |
| GET | `/classes/export` | 현재 학반 Excel 내보내기 | blob |
| POST | `/classes/import` | 학반 일괄 등록 (upsert) | `ImportResult` |
| PUT | `/classes/:grade/:class_no` | 단건 upsert | 200 |
| DELETE | `/classes/:grade/:class_no` | 단건 삭제 | 204 |

---

## 학생 관리 (Students) — 관리자

| 메서드 | 경로 | 설명 | 응답 |
|---|---|---|---|
| GET | `/students` | 전체 학생 목록. `?grade=&class_no=&is_enrolled=` 쿼리 | `[StudentRow]` |
| GET | `/students/grade-options` | 학년 옵션 목록 | `[{grade}]` |
| GET | `/students/template` | 전체 학생 xlsx 양식 | blob |
| GET | `/students/export` | 전체 학생 내보내기 | blob |
| POST | `/students/import` | 전체 학생 upsert import | `ImportResult` |
| GET | `/students/enrolled/template` | 재학생 xlsx 양식 | blob |
| GET | `/students/enrolled/export` | 재학생 내보내기 | blob |
| POST | `/students/enrolled/import` | 재학생 upsert import | `ImportResult` |
| POST | `/students/enrolled/add` | 재학생 1명 추가. Body: `{grade, class_no, seq_no, name}` | 201 |
| GET | `/students/graduated/template` | 졸업생 xlsx 양식 | blob |
| GET | `/students/graduated/export` | 졸업생 내보내기 | blob |
| POST | `/students/graduated/import` | 졸업생 upsert import | `ImportResult` |
| POST | `/students/graduated/add` | 졸업생 1명 추가. Body: `{student_code, name}` | 201 |
| DELETE | `/students/:id` | 학생 삭제 | 204 |

---

## 전형요소 (Areas) — 관리자

| 메서드 | 경로 | 설명 | 응답 |
|---|---|---|---|
| GET | `/areas` | 전형요소 목록 | `[AreaRow]` |
| POST | `/areas` | 전형요소 생성 | 201 + `{id}` |
| PUT | `/areas/:id` | 전형요소 수정 | 200 |
| DELETE | `/areas/:id` | 전형요소 삭제 | 204 |
| GET | `/areas/score-template/:name` | 점수 기준 xlsx 양식 (이름 기반) | blob |

### 점수 기준 (numeric_table) — 관리자

| 메서드 | 경로 | 설명 |
|---|---|---|
| GET | `/areas/:id/numeric-table/list` | 페이지네이션 조회. `?page=&per_page=` |
| GET | `/areas/:id/numeric-table/template` | xlsx 양식 |
| GET | `/areas/:id/numeric-table/export` | 현재 기준 내보내기 |
| POST | `/areas/:id/numeric-table/import` | import (CLOSED 라운드 차단) |

### 범주 기준 (category_map) — 관리자

| 메서드 | 경로 | 설명 |
|---|---|---|
| GET | `/areas/:id/category-map/list` | 페이지네이션 조회 |
| GET | `/areas/:id/category-map/template` | xlsx 양식 |
| GET | `/areas/:id/category-map/export` | 현재 기준 내보내기 |
| POST | `/areas/:id/category-map/import` | import (CLOSED 라운드 차단) |

### 기초 데이터 (base_data) — 관리자

| 메서드 | 경로 | 설명 |
|---|---|---|
| GET | `/areas/:id/base-data/list` | 페이지네이션. `?page=&per_page=&student_type=enrolled\|graduated` |
| GET | `/areas/:id/base-data/template` | xlsx 양식. `?student_type=enrolled\|graduated` |
| GET | `/areas/:id/base-data/export` | 내보내기. `?student_type=enrolled\|graduated` — import와 동일 헤더(왕복 가능) |
| POST | `/areas/:id/base-data/import` | import. `?student_type=enrolled\|graduated` |

`student_type`은 `enrolled`/`graduated` 외의 값이면 400 (silent fallback 없음). 복수값 전형요소 import에서 CLOSED 라운드 지원자의 데이터 교체 시도는 422 + 학생코드 안내.

### 외부 가져오기 — 관리자

| 메서드 | 경로 | 설명 |
|---|---|---|
| POST | `/areas/:id/base-data/external/daegyo/preview` | 대교협 미리보기 (xlsx) |
| POST | `/areas/:id/base-data/external/daegyo/import` | 대교협 import. Form: `file + univ_name + track_name` |
| POST | `/areas/:id/base-data/external/univ/preview` | 유니브 미리보기 (xls) |
| POST | `/areas/:id/base-data/external/univ/import` | 유니브 import. Form: `file + univ_name + track_name` |

---

## 대학 / 모집단위 (Universities / Tracks) — 관리자

| 메서드 | 경로 | 설명 | 응답 |
|---|---|---|---|
| GET | `/universities` | 대학 목록 | `[UnivRow]` |
| POST | `/universities` | 대학 생성 | 201 + `{id}` |
| PUT | `/universities/:id` | 대학 수정 | 200 |
| DELETE | `/universities/:id` | 대학 삭제 (applications 존재 시 409) | 204 |
| GET | `/universities/quota-stats` | 잔여석 통계 | `[QuotaStatRow]` |
| GET | `/universities/quota-stats/export` | 잔여석 통계 xlsx. `?univ_id=` | blob |
| GET | `/universities/:id/tracks` | 모집단위 목록 | `[TrackRow]` |
| POST | `/universities/:id/tracks` | 모집단위 생성 | 201 + `{id}` |
| GET | `/univ-tracks` | 전체 모집단위 (대학명 포함) | `[TrackRow]` |
| PUT | `/univ-tracks/:id` | 모집단위 수정 | 200 |
| DELETE | `/univ-tracks/:id` | 모집단위 삭제 (applications 존재 시 409) | 204 |
| GET | `/univ-tracks/:id/recommended-list` | 모집단위별 추천 목록 | `[...]` |

---

## 라운드 (Rounds) — 관리자

| 메서드 | 경로 | 설명 | 에러 |
|---|---|---|---|
| GET | `/rounds` | 라운드 목록 (최신순) | |
| POST | `/rounds/open` | 새 라운드 오픈 | 409: 진행 중 라운드 있음 |
| PUT | `/rounds/:id/close` | OPEN→CLOSED + 점수 계산. 기초데이터 누락 시 422+OPEN 유지 | 422: 누락 / 404: 라운드 없음 |
| PUT | `/rounds/:id/reopen` | CLOSED→OPEN. 추천/순위 초기화 | 404: CLOSED 아님 |
| PUT | `/rounds/:id/finalize` | CLOSED→FINALIZED. 미결정 있으면 422 (전원 명단), 정원 초과 있으면 422 (위반 목록) | 422: 미결정 / 422: 정원 초과 / 404 |

### close_round 상세

트랜잭션 (`BEGIN IMMEDIATE`):
1. CROSS JOIN으로 모든 지원자×전형요소 누락 검증 (최대 5건 출력)
2. `UPDATE rounds SET status='CLOSED'`
3. `run_calculate_scores_on_conn`: 전체 점수 계산 → results UPSERT → 순위 계산
4. 오류 시 ROLLBACK → 라운드 OPEN 상태 유지

### finalize_round 상세

트랜잭션 (`BEGIN IMMEDIATE`):
1. CLOSED 상태 확인
2. **미결정 지원 검증**: `excluded=0 AND COALESCE(r.recommended,0)=0`. LIMIT 없음 — 미결정 전원 반환. 있으면 422 + `{"error":..., "undecided":[...]}`
3. 모집단위 정원 초과 검증 (`unit_quota` 있는 트랙, 최대 5건)
4. 대학 전체 정원 초과 검증 (`total_quota` 있는 대학, 최대 5건)
5. 위반 있으면 422 + `{"error":..., "track_violations":[...], "univ_violations":[...]}` JSON 반환
6. `UPDATE rounds SET status='FINALIZED'`

---

## 점수 / 결과 (Scoring) — 관리자

| 메서드 | 경로 | 설명 | 에러 |
|---|---|---|---|
| POST | `/rounds/:id/calculate` | CLOSED 라운드 점수 재계산 | 400: CLOSED 아님 |
| POST | `/rounds/:id/auto-recommend` | CLOSED 라운드 자동 추천 확정(전 대학). 2단계(모집단위 정원 채움 → 대학 전체 순위 컷). 동점이 정원 경계를 가르면 그 동점 그룹만 manual로 반환. 부분 성공도 200 | 400: CLOSED 아님 / 404: 없음 |
| POST | `/rounds/:id/auto-recommend/univ/:univ_id` | 위와 동일하되 지정 대학의 모집단위만 처리 | 400: CLOSED 아님 / 404: 라운드·대학 없음 |
| GET | `/rounds/:id/results` | 결과 조회. `?track_id=` | `[ResultRow]` |
| GET | `/rounds/:id/results/export` | 결과 xlsx (전체결과 시트) | blob |
| GET | `/rounds/:id/summary/export` | 라운드 요약 xlsx (라운드결과+지원자결과 시트) | blob |
| GET | `/score-preview` | 점수 미리보기. `?student_id=&track_id=` | `ScorePreviewResponse` |
| PUT | `/results/:sid/:tid/:rid/recommend` | 추천 확정. CLOSED에서만. 정원 체크 (`BEGIN IMMEDIATE`) | 409: 정원 찼음 |
| PUT | `/results/:sid/:tid/:rid/unrecommend` | 추천 취소. CLOSED에서만 | |

### recommend_result 상세

`BEGIN IMMEDIATE` 트랜잭션:
1. round status = CLOSED 확인 (아니면 400/404)
1b. `excluded=1`이면 → 409 "미선발 처리된 지원은 추천할 수 없습니다"
2. 모집단위 `unit_quota` 조회
3. 전체 라운드 합산 추천 확정 수 (`recommended=1 AND abandoned=0`) ≥ `unit_quota` → 409
4. 대학 `total_quota` 조회
5. 대학 전체 추천 확정 수 ≥ `total_quota` → 409
5b. 같은 모집단위 내 `track_rank`가 현재 지원자보다 낮은 순위이면서 미결정(미추천·미포기·미선발)인 학생 있으면 → 409 "상위 순위 지원자 미결정". 동점자(`track_rank` 동일)는 서로 막지 않음
6. `UPDATE results SET recommended=1`. `rows_affected=0`이면 404 (결과 행 없음 — 점수 미계산)

---

## 지원 관리 (Applications) — 관리자

| 메서드 | 경로 | 설명 |
|---|---|---|
| GET | `/applications` | 지원 목록. `?round_id=&track_id=` |
| PUT | `/applications/:sid/:tid/:rid/abandon` | 포기. FINALIZED에서만. 지원 없으면 404 |
| PUT | `/applications/:sid/:tid/:rid/exclude` | 미선발 처리. CLOSED에서만. Body: `{"reason":"..."}` 사유 필수. 이미 추천 확정이면 409. 이미 미선발이면 409 |
| DELETE | `/applications/:sid/:tid/:rid/exclude` | 미선발 해제. CLOSED에서만. 미선발 상태 아니면 409 |

---

## 시스템 (System) — 관리자

| 메서드 | 경로 | 설명 |
|---|---|---|
| GET | `/version` | 앱 버전 (공개) |
| GET | `/db-backup` | 백업 zip 다운로드 (`pcm/` 폴더 모양) |

---

## 담임 전용 엔드포인트 (`/api/teacher/`)

| 메서드 | 경로 | 설명 | 주의 |
|---|---|---|---|
| GET | `/teacher/students` | 담당 학생 목록 (grade/class_no 필터) | 졸업생 담임(0/0): is_enrolled=0 전체 |
| GET | `/teacher/universities` | 대학 목록 | 공유 핸들러 |
| GET | `/teacher/universities/:id/tracks` | 모집단위 목록 | 공유 핸들러 |
| GET | `/teacher/univ-tracks` | 전체 모집단위 | 공유 핸들러 |
| GET | `/teacher/applications` | 담당 학생 지원 목록. `?round_id=` | |
| POST | `/teacher/applications` | 지원 등록+기초데이터+점수계산 (단일 tx) | 아래 상세 참조 |
| DELETE | `/teacher/applications/:sid/:tid/:rid` | 지원 취소 (OPEN에서만) | results도 함께 삭제 |
| PUT | `/teacher/applications/:sid/:tid/:rid/abandon` | 포기 (FINALIZED에서만). 지원 없으면 404 | 담당 학생 검증 |
| PUT | `/teacher/password` | 담임 비밀번호 변경 | 졸업생 담임 불가 |
| GET | `/teacher/area-context` | 전형요소+저장된 기초데이터. `?student_id=&track_id=` | |
| POST | `/teacher/area-score-preview` | 입력값 기반 점수 미리보기 (비저장) | |
| GET | `/teacher/results` | FINALIZED 라운드 결과 (담당 학생) | |

### POST /teacher/applications 상세

Body:
```json
{
  "student_id": 1,
  "track_id": 2,
  "round_id": 3,
  "department_name": "컴퓨터공학과",
  "base_data_entries": [
    {"area_id": 1, "values": ["3.2"]},
    {"area_id": 2, "values": ["수상", "봉사"]}
  ]
}
```

처리 순서:
1. 라운드 OPEN 확인 (tx 밖)
2. 학생 소속 검증 (담당 반/졸업생)
3. department_name trim+비어있으면 400
4. 전형요소 정보 일괄 로드
5. teacher_editable 아닌 전형요소에 값 입력 시 403
6. teacher_editable인 전형요소 전부 값 있어야 함 (누락 시 422)
7. 값 인코딩 (×100000, Category: 그대로)
8. **트랜잭션 시작 (`BEGIN IMMEDIATE` — 시작 시점 쓰기 잠금)**
9. tx 안에서 라운드 OPEN 재확인 (TOCTOU 방지 — IMMEDIATE 잠금으로 재확인이 확정적)
10. base_data 저장 (multi_value=1: DELETE+INSERT, single: INSERT OR REPLACE)
11. applications UPSERT (department_name 업데이트 허용)
12. 전형요소 전체 점수 계산 (calc_area_score)
13. results UPSERT
14. tx.commit()

---

## 공통 에러 응답

| 상태 코드 | 의미 |
|---|---|
| 400 | 잘못된 요청 (파싱 실패, 유효성 오류) |
| 401 | 미인증 (토큰 없음/만료) |
| 403 | 권한 부족 |
| 404 | 리소스 없음 |
| 409 | 충돌 (진행 중 라운드 / 정원 초과) |
| 422 | 처리 불가 (import 오류 / 기초데이터 누락 / 점수 계산 실패 / 정원 초과 확정) |
| 500 | 서버 내부 오류 |
| 503 | DB 초기화 실패 (Degraded 모드) |

에러 바디: 평문 문자열 (text/plain) 또는 JSON (finalize_round의 위반 목록).

---

## 응답 타입 참조

### RoundRow
```json
{"id": 1, "status": "OPEN|CLOSED|FINALIZED", "opened_at": "...", "closed_at": null, "finalized_at": null}
```

### ResultRow
```json
{
  "student_id": 1, "track_id": 2, "round_id": 3,
  "total_score": "85.5",
  "score_detail": {"1": "30", "2": "55.5"},
  "ranking": 1, "recommended": false, "abandoned": false,
  "excluded": false, "excluded_reason": null,
  "student_code": "A001", "name": "홍길동",
  "grade": 3, "class_no": 2, "seq_no": 5, "is_enrolled": true,
  "univ_name": "서울대", "track_name": "컴퓨터공학부", "department_name": "컴퓨터공학과"
}
```

Score 타입: `serde`로 직렬화 시 `f64`로 출력 (×100000 raw 값이 아님). 프론트엔드에서 직접 사용 가능.

### ImportResult
```json
{"rows": 42, "errors": [], "warnings": ["모집단위 자동 추가됨"]}
```
