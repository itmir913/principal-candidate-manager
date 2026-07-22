# 기여 안내 (Contributing)

PCM(학교장추천자 선발 관리 시스템)에 기여해 주셔서 감사합니다.

이 프로젝트는 **실제 고등학교의 입시 자료를 다룹니다.** 점수·순위 계산이 틀리면
학생의 진학에 직접 영향을 주기 때문에, 아래 규칙은 취향이 아니라 **안전 장치**입니다.
PR을 보내기 전에 반드시 읽어 주세요.

---

## Contribution License

> 이 절은 법적 효력을 가지는 원문입니다. 번역본은 참고용이며 해석이 갈릴 경우
> 아래 영문이 우선합니다.

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

**요약(참고용)**: 기여물의 저작권은 프로젝트 소유자에게 양도되며, 소유자는 현재
라이선스와 다른 조건으로도 재라이선스할 수 있습니다. 본인이 단독 저작자여야 하고,
프로젝트 라이선스(PolyForm Noncommercial 1.0.0)와 충돌하는 의존성을 들여오면 안 됩니다.

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
| `npm test` | `cargo test` — 전체 테스트 |
| `npm run build` | 릴리스 빌드 (`target/release/principal-candidate-manager.exe`) |

---

## 기여 절차

1. 저장소를 fork 합니다
2. 작업 브랜치를 만듭니다 (`fix/...`, `feat/...`)
3. **로컬에서 `npm test`가 전부 통과하는지 확인합니다**
4. 무엇을 왜 바꿨는지 설명을 담아 PR을 보냅니다

> PR을 올리면 `.github/workflows/test.yml`이 위와 **동일한 `npm test`**를
> windows-latest에서 실행합니다. 다만 **CI는 백엔드 테스트만 검증합니다** —
> 프론트엔드에는 자동화 테스트가 없으므로 Vue 변경은 여전히 사람이 확인해야 합니다.

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

**아직 배포 전 버전 관리 체계이므로 `migrations/v2`를 추가하지 마세요.**
`migrations/v1/*.sql` 조각 파일을 직접 수정하고 DB를 재생성합니다.
실행 순서는 `src/db.rs`의 `V1_FRAGMENTS` 배열이 결정하므로, **새 조각 파일을
추가했다면 그 배열에도 등록**해야 합니다.

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
