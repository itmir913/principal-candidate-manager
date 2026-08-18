# 프론트엔드-백엔드 계약 명세

## 인증 흐름 (auth store)

**저장소**: `localStorage` (`pcm_token`, `pcm_role`, `pcm_grade`, `pcm_class_no`, `pcm_teacher_name`)

**초기화 체크**: 라우터 `beforeEach`에서 `GET /api/auth/admin/status` 호출
- 응답: `{"initialized": true|false}`
- `initialized=false` → `/welcome` 강제 이동 (관리자 초기 비밀번호 설정)
- `initialized=true` → 정상 라우팅

**Axios 인터셉터**:
- 모든 요청: `Authorization: Bearer {token}` 자동 주입
- 401 응답: 토큰 삭제 + localStorage 초기화 + `/login` 리다이렉트
- 503 응답: `ServerErrorView`로 리다이렉트. `{code, message, db_ver, app_ver}` 쿼리로 전달

**로그인 응답 계약**:
- 관리자: `POST /auth/admin` → `{token: string}`
- 담임: `POST /auth/teacher` → `{token: string, grade: number, class_no: number, teacher_name?: string}`

---

## 라우팅

| 경로 | 컴포넌트 | 가드 |
|---|---|---|
| `/` | → `/login` | 리다이렉트 |
| `/login` | `LoginView.vue` | 로그인 상태면 `/admin` 또는 `/teacher` |
| `/welcome` | `WelcomeView.vue` | initialized=false일 때만 |
| `/admin` | `AdminView.vue` | `requiresAdmin` (role=admin) |
| `/teacher` | `TeacherView.vue` | `requiresTeacher` (role=teacher) |
| `/server-error` | `ServerErrorView.vue` | 가드 없음 (503 자동 이동) |

---

## API 모듈 매핑

### `frontend/src/api/admin.js`

AdminView.vue 및 하위 탭 컴포넌트에서 사용.

