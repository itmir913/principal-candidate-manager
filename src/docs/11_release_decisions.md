# 11. 배포·운영 계층 결정

배포 시점에 확정한 운영·인프라·미구현 결정을 모아 둔다. 코드 감사 결과가 아니라
소유자·오케스트레이터 판단의 최종 상태 명세다. 특정 코드 영역이 아니라 시스템 전체
계층의 결정이라 별도 파일로 유지한다.

각 항목마다 **미래 개발자가 이 결정을 뒤집기 전에 확인할 근거**를 밝힌다. 관행이나
표준을 이유로 이 결정을 무비판적으로 바꾸지 않도록 하기 위함이다.

---

## 1. 단일 활성 라운드 제약

**결정**: 진행 중(비-FINALIZED) 라운드는 전교에서 최대 1개.

**구현**: `migrations/v1/003-rounds.sql:19-20`
```sql
CREATE UNIQUE INDEX IF NOT EXISTS idx_one_active_round
    ON rounds((1)) WHERE status != 'FINALIZED';
```

표현식 유니크 인덱스가 `(1)` 상수를 키로 하므로 `status != 'FINALIZED'`인 행이 전체
DB에서 1개로 강제된다. 앱 레벨의 `open_round` 원자적 검사(`INSERT ... WHERE NOT
EXISTS`)에 대한 DB 방어선.

**배경**: 학교장추천전형은 사실상 3학년·졸업생 대상이라 1·2학년이 각자 라운드를 병행할
시나리오가 없다. 여러 학년이 병행하고 싶다면 이 서버 프로그램을 학년 수만큼 독립
실행하도록 배포가 간편하게 구성되어 있다(단일 exe + `config.json` port 지정).

**뒤집기 전 확인**: 실제 학교 운영에서 다중 학년 동시 라운드가 요구된다면, 다중 인스턴스
배포가 아니라 스키마 변경(예: 학년별 `active_round`)이 정말 필요한지 소유자와 확인.
스키마 변경은 감사 재실행 대상이 될 만큼 큰 결정이다.

---

## 2. `busy_timeout` 명시 (기본값 명시)

**결정**: `SqliteConnectOptions`에 `busy_timeout(Duration::from_secs(5))` 명시.

**구현**: `src/db.rs::init_pool`

**배경**: sqlx-sqlite 0.8.6 기본값이 이미 5초이므로 명시 시점의 동작 변화는 없다.
sqlx 버전 업그레이드 시 기본값이 조용히 바뀌는 것을 방지하기 위한 방어.

**뒤집기 전 확인**: 실측한 워크로드가 5초를 초과하는 tx를 발생시키는 것이 확인되면
값 조정 가능. 다만 `close_round`·`run_auto_recommend`·대량 base_data import 정도가
5초를 넘길 후보인데, LAN 학교 규모(수십 명, 시즌성 사용)에서 실측 초과 사례는 아직
없다.

---

## 3. 파일 로깅

**결정**: `pcm/logs/pcm.<yyyy-MM-dd>.log` 일별 롤링, 무제한 보관, `info` 이상 레벨.

**구현**: `src/main.rs::init_logging`
- crate: `tracing-appender 0.2`
- 경로: `data_dir()/logs/` (즉 `exe_dir/pcm/logs/`)
- 롤링: `tracing_appender::rolling::Builder` + `Rotation::DAILY`
  (`rolling::daily` 헬퍼가 아니다 — prefix/suffix를 따로 지정하기 위해 Builder를 쓴다)
- 파일명: prefix `pcm` + suffix `log` → `pcm.2026-07-21.log`
  (확장자가 뒤에 오도록 커밋 `7d35d44`에서 `pcm.log.<날짜>`에서 바꿨다.
  `.log` 연결 프로그램으로 바로 열리게 하기 위함)
- dev·release 둘 다 콘솔 + 파일 이중 로깅
- 로그 디렉토리 생성 실패 시 콘솔 로그만으로 폴백 (서버 기동은 계속)

