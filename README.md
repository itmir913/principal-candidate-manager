# 학교장 추천자 선발 관리 시스템

> Principal Candidate Manager — 고등학교 학교장추천전형 지원자 점수 계산·순위·추천 관리 도구

[![GitHub release](https://img.shields.io/github/v/release/itmir913/principal-candidate-manager)](https://github.com/itmir913/principal-candidate-manager/releases/latest)
[![License: PolyForm Noncommercial](https://img.shields.io/badge/License-PolyForm%20Noncommercial%201.0.0-green)](https://polyformproject.org/licenses/noncommercial/1.0.0)
[![Platform: Windows](https://img.shields.io/badge/Platform-Windows-0078D4?logo=windows)](https://github.com/itmir913/principal-candidate-manager/releases/latest)

---

## 소개

**학교장 추천자 선발 관리 시스템**은 고등학교에서 학교장추천전형 지원자를 선발할 때 필요한 전 과정을 지원하는 내부망(LAN) 전용 웹 애플리케이션입니다.

- 단일 `.exe` 파일로 배포 — 별도 설치·서버 운영 불필요
- 내부망(LAN)에서 담임교사가 브라우저로 접속해 지원자 정보를 직접 입력
- 관리자가 점수 계산·순위 확인·추천 확정을 한 화면에서 처리
- Excel 일괄 업로드·다운로드로 기존 업무 흐름 유지

---

## 주요 기능

### 관리자

| 기능 | 설명                                                                 |
|------|--------------------------------------------------------------------|
| **학생 관리** | 재학생·졸업생 기초데이터 Excel 일괄 업로드                                         |
| **학급·담임 관리** | 학년·반별 담임교사 계정 일괄 생성 및 비밀번호 관리                                      |
| **대학·모집단위 관리** | 지원 가능 대학과 모집단위(트랙), 모집 정원 설정                                       |
| **전형요소 설정** | NUMERIC / CATEGORY / MANUAL 세 가지 유형, 전역(SIMPLE) / 조합별(COMPOSITE) 범위 설정 |
| **라운드 관리** | OPEN → CLOSED → FINALIZED 상태 전이, 라운드별 지원 현황·점수 확인                  |
| **추천 확정** | 점수 순으로 정렬된 목록에서 담당자가 추천 확정·취소 가능, 동점자 초과 시 수동 선택                   |
| **결과 내보내기** | 전체 결과 Excel 파일 다운로드                                                |
| **DB 백업** | 현재 데이터베이스 파일 즉시 다운로드                                               |
| **버전 확인** | GitHub 최신 릴리스와 현재 버전 자동 비교                                         |

### 담임교사

| 기능 | 설명 |
|------|------|
| **지원자 등록** | 담당 학급 학생 중 지원자를 선택하고 전형요소 값 입력 |
| **점수 미리보기** | 입력한 값으로 예상 점수를 실시간 확인 (백엔드 계산) |
| **결과 조회** | 라운드 종료 후 담당 학생의 최종 결과 확인 |
| **지원 포기** | FINALIZED 이후 추천 확정된 학생의 지원 포기 처리 |

### 점수 계산

- **Float-Free 아키텍처**: 모든 점수는 ×100,000 정수로 저장해 부동소수점 오류 원천 차단
- **백엔드 전담**: 점수 계산 로직은 전부 서버에서 실행, 프론트엔드는 표시 전용
- **Fail-Fast**: 점수 계산 오류 시 즉시 `Err` 반환, 묵시적 기본값(`0`) 사용 없음

---

## 기술 스택

| 구분 | 기술                                  |
|------|-------------------------------------|
| 백엔드 | Rust, Axum, SQLx, SQLite            |
| 프론트엔드 | Vue 3, Vite, Tailwind CSS v4        |
| 배포 | rust-embed (프론트엔드 정적 파일 바이너리 내장)    |
| 플랫폼 | Windows Only (시스템 트레이, 자동 실행 레지스트리) |

---

## 시작하기

### 요구 환경

- Windows 10/11 x64
- (빌드 시) Rust stable, Node.js 18+, npm

### 설치 및 실행

1. [Releases](https://github.com/itmir913/principal-candidate-manager/releases/latest) 페이지에서 최신 `.zip` 파일을 내려받습니다.
2. 원하는 폴더에 압축을 풉니다.
3. `principal-candidate-manager.exe`를 실행합니다.
4. 시스템 트레이(화면 오른쪽 아래)에 생긴 아이콘을 클릭한 후 **열기**를 선택합니다. 브라우저가 열리지 않으면 `http://localhost:8080`으로 직접 접속합니다.

> **데이터 경로**: 실행 파일 옆 `pcm\data.db` 에 모든 데이터가 저장됩니다.

### 포트 변경

실행 파일 옆 `pcm\config.json`을 수정합니다.

```json
{
  "port": 8080
}
```

---

## 개발 환경 설정

```bash
# 저장소 클론
git clone https://github.com/itmir913/principal-candidate-manager.git
cd principal-candidate-manager

# 의존성 일괄 설치 (npm 패키지 + cargo-watch)
npm run setup
```

### 개발 서버 실행

```bash
# Rust 백엔드 + Vite 프론트엔드 동시 실행 (백엔드 준비 후 프론트엔드 자동 시작)
npm run dev

# 백엔드 소스 변경 시 자동 재시작 (cargo-watch 사용)
npm run dev:watch
```

### 릴리스 빌드

```bash
npm run build
```

빌드 결과물은 `target/release/principal-candidate-manager.exe` 입니다.

### 테스트

```bash
npm run test
```

> PR이 올라오면 `.github/workflows/test.yml`이 같은 `npm test`를 자동 실행합니다.
> `master` 직접 푸시는 CI를 거치지 않으므로 로컬 확인이 필요합니다.
> 프론트엔드에는 자동화 테스트가 없으므로 Vue 변경은 사람이 확인해야 합니다.

---

## 기여

이 프로젝트는 실제 고등학교의 입시 자료를 다루므로, 점수·순위 계산에 관한 규칙이
엄격합니다. PR을 보내기 전에 [기여 안내(CONTRIBUTING.md)](CONTRIBUTING.md)를 읽어 주세요.

- **기여 라이선스**: 기여물의 저작권은 프로젝트 소유자에게 양도됩니다 (CONTRIBUTING.md 참조)
- **절대 규칙**: Float-Free(점수 ×100000 정수), Fail-Fast(silent fallback 금지),
  점수 계산 백엔드 전담, Import All-or-Nothing 등 8개
- **커밋**: GPG 서명 필수, Conventional Commits
- **행동 규범**: [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)

이슈·PR에 **학생 개인정보(이름·학번·성적)를 그대로 올리지 마세요.**
재현용 자료는 가명 처리해 주시기 바랍니다.

---

## 라이선스

본 프로젝트는 PolyForm Noncommercial 1.0.0 라이선스를 따릅니다. ([LICENSE](LICENSE) 파일 참조) **교육 및 비상업적 목적**에 한해 자유롭게 사용할 수 있으며, **상업적 이용은 엄격히 금지**됩니다.

본 섹션은 동 라이선스에 대한 저작권자의 공식 해석 및 추가 조건(Additional Terms)으로 간주됩니다.

### 허용되는 사용

본 프로그램은 공익적 목적의 학교 교육을 지원하기 위해 개발되었습니다.

- 공교육 학교(공립·사립) 관리자 및 교사
- 교육청 및 공공 교육기관
- 교사 개인이 제작하는 **무료** 소개·활용 강의 또는 영상 콘텐츠
- 교육청, 학교, 교과연구회 등 **공공·비영리 교육기관이 주관**하는 교사 대상 연수 및 컨설팅

### 허용되지 않는 사용

다음과 같은 사용은 상업적 이용으로 간주되어 허용되지 않습니다.

- 본 소프트웨어 또는 수정본의 **판매**
- **유료** 서비스(SaaS, 구독형 서비스 등)로 제공하거나 상업적 제품·서비스에 포함하는 행위
- 외주 개발, 납품 등 **상업 계약의 일부**로 사용하는 행위
- 기업 또는 영리 조직 내부에서 업무 운영 목적으로 사용하는 경우
- 민간 사업자의 유상 컨설팅, 연수, 유지보수 등의 일부로 사용하는 경우
- 학원(보습, 입시, 전문학원 등), 유료 입시 컨설팅 업체 등 **영리 목적의 사교육 기관**이 사업 운영 목적으로 사용하는 경우

### 교육기관 범위에 대한 저작권자 해석

PolyForm Noncommercial 1.0.0은 'educational institution'의 사용을 허용하나 이 용어를 정의하지 않습니다. 본 저작권자는 이 용어를 다음과 같이 해석합니다.

- **해당:** 「초·중등교육법」·「고등교육법」·「사립학교법」에 따라 설립된 정규 교육기관 및 이에 준하는 공공·비영리 교육기관
- **미해당:** 「학원의 설립·운영 및 과외교습에 관한 법률」상 학원·교습소 등 영리 목적의 사설 교육시설

### 라이선스 위반 제보 및 대응

- **위반 사례 제보:** 학원, 유료 입시 컨설팅, 기업 등 영리 목적의 무단 사용이나 재배포 사례를 발견하시면 아래 메일로 제보해 주시기 바랍니다.
- **제보 방법:** 라이선스 위반 사실을 확인할 수 있는 **캡처 이미지, 영상 또는 해당 서비스의 URL**을 함께 첨부해 주세요.
- **이메일:** [hello@luminousky.com](mailto:hello@luminousky.com)
- **대응 방침:** 제보된 위반 사례에 대해서는 저작권 보호를 위해 **법률 대리인을 통한 민·형사상 대응 및 라이선스 종료 조치**를 취할 수 있습니다.

© 2026 luminousky
