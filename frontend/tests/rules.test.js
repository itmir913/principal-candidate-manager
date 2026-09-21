/**
 * CLAUDE.md 의 절대 규칙 중 **기계로 확인할 수 있는 것**을 소스에서 직접 검사한다.
 *
 * 이 파일이 `src/` 가 아니라 `tests/` 에 있는 이유: `fs` 를 쓰므로 실수로 어디선가
 * import 되면 브라우저 번들이 깨진다. 소스 트리 밖에 두어 그 경로를 막는다.
 *
 * 2026-09-21 감사 반영 — 첫 판에서 드러난 구멍 넷을 메웠다:
 *   S-3 규칙 8 의 `font-size` 절반을 안 보고 있었다. 이 저장소는 크기를 대부분
 *       인라인 style 로 쓰므로 **실제로 쓰이는 쪽이 안 잠겨** 있었다.
 *   S-4 규칙 1 그물이 성겨 6종 중 1종만 잡았다(`1e+5` 조차 놓쳤다).
 *   S-5 `stripComments` 가 여러 줄 주석에서 오탐, 줄 중간 `//` 에서 누락을 냈다.
 *   S-6 예외 허용 목록의 줄 번호가 무관한 편집 하나에 깨졌다 — 내용 키로 바꿨다.
 */
import { describe, it, expect } from 'vitest'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const SRC = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', 'src')

/**
 * src 아래의 .vue / .js / .css 를 모은다. 테스트 파일 자신은 뺀다.
 * `.css` 를 넣은 이유: 규칙 8 을 인라인까지 넓히면서 스타일시트를 빠뜨렸었다 —
 * `style.css`·`manual.css` 에 작은 폰트를 두면 검사 밖이었다(감사 중-8).
 */
function sourceFiles(dir = SRC, out = []) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name)
    if (e.isDirectory()) sourceFiles(p, out)
    else if (/\.(vue|js|css)$/.test(e.name) && !/\.test\.js$/.test(e.name)) out.push(p)
  }
  return out
}

const rel = (p) => path.relative(SRC, p).replace(/\\/g, '/')

/**
 * 파일 **전체**를 훑어 주석을 공백으로 바꾼다. 줄 단위로는 여러 줄 주석의 가운데를
 * 알 수 없어 오탐이 났다(`*` 로 시작하지 않는 블록 주석 줄, 여러 줄 `<!-- -->`).
 * 줄 번호를 보존해야 하므로 지우지 않고 **같은 길이의 공백**으로 덮는다.
 *
 * 문자열 리터럴 안의 `//` 도 주석으로 오인해 그 뒤를 날려 먹었다(실제 누락 사례:
 * `'badge // text-sm font-bold'`). 그래서 따옴표·백틱 구간을 건너뛴다.
 */
function blankComments(src) {
  const out = src.split('')
  let i = 0
  const n = src.length
  const blank = (from, to) => {
    for (let k = from; k < to && k < n; k++) if (out[k] !== '\n') out[k] = ' '
  }
  while (i < n) {
    const two = src.slice(i, i + 2)
    const c = src[i]
    if (c === '"' || c === "'" || c === '`') {
      // 문자열 리터럴 — 통째로 건너뛴다(줄바꿈을 만나면 끊는다: 따옴표 짝이 안 맞는
      // 템플릿 속성에서 파일 끝까지 먹지 않도록)
      let j = i + 1
      while (j < n && src[j] !== c && !(c !== '`' && src[j] === '\n')) {
        if (src[j] === '\\') j++
        j++
      }
      i = j + 1
    } else if (two === '//') {
      let j = src.indexOf('\n', i); if (j === -1) j = n
      blank(i, j); i = j
    } else if (two === '/*') {
      let j = src.indexOf('*/', i + 2); j = j === -1 ? n : j + 2
      blank(i, j); i = j
    } else if (src.startsWith('<!--', i)) {
      let j = src.indexOf('-->', i + 4); j = j === -1 ? n : j + 3
      blank(i, j); i = j
    } else {
      i++
    }
  }
  return out.join('')
}

const FILES = sourceFiles()