**배경**: release 빌드는 `#![windows_subsystem = "windows"]` 때문에 콘솔이 없어
stdout 로그가 소멸된다. 배포 후 학교 담당자가 오류 진단할 재료가 없어지는 문제를
방지하기 위해 파일 로깅이 실질적인 진단 창구.

**보관 정책 무제한 이유**: 학교 규모에서 로그 크기가 문제될 만큼 커지지 않는다.
자동 삭제 정책을 넣으면 오래된 사고의 원인 추적이 불가능해질 수 있다. 필요 시 학교
담당자가 수동 삭제.

**뒤집기 전 확인**: 자동 삭제 정책을 도입하려면 실제 로그 크기 증가 추이를 확인하고
"몇 년치 로그가 필요한가"의 운영 요구를 소유자와 확인.

---

## 4. 업로드 상한 20MB + 413 한국어 응답

**결정**: axum `DefaultBodyLimit::max(20 * 1024 * 1024)` + 초과 시 한국어 413 응답.

**구현**:
- 상수: `src/middleware.rs::UPLOAD_LIMIT_BYTES`
- 미들웨어: `src/middleware.rs::korean_body_limit_message` (라우터 최상위)
- Multipart 오류 매핑: `src/middleware.rs::multipart_err`

**배경**: axum 0.7 기본 2MB는 NEIS류 시스템에서 내려받은 서식·수식 포함 파일이나
대형 COMPOSITE 점수표에 부족할 수 있다. 20MB 여유. 초과 시 기본 영문 응답 대신
"업로드 파일 크기가 너무 큽니다 (최대 20MB)" 한국어 안내.

**핸들러 오류 매핑 규약**: 4개 import 핸들러(`students`/`classes`/`area_data`/
`external_import`)는 `Multipart::next_field().await` 결과의 `MultipartError`를
반드시 `multipart_err`로 매핑한다. 크기 초과(413)만 한국어 매핑, 그 외 파싱 오류는
기존 400 유지.

**뒤집기 전 확인**: 상한을 낮추려면 실제 파일 샘플 크기 확인. 상한을 크게 올리려면
서버 메모리 사용량(multipart 처리는 body를 메모리에 로드)과 학교 서버 사양 확인.

---

## 5. 담임 로그인 brute-force 방어 미구현

세부 명세는 `01_auth.md`의 "담임 로그인 brute-force 방어 (미구현)" 절 참조.

**요약**: 소유자 결정(2026-07-21)으로 릴리즈 시점에 미구현 상태 유지. 다음 라운드에서
지수 백오프 방식으로 도입 예정. 표준 관행(임계값 잠금) 대신 지수 백오프를 선택한
근거는 해당 절에 명시.

---

## 6. 라이선스

**결정**: PolyForm Noncommercial 1.0.0 유지.

**구현**: `LICENSE`, `README.md`

**배경**: 이 시스템은 상업 판매 계획이 없다. 학교 내부 배포 및 교육 목적 사용만
전제. PolyForm Noncommercial 1.0.0의 "educational institution" 사용 허용 조항이
정합한다(README.md의 소유자 해석 참조).

**뒤집기 전 확인**: 상업 판매를 검토하게 되면 재라이선스가 법적 선결 사항. 다중
관리자·복원 절차·rate limit·감사 로그 크기 관리 등 상업 배포 시점에 함께 재점검할
항목이 있다.

---

## 참고 — 이월 항목 (다음 라운드에서 판단)

배포 이후 별도 라운드에서 판단·구현 예정인 항목들. 이 명세 파일에는 결정 사항만
남기지만, 이월 항목이 있다는 사실 자체를 명시해둔다.

- 담임 로그인 지수 백오프 도입 (본 문서 §5)
- 프론트엔드 감사 1회
- 구조적 테스트 강화 (라우터 권한 테스트, fault-injection)
- 학년 전환(3월) 데이터 이관 절차 명세
- 운영 매뉴얼 (백업 복원 실측 검증, 관리자 비번 분실 대응, 인수인계)

운영 매뉴얼은 코드 명세가 아니라 사용자 대상 문서라 별도 매뉴얼 갱신 프로젝트에서
다룬다.
