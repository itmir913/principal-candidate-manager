# 기여 안내 (Contributing)

PCM(학교장추천자 선발 관리 시스템)에 기여해 주셔서 감사합니다.

이 프로젝트는 **실제 고등학교의 입시 자료를 다룹니다.** 점수·순위 계산이 틀리면
학생의 진학에 직접 영향을 주기 때문에, 아래 규칙은 취향이 아니라 **안전 장치**입니다.
PR을 보내기 전에 반드시 읽어 주세요.

---

## Contribution License

> 영문이 법적 효력을 가지는 원문입니다. 아래 한국어 번역은 참고용이며 해석이 갈릴 경우
> 영문이 우선합니다.

### Why copyright is assigned

PCM computes students' scores and rankings and determines who is recommended for university admission. Because an error can directly harm a student's admission, the project is deliberately maintained under a single, concentrated point of responsibility rather than a structure in which ownership and maintenance duties are distributed among contributors. To carry that responsibility, the project owner must be able to freely modify, rewrite, relicense, or remove any code in the project — including code received through pull requests — without needing the consent of each contributor. For this reason contributions are accepted by assignment of copyright, and the rights to use the contribution are licensed back to the contributor (Section 7).

### Terms

By submitting a contribution, you agree that:

1. You assign to the project owner (itmir913, luminousky.com) all copyright and related rights in your contribution, worldwide and in perpetuity. This assignment takes effect upon submission of your contribution.

2. This assignment allows the project owner to use, modify, distribute, sublicense, and relicense your contribution under any terms, including terms different from the current project license, at their sole discretion.

3. You represent that:
    - you are the sole author of the contribution and have the legal right to assign these rights,
    - it does not violate any third-party rights, and
    - it does not introduce any license terms or dependencies that conflict with the project license.

4. You assign any patent rights necessary to use, modify, distribute, and sublicense your contribution as part of the project.

5. Contributions are provided "as is", without warranty of any kind.

6. The project owner reserves the right to accept, reject, modify, or remove contributions at their sole discretion.

7. **License back.** Upon the assignment in Section 1, the project owner grants you a perpetual, worldwide, non-exclusive, royalty-free, irrevocable license to use, reproduce, modify, display, and distribute your own contribution, including for the purpose of presenting it in your personal portfolio, résumé, or similar showcase of your work, without restriction as to time or place. This license-back does not grant any rights to the rest of the project, which remains governed by the project license.

### 한국어 번역 (참고용)

> 해석이 갈릴 경우 위 영문이 우선합니다.

#### 저작권을 양도받는 이유

PCM은 학생의 점수와 순위를 계산하고 대학 추천 대상자를 결정합니다. 오류 하나가 학생의
대입에 직접 피해를 줄 수 있으므로, 이 프로젝트는 소유권과 유지보수 책임을 기여자들에게
분산시키는 구조가 아니라 **한 곳에 집중된 책임 체계**로 관리합니다. 그 책임을 지려면
프로젝트 소유자가 풀 리퀘스트로 받은 코드를 포함해 프로젝트의 모든 코드를 각 기여자의
동의 없이 자유롭게 수정·재작성·재라이선스·삭제할 수 있어야 합니다. 이 때문에 기여는
저작권 양도 방식으로 받으며, 기여물의 이용 권리는 기여자에게 되돌려 드립니다(7항).

#### 조항

기여물을 제출함으로써 귀하는 다음에 동의합니다.

1. 귀하는 기여물에 대한 모든 저작권 및 관련 권리를 전 세계적으로, 영구히 프로젝트
   소유자(itmir913, luminousky.com)에게 양도합니다. 이 양도는 기여물을 제출한 때에
   효력이 발생합니다.

2. 이 양도에 따라 프로젝트 소유자는 자신의 단독 재량으로, 현재 프로젝트 라이선스와 다른
   조건을 포함한 어떠한 조건으로도 기여물을 이용·수정·배포·재허락·재라이선스할 수 있습니다.

3. 귀하는 다음을 진술합니다.
    - 귀하가 기여물의 단독 저작자이며 이 권리들을 양도할 법적 권한이 있다는 것,
    - 기여물이 제3자의 권리를 침해하지 않는다는 것,
    - 기여물이 프로젝트 라이선스와 충돌하는 라이선스 조건이나 의존성을 들여오지 않는다는 것.

4. 귀하는 기여물을 프로젝트의 일부로 이용·수정·배포·재허락하는 데 필요한 모든 특허권을
   양도합니다.

5. 기여물은 어떠한 종류의 보증도 없이 "있는 그대로" 제공됩니다.

6. 프로젝트 소유자는 자신의 단독 재량으로 기여물을 수락·거절·수정·삭제할 권리를 가집니다.

7. **이용 권리의 반환(license-back).** 1항의 양도와 함께, 프로젝트 소유자는 귀하에게
   **귀하 자신의 기여물**을 이용·복제·수정·전시·배포할 수 있는 영구적·전 세계적·비독점적·
   무상·취소 불가능한 이용 허락을 부여합니다. 여기에는 개인 포트폴리오, 이력서 또는 이와
   유사한 작업물 소개에 사용하는 것이 포함되며, 시간이나 장소의 제약을 받지 않습니다.
   이 이용 허락은 프로젝트의 나머지 부분에 대한 권리를 부여하지 않으며, 나머지 부분은
   프로젝트 라이선스(PolyForm Noncommercial 1.0.0)를 따릅니다.

