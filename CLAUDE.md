# PCM — 학교장추천자 선발 관리 시스템

한국 고등학교 학교장추천전형 점수·순위·추천 관리 시스템. 내부망(LAN) 단일 exe 배포.

**스택**: Rust + Axum + SQLx + SQLite / Vue 3 + Vite + Tailwind CSS v4  
**스키마**: `migrations/v1.sql` 직접 수정 (배포 전이므로 v2.sql 추가 금지, DB 재생성)  
**커밋**: 반드시 GPG 서명 (`--no-gpg-sign` 우회 금지)  
**테스트**: `cargo run` / `npm run dev` 금지 — 검증은 테스트 코드로

---

## 절대 규칙

**1. Float-Free**: 모든 점수·측정값은 ×100000 정수로 DB 저장. 프론트엔드에서 ÷100000 수동 계산 금지 — `Score` newtype이 자동 처리.

**2. Fail-Fast**: 점수 계산 오류는 즉시 `Err` 반환. `unwrap_or(0)` / `unwrap_or_default()` 등 silent fallback 전면 금지. 허용 예외는 `src/docs/silent_fallback_allowed.md`에 명시된 위치만.

**3. 점수 계산은 백엔드 전담**: 프론트엔드는 표시만. 점수 미리보기도 API 호출.

**4. Import는 All-or-Nothing**: 오류 하나라도 rollback + 422. 부분 저장 없음. 중복 행은 error (warning 아님).

**5. Excel 파싱은 헤더 이름 기반**: 열 인덱스(`cols[0]`) 직접 참조 금지. `excel::col_map` + `require_cols` 사용.

**6. 다중 쓰기는 트랜잭션**: DELETE+INSERT, 루프 INSERT/UPDATE는 반드시 tx. `find_or_create_track`은 항상 `&mut *tx` 전달 (pool 직접 전달 금지).

**7. base_data bulk delete는 student_type 필터 필수**: 재학생 업로드가 졸업생 데이터를 지우면 점수 계산 실패.

**8. 폰트 최소 `text-base`**: `text-sm` / `text-xs` / `font-size: 14px` 이하 금지.
