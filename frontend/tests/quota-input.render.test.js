// @vitest-environment jsdom
/**
 * 정원 입력 — **화면에서** 값이 조용히 바뀌지 않는지 본다 (F-013).
 *
 * 왜 따로 파일을 두나: `parseQuotaInput` 단위 테스트는 **함수**를 지키지, 컴포넌트가
 * 그 함수를 쓰는지를 지키지 못한다. 실제로 감사에서 두 가지가 전 검증을 통과했다.
 *   - `Number(String(e.target.value)) || 1`   ← 중첩 괄호로 소스 정규식 가드를 피함
 *   - `parseQuotaInput(e.target.value) ?? 1`  ← 함수는 쓰되 결과를 덮어씀
 * 둘 다 관리자가 0 을 넣으면 **말없이 1** 이 되는, F-013 그 자체다.
 *
 * `tools/oracle/front_check.mjs` 의 3d 소스 가드는 이 종류를 **두 번 연속 놓쳤다**
 * (`parseInt` 철자만 보던 판, 숫자 변환 뒤 `||` 만 보던 판). 문자열 모양으로 배선을
 * 지키려는 시도가 F-014 에서도 같은 이유로 실패했다 — 그래서 행동으로 옮긴다.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'

vi.mock('axios', () => {
  const res = () => Promise.resolve({ data: [], headers: {} })
  const axios = {
    get: res, post: res, put: res, patch: res, delete: res,
    interceptors: { request: { use: () => {} }, response: { use: () => {} } },
  }
  return { default: axios, ...axios }
})

const load = () => import('../src/components/admin/UniversitiesTab.vue')

/** [+ 대학 추가] 폼을 열고 {정원칸, 저장버튼} 을 돌려준다. */
async function openAddForm(wrapper) {
  const add = wrapper.findAll('button').find(b => b.text().includes('대학 추가'))
  expect(add, '[+ 대학 추가] 버튼을 찾지 못했다').toBeTruthy()
  await add.trigger('click')

  // **폼 안으로 범위를 좁힌다.** 화면 위쪽에도 텍스트 입력칸이 있어서, 그냥
  // `find('input[type="text"]')` 하면 엉뚱한 칸을 채우고 폼은 계속 비어 있다.
  // (실제로 그렇게 "정상 입력인데 저장이 잠겼다"가 났다.)
  const form = wrapper.findAll('div').find(d => d.text().startsWith('새 대학 추가'))
  expect(form, '[새 대학 추가] 폼이 열리지 않았다').toBeTruthy()

  const name = form.find('input[type="text"]')
  expect(name.exists(), '대학명 입력칸이 없다').toBe(true)
  await name.setValue('가대학')                    // 이름은 채워 둔다 — 정원만 보기 위해

  // 기본값이 `unlimited: true` 라 정원 칸이 아예 렌더되지 않는다(emptyUnivForm).
  // "무제한"을 꺼야 입력칸이 나온다.
  const unlimited = form.find('input[type="checkbox"]')
  expect(unlimited.exists(), '"무제한" 체크박스를 찾지 못했다').toBe(true)
  await unlimited.setValue(false)
  await new Promise(r => setTimeout(r, 0))

  const quota = form.find('input[type="number"]')
  expect(quota.exists(), '정원 입력칸이 없다 — "무제한"이 꺼졌는지 확인하라').toBe(true)

  const save = form.findAll('button').find(b => b.text() === '저장')
  expect(save, '저장 버튼을 찾지 못했다').toBeTruthy()
  return { form, quota, save }
}

describe('정원 입력 (F-013) — 화면 동작', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.spyOn(console, 'warn').mockImplementation(() => {})
  })
  afterEach(() => { vi.restoreAllMocks() })

  it.each([
    ['0',   '0 은 1 로 바뀌지 않는다'],
    ['-3',  '음수는 통과하지 않는다'],
    ['5.7', '소수는 잘려서 저장되지 않는다'],
    ['',    '빈 칸은 0 으로 확정되지 않는다'],
    ['abc', '숫자가 아닌 입력'],
  ])('정원에 %s 를 넣으면 저장이 잠긴다 (%s)', async (typed) => {
    const wrapper = mount((await load()).default)
    await new Promise(r => setTimeout(r, 0))
    const { quota, save } = await openAddForm(wrapper)

    await quota.setValue(typed)
    await new Promise(r => setTimeout(r, 0))

    expect(save.attributes('disabled'),
      `정원 "${typed}" 인데 저장 버튼이 열려 있다 — 값이 조용히 보정됐을 수 있다(F-013)`)
      .toBeDefined()
    wrapper.unmount()
  })

  it('정원 1 이상이면 저장이 열린다', async () => {
    // 위 단언이 "항상 잠겨 있다"로 통과하면 아무것도 지키지 못한다.
    const wrapper = mount((await load()).default)
    await new Promise(r => setTimeout(r, 0))
    const { quota, save } = await openAddForm(wrapper)

    await quota.setValue('3')
    await new Promise(r => setTimeout(r, 0))

    expect(save.attributes('disabled'), '정상 입력인데 저장이 잠겼다').toBeUndefined()
    wrapper.unmount()
  })

  it('0 을 넣어도 화면의 값이 1 로 바뀌지 않는다', async () => {
    // 저장이 잠기는 것과 별개로, **입력칸의 값 자체**가 바뀌면 관리자는 자기가 1 을
    // 넣은 줄 안다. F-013 의 증상이 정확히 이것이었다.
    const wrapper = mount((await load()).default)
    await new Promise(r => setTimeout(r, 0))
    const { quota } = await openAddForm(wrapper)

    await quota.setValue('0')
    await new Promise(r => setTimeout(r, 0))

    expect(quota.element.value, '입력칸의 값이 조용히 바뀌었다').not.toBe('1')
    wrapper.unmount()
  })

  it('안내 문구로 이유를 알려 준다', async () => {
    const wrapper = mount((await load()).default)
    await new Promise(r => setTimeout(r, 0))
    const { form, quota } = await openAddForm(wrapper)

    await quota.setValue('0')
    await new Promise(r => setTimeout(r, 0))

    expect(form.text(), '왜 저장이 안 되는지 화면이 말해 주지 않는다')
      .toContain('1명 이상')
    wrapper.unmount()
  })
})