---

## 개발 환경 준비

```bash
npm run setup
```

Rust(stable)와 Node.js가 필요합니다. `setup`이 npm 패키지와 `cargo-watch`를 함께 설치합니다.

| 명령 | 용도 |
|---|---|
| `npm run dev` | 백엔드 + 프론트엔드 동시 실행 |
| `npm run dev:watch` | 백엔드 소스 변경 시 자동 재시작 |
| `npm run ci` | 테스트 전체 — 프론트(vitest)·러스트·독립 오라클 대조를 순서대로. **CI 가 돌리는 것과 같다** (Python 3 필요) |
| `npm run build` | 릴리스 빌드 (`target/release/principal-candidate-manager.exe`) |

---

## 기여 절차

1. 저장소를 fork 합니다
2. 작업 브랜치를 만듭니다 (`fix/...`, `feat/...`)
3. **로컬에서 `npm run ci`가 전부 통과하는지 확인합니다**
4. 무엇을 왜 바꿨는지 설명을 담아 PR을 보냅니다

> PR을 올리면 `.github/workflows/CI.yml`이 위와 **동일한 `npm run ci`**를
> windows-latest에서 실행합니다. 프론트엔드 검증은 여러 겹입니다 — `vitest`가
> `.vue` 밖의 순수 모듈(점수 표기·오류 문자열·라벨)을 직접 호출해 검사하고,
> `tools/oracle/front_check.mjs`가 백엔드 실측값으로 파생 로직(점수 표기·동점
> 표식·재정렬)을 대조하며, **정적 스캔**이 `.vue` 템플릿의 미정의 식별자를 찾고, **소스 규칙 검사**가
> CLAUDE.md 규칙 1·8 위반을 잡고,
> **렌더 테스트**가 모든 화면을 마운트해 "열리기는 하는가"와 정원 입력이 값을
> 조용히 바꾸지 않는가(F-013)를 확인합니다. 다만 외관·레이아웃은 단언하지
> 않으므로, 화면이 제대로 보이는지는 여전히 사람이 확인해야 합니다.

### 릴리스

버전은 `Cargo.toml` 하나가 진실의 출처다(`publish.yml`이 여기서 읽는다).
변경 내역은 `CHANGELOG.md`에 **0.2.13부터** 관리한다.

1. `Cargo.toml` 버전 상향 + `CHANGELOG.md`에 해당 절 작성 → `Version X.Y.Z` 커밋
2. CI 초록 확인 후 `git tag X.Y.Z && git push origin X.Y.Z`
   (CI는 `master` push·PR에만 걸린다 — 태그 push로는 돌지 않는다)
3. Actions 탭에서 `publish.yml` 수동 실행 → **초안(draft) 릴리스**가 만들어진다
4. 초안 본문에 `CHANGELOG.md`의 해당 절을 붙여 넣고 공개

**배포 시점은 학사 일정을 본다.** 선발(라운드) 진행 중 업그레이드는 관리자의 작업을
가로막을 수 있다 — 0.2.13의 "업그레이드 직후 달라지는 것"이 그 사례다.

### 커밋 규약

- **GPG 서명 필수.** `--no-gpg-sign` / `--no-verify` 우회 금지
- Conventional Commits 형식을 씁니다: `feat(scope): ...`, `fix(ui): ...`,
  `test(audit): ...`, `docs(schema): ...`
- 본문에는 "무엇을 했는가"보다 **"왜 그렇게 했는가"**를 씁니다

---

## 절대 규칙

`CLAUDE.md`의 8대 규칙과 동일합니다. 둘이 어긋나면 `CLAUDE.md`가 기준입니다.

**1. Float-Free** — 모든 점수·측정값은 ×100000 정수로 DB에 저장합니다. 점수에
`f32`/`f64`를 쓰지 말고, 프론트엔드에서 `÷100000`을 손으로 계산하지 마세요.
`Score` newtype이 자동 처리합니다.

**2. Fail-Fast** — 점수 계산 오류는 즉시 `Err`를 반환합니다. `unwrap_or(0)`,
`unwrap_or_default()` 같은 silent fallback은 전면 금지입니다. 허용 예외는
`src/docs/silent_fallback_allowed.md`에 명시된 위치뿐입니다.

**3. 점수 계산은 백엔드 전담** — 프론트엔드는 표시만 합니다. 미리보기 점수도
API를 호출해서 받습니다. Vue 컴포넌트 안에 점수 로직을 넣지 마세요.

**4. Import는 All-or-Nothing** — 오류가 하나라도 있으면 전체 rollback + 422입니다.
부분 저장은 없습니다. 중복 행은 warning이 아니라 error입니다.
유일한 예외는 외부 석차연명부의 **석차 값** 누락·변환 실패로, 이때만 행을 건너뛰고
warning을 남깁니다 (`src/docs/08_excel_import.md` §7-1).