| 함수 | 엔드포인트 | 응답 타입 |
|---|---|---|
| `getOverview` | GET /overview | 개요 통계 |
| `getClasses` | GET /classes | `[{grade, class_no, ...}]` |
| `upsertClass(grade, classNo, body)` | PUT /classes/:grade/:class_no | - |
| `deleteClass(grade, classNo)` | DELETE /classes/:grade/:class_no | - |
| `downloadClassTemplate` | GET /classes/template | blob |
| `exportClasses` | GET /classes/export | blob |
| `importClasses(file)` | POST /classes/import | `ImportResult` |
| `getAreas` | GET /areas | `[AreaRow]` |
| `createArea(body)` | POST /areas | `{id}` |
| `updateArea(id, body)` | PUT /areas/:id | - |
| `deleteArea(id)` | DELETE /areas/:id | - |
| `downloadAreaScoreTemplate(name)` | GET /areas/score-template/:name | blob |
| `getNumericTableList(id, page, perPage)` | GET /areas/:id/numeric-table/list | `NumericTablePage` |
| `getCategoryMapList(id, page, perPage)` | GET /areas/:id/category-map/list | `CategoryMapPage` |
| `getBaseDataList(id, page, perPage, studentType)` | GET /areas/:id/base-data/list | `BaseDataPage` |
| `importNumericTable(id, file)` | POST /areas/:id/numeric-table/import | `ImportResult` |
| `importCategoryMap(id, file)` | POST /areas/:id/category-map/import | `ImportResult` |
| `importBaseData(id, file, studentType)` | POST /areas/:id/base-data/import | `ImportResult` |
| `previewDaegyoImport(id, file)` | POST /areas/:id/base-data/external/daegyo/preview | `ExternalPreview` |
| `importDaegyo(id, file, univName, trackName)` | POST /areas/:id/base-data/external/daegyo/import | - |
| `previewUnivImport(id, file)` | POST /areas/:id/base-data/external/univ/preview | `ExternalPreview` |
| `importUniv(id, file, univName, trackName)` | POST /areas/:id/base-data/external/univ/import | - |
| `getStudents(params)` | GET /students | `[StudentRow]` |
| `getStudentGradeOptions` | GET /students/grade-options | `[{grade}]` |
| `importStudents(file)` | POST /students/import | `ImportResult` |
| `importEnrolled(file)` | POST /students/enrolled/import | `ImportResult` |
| `importGraduated(file)` | POST /students/graduated/import | `ImportResult` |
| `addEnrolledStudent(body)` | POST /students/enrolled/add | - |
| `addGraduatedStudent(body)` | POST /students/graduated/add | - |
| `deleteStudent(id)` | DELETE /students/:id | - |
| `getUniversities` | GET /universities | `[UnivRow]` |
| `createUniversity(body)` | POST /universities | `{id}` |
| `updateUniversity(id, body)` | PUT /universities/:id | - |
| `deleteUniversity(id)` | DELETE /universities/:id | - |
| `getUnivTracks(univId)` | GET /universities/:id/tracks | `[TrackRow]` |
| `getAllTracks` | GET /univ-tracks | `[TrackRow]` |
| `createTrack(univId, body)` | POST /universities/:id/tracks | `{id}` |
| `updateTrack(id, body)` | PUT /univ-tracks/:id | - |
| `deleteTrack(id)` | DELETE /univ-tracks/:id | - |
| `getTrackRecommendedList(trackId)` | GET /univ-tracks/:id/recommended-list | - |
| `getRounds` | GET /rounds | `[RoundRow]` |
| `getCurrentRound` | GET /rounds/current | `RoundRow \| null` |
| `openRound` | POST /rounds/open | `{id}` |
| `closeRound(id)` | PUT /rounds/:id/close | `{calculated}` |
| `reopenRound(id)` | PUT /rounds/:id/reopen | - |
| `finalizeRound(id)` | PUT /rounds/:id/finalize | - |
| `calculateScores(roundId)` | POST /rounds/:id/calculate | `{calculated}` |
| `getResults(roundId, trackId?)` | GET /rounds/:id/results | `[ResultRow]` |
| `exportResultsExcel(roundId)` | GET /rounds/:id/results/export | blob |
| `exportRoundSummary(roundId)` | GET /rounds/:id/summary/export | blob |
| `recommendResult(sid, tid, rid)` | PUT /results/:sid/:tid/:rid/recommend | - |
| `unrecommendResult(sid, tid, rid)` | PUT /results/:sid/:tid/:rid/unrecommend | - |
| `getApplications(roundId, trackId?)` | GET /applications | `[ApplicationRow]` |
| `abandonApplication(sid, tid, rid)` | PUT /applications/:sid/:tid/:rid/abandon | - |
| `excludeApplication(sid, tid, rid, reason)` | PUT /applications/:sid/:tid/:rid/exclude | - |
| `clearApplicationExclusion(sid, tid, rid)` | DELETE /applications/:sid/:tid/:rid/exclude | - |
| `changeAdminPassword(current, new)` | PUT /auth/admin/password | - |
| `scorePreview(studentId, trackId)` | GET /score-preview | `ScorePreviewResponse` |
| `adminAreaScorePreview(areaId, trackId, values)` | POST /area-score-preview | `{score, matched_keys, warning, error}` |
| `getQuotaStats` | GET /universities/quota-stats | `[QuotaStatRow]` |
| `exportQuotaStats(univId?)` | GET /universities/quota-stats/export | blob |
| `autoRecommend(roundId)` | POST /rounds/:id/auto-recommend | `AutoRecommendResponse {confirmed, manual}` |
| `autoRecommendUniv(roundId, univId)` | POST /rounds/:id/auto-recommend/univ/:univ_id | `AutoRecommendResponse` |
| `getRoundConfirmationStatus(roundId)` | GET /rounds/:id/confirmation-status | `{classes: [{grade, class_no, teacher_name, confirmed, confirmed_at}]}` |
| `getAuditLogs(params)` | GET /audit-logs | `AuditPage {rows, total, page, per_page}` — `AuditRow`에 `actor_ip` 포함 |
| `exportAuditLogs` | GET /audit-logs/export | blob |

### `frontend/src/api/teacher.js`

TeacherView.vue 및 하위 탭 컴포넌트에서 사용.