/** 주석을 걷어낸 파일을 줄 단위로 훑어 `경로:줄 내용` 목록을 만든다. */
function scan(test) {
  const hits = []
  for (const f of FILES) {
    blankComments(fs.readFileSync(f, 'utf8')).split('\n').forEach((line, i) => {
      if (test(line)) hits.push({ at: `${rel(f)}:${i + 1}`, text: line.trim() })
    })
  }
  return hits
}

const fmt = (hits) => hits.map(h => `${h.at}  ${h.text.slice(0, 80)}`)

describe('규칙 8 — 본문 폰트는 text-base 이상', () => {
  // 읽는 텍스트가 아니라 **표식**(배지·pill)이라 예외인 곳.
  //
  // 키는 `파일|그 줄의 내용`이다. 줄 번호를 쓰면 위쪽에 줄 하나만 넣어도 깨지는데,
  // RoundsTab.vue 는 1400줄이 넘고 가장 자주 손대는 파일이라 실패가 규칙 8 과 무관하게
  // 쏟아진다(감사 S-6). 내용 키는 줄이 밀려도 살아 있고, 내용이 바뀌면 깨진다 —
  // 원하던 리뷰 지점은 그대로다.
  const ALLOWED = new Map([
    ['components/admin/RoundsTab.vue|class="text-xs font-semibold rounded-full whitespace-nowrap"',
     '"재계산 필요" 배지 (F-017)'],
    ['views/AdminView.vue|class="ml-auto text-xs font-bold"',
     '사이드바 "NEW" 배지'],
  ])

  const SMALL_CLASS = /\btext-(sm|xs)\b/
  // px/pt/rem 이 섞여 있어 단위별로 본다. 기준은 16px = 1rem = 12pt.
  const SMALL_INLINE = new RegExp([
    String.raw`font-size\s*:\s*(?:[0-9]|1[0-5])(?:\.\d+)?\s*px`,
    // `.9rem` 처럼 앞자리 0 을 생략한 표기(= 14.4px)도 잡는다. 처음엔 놓쳤다.
    // **1rem 은 16px 이라 위반이 아니다** — `1(?:\.0+)?rem` 까지 잡아 오탐을 냈었다.
    String.raw`font-size\s*:\s*0?\.\d+\s*(?:rem|em)\b`,
    String.raw`font-size\s*:\s*(?:[0-9]|1[01])(?:\.\d+)?\s*pt`,
    String.raw`fontSize\s*(?::|=)\s*['"\`](?:[0-9]|1[0-5])(?:\.\d+)?px`,   // 객체 리터럴 + DOM 대입
    // Tailwind 임의값: text-[13px] / text-[0.8rem] / text-[10pt]
    String.raw`text-\[(?:[0-9]|1[0-5])(?:\.\d+)?px\]`,
    String.raw`text-\[0?\.\d+(?:rem|em)\]`,
    String.raw`text-\[(?:[0-9]|1[01])(?:\.\d+)?pt\]`,
    // CSS 단축 `font: 12px/1.4 ...` — font-size 없이 크기를 준다
    String.raw`\bfont\s*:\s*(?:[a-z-]+\s+)*(?:[0-9]|1[0-5])(?:\.\d+)?px\b`,
    // JS 로 직접 꽂는 경우
    String.raw`setProperty\(\s*['"\`]font-size['"\`]\s*,\s*['"\`](?:[0-9]|1[0-5])(?:\.\d+)?px`,
  ].join('|'))

  const key = (h) => `${h.at.split(':')[0]}|${h.text}`

  it('text-sm / text-xs 는 허용 목록에 적힌 배지에서만 쓴다', () => {
    const violations = fmt(scan(l => SMALL_CLASS.test(l)).filter(h => !ALLOWED.has(key(h))))
    expect(violations,
      '본문 폰트는 text-base 이상이다. 배지·pill 이라면 이 테스트의 ALLOWED 에 추가하라').toEqual([])
  })

  it('인라인 font-size 도 14px 이하를 쓰지 않는다', () => {
    // 이 저장소는 크기를 대부분 인라인 style 로 준다 — 클래스만 보면 규칙의 절반이
    // 열려 있는 셈이다(감사 S-3).
    expect(fmt(scan(l => SMALL_INLINE.test(l))),
      '인라인 font-size 도 규칙 8 의 대상이다(16px = 1rem = 12pt 기준)').toEqual([])
  })

  it('허용 목록에 낡은 항목이 남아 있지 않다', () => {
    // 배지를 지웠는데 목록만 남으면, 다음 사람이 "여기는 예외"라고 오해한다.
    const live = new Set(scan(l => SMALL_CLASS.test(l)).map(key))
    const stale = [...ALLOWED.keys()].filter(k => !live.has(k))
    expect(stale, 'ALLOWED 항목과 같은 줄이 소스에 없다 — 지우거나 내용을 맞춰라').toEqual([])
  })

  it('허용 목록의 키가 파일 안에서 유일하다', () => {
    // 같은 줄이 두 번 나오면 내용 키가 둘 다 덮어 버린다 — 예외가 조용히 번진다.
    const dup = []
    for (const k of ALLOWED.keys()) {
      const [file, text] = k.split('|')
      const count = blankComments(fs.readFileSync(path.join(SRC, file), 'utf8'))
        .split('\n').filter(l => l.trim() === text).length
      if (count !== 1) dup.push(`${k} (${count}회)`)
    }
    expect(dup, '예외 키가 유일하지 않다 — 그 줄에만 걸리도록 내용을 구분하라').toEqual([])
  })
})

