# Silent Fallback 허용 목록

이 프로젝트는 Fail-Fast 정책(CLAUDE.md §2)에 따라 `unwrap_or` 계열 표현을 전면 금지한다.
아래 목록은 전수 감사 후 허용 판정된 위치만 기록한다.

**새 예외 추가 규칙:**
1. 이 파일에 위치·이유·조건을 먼저 기록한다.
2. "점수 계산·학생 식별·추천 결정과 무관함"을 입증할 수 있어야 한다.
3. 상위 레이어에서 명시적 오류 처리가 보장됨을 코드로 확인한다.

---

## 허용 목록 (2026-05-30 전수 감사 기준)

### 1. `src/handlers/auth.rs` — `unwrap_or(false)` (admin_status)
`admin_password_hash` 행이 없으면 미초기화 상태 → `false`가 올바른 값. None이 "데이터 오류"가 아니라 "아직 설정 안 됨"을 뜻하는 유일한 정상 상태.  
**조건**: `admin_status` 엔드포인트에서만.

### 2. `src/handlers/auth.rs` — `unwrap_or_default()` (admin_login)
DB 행 없음 = 아직 비밀번호 미설정. 바로 다음 라인에서 `if hash.is_empty()` 분기로 최초 로그인 흐름 처리. 오류 은폐가 아니라 지연된 명시적 처리.  
**조건**: `admin_login` 함수의 이 위치에서만.

### 3. `src/handlers/external_import.rs` — `unwrap_or("")`, `unwrap_or_default()` (preview 힌트)
preview 응답의 힌트 필드로만 사용. 실제 import 시 대학명은 multipart form 필드에서 별도 수신. 파싱 실패가 import 데이터에 영향 없음.  
**조건**: preview 경로(`daegyo_preview`, `univ_preview`)에서만.

### 4. `src/handlers/external_import.rs` — `unwrap_or_default()` (헤더 행)
헤더 행 없으면 `col_map(&[])` = 빈 HashMap → 바로 다음 `require_cols` 루프에서 명시적 오류 반환. 지연된 명시적 오류.  
**조건**: 반드시 뒤따르는 `require_cols` 검증이 존재해야 함.

### 5. `src/excel.rs` — `unwrap_or("")` (`get_col`)
`get_col`은 항상 `require_cols` 이후에 호출된다는 설계 계약. 빈 문자열 = "행의 해당 열이 비어 있음". 비어있는 필수 필드는 상위 레이어에서 Err 처리.  
**조건**: `require_cols` 없이 단독으로 `get_col` 사용 시 허용 안 됨.

### 6. `src/handlers/students.rs` — `parse_i64 → Option<i64>`
`.ok()`가 반환한 `None`은 `upsert_student`에서 `(Some, Some, Some)` 패턴 매칭 실패 → 명시적 오류.  
**조건**: `upsert_student`의 None 검증 로직이 제거되면 위반.

### 7. `src/handlers/classes.rs` — `.parse().ok()` + `match Some(v) if v > 0`
`.ok()`가 None이어도 `_ =>` 분기에서 즉시 `errors.push` + `continue`. `.ok()` 자체가 silent가 아니라 패턴 매칭으로 오류를 처리하는 관용구.  
**조건**: `_` 분기에서 반드시 errors에 추가하고 행 처리를 건너뛰어야 함.

### 8. `src/handlers/scoring.rs` — `unwrap_or_else` (MatchMode::Lower 상한 초과 fallback)
value가 테이블 최대 threshold 초과 시 최대 구간 점수 반환 — 의도된 설계. `rows.is_empty()` 검사가 바로 위에서 통과됨.  
**조건**: 이 로직 앞에 `rows.is_empty()` 조기 반환이 반드시 존재해야 함.

### 9. `src/auth.rs` — `.unwrap()` (`duration_since(UNIX_EPOCH)`)
현재 시각이 1970년 이전일 수 없으므로 항상 Ok. 실패 시 panic으로 명시적 장애 발생.  
**조건**: `expiry_secs` 함수 내에서만.

### 10. `src/excel.rs` — `.unwrap()` (`xlsx_response`)
`Response::builder()`에 정상 status/header/body만 사용. 빌드 실패 경우 없음.  
**조건**: `xlsx_response` 함수 내부에서만. 헤더 값에 non-ASCII 포함 시 위반.

### 11. `src/handlers/teacher_areas.rs` — `.unwrap_or(v)` (`load_base_data_display`)
표시용 문자열 변환 경로만. 파싱 실패 시 원본 문자열 표시 → 사용자가 데이터 이상 인지 가능. 점수 계산 경로와 무관.  
**조건**: 점수 계산(`calc_area_score`)에 입력되는 경로가 아닌 순수 표시 경로에서만.

### 12. `src/handlers/teacher_areas.rs` — `.unwrap_or_default()` (`matched_keys`)
프론트엔드가 점수표 행을 하이라이팅하는 데만 사용. None이면 하이라이팅 없음. 점수 계산·추천 결정에 영향 없음.  
**조건**: `area-score-preview` 응답의 `matched_keys` 필드에서만.

### 13. `src/handlers/teacher_areas.rs` — `.unwrap()` (`CategoryAgg::Max`)
바로 위 `if scores.is_empty() { return Ok(...) }` 통과 후. `scores`가 비어있지 않으므로 `max()`는 반드시 `Some`.  
**조건**: 이 `unwrap()` 앞에 `scores.is_empty()` 조기 반환이 반드시 존재해야 함.

### 14. `src/handlers/universities.rs` — `.unwrap_or_default()` (HashMap remove)
라운드·모집단위가 없는 대학의 경우 빈 Vec가 올바른 값. 통계 표시용, 점수·추천과 무관.  
**조건**: `fetch_quota_stats` 내부의 통계 집계 경로에서만.