| 함수 | 엔드포인트 | 응답 타입 |
|---|---|---|
| `getCurrentRound` | GET /rounds/current | `RoundRow \| null` |
| `teacherGetStudents` | GET /teacher/students | `[StudentRow]` |
| `teacherGetUniversities` | GET /teacher/universities | `[UnivRow]` |
| `teacherGetUnivTracks(univId)` | GET /teacher/universities/:id/tracks | `[TrackRow]` |
| `teacherGetAreaContext(studentId, trackId)` | GET /teacher/area-context | `AreaContextResponse` |
| `teacherAreaScorePreview(areaId, trackId, values)` | POST /teacher/area-score-preview | `{score, matched_keys, warning, error}` — ApplicationTab.vue가 4개 필드 전부 사용 |
| `teacherGetAllTracks` | GET /teacher/univ-tracks | `[TrackRow]` |
| `teacherGetApplications(roundId)` | GET /teacher/applications | `[ApplicationRow]` |
| `teacherCreateApplication(body)` | POST /teacher/applications | 201 |
| `teacherDeleteApplication(sid, tid, rid)` | DELETE /teacher/applications/:sid/:tid/:rid | - |
| `teacherChangePassword(current, new)` | PUT /teacher/password | - |
| `teacherGetResults` | GET /teacher/results | `TeacherResultsResponse` |
| `teacherAbandonApplication(sid, tid, rid)` | PUT /teacher/applications/:sid/:tid/:rid/abandon | - |
| `teacherGetRoundConfirmation(roundId)` | GET /teacher/rounds/:id/confirm | `{confirmed, confirmed_at}` |
| `teacherConfirmRound(roundId)` | POST /teacher/rounds/:id/confirm | - (OPEN에서만, 그 외 400) |
| `teacherRevokeRoundConfirmation(roundId)` | DELETE /teacher/rounds/:id/confirm | - (OPEN에서만, 그 외 400) |

---

## 컴포넌트별 사용 API

### AdminView.vue
- 탭 관리 (ClassesTab, StudentsTab, AreasTab, UniversitiesTab, RoundsTab, OverviewTab, ManualTab, UpdateTab)
- `getCurrentRound` (공통 상태)

### OverviewTab.vue
- `getOverview`

### ClassesTab.vue
- `getClasses`, `upsertClass`, `deleteClass`
- `downloadClassTemplate`, `exportClasses`, `importClasses`
- inline form 패턴 (모달 아님)

### StudentsTab.vue
- `getStudents(params)`, `deleteStudent`
- `addEnrolledStudent`, `addGraduatedStudent`
- `downloadStudentTemplate`, `exportStudents`, `importStudents`
- `downloadEnrolledTemplate`, `exportEnrolled`, `importEnrolled`
- `downloadGraduatedTemplate`, `exportGraduated`, `importGraduated`
- `getStudentGradeOptions` (학년 필터용)

### AreasTab.vue
- `getAreas`, `createArea`, `updateArea`, `deleteArea`
- `downloadAreaScoreTemplate`, `downloadNumericTableTemplate`, `downloadCategoryMapTemplate`
- `exportNumericTable`, `exportCategoryMap`, `exportBaseData`
- `importNumericTable`, `importCategoryMap`, `importBaseData`
- `getNumericTableList`, `getCategoryMapList`, `getBaseDataList`
- `previewDaegyoImport`, `importDaegyo`
- `previewUnivImport`, `importUniv`
- `downloadBaseDataTemplate(id, studentType)`
- 재학생/졸업생 라디오 패턴: `baseStudentType` ref → `watch` → API 호출

### UniversitiesTab.vue
- `getUniversities`, `createUniversity`, `updateUniversity`, `deleteUniversity`
- `getUnivTracks(univId)`, `getAllTracks`, `createTrack`, `updateTrack`, `deleteTrack`
- `getQuotaStats`, `exportQuotaStats`
- `getTrackRecommendedList(trackId)`

### RoundsTab.vue
- `getRounds`, `openRound`, `closeRound`, `reopenRound`, `finalizeRound`
- `calculateScores`, `getResults`, `exportResultsExcel`, `exportRoundSummary`
- `recommendResult`, `unrecommendResult`
- `getApplications`, `abandonApplication`
- `scorePreview`

### TeacherView.vue
- `getCurrentRound`
- 탭: ApplicationTab, ClassTab, ResultsTab

### ApplicationTab.vue (담임)
- `teacherGetStudents`, `teacherGetUniversities`, `teacherGetUnivTracks`
- `teacherGetAllTracks`, `teacherGetApplications`
- `teacherGetAreaContext` (전형요소+기저장 값)
- `teacherAreaScorePreview` (실시간 점수 미리보기)
- `teacherCreateApplication` (지원 등록)
- `teacherDeleteApplication` (지원 취소)
- `teacherAbandonApplication` (포기)

### ClassTab.vue (담임)
- 학급 관리 (읽기 전용 또는 담임 제한 기능)

### ResultsTab.vue (담임)
- `teacherGetResults` → `{rounds: RoundInfo[], results: ResultRow[]}`

---

## 응답 필드 계약 (프론트가 기대하는 필드명)