**5. Excel 파싱은 헤더 이름 기반** — 열 인덱스(`cols[0]`)를 직접 참조하지 마세요.
`excel::col_map` + `require_cols`를 사용합니다. 실제 학교에서 오는 파일은 열 순서가
제각각입니다.

**6. 다중 쓰기는 트랜잭션** — DELETE+INSERT, 루프 INSERT/UPDATE는 반드시 tx로 묶습니다.
`find_or_create_track`에는 항상 `&mut *tx`를 넘기세요 (pool 직접 전달 금지).

**7. `base_data` 일괄 삭제에는 `student_type` 필터 필수** — 재학생 업로드가 졸업생
데이터를 지우면 점수 계산이 실패합니다.

**8. 폰트 최소 `text-base`** — `text-sm`, `text-xs`, `font-size: 14px` 이하 금지.
유일한 예외는 본문이 아닌 **배지·pill 라벨**(예: 사이드바 "NEW" 배지)이며,
예외를 쓸 때는 해당 줄에 주석을 남깁니다.

### 스키마 변경

**출시 후에는 이미 배포된 버전의 조각 파일을 고치지 마세요.** 현장 DB는 그 스키마로
만들어져 있고 `PRAGMA user_version`으로 자기 버전을 알립니다. `migrations/v1/*.sql`을
조용히 고치면 새로 만든 DB와 기존 DB의 구조가 갈라지는데, 둘 다 `user_version`이 1이라
앱은 그 차이를 영영 감지하지 못합니다.

스키마를 바꾸려면 **새 버전을 추가**하세요.

1. `migrations/v2/` 에 조각 파일을 만듭니다. 이미 배포된 v1 DB 위에서 도는
   `ALTER`/`CREATE` 문이어야 합니다 (v1처럼 맨바닥에서 만드는 `CREATE` 전문이 아닙니다).
2. `src/db.rs`에 `V2_FRAGMENTS` 상수를 만들고 `MIGRATION_FRAGMENTS` 배열에 추가합니다.
   실행 순서는 파일명이 아니라 이 배열이 결정합니다.
3. `SCHEMA_VERSION`을 올립니다 (안 올리면 컴파일 타임 assert가 막습니다).
4. 새 버전의 지문을 만들고 커밋합니다.

   ```powershell
   $env:PCM_WRITE_SCHEMA_SNAPSHOT=1; cargo test --test schema_freeze
   ```

`tests/schema_freeze.rs`가 `tests/schema_snapshots/v{N}.sql`의 지문과 실제 스키마를
대조합니다. 배포된 버전의 조각을 고치면 이 테스트가 깨집니다. **그때 스냅샷 파일을
현재 스키마에 맞춰 고치는 것은 해결이 아닙니다** — 위 절차대로 새 버전을 만드세요.
주석·들여쓰기만 바꾸는 것은 지문에 영향을 주지 않습니다(정규화 후 비교).

---

## 테스트

새 검증 로직에는 **유효·경계·거부** 시나리오가 모두 있어야 합니다. 감점 로직을
건드렸다면 감점 시나리오도 포함합니다.

거부 경로 테스트는 상태 코드만 보지 말고 세 가지를 함께 단언하세요.

1. 상태 코드 (422/409 등)
2. **DB 행이 실제로 변하지 않았는지** (COUNT 등으로 확인)
3. **오류 메시지의 행번호·원인** (`"3행: ..."` 형태)

### 판별력 있는 테스트를 쓰세요

이 저장소에서 실제로 문제가 됐던 패턴입니다. 통과하는 테스트와 지켜주는 테스트는
다릅니다. 스스로에게 물어보세요 — **"이 단언을 그대로 두고 구현이 틀릴 수 있는가?"**

- `assert!(result.is_ok())`만 하고 값이나 DB 상태를 안 보는 단언
- 산출물(Excel 등)에서 **값이 어느 열 아래에 있는지**를 안 보고 집합 소속만 보는 단언
  (`row.iter().any(|c| c == "145")`) — 열 순서가 어긋나도 통과합니다
- 픽스처가 트리거·제약을 직접 SQL로 우회해 놓고 "핸들러 가드를 통과했다"고 결론내는 구조

권한 경계, 학급 격리 필터처럼 **조용히 망가지는** 자리에 테스트를 추가했다면,
그 가드를 일부러 지우고 **새 테스트만 실패하는지** 확인해 보세요. 실패하지 않는다면
그 테스트는 아무것도 지키지 못합니다.

---

## 이슈 제보

버그 제보에는 다음을 포함해 주세요.

- 프로그램 버전 (관리자 화면 하단 또는 `Cargo.toml`)
- 재현 절차와 기대한 동작
- 실제로 나온 오류 메시지 전문

**학생 개인정보(이름·학번·성적)를 이슈나 스크린샷에 그대로 올리지 마세요.**
재현용 데이터는 반드시 가공하거나 가명 처리해 주세요.

---

## 행동 규범

이 프로젝트는 [행동 규범](CODE_OF_CONDUCT.md)을 따릅니다.
