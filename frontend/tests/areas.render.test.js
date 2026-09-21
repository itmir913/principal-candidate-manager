// @vitest-environment jsdom
/**
 * 전형요소 화면 — **요소를 골라 점수표까지** 그려 본다.
 *
 * 왜 필요한가: `AreasTab.vue` 는 1336줄인데 스모크의 마운트 렌더는 **135자**뿐이었다.
 * 오른쪽 패널(기본 정보·점수표·기초데이터)이 전부 `selected` 뒤에 있어, 요소를
 * 클릭하지 않으면 통째로 검사 밖이다 — 저장소의 **미검증 표면 2위**였다
 * (5차 감사 메타 판단: "테스트 수를 늘리지 말고 이 둘에 힘을 써라").
 *
 * 여기서 지키는 것: 요소 목록이 그려지는가, 고르면 점수표가 열리는가, 만점 합계가
 * 맞게 표시되는가, 탭을 옮기면 기초데이터가 나오는가.
 * **외관은 여전히 보지 않는다** — 그려졌는지와 값이 맞는지만 본다.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'

const AREAS = [
  { id: 1, name: '교과성적', calc_type: 'NUMERIC', match_mode: 'UPPER', category_agg: null,
    lookup_scope: 'SIMPLE', multi_value: 0, teacher_editable: 1, max_score: 40, sort_order: 1 },
  { id: 2, name: '출결', calc_type: 'CATEGORY', match_mode: null, category_agg: 'MAX',
    lookup_scope: 'SIMPLE', multi_value: 0, teacher_editable: 1, max_score: 10, sort_order: 2 },
  { id: 3, name: '교사추천', calc_type: 'MANUAL', match_mode: null, category_agg: null,
    lookup_scope: 'SIMPLE', multi_value: 0, teacher_editable: 1, max_score: 50, sort_order: 3 },
]
// getNumericTableList 의 응답. threshold·score 는 백엔드가 Score 로 직렬화해 내려준다.
const NUMERIC_ROWS = {
  rows: [
    { id: 11, area_id: 1, track_id: null, threshold: 1.5, score: 40 },
    { id: 12, area_id: 1, track_id: null, threshold: 2.0, score: 35 },
  ],
  total: 2, page: 1, per_page: 50,
}
const BASE_ROWS = { rows: [], total: 0, page: 1, per_page: 50 }

vi.mock('axios', () => {
  const res = (url = '') => {
    const u = String(url)
    const data =
      /numeric-table\/list/.test(u) ? NUMERIC_ROWS
      : /category-map\/list/.test(u) ? { rows: [{ id: 21, area_id: 2, track_id: null, category: '무단결석 0회', score: 10 }], total: 1, page: 1, per_page: 50 }
      : /base-data\/list/.test(u)   ? BASE_ROWS
      : /\/api\/areas$/.test(u)     ? AREAS
      : /\/api\/rounds/.test(u)     ? []
      : []
    return Promise.resolve({ data, headers: {} })
  }
  const axios = { get: res, post: res, put: res, patch: res, delete: res,
    interceptors: { request: { use: () => {} }, response: { use: () => {} } } }
  return { default: axios, ...axios }
})

const tick = async () => { await new Promise(r => setTimeout(r, 0)); await new Promise(r => setTimeout(r, 0)) }
const load = () => import('../src/components/admin/AreasTab.vue')

/** 요소 목록에서 이름으로 하나를 고른다. */
async function pickArea(wrapper, name) {
  const row = wrapper.findAll('.cursor-pointer').find(d => d.text().includes(name))
  expect(row, `[${name}] 요소가 목록에 없다`).toBeTruthy()
  await row.trigger('click')
  await tick()
}

