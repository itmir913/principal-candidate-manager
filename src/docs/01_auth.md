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
- 현재 비밀번호 검증: 기존 해시를 조회해 `bcrypt::verify`로 확인.
- 새 비밀번호 길이 검증: 8자 미만이면 400 Bad Request.
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