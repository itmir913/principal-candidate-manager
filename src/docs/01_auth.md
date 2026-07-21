# 01. 인증·권한 명세

## 관리자 로그인 (`POST /api/auth/admin`)

**입력값 검증**
- 요청 바디에서 `password` 문자열 하나만 받는다.
- 별도의 형식 검증(길이 등)은 없으며, DB에서 기존 해시를 조회해 초기화 여부를 판단한다.

**초기 비밀번호 설정 여부 판단 기준**
- `app_configs` 테이블에서 `key = 'admin_password_hash'`인 행을 조회한다.
- 행이 없거나 값이 빈 문자열(`''`)이면 **미초기화 상태**로 판단한다.
- 미초기화 상태일 때 로그인을 시도하면, 입력된 비밀번호가 그대로 최초 비밀번호로 등록된다. 즉, 첫 로그인이 곧 비밀번호 설정이다.

**bcrypt 비교 흐름**
- 초기화가 된 경우에는 `bcrypt::verify`로 입력값과 저장된 해시를 비교한다.
- 불일치 시 401 Unauthorized 반환.

**JWT 발급**
- 검증 통과 후 `encode_admin_token`을 호출해 토큰을 발급한다.
- 토큰 만료 시간은 발급 시점 기준 12시간.

---

## 담임 로그인 (`POST /api/auth/teacher`)

**입력값**: `grade`, `class_no`, `password` 세 가지.

**특수 계정 처리 (졸업생 담당 계정)**
- `grade=0, class_no=0` 조합을 수신하면 졸업생 전용 특수 계정으로 처리한다.
- 이 경우 `classes` 테이블이 아닌 관리자 비밀번호(`app_configs.admin_password_hash`)로 인증한다.
- 관리자 비밀번호가 미설정 상태이면 401 반환.
- 인증 성공 시 `grade=0, class_no=0`으로 교사 토큰을 발급하고, `teacher_name`을 `"졸업생"`으로 고정 응답한다.

**일반 담임 계정 처리**
- `classes` 테이블에서 `grade = ?` AND `class_no = ?`인 행을 조회한다.
- 행이 없으면 404 Not Found.
- `password_hash`가 비어 있으면 401 (비밀번호 미설정).
- `bcrypt::verify`로 비교 후 통과 시 해당 grade, class_no를 담은 교사 토큰 발급.
- 응답에 `grade`, `class_no`, `teacher_name`을 함께 반환한다.

---

## JWT 구조

**AdminClaims**
- `role`: 고정값 `"admin"`
- `exp`: Unix timestamp (발급 시각 + 12시간)

**TeacherClaims**
- `role`: 고정값 `"teacher"`
- `grade`: 학년 (i64)
- `class_no`: 반 번호 (i64)
- `exp`: Unix timestamp (발급 시각 + 12시간)

**시크릿 관리**
- 서버 시작 시 `OsRng`로 32바이트 난수를 생성해 hex 문자열로 변환, 메모리(`AppState.jwt_secret`)에만 보관한다.
- DB에 저장하지 않으므로 서버 재시작 시 시크릿이 새로 생성되고 기존 토큰은 즉시 무효화된다. 이는 **의도된 설계**이다.

---

## 미들웨어

**`require_admin`**
- `Authorization: Bearer <token>` 헤더에서 토큰 추출.
- `decode_admin_token`으로 디코딩하면서 `role == "admin"` 여부까지 동시에 검증.
- 성공 시 `AdminClaims`를 request extension에 삽입해 다음 핸들러에서 `Extension<AdminClaims>`로 추출 가능하게 한다.
- 실패(토큰 없음, 파싱 실패, 만료, role 불일치) 시 401 Unauthorized + 문자열 메시지 반환.

**`require_teacher`**
- 동일한 구조로 동작하며, `decode_teacher_token`으로 `role == "teacher"` 검증.
- 성공 시 `TeacherClaims`를 extension에 삽입.
- 실패 시 401 Unauthorized.

**두 미들웨어의 차이**
- 검증 대상 Claims 타입과 role 값이 다를 뿐, 동작 방식은 동일하다.
- 관리자 라우트와 담임 라우트가 완전히 분리된 라우터 레이어에 각각 적용된다.

---

## 비밀번호 변경

**관리자 비밀번호 변경 (`PUT /api/auth/admin/password`)**
- `require_admin` 미들웨어를 통과해야 호출 가능. 즉, 로그인된 관리자만 변경 가능.
- 처리 순서: ① 현재 비밀번호 `bcrypt::verify` 확인 → 불일치 시 **400 Bad Request** 반환 → ② 새 비밀번호 길이 검증(8자 미만이면 400) → ③ bcrypt 해시 계산 → ④ DB UPDATE.
- bcrypt 해시 계산은 DB 접근 전 미리 수행 (CPU 집약 작업이므로 트랜잭션 없이 단순 UPDATE 전에 처리).
- 성공 시 204 No Content.

**담임 비밀번호 변경 (`PUT /api/teacher/password`)**
- `require_teacher` 미들웨어를 통과해야 호출 가능.
- 졸업생 특수 계정(`grade=0, class_no=0`)은 비밀번호 변경 불가 → 403 Forbidden.
- 새 비밀번호 길이 검증: 4자 미만이면 400 Bad Request (관리자보다 기준이 낮음).
- 현재 비밀번호 검증 후 bcrypt 해시를 계산한 다음 `classes` 테이블 업데이트.
- bcrypt 계산은 마찬가지로 DB 접근 전 미리 수행.
- 성공 시 204 No Content.