describe('전형요소 화면 — 요소를 골라 점수표까지', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.spyOn(console, 'warn').mockImplementation(() => {})
  })
  afterEach(() => { vi.restoreAllMocks() })

  it('요소 목록과 만점 합계가 그려진다', async () => {
    const wrapper = mount((await load()).default)
    await tick()

    const t = wrapper.text()
    for (const a of AREAS) expect(t, `[${a.name}] 이 목록에 없다`).toContain(a.name)
    // 40 + 10 + 50 = 100. totalMaxScore(logic/areaTotals.js)가 화면에 닿는 유일한 지점이다.
    expect(t, '만점 합계가 틀리거나 안 그려졌다').toContain('100점')
    wrapper.unmount()
  })

  it('NUMERIC 요소를 고르면 점수표가 열린다', async () => {
    const wrapper = mount((await load()).default)
    await tick()
    await pickArea(wrapper, '교과성적')

    const t = wrapper.text()
    expect(t, '기본 정보가 안 보인다').toContain('교과성적')
    // 점수표 행이 실제로 그려져야 한다 — "등록된 점수 기준 없음" 이면 안 된다.
    expect(t, '점수 기준이 비어 있다고 나온다 — 픽스처가 닿지 않았다')
      .not.toContain('등록된 점수 기준 없음')
    // **'40' 은 쓰지 않는다** — 만점 표기·양식 예시·설명문에도 있어 점수 열을 비워도
    // 통과했다(6차 감사 중-1). 점수표에만 나오는 값으로 고른다.
    expect(t, '점수표 기준값이 안 보인다').toContain('1.5')
    expect(t, '점수표 두 번째 행이 안 보인다').toContain('2')
    expect(t, '점수 열이 비어 있다').toContain('35')
    wrapper.unmount()
  })

  it('CATEGORY 요소는 범주 목록을 보여 준다', async () => {
    // 같은 패널이 calc_type 에 따라 다른 표를 그린다. 하나만 보면 나머지가 깨져도 모른다.
    const wrapper = mount((await load()).default)
    await tick()
    await pickArea(wrapper, '출결')

    const t = wrapper.text()
    expect(t, '범주 값이 안 보인다').toContain('무단결석 0회')
    // `/최대|MAX/` 는 점수 기준 탭의 고정 안내문 "소수점 최대 5자리" 에 항상 걸린다 —
    // 범주 집계 span 을 통째로 지워도 통과했다. 라벨과 값을 함께 본다.
    expect(t, '범주 집계 라벨이 없다').toContain('범주 집계')
    wrapper.unmount()
  })

  it('MANUAL 요소는 점수표 대신 기초데이터 탭으로 연다', async () => {
    // selectArea 가 calc_type 으로 첫 탭을 고른다(AreasTab.vue:945).
    // 이 분기가 뒤집히면 MANUAL 요소에서 빈 점수표가 열린다.
    const wrapper = mount((await load()).default)
    await tick()
    await pickArea(wrapper, '교사추천')

    // 부정문 하나만 두면 **오른쪽 패널이 통째로 비어도** 통과한다(6차 감사 중-1).
    // 무엇이 열렸는지 긍정으로 단언한다.
    const t = wrapper.text()
    expect(t, 'MANUAL 인데 점수 기준 화면이 열렸다').not.toContain('등록된 점수 기준 없음')
    expect(t, '오른쪽 패널이 비었다 — 기초 데이터 탭이 열려야 한다').toContain('교사추천')
    expect(t, '기초 데이터 탭이 아니다').toContain('기초 데이터')
    wrapper.unmount()
  })

  it('요소를 바꾸면 표도 함께 바뀐다', async () => {
    // 선택만 바뀌고 표가 안 따라오면 **이전 요소의 점수표를 보면서 편집**하게 된다.
    const wrapper = mount((await load()).default)
    await tick()

    await pickArea(wrapper, '교과성적')
    expect(wrapper.text()).toContain('1.5')

    await pickArea(wrapper, '출결')
    const t = wrapper.text()
    expect(t, '요소를 바꿨는데 이전 점수표가 남아 있다').not.toContain('1.5')
    expect(t).toContain('무단결석 0회')
    wrapper.unmount()
  })
})