describe('규칙 1 — 프론트에서 점수를 ÷100000 하지 않는다', () => {
  // 점수는 ×100000 정수로 저장되고, 나누는 것은 백엔드 Score newtype 의 몫이다.
  // 프론트가 직접 나누면 부동소수점 오차가 표시에 섞이고, 규칙 3(계산은 백엔드
  // 전담)도 함께 무너진다.
  //
  // 지금 위반은 0건이다 — 새로 들어오는 것을 막는 장치다. 첫 판의 그물은
  // 6종 중 1종만 잡았다(감사 S-4). 표기 변형과 상수 경유를 함께 본다.
  // `1e+05` 처럼 지수에 0 을 채운 표기까지 본다 — 앞서 `1e+5` 를 고치면서 이건 놓쳤다.
  const LITERAL = String.raw`100_?000|1[eE]\+?0*5|0\.00001|1[eE]-0*5`
  const PATTERNS = [
    // 직접 나눗셈·곱셈. 괄호로 감싼 것(`v / (100000)`)도 본다.
    new RegExp(String.raw`[/*]\s*\(?\s*(?:${LITERAL})\b`),
    // 함수 기본 인자로 숨기는 것 (`function f(scale = 100000)`)
    new RegExp(String.raw`\w+\s*=\s*(?:${LITERAL})\s*[,)]`),
    // 거듭제곱 표기
    /[/*]\s*(?:Math\.pow\(\s*10\s*,\s*5\s*\)|\(?\s*10\s*\*\*\s*5\s*\)?)/,
    // 상수에 담아 두는 것 — 이름이 무엇이든 이 값을 프론트에 두지 않는다.
    // 줄 단위로 훑으므로 끝 앵커에 `$` 를 반드시 넣는다(`[;\n]` 만 쓰면 세미콜론
    // 없는 줄을 통째로 놓친다 — 실제로 `const SCALE = 100000` 이 빠져나갔다).
    new RegExp(String.raw`(?:const|let|var)\s+\w+\s*=\s*(?:${LITERAL})\s*(?:[;,)\]}]|$)`),
    // 객체 속성 / 배열 원소에 숨겨 두는 것도 같은 값이다
    new RegExp(String.raw`\w+\s*:\s*(?:${LITERAL})\s*(?:[,}]|$)`),
    new RegExp(String.raw`\[\s*(?:${LITERAL})\s*\]`),
  ]

  it('점수 배율을 프론트에서 직접 계산하지 않는다', () => {
    expect(fmt(scan(l => PATTERNS.some(re => re.test(l)))),
      '점수 배율 계산은 백엔드 몫이다(CLAUDE.md 규칙 1·3)').toEqual([])
  })
})
