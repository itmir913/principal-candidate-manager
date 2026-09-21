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

// 대학 카드가 하나 있어야 [편집]·[+ 모집단위]·[모집단위 편집] 폼을 열 수 있다.
const UNIV = {
  id: 1, univ_name: '가대학', total_quota: 5, unlimited: false, prioritize_enrolled: 1,
  tracks: [{ id: 10, univ_id: 1, track_name: '가모집단위', unit_quota: 2,
             unlimited: false, prioritize_enrolled: 0 }],
}
const QUOTA_STATS = {
  all_round_ids: [], univs: [{ univ_id: 1, univ_name: '가대학', total_quota: 5, total_used: 0,
    tracks: [{ track_id: 10, track_name: '가모집단위', unit_quota: 2, unit_used: 0, by_round: [] }] }],
}

vi.mock('axios', () => {
  // 모집단위는 `/api/univ-tracks` 로 **따로** 온다(admin.js:160) — 대학 응답에 넣어도
  // 표가 그려지지 않는다. 그래서 [모집단위 편집] 폼을 못 열고 있었다.
  const res = (url = '') => Promise.resolve({
    data: /quota-stats/.test(String(url)) ? QUOTA_STATS
        : /univ-tracks/.test(String(url)) ? UNIV.tracks
        : /universities/.test(String(url)) ? [UNIV]
        : [],
    headers: {},
  })
  const axios = {
    get: res, post: res, put: res, patch: res, delete: res,
    interceptors: { request: { use: () => {} }, response: { use: () => {} } },
  }
  return { default: axios, ...axios }
})

const load = () => import('../src/components/admin/UniversitiesTab.vue')

/**
 * 정원을 입력할 수 있는 **네 폼**. 하나만 시험하면 나머지 셋은 가드를 빼도 아무도
 * 모른다 — 실제로 그랬다(4차 감사 치-1, 저장소 규칙 feedback_guard_all_entry_points).
 * 폼이 늘면 여기에도 추가해야 하고, 아래 "전 진입점" 검사가 누락을 잡는다.
 */
const FORMS = [
  ['대학 추가',      'univ-add-form'],
  ['대학 편집',      'univ-edit-form'],
  ['모집단위 추가',  'track-add-form'],
  ['모집단위 편집',  'track-edit-form'],
]

