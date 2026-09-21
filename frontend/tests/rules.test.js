/**
 * CLAUDE.md 의 절대 규칙 중 **기계로 확인할 수 있는 것**을 소스에서 직접 검사한다.
 *
 * 이 파일이 `src/` 가 아니라 `tests/` 에 있는 이유: `fs` 를 쓰므로 실수로 어디선가
 * import 되면 브라우저 번들이 깨진다. 소스 트리 밖에 두어 그 경로를 막는다.
 *
 * 예외 처리 방식 — **명시적 허용 목록**:
 * "같은 줄/인접 줄 주석" 같은 위치 휴리스틱은 쓰지 않는다. 실제로 기존 두 예외는
 * 주석이 2~3줄 떨어져 있었고, Vue 템플릿에서는 여는 태그 속성 사이에 주석을 넣을
 * 수도 없다(넣으면 파서가 깨진다). 그래서 허용 항목을 이 파일에 적는다 —
 * 새 예외를 만들려면 이 목록을 고쳐야 하고, 그 자체가 리뷰 지점이 된다.
 */
import { describe, it, expect } from 'vitest'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const SRC = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', 'src')

/** src 아래의 .vue / .js 를 모은다. 테스트 파일 자신은 뺀다. */
function sourceFiles(dir = SRC, out = []) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name)
    if (e.isDirectory()) sourceFiles(p, out)
    else if (/\.(vue|js)$/.test(e.name) && !/\.test\.js$/.test(e.name)) out.push(p)
  }
  return out
}

const rel = (p) => path.relative(SRC, p).replace(/\\/g, '/')

/**
 * 주석을 걷어낸다 — 규칙을 설명하는 주석이 규칙 위반으로 잡히면 안 된다.
 * 줄을 통째로 건너뛰지 않고 **주석 구간만** 지운다. 실제로 AdminView.vue 의 예외
 * 주석은 `>NEW</span><!-- … text-xs … -->` 처럼 코드 뒤에 붙어 있어, 줄 단위
 * 판정으로는 오탐이 났다.
 */
function stripComments(line) {
  return line
    .replace(/<!--[\s\S]*?-->/g, '')   // <!-- ... --> (한 줄 안에서 닫힌 것)
    .replace(/(^|\s)\/\/.*$/, '$1')    // // 이후 (https:// 는 앞이 ':' 라 걸리지 않는다)
    .replace(/^\s*(\/\*|\*).*$/, '')   // 블록 주석 줄
}

const FILES = sourceFiles()

/** 주석을 걷어낸 각 줄을 [상대경로:줄번호, 원본줄] 로 훑는다. */
function scan(test) {
  const hits = []
  for (const f of FILES) {
    fs.readFileSync(f, 'utf8').split('\n').forEach((line, i) => {
      if (test(stripComments(line))) hits.push(`${rel(f)}:${i + 1}  ${line.trim().slice(0, 80)}`)
    })
  }
  return hits
}

const TEXT_SMALL = /\btext-(sm|xs)\b/

describe('규칙 8 — 본문 폰트는 text-base 이상', () => {
  // 읽는 텍스트가 아니라 **표식**(배지·pill)이라 예외인 곳.
  // 형식: `파일:줄번호` — 줄이 밀리면 실패하므로, 옮길 때 함께 고치게 된다.
  const ALLOWED = new Set([
    'components/admin/RoundsTab.vue:63',  // "재계산 필요" 배지 (F-017)
    'views/AdminView.vue:109',            // 사이드바 "NEW" 배지
  ])

  it('text-sm / text-xs 는 허용 목록에 적힌 배지에서만 쓴다', () => {
    const violations = scan(l => TEXT_SMALL.test(l))
      .filter(h => !ALLOWED.has(h.split('  ')[0]))
    expect(violations,
      '본문 폰트는 text-base 이상이다. 배지·pill 이라면 이 테스트의 ALLOWED 에 추가하라').toEqual([])
  })

  it('허용 목록에 낡은 항목이 남아 있지 않다', () => {
    // 배지를 지웠는데 목록만 남으면, 다음 사람이 "여기는 예외"라고 오해한다.
    const stale = []
    for (const at of ALLOWED) {
      const [file, lineNo] = at.split(':')
      const line = fs.readFileSync(path.join(SRC, file), 'utf8').split('\n')[Number(lineNo) - 1]
      if (!line || !TEXT_SMALL.test(stripComments(line))) stale.push(at)
    }
    expect(stale, 'ALLOWED 항목이 가리키는 줄에 text-xs/sm 이 없다 — 지우거나 줄번호를 고쳐라').toEqual([])
  })
})

describe('규칙 1 — 프론트에서 점수를 ÷100000 하지 않는다', () => {
  // 점수는 ×100000 정수로 저장되고, 나누는 것은 백엔드 Score newtype 의 몫이다.
  // 프론트가 직접 나누면 부동소수점 오차가 표시에 섞이고, 규칙 3(계산은 백엔드
  // 전담)도 함께 무너진다.
  //
  // 지금 위반은 0건이다 — 이 검사는 새로 들어오는 것을 막기 위한 것이다.
  // 리터럴 `100000` 만 보면 `1e5`·`100_000` 같은 변형을 놓치므로 함께 잡는다.
  const SCALE = /(?:\/|\*)\s*(?:100_?000|1e5|1e-5)\b/

  it('점수 배율을 프론트에서 직접 계산하지 않는다', () => {
    expect(scan(l => SCALE.test(l)),
      '점수 배율 계산은 백엔드 몫이다(CLAUDE.md 규칙 1·3)').toEqual([])
  })
})