### 15. `src/handlers/universities.rs` — `.unwrap_or_else(|| "대학".to_string())` (파일명)
`univ_id`가 잘못된 경우 일반 파일명 "대학" 사용. 파일명 생성 경로, 사용자 데이터 영향 없음.  
**조건**: `export_quota_stats`의 파일명 생성 경로에서만.

### 16. `src/handlers/universities.rs` — `.unwrap_or(0)` (라운드별 추천 수)
특정 라운드에서 해당 모집단위 추천이 없으면 count=0이 올바른 값. Excel 내보내기 통계 경로, 점수·추천과 무관.  
**조건**: `export_quota_stats`의 Excel 렌더링 경로에서만.

### 17. `src/main.rs` — `.unwrap()` (SPA 정적 파일 핸들러)
`Response::builder()` status/body 모두 컴파일 타임에 유효성 보장. 실패 시 SPA 미제공, API 영향 없음.  
**조건**: `static_handler` 함수 내부에서만.

### 18. `src/handlers/students.rs` — `filter_map(|k| k.parse().ok())` (grade_options)
`by_grade`의 키는 DB `grade` 컬럼값으로 항상 정수 문자열. 파싱 실패 실제 발생 불가. 표시용 드롭다운 옵션, 점수·추천과 무관.  
**조건**: `grade_options` 엔드포인트의 표시 경로에서만.

### 19. `src/handlers/classes.rs` — `.unwrap_or("")` (Excel export `teacher_name`)
`teacher_name`이 NULL인 반(담임 미지정 상태)에서 빈 문자열 기재가 올바른 표현. Excel 내보내기 경로, 점수·추천과 무관.  
**조건**: `export_classes`의 Excel 렌더링 경로에서만.

### 20. `src/handlers/area_data.rs` — `.unwrap_or(0)` (단조성 경고 최소값)
`min_th` 계산 시 사용. `seen.iter().filter(...)`는 by construction 항상 비어있지 않아 실제 도달 불가 분기. 경고 메시지 문자열 생성 전용, 점수 계산 무관.  
**조건**: `numeric_table_import`의 단조성 경고 생성 경로에서만.

### 21. `src/handlers/auth.rs` — `.unwrap_or_default()` (teacher_login 졸업생 분기)
`teacher_login`의 grade=0/class_no=0 분기. 바로 다음 줄 `if hash.is_empty() { return Err(UNAUTHORIZED) }`로 명시적 처리. 허용 목록 #2(admin_login)와 동일 패턴.  
**조건**: `teacher_login` 함수의 졸업생 분기에서만.

### 22. `src/handlers/teacher_areas.rs` — `.unwrap_or_default()` (`matched_keys`, 라인 drift)
허용 목록 #12와 동일 위치. 코드 수정으로 라인 번호가 346으로 drift됨.  
**조건**: #12와 동일.

### 23. `src/main.rs` — `.unwrap_or_default()` (자동시작 exe 경로)
`std::env::current_exe()` 실패 시 빈 문자열 → autostart 레지스트리 등록이 잘못되지만 서버 시작·점수 계산에 영향 없음. Windows 정상 환경에서 발생 불가.  
**조건**: `main.rs` autostart 초기화 경로에서만.

### 24. `src/handlers/system.rs` — `.unwrap_or(Path::new("."))` (백업 임시 파일 경로)
`db_path.parent()`가 None인 경우(루트 경로) 현재 디렉토리 fallback. 백업 다운로드 기능에만 영향. 실패 시 사용자에게 파일 다운로드 오류 표시됨. 점수·추천과 무관.  
**조건**: `download_db_backup`의 임시 파일 경로 생성에서만.

### 25. (해소됨) `src/handlers/rounds.rs` / `src/handlers/scoring.rs` — `ROLLBACK ... .ok()`
2026-07-15 수정으로 수동 `BEGIN IMMEDIATE`/`COMMIT`/`ROLLBACK` 패턴을 sqlx 관리 트랜잭션(`Pool::begin_with("BEGIN IMMEDIATE")`)으로 전환. 오류 경로는 tx drop 시 sqlx가 롤백을 관리하며, 실패한 커넥션이 열린 tx를 문 채 풀로 반환되는 경로가 사라짐. 더 이상 `.ok()` 호출 없음 — 번호 유지를 위해 항목만 남긴다.

### 26. `src/handlers/system.rs` — `remove_file(...).ok()` (백업 임시 파일 정리)
백업 응답 생성 후 임시 파일 삭제 실패는 디스크에 잔존 파일만 남길 뿐 다운로드 결과·점수·추천에 영향 없음.  
**조건**: `download_db_backup`의 임시 파일 정리 경로에서만.

### 27. (해소됨) `src/main.rs` — `create_dir_all(...).ok()` (`data_dir`)
2026-07-15 수정으로 `data_dir`가 `anyhow::Result`를 반환하며 경로 취득·디렉토리 생성 실패를 즉시 전파한다 (dev: 기동 중단, release: 오류 대화상자 후 종료). 더 이상 fallback 아님 — 번호 유지를 위해 항목만 남긴다.

### 28. `src/main.rs` — `.unwrap_or(true)` (`get_autostart` 행 없음 기본값)
`app_configs`에 autostart 행이 없는 것은 오류가 아니라 "최초 실행 미설정" 상태 — 기본값 활성화가 올바른 값. DB 조회 **오류**는 별도로 `Err`로 전파되어 호출자가 레지스트리를 변경하지 않는다 (#1 admin_status와 동일 패턴).  
**조건**: 쿼리 성공 후 `Option::unwrap_or`에만 적용. `Result`에 대한 fallback으로 확장 시 위반.