### ResultRow (백엔드 → 프론트)
```
student_id, track_id, round_id
total_score         // Score 타입 → JSON: 소수 문자열 or 숫자 (serde로 f64 출력)
score_detail        // 백: JSON 문자열 "{\"1\": Score, ...}" → 프론트: 객체 {area_id: Score}
ranking             // 대학 전체 순위. null 가능
track_rank          // 모집단위 순위 (track_rank_window 파생). null 가능
recommended         // bool
abandoned           // bool
excluded            // bool (미선발 여부. false면 excluded_reason은 null)
excluded_reason     // string | null (미선발 사유. excluded=true일 때만 non-null 보장)
student_code, name, grade, class_no, seq_no, is_enrolled
univ_name, track_name, department_name
```

> `score_detail`은 백엔드에서 `serialize_with = "score_detail_as_map"` 커스텀 직렬화 → `{area_id_str: Score}` 형태로 전달됨

### ApplicationRow (백엔드 → 프론트)
```
student_id, track_id, round_id
abandoned       // bool
excluded        // bool (미선발 여부)
excluded_reason // string | null
department_name // string
student_code, name, grade, class_no, seq_no, is_enrolled
univ_id         // ApplicationTab.vue가 모집단위 재조회에 사용
univ_name, track_name
recommended     // null | bool (results 테이블 LEFT JOIN)
round_status    // "OPEN" | "CLOSED" | "FINALIZED"
```

### RoundRow
```
id, status, opened_at, closed_at (null 가능), finalized_at (null 가능), needs_recalc (bool)
```

`needs_recalc` — 마지막 점수 계산 이후 기초데이터가 바뀌었는가. CLOSED 라운드에서만 true가 될 수 있다.
프론트는 이 값으로 "재계산 필요" 배지와 결과 탭 경고를 띄운다(`RoundsTab.vue`).
표시는 안내일 뿐이고 **실제 차단은 백엔드가 한다** — `00_spec_round_and_scoring.md` §2.6.

### Score 타입 직렬화
- DB: `i64` (×100000)
- JSON 응답: `serde` `Serialize` → `f64`로 출력 (÷100000 자동)
- 프론트: 받은 값을 그대로 표시 (직접 ÷100000 계산 금지)

---

## 오류 처리 패턴 (프론트)

**blob 요청 오류**: `blobErrMsg(e)` 헬퍼 사용
- `e.response.data`가 `Blob`이면 `.text()`로 읽어 문자열 반환
- 일반 에러: `e.response?.data` 또는 `e.message`

**일반 API 오류**:
- `e.response?.data` — 서버가 평문 문자열 또는 JSON 오류 바디 반환
- `finalize_round` 422 오류: JSON `{error, track_violations, univ_violations}` 형태

**401 자동 처리**: auth store 인터셉터가 자동으로 `/login` 이동

**503 자동 처리**: auth store 인터셉터가 자동으로 `/server-error` 이동

---

## 잠재적 계약 불일치 주의 사항

1. **`score_detail` 타입**: 백엔드는 `String` (JSON 직렬화)를 커스텀 직렬화로 `Map<String, Score>`로 변환. 프론트가 `r.score_detail[area.id]`로 접근 시 key가 숫자형 area_id여야 함 → 백엔드는 `area.id.to_string()`으로 문자열 키 사용. 프론트는 `String(area.id)` 또는 동일한 문자열 키로 접근 필요.

2. **`recommended` nullable**: `ApplicationRow.recommended`는 `Option<bool>` (LEFT JOIN). 프론트에서 `null` 체크 필수.

3. **담임 결과 조회**: `teacherGetResults` → `{rounds, results}` 구조. `rounds`는 전체 라운드, `results`는 담당 학생의 FINALIZED 라운드 결과만.

4. **졸업생 담임 (grade=0, class_no=0)**: 비밀번호 변경 불가 (403). 학생 목록: `is_enrolled=0` 전체.

5. **정원 초과 확정 오류 (422)**: 일반 문자열이 아닌 JSON 바디. axios가 text/plain이어도 JSON 문자열을 자동 파싱해 객체로 만들므로, 문자열 가정 시 `[object Object]`가 표시된다. RoundsTab의 `finalizeErrMsg` 헬퍼가 위반 목록을 사람이 읽을 수 있는 줄 단위 텍스트로 펼친다:
   ```json
   {"error": "...", "track_violations": [...], "univ_violations": [...]}
   ```
