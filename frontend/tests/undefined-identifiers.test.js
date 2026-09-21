/**
 * `.vue` 전 파일의 **미정의 식별자**를 정적으로 훑는다.
 *
 * `src/docs/13_frontend_pitfalls.md` §4 의 "훑는 방법" 절이 글로만 적어 두었던 것을
 * 실행 가능한 검사로 만든다. 스모크 렌더(smoke-render.test.js)와 역할이 다르다 —
 * 스모크는 **실제로 실행되는 경로**만 지나므로 `v-if` 뒤나 클릭 핸들러 안은 못 본다.
 * 여기는 실행하지 않고 **전 파일을 본다.** 둘은 보완 관계다.
 *
 * 무엇을 잡나: 스크립트·템플릿에서 이름을 썼는데 그 파일 어디에도 정의가 없는 경우.
 * 실제 사고(`cancelDeptEdit` ↔ `cancelEdit`)와 2026-09-21 의 AreasTab import 누락이
 * 전부 이 유형이고, 둘 다 `npm run build` 는 통과했다.
 */
import { describe, it, expect } from 'vitest'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { parse, compileScript, compileTemplate } from 'vue/compiler-sfc'

const SRC = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', 'src')

function vueFiles(dir = SRC, out = []) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name)
    if (e.isDirectory()) vueFiles(p, out)
    else if (e.name.endsWith('.vue')) out.push(p)
  }
  return out
}

/**
 * 템플릿 렌더 코드에서 `_ctx.<name>` 을 뽑는다.
 *
 * `<script setup>` 에서는 스크립트에 바인딩이 있는 이름이 `$setup.<name>` 으로 컴파일된다.
 * 바인딩이 없으면 `_ctx.<name>` 으로 떨어지는데, 그것이 곧 **이 파일에 정의가 없다**는 뜻이다.
 * 런타임에는 조용히 undefined 가 되어 화면만 비거나, 호출하는 순간 터진다.
 */
function templateUnknowns(descriptor, id) {
  if (!descriptor.template) return []
  // 스크립트가 없는 SFC(정적 템플릿만)도 합법이다. compileScript 는 그 경우
  // "SFC contains no <script> tags" 로 던지므로 먼저 걸러 낸다 —
  // 검사 도구가 자기 예외로 죽으면 그 자체가 오탐이다.
  const hasScript = !!(descriptor.script || descriptor.scriptSetup)
  const bindings = hasScript ? (compileScript(descriptor, { id }).bindings ?? {}) : {}
  const { code } = compileTemplate({
    source: descriptor.template.content,
    filename: id,
    id,
    compilerOptions: { bindingMetadata: bindings, prefixIdentifiers: true },
  })
  const found = new Set()
  for (const m of code.matchAll(/_ctx\.([A-Za-z_$][\w$]*)/g)) found.add(m[1])
  return [...found]
}

/**
 * Vue 런타임이 자동으로 채워 주거나 전역에서 오는 이름. 미정의로 보지 않는다.
 * 목록이 길어지면 검사가 무뎌지므로, 새 항목을 넣을 때는 **왜 전역인지** 함께 적는다.
 */
const AMBIENT = new Set([
  '$slots', '$attrs', '$props', '$emit', '$refs', '$el', '$options', '$forceUpdate',
  '$nextTick', '$watch', '$parent', '$root', '$data',
  '$route', '$router',          // vue-router 전역 등록
  'console', 'window', 'document', 'Math', 'JSON', 'Object', 'Array', 'Number', 'String',
  'Boolean', 'Date', 'RegExp', 'Set', 'Map', 'Promise', 'Intl', 'URL', 'Blob',
  'undefined', 'null', 'true', 'false', 'NaN', 'Infinity',
])

const FILES = vueFiles()

describe('미정의 식별자 (.vue 전수)', () => {
  it('검사 대상이 비어 있지 않다', () => {
    // glob 이 어긋나면 0개를 돌면서 초록이 뜬다.
    expect(FILES.length).toBeGreaterThanOrEqual(20)
  })

  it('템플릿이 참조하는 이름은 전부 그 파일 안에 정의돼 있다', () => {
    const violations = []
    for (const f of FILES) {
      const src = fs.readFileSync(f, 'utf8')
      const id = path.relative(SRC, f).replace(/\\/g, '/')
      const { descriptor, errors } = parse(src, { filename: id })
      if (errors.length) {
        violations.push(`${id}: SFC 파싱 실패 — ${errors[0].message}`)
        continue
      }
      for (const name of templateUnknowns(descriptor, id)) {
        if (AMBIENT.has(name)) continue
        violations.push(`${id}: 템플릿이 쓰는 '${name}' 의 정의가 이 파일에 없다`)
      }
    }
    expect(violations,
      '빌드는 통과하지만 화면이 비거나 터진다 — src/docs/13_frontend_pitfalls.md §4').toEqual([])
  })
})