**설계 의도 추정**: 담임은 간단한 숫자 비밀번호도 허용하기 위해 4자 기준을 적용한 것으로 보인다.

**감사 로그와 원자성**: 관리자·담임 비밀번호 변경 모두 `UPDATE`와 `audit::log_with_ip`
호출을 같은 트랜잭션(`state.db.begin()`)으로 묶어 원자성을 확보한다. bcrypt 계산은
tx 진입 전에 완료해 tx 보유 시간을 최소화한다.

---

## 감사 로그 (`audit::log_with_ip`, `audit_log` 테이블)

### 기본 규약

모든 `AuditAction` variant(`src/enums.rs`)는 본 작업과 **같은 트랜잭션**에서 기록한다.
`audit::log` (IP 없음) 또는 `audit::log_with_ip` (IP 포함) 두 함수만 진입점이며,
호출자는 반드시 `&mut *tx`로 전달한다 (pool 직접 전달 금지). 실패 시 Err 전파 →
본 작업까지 롤백 (fail-fast).

`audit_log` 테이블은 DB 트리거로 UPDATE·DELETE가 전면 차단되어 있어 SQL 직접
접근으로도 위변조 불가.

### `actor_ip` 필드 규약

계정 보안 이벤트에 한해 요청 클라이언트 IP를 함께 기록한다. 이외 액션은 `NULL`.

**IP 필수 액션** (`audit::log_with_ip` 사용):

| AuditAction | 배경 |
|---|---|
| `DbBackupDownloaded` | 전교생 PII 전량 반출. 사고 시 "어느 단말에서" 반출됐는지 필수 |
| `TeacherPasswordChanged` | 계정 탈취 공격의 전형적 첫 행동. 언제 어느 단말에서 바뀌었는지 추적 근거 |
| `AdminPasswordChanged` | 관리자 계정 탈취 시 최우선 조사 대상 |

**IP 미기록 액션**: 위 3종 외 나머지. 라운드·추천·지원·학급·학생·전형요소·대학·모집단위
관련 액션은 관리자·담임 워크플로의 정상 흐름이므로 IP 없이 actor 정보(관리자/담임 grade·class_no·teacher_name)만으로 충분.

### IP 캡처 방식

`main.rs`의 `axum::serve`가 `into_make_service_with_connect_info::<SocketAddr>()`로
감싸져 있어, 핸들러가 `ConnectInfo<SocketAddr>`로 클라이언트 IP를 추출할 수 있다.
계정 보안 이벤트 핸들러는 이 익스트랙터를 매개변수에 포함하고 `client.ip().to_string()`을
`audit::log_with_ip`에 전달한다.

LAN 배포이므로 대개 사설 IP(예: `192.168.x.x`, `10.x.x.x`)지만, 학교 IT 담당자가
어느 컴퓨터(교무실·담임 노트북 등)에서 발생했는지 특정하는 최소 단서가 된다.

### 새 액션 추가 시

새 `AuditAction`을 추가할 때 다음을 판단하라.

1. **계정 보안·PII 반출 관련인가**: 그렇다면 `log_with_ip` 사용 + 위 표에 등재
2. **정상 워크플로 이벤트인가**: `log` 사용 (IP 없음)

애매하면 `log_with_ip`가 안전한 기본값. 스키마상 `actor_ip`는 nullable이라 나중에
필드를 채우도록 정책이 바뀌어도 하위 호환.

---

## 담임 로그인 brute-force 방어 (미구현)

**현재 상태**: 담임 로그인(`teacher_login`)에 rate limit·backoff 방어가 없다.
담임 비밀번호 최소 4자 + `/api/classes`가 무인증으로 반 목록 노출하는 조합 때문에,
학교 LAN 안에서 같은 반 학생이 자기 담임 계정을 표적화한 시도가 이론적으로 가능하다.

**소유자 결정 (2026-07-21)**: 미구현 상태로 릴리즈. 다음 라운드에서 도입 예정.

**도입 예정 설계**: **지수 백오프**. 응답 자체를 지연시키는 방식.

- 실패 카운터 증가에 따라 지연: `0 → 0.5s → 1s → 2s → 3s → 5s`, 5초에서 캡
- 성공 로그인 시 카운터 리셋
- 상태는 인메모리 HashMap (재시작 시 초기화, 단일 exe라 재시작이 자연스러운 복구)
- 카운터 키 단위, 감쇠 정책, `admin_login` 적용 여부는 도입 시점 소유자 확정

**표준 관행(임계값 잠금) 대신 지수 백오프를 선택한 이유**:

- 잠금 상태 관리 불필요 (`locked_until` 없음, "당신은 잠겼다" UI 없음)
- 정상 담임의 첫 실패는 즉시 응답 → UX 부담 낮음
- 같은 반 학생 우발적 다중 오입력이 담임 본인을 잠그지 않음
- LAN 학교 규모(수십 명 동접)에서 tokio sleep 자원 문제 없음
- 방어 강도: 5초 캡에서 초당 0.2회로 제한, 4자리 숫자 비번도 브루트포스 실질 불가

**주의**: 이 미구현 상태를 발견한 미래 개발자가 별도 판단으로 잠금 방식을 급하게
구현하지 말 것. 표준 관행이 아니라 이 시스템 컨텍스트(LAN, 우호적 사용자, 소규모)에
맞춘 결정이다.