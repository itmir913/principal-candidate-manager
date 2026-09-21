/**
 * 되돌리기 어려운 행위가 **2단계 확인**을 거치는지 — 호출부를 소스에서 전수로 센다.
 *
 * 왜 필요한가: `dialog.render.test.js` 는 `DialogHost` **컴포넌트**가 2단계를 그릴 수
 * 있음을 고정한다. 그러나 2단계를 **요구하는 쪽**(`level: 'danger'`)은 그 테스트가
 * 보지 않는다. 실제로 담임 [추천 포기]의 `level: 'danger'` 를 `'warn'` 으로 낮추는
 * 변이가 전 검증을 통과했다(4차 감사 치-2) — 문서가 "되돌릴 수 없다"고 못 박은 행위가
 * 한 번 클릭으로 실행되는 회귀다.
 *
 * 이것은 3차 감사가 지적한 실패의 **재발**이다: 고친 *함수*를 시험하고 *배선*을
 * 시험하지 않았다. 배선은 두 가지로 지킨다 —
 *   ① 여기(소스 전수): 파괴적 행위 목록을 적어 두고, 각 호출이 danger 인지 센다.
 *   ② dialog.render.test.js(동작): danger 가 실제로 두 번 묻는지.
 *
 * 목록을 손으로 적는 것이 약점이다. 그래서 **개수**도 함께 고정한다 — 새 파괴적
 * 행위가 늘면 목록을 고치지 않는 한 실패한다.
 */
import { describe, it, expect } from 'vitest'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const SRC = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', 'src')
const read = (rel) => fs.readFileSync(path.join(SRC, rel), 'utf8')

/**
 * 되돌리기 어려운 행위와 그 호출부. `파일 → 그 파일에서 danger 여야 하는 개수`.
 * 왜 이 행위가 danger 인지는 각 호출부의 `dangerNotice` 가 말한다.
 */
const DANGEROUS = {
  'components/admin/AreasTab.vue': 1,          // 전형요소 삭제 (점수 기준이 사라진다)
  'components/admin/ClassesTab.vue': 1,        // 학급 삭제
  'components/admin/RoundsTab.vue': 2,         // 라운드 마감 / 재오픈
  'components/admin/StudentsTab.vue': 1,       // 학생 일괄 교체
  'components/admin/UniversitiesTab.vue': 2,   // 대학 삭제 / 모집단위 삭제
  'components/teacher/ResultsTab.vue': 1,      // 추천 포기
}

describe('파괴적 행위는 2단계로 확인한다', () => {
  it.each(Object.entries(DANGEROUS))('%s 에 danger 확인이 %i개 있다', (file, want) => {
    const n = (read(file).match(/level:\s*'danger'/g) ?? []).length
    expect(n, `${file} 의 danger 확인 수가 달라졌다 — 파괴적 행위의 2단계가 사라졌거나, ` +
              '새 행위가 늘었다면 이 목록도 고쳐라').toBe(want)
  })

  it('저장소 전체 danger 개수와 목록이 일치한다', () => {
    // 목록에 없는 파일에서 danger 를 쓰면 여기서 드러난다(반대 방향).
    const all = []
    const walk = (dir) => {
      for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
        const p = path.join(dir, e.name)
        if (e.isDirectory()) walk(p)
        else if (e.name.endsWith('.vue')) {
          const n = (fs.readFileSync(p, 'utf8').match(/level:\s*'danger'/g) ?? []).length
          if (n) all.push([path.relative(SRC, p).replace(/\\/g, '/'), n])
        }
      }
    }
    walk(SRC)
    expect(Object.fromEntries(all), '목록에 없는 곳에서 danger 를 쓰거나, 목록이 낡았다')
      .toEqual(DANGEROUS)
  })

  it('danger 호출은 2단계 문구를 함께 준다', () => {
    // level 만 danger 이고 dangerNotice·finalConfirmText 가 없으면, 2단계 화면이
    // 빈 경고로 뜬다 — 관리자는 무엇을 확인하는지 모른 채 한 번 더 누른다.
    const missing = []
    for (const file of Object.keys(DANGEROUS)) {
      const src = read(file)
      // `dialog.confirm({ ... level: 'danger' ... })` 한 덩어리씩 본다
      for (const m of src.matchAll(/dialog\.confirm\(\{([\s\S]*?)\n\s*\}\)/g)) {
        const body = m[1]
        if (!/level:\s*'danger'/.test(body)) continue
        if (!/dangerNotice:/.test(body)) missing.push(`${file}: dangerNotice 없음`)
        if (!/finalConfirmText:/.test(body)) missing.push(`${file}: finalConfirmText 없음`)
      }
    }
    expect(missing, '2단계 확인 화면에 보여 줄 문구가 없다').toEqual([])
  })
})