/** 이름표로 폼을 열고 {폼, 정원칸, 저장버튼} 을 돌려준다. */
async function openForm(wrapper, which) {
  const click = async (pred, what) => {
    const b = wrapper.findAll('button').find(pred)
    expect(b, `${what} 버튼을 찾지 못했다`).toBeTruthy()
    await b.trigger('click')
    await new Promise(r => setTimeout(r, 0))
  }
  if (which === '대학 추가')      await click(b => b.text().includes('대학 추가'), '[+ 대학 추가]')
  if (which === '대학 편집')      await click(b => b.text() === '편집', '[편집]')
  if (which === '모집단위 추가')  await click(b => b.text().includes('모집단위'), '[+ 모집단위]')
  if (which === '모집단위 편집') {
    // 대학 카드의 [편집]과 모집단위 행의 [편집]이 같은 라벨이다. 표 안(tr/td)에 있는
    // 쪽이 모집단위 것 — 라벨만 보고 고르면 대학 편집 폼이 열려 조용히 다른 것을
    // 시험하게 된다.
    const edits = wrapper.findAll('td button').filter(b => b.text() === '편집')
    expect(edits.length, '모집단위 [편집] 버튼이 없다 — 모집단위 표가 그려졌는지 확인하라')
      .toBeGreaterThan(0)
    await edits[0].trigger('click')
    await new Promise(r => setTimeout(r, 0))
  }

  // **폼을 신원으로 고른다.** 예전에는 "체크박스와 저장 버튼이 있는 마지막 div" 를
  // 잡았는데, 어떤 폼인지 확인하지 않아 [모집단위 편집]이 대학 편집 폼을 열어도
  // 6건이 조용히 대학 폼을 다시 시험했다(5차 감사 중-1).
  // 편집 폼에는 제목이 없어 텍스트로는 구분할 수 없으므로 `data-testid` 를 쓴다.
  const testid = FORMS.find(([k]) => k === which)[1]
  const form = wrapper.find(`[data-testid="${testid}"]`)
  expect(form.exists(),
    `${which} 폼(${testid})이 열리지 않았다 — 버튼이 다른 폼을 열었을 수 있다`).toBe(true)

  // 이름은 채워 둔다 — 정원만 보기 위해. 편집 폼은 이미 값이 있으므로 없을 수도 있으나,
  // **있는데 못 찾는 것**과 구분하려고 폼 안에서만 찾는다.
  const name = form.find('input[type="text"]')
  if (name.exists()) await name.setValue('가대학')

  // 추가 폼은 기본값이 `unlimited: true` 라 정원 칸이 아예 렌더되지 않는다.
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

/**
 * 실제 브라우저에서 **타이핑**이 일으키는 것은 `input` 이다(`change` 는 blur·Enter).
 * `@vue/test-utils` 의 `setValue` 는 둘 다 쏘므로, 그걸 쓰면 `onInput` 을 `onChange`
 * 로 바꾸는 회귀가 통과한다(4차 감사 중-B). `input` 만 쏜다.
 */
async function type(el, value) {
  el.element.value = value
  await el.trigger('input')
  await new Promise(r => setTimeout(r, 0))
}

describe('정원 입력 (F-013) — 화면 동작', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.spyOn(console, 'warn').mockImplementation(() => {})
  })
  afterEach(() => { vi.restoreAllMocks() })

  const BAD = ['0', '-3', '5.7', '', 'abc']

  // **네 폼 전부**를 돈다. 하나만 시험하던 동안, 나머지 셋에서 가드를 빼도
  // 전 검증이 초록이었다(4차 감사 치-1).
  for (const [which] of FORMS) {
    it.each(BAD)(`[${'%s'}] ${which}: 잘못된 정원이면 저장이 잠긴다`, async (typed) => {
      const wrapper = mount((await load()).default)
      await new Promise(r => setTimeout(r, 0))
      const { quota, save } = await openForm(wrapper, which)

      await type(quota, typed)

      expect(save.attributes('disabled'),
        `${which}: 정원 "${typed}" 인데 저장이 열려 있다 — 값이 조용히 보정됐을 수 있다(F-013)`)
        .toBeDefined()
      wrapper.unmount()
    })

    it(`${which}: 정원 1 이상이면 저장이 열린다`, async () => {
      // 위 단언이 "항상 잠겨 있다"로 통과하면 아무것도 지키지 못한다.
      const wrapper = mount((await load()).default)
      await new Promise(r => setTimeout(r, 0))
      const { quota, save } = await openForm(wrapper, which)

      await type(quota, '3')

      expect(save.attributes('disabled'), `${which}: 정상 입력인데 저장이 잠겼다`).toBeUndefined()
      wrapper.unmount()
    })
  }

  it('0 을 넣어도 화면의 값이 1 로 바뀌지 않는다', async () => {
    // 저장이 잠기는 것과 별개로, **입력칸의 값 자체**가 바뀌면 관리자는 자기가 1 을
    // 넣은 줄 안다. F-013 의 증상이 정확히 이것이었다.
    const wrapper = mount((await load()).default)
    await new Promise(r => setTimeout(r, 0))
    const { quota } = await openForm(wrapper, '대학 추가')

    await type(quota, '0')

    expect(quota.element.value, '입력칸의 값이 조용히 바뀌었다').not.toBe('1')
    wrapper.unmount()
  })

  it('안내 문구로 이유를 알려 준다', async () => {
    const wrapper = mount((await load()).default)
    await new Promise(r => setTimeout(r, 0))
    const { form, quota } = await openForm(wrapper, '대학 추가')

    await type(quota, '0')

    expect(form.text(), '왜 저장이 안 되는지 화면이 말해 주지 않는다')
      .toContain('1명 이상')
    wrapper.unmount()
  })

  it('정원을 다루는 저장 버튼이 전부 가드에 걸려 있다', async () => {
    // 위 테스트들은 **내가 아는 폼**만 돈다. 폼이 하나 늘었는데 FORMS 에 안 적으면
    // 그 폼은 조용히 검사 밖이 된다 — 4차 감사에서 정확히 그 상태였다.
    // 소스에서 세어, 가드 없는 저장 버튼이 남아 있으면 실패한다.
    const [{ default: fs }, { default: path }, { fileURLToPath }] =
      await Promise.all([import('node:fs'), import('node:path'), import('node:url')])
    const here = path.dirname(fileURLToPath(import.meta.url))
    const src = fs.readFileSync(
      path.join(here, '..', 'src', 'components', 'admin', 'UniversitiesTab.vue'), 'utf8')
    // **세는 방향을 뒤집는다.** 예전에는 *가드가 걸린* 버튼 수만 세어, 가드 없는 폼을
    // 새로 추가해도 4 == 4 로 통과했다(5차 감사 치-2). 이제 **정원을 다루는 폼**의
    // 수를 세고, 그 전부가 가드에 걸려 있는지 본다.
    const quotaForms = (src.match(/<QuotaInput/g) ?? []).length
    const guarded = (src.match(/:disabled="saving \|\| !(univ|track)FormValid"/g) ?? []).length
    expect(quotaForms, '정원 폼이 늘었는데 FORMS 목록에 없다 — 그 폼은 검사 밖이다')
      .toBe(FORMS.length)
    expect(guarded, '정원 폼 수와 가드 수가 어긋난다 — 가드 없는 저장 버튼이 있다')
      .toBe(quotaForms)
    // 표식도 폼 수만큼 있어야 한다 — 없으면 위 openForm 이 그 폼을 못 연다.
    expect((src.match(/data-testid="(?:univ|track)-(?:add|edit)-form"/g) ?? []).length,
      '폼 신원 표식이 빠졌다').toBe(FORMS.length)
  })
})
