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

### 13. `src/handlers/scoring.rs::compute_area_score` — `.expect()` (`CategoryAgg::Max`)
바로 위에서 범주 0건을 오류로 걸러내므로 `scores`가 비어있지 않고 `max()`는 반드시 `Some`.  
**조건**: 이 `expect()` 앞에 "범주 0건 → Err" 검사가 반드시 존재해야 함.  
**이동 이력**: 원래 `teacher_areas.rs`의 `.unwrap()`이었으나 2026-07-19 공용 헬퍼
(`compute_area_score`) 추출로 `scoring.rs`로 옮겨졌다. 문서가 옛 위치를 가리키던 것을
2026-08-18 감사에서 발견해 바로잡았다(F-016).

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

### 24. (해소됨) `src/handlers/system.rs` — `.unwrap_or(Path::new("."))` (백업 임시 파일 경로)
폴백이 제거됐다. 현재 코드(`system.rs::download_db_backup`)는 `ok_or_else`로 **즉시 500**을 반환한다 —
자동시작 시 CWD가 System32라 전교생 PII가 담긴 파일이 조용히 엉뚱한 위치에 생기기 때문이다.
더 이상 폴백 없음 — 번호 유지를 위해 항목만 남긴다.
문서가 제거된 폴백을 계속 허용으로 기재하던 것을 2026-08-18 감사에서 발견했다(F-002).

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

### 29. `src/handlers/external_import.rs` — 석차 값 누락·변환 실패 시 행 skip (`do_import`)
석차연명부(대교협·유니브)에서 **석차 값 열이 비어 있거나 `parse_display_value`가 실패**하면 error(전체 422)가 아니라 warning + 해당 행 skip으로 처리한다. 전출·자퇴 학생은 외부 프로그램이 등급을 `-`/공백으로 내보내며, 이를 전체 거부로 다루면 관리자가 매 업로드마다 원본 파일을 손대야 하고 그 과정에서 정상 행을 지울 위험이 생긴다.

**은폐가 아닌 근거**:
- skip된 학생은 잘못된 값이 저장되는 것이 아니라 **base_data가 존재하지 않는 상태**로 남는다. 값 오염 경로가 아니다.
- 건너뛴 행은 학년·반·번호·이름과 원본 값이 포함된 warning으로 **매 업로드마다 관리자에게 전량 표시**된다 (`ImportResultBox`).
- 그 학생이 실제로 지원하면 `close_round`의 base_data 누락 검증(`rounds.rs:113-146`)에서 422로 다시 막힌다 — 점수가 조용히 0으로 계산되는 경로가 없다.
- 모든 행이 skip되어 저장 행이 0건이면 422로 거부한다 (값 열을 잘못 고른 파일이 "완료 — 0건"으로 통과하는 것 방지).

**조건**:
- `external_import.rs::do_import`의 **석차 값 한 곳에서만**. 학년·반·번호 변환 실패, 미등록 학생, 파일 내 중복 행은 종전대로 error → 전체 422 (학생 식별 실패는 skip 대상이 아님).
- 기초 데이터 업로드(`area_data.rs::base_data_import`)와 점수 기준 import로 확장 금지 — 그 경로들은 값 누락이 곧 관리자 입력 실수다.
- skip 사유 warning을 응답에서 빼면 위반. 명세: `08_excel_import.md` §7-1

---

## 2026-08-18 추가 (F-015) — 목록 밖에 있던 12곳

채점 정합성 감사의 Fail-Fast 전수 조사에서 **목록에 없는** silent fallback 12곳이 발견됐다.
전부 "점수·학생 식별·추천 결정과 무관" 요건은 만족하지만, CLAUDE.md 규칙 2는
"허용 예외는 이 문서에 명시된 위치만"이라고 규정하므로 목록에 없는 것 자체가 규칙 위반이었다.
검토 후 아래와 같이 등재한다.

### 30. `src/handlers/scoring.rs::write_roster_sheet` — `excluded_reason.as_deref().unwrap_or("")`
미선발이 아니면 사유는 NULL이고, 엑셀에는 빈 칸이 올바른 값이다. #19와 동형.  
**조건**: 명단 시트의 "미선발 사유" 열 기록에서만.

### 31. `src/handlers/scoring.rs::run_auto_recommend` — `univ_pool.remove(&univ_id).unwrap_or_default()`
순회하는 `univ_ids`가 `univ_pool.keys()`에서 나오므로 None은 도달 불가.  
**조건**: 같은 함수 안에서 키 출처가 `univ_pool` 자신일 것.

### 32. `src/handlers/universities.rs::export_quota_stats` — `apab.get(..).copied().unwrap_or((0, 0))`
지원 0건 모집단위는 (지원 0, 포기 0)이 올바른 값. #16과 동형.  
**조건**: 통계 집계 경로에서만.

### 33. `src/handlers/universities.rs::settings_export` — `by_univ.remove(&u.id).unwrap_or_default()`
모집단위가 없는 대학은 빈 Vec가 올바른 값. #14와 동형.

### 34. `src/handlers/universities.rs::compute_settings_changes` — `accs.remove(&name).unwrap()`
`order`의 원소가 모두 `accs`에 존재하도록 바로 위에서 함께 채운다.  
**조건**: `order`와 `accs`를 같은 루프에서 채울 것.

### 35. `src/handlers/areas.rs::score_template` — `Response::builder()...unwrap()`
상수 status·header로만 만드는 응답이라 실패 경로가 없다. #10·#17과 동형.

### 36·37. `src/main.rs` — `unwrap_or_else(|_| "info".into())` (로그 레벨 2곳)
`RUST_LOG` 파싱 실패 시 기본 로그 레벨. 로깅 설정이며 점수·추천과 무관.

### 38·39. `src/main.rs` — `let _ = ready_tx.send(..)` (기동 신호 2곳)
수신자가 이미 drop된 경우(기동 중 종료) 신호를 버린다. 서버 기동 경로이며 데이터와 무관.

### 40. `src/main.rs` — `let _ = webbrowser::open(&url)`
브라우저 자동 실행 실패는 사용자가 직접 주소를 열면 된다. 기능 동작과 무관.

### 41. `src/middleware.rs::bearer_token` — `.and_then(|v| v.to_str().ok())`
Authorization 헤더가 비-ASCII라 디코드 실패하면 "토큰 없음"으로 처리되고, 호출자가 **401을
명시적으로 반환**한다. 조용히 통과시키는 것이 아니라 **지연된 명시적 오류**다.  
**조건**: 이 값을 받는 쪽이 None을 반드시 401로 변환할 것.
