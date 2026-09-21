// @vitest-environment jsdom
/**
 * 스모크 렌더 — 모든 화면을 **실제로 마운트해 보고 터지지 않는지**만 본다.
 *
 * 왜 필요한가: `src/docs/13_frontend_pitfalls.md` §4 가 적어 둔 공백이다.
 * vite 는 함수 **본문 안의** 미정의 식별자를 컴파일 타임에 보지 않고, 이 저장소에는
 * eslint 도 타입 검사도 없다. 실제로 그 때문에 담임 [라운드 결과] 화면이 **아예 열리지
 * 않은** 사고가 있었다(`cancelDeptEdit` ↔ `cancelEdit` 오타, ReferenceError).
 * 백엔드 테스트·오라클·나머지 vitest 중 어느 것도 `.vue` 런타임을 실행하지 않으므로,
 * 그 유형은 사람이 화면을 열기 전까지 아무도 모른다.
 *
 * **이 파일이 하지 않는 것**: 외관·레이아웃·페인트는 단언하지 않는다. 그쪽은 여전히
 * 사람이 화면을 열어 확인하는 영역이다(13_frontend_pitfalls 의 나머지 항목 전부가
 * 그 종류다). 여기서 잡는 것은 "열리지도 않는다" 하나뿐이다.
 *
 * **한계**: 마운트와 첫 렌더에서 실행되는 코드만 지난다. 클릭해야 도달하는 핸들러
 * 안의 미정의 식별자는 여전히 잡히지 않는다.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'

// 모든 화면은 axios 로 서버와 말한다. 여기 한 곳만 막으면 api 모듈(admin.js/teacher.js)은
// **실물이 그대로 실행된다** — 래퍼의 오타도 함께 지나간다.
//
// 응답 형태가 엔드포인트마다 달라(배열 / {rows,total} / {univs:[]} …) 하나로 맞출 수 없다.
// 그래서 **빈 배열이면서 흔히 쓰이는 속성도 가진** 값을 준다. 형태가 안 맞아 나는 오류는
// 이 테스트가 잡으려는 결함이 아니라 잡음이므로, 키를 채워 조용히 지나가게 한다.
// (모든 속성에 응답하는 Proxy 도 시도했으나 Vue 반응성이 무한 재귀에 빠졌다 — 쓰지 마라.)
const flexible = () => Object.assign([], {
  univs: [], rows: [], items: [], tracks: [], areas: [], students: [], classes: [],
  logs: [], results: [], applications: [], rounds: [], all_round_ids: [], entries: [],
  total: 0, page: 1, per_page: 50, count: 0,
  // 중첩 응답 — 옵셔널 체이닝 없이 바로 파고드는 곳이 있어 형태를 맞춰 준다.
  // (예: OverviewTab 의 `data.value.all_time.total_rounds`)
  all_time: { total_rounds: 0, total_students: 0, total_applications: 0 },
  round: null, graduated: null, enrolled: null,
  by_status: {}, by_univ: [], recent: [], summary: {}, by_grade: {}, grades: [],
})

// ── 최소 픽스처 ─────────────────────────────────────────────────
// 빈 응답만 주면 화면 대부분이 `v-if` 뒤에서 렌더되지 않는다. 실제로 그 상태에서는
// "템플릿이 없는 이름을 참조" 같은 결함을 하나도 잡지 못했다(변이로 확인).
// 그래서 **목록이 하나씩은 있는** 상태를 만든다 — 표·카드·행이 실제로 그려진다.
// 필드는 백엔드 응답에서 가져왔다(results 행은 tools/oracle/actual.json 실측 형태).
const ROUND = { id: 1, status: 'CLOSED', opened_at: '2026-03-02T00:00:00Z',
                closed_at: '2026-03-10T00:00:00Z', finalized_at: null, needs_recalc: false }
const RESULT = {
  student_id: 1, track_id: 1, round_id: 1, name: '학생01', student_code: '2026001',
  grade: 3, class_no: 1, seq_no: 1, is_enrolled: true,
  univ_name: '가대학', track_name: '가모집단위', department_name: '컴퓨터공학과',
  total_score: 5.31304, score_detail: { 1: 5.31304 },
  ranking: 1, track_rank: 1, recommended: false, excluded: false,
  excluded_reason: null, abandoned: false,
}
const AREA = {
  id: 1, area_id: 1, name: '요소1', calc_type: 'NUMERIC', match_mode: 'UPPER',
  category_agg: null, lookup_scope: 'SIMPLE', multi_value: 0, teacher_editable: 1,
  max_score: 10, current_values: [''], sort_order: 1,
}
const UNIV = {
  id: 1, univ_id: 1, univ_name: '가대학', total_quota: 5, total_used: 1,
  prioritize_enrolled: 0,
  tracks: [{ id: 1, track_id: 1, track_name: '가모집단위', unit_quota: 2, unit_used: 1,
             prioritize_enrolled: 0, by_round: [] }],
}

/** URL 로 응답을 고른다. 못 찾으면 위의 느슨한 기본값. */
function fixtureFor(url = '') {
  const u = String(url)
  if (/\/api\/rounds\/\d+\/results/.test(u)) return [RESULT]
  if (/\/api\/rounds$/.test(u))                  return [ROUND]
  if (/\/api\/rounds\/\d+\/confirmation/.test(u)) return { total: 1, confirmed: 1, pending: [] }
  if (/\/api\/applications/.test(u))             return [RESULT]
  if (/quota-stats/.test(u))                      return { all_round_ids: [1], univs: [UNIV] }
  if (/\/api\/universities/.test(u))             return [UNIV]
  if (/\/api\/areas\/\d+\/(numeric-table|category-map|base-data)\/list/.test(u))
    return { rows: [], total: 0, page: 1, per_page: 50 }
  if (/\/api\/areas/.test(u))                    return [AREA]
  if (/\/api\/students/.test(u))                 return { rows: [RESULT], total: 1, page: 1, per_page: 50, by_grade: { 3: [1] } }
  if (/\/api\/classes/.test(u))                  return [{ grade: 3, class_no: 1, teacher_name: '담임', has_password: true }]
  if (/\/api\/audit-logs/.test(u))               return { rows: [], total: 0, page: 1, per_page: 50 }
  if (/\/api\/app-info/.test(u))                 return { version: '0.2.21', server_addr: '127.0.0.1:8080' }
  return flexible()
}

vi.mock('axios', () => {
  const res = (url) => Promise.resolve({ data: fixtureFor(url), headers: {} })
  const axios = {
    get: res, post: res, put: res, patch: res, delete: res,
    interceptors: { request: { use: () => {} }, response: { use: () => {} } },
  }
  return { default: axios, ...axios }
})

// 일부 화면은 `useRoute()`/`useRouter()` 를 쓴다(옵션 API 의 $route 목으로는 안 닿는다).
vi.mock('vue-router', async (orig) => ({
  ...(await orig()),
  useRoute:  () => ({ path: '/', params: {}, query: {}, name: 'x', fullPath: '/' }),
  useRouter: () => ({ push: () => {}, replace: () => {}, go: () => {}, back: () => {} }),
}))

// import 는 vi.mock 이후에 평가된다(vitest 가 호이스팅한다).
const views = import.meta.glob('../src/views/*.vue')
const adminTabs = import.meta.glob('../src/components/admin/*.vue')
const teacherTabs = import.meta.glob('../src/components/teacher/*.vue')
const common = import.meta.glob('../src/components/common/*.vue')

/**
 * props 를 요구하는 컴포넌트는 부모가 늘 값을 주므로 단독 마운트 대상이 아니다.
 * 목록을 여기 적어 두는 이유: 새 컴포넌트가 조용히 검사 밖으로 빠지지 않게 하려는 것이다
 * (아래 `모든 .vue 가 스모크 대상이거나 명시적으로 제외되어 있다` 테스트가 대조한다).
 */
const NEEDS_PROPS = new Set([
  '../src/components/admin/MiniPie.vue',            // value/총합을 부모가 준다
  '../src/components/admin/ScoreDemoCard.vue',      // 전형요소 한 건을 받는다
  '../src/components/common/HelpBox.vue',           // 안내 문구를 받는다
  '../src/components/common/ProductInfoCard.vue',   // 버전 정보를 받는다
  '../src/components/teacher/ApplicationDetailModal.vue', // 지원 한 건을 받는다
])

/** 라우터를 쓰는 화면이 있으므로 최소 스텁을 끼운다. */
const global = {
  stubs: { RouterLink: true, RouterView: true },
  mocks: {
    $route: { path: '/', params: {}, query: {} },
    $router: { push: () => {}, replace: () => {} },
  },
}

const all = { ...views, ...adminTabs, ...teacherTabs, ...common }
const targets = Object.keys(all).filter(p => !NEEDS_PROPS.has(p) && !p.endsWith('/App.vue'))

/**
 * 치명으로 볼 신호. **없는 이름을 쓴 것**만 고른다 —
 * prop 형태 경고나 개발 편의 경고까지 잡으면 잡음에 묻혀 아무도 안 본다.
 */
const FATAL = new RegExp([
  'ReferenceError',
  'is not defined',                              // ReferenceError 본문 + Vue 경고 양쪽
  'is not a function',
  'was accessed during render',                  // 템플릿이 없는 이름을 읽었다
  'Failed to resolve component',                 // 컴포넌트 이름 오타
  'Invalid vnode type',
].join('|'))

describe('스모크 렌더', () => {
  let errors

  beforeEach(() => {
    setActivePinia(createPinia())
    localStorage.clear()
    errors = []
    // error 와 warn 을 **둘 다** 모은다. 미정의 식별자는 예외가 아니라 경고로 나오는
    // 경우가 많다 — Vue 는 템플릿에서 없는 이름을 읽으면
    // "Property X was accessed during render but is not defined" 를 warn 으로 흘린다.
    // 처음에 warn 을 버렸더니 바로 그 유형(템플릿이 제거된 식별자를 참조)을 놓쳤다.
    const collect = (...a) => errors.push(a.map(x => (x && x.message) || String(x)).join(' '))
    vi.spyOn(console, 'error').mockImplementation(collect)
    vi.spyOn(console, 'warn').mockImplementation(collect)
  })

  afterEach(() => { vi.restoreAllMocks() })

  it('마운트 대상이 비어 있지 않다', () => {
    // glob 경로가 어긋나면 0개를 돌면서 초록이 뜬다 — 그 침묵부터 막는다.
    expect(targets.length).toBeGreaterThanOrEqual(15)
  })

  it.each(targets)('%s 이(가) 열린다', async (path) => {
    // 비동기 렌더(computed 가 로드 뒤에 처음 평가되는 경우)에서 터지면 console 이 아니라
    // 처리되지 않은 rejection 으로 샌다. 그 상태로 두면 vitest 가 "어느 화면인지"를
    // 알려 주지 못한다 — 실제로 AreasTab 의 import 누락이 그렇게 보고됐다.
    const onRejection = (e) => errors.push(String(e?.reason?.message ?? e?.reason ?? e))
    process.on('unhandledRejection', onRejection)

    const mod = await all[path]()
    const wrapper = mount(mod.default, { global })
    // onMounted 의 비동기 로드까지 흘려보낸다 — 사고가 났던 지점이 거기였다.
    await new Promise(r => setTimeout(r, 0))
    await new Promise(r => setTimeout(r, 0))
    process.off('unhandledRejection', onRejection)

    const fatal = errors.filter(e => FATAL.test(e))
    expect(fatal, `${path} 렌더 중 치명 오류`).toEqual([])
    expect(wrapper.html()).toBeTruthy()
    wrapper.unmount()
  })
})

/**
 * 결과 패널은 라운드를 **클릭해야** 열린다. 마운트만으로는 `v-if="!selected"` 뒤에
 * 가려져 있어, 그 안의 결함(템플릿이 없는 이름을 참조 / computed 가 없는 함수를 호출)을
 * 하나도 잡지 못했다 — 변이로 확인했다.
 *
 * RoundsTab 만 따로 한 단계 더 밟는 이유: 이 저장소에서 가장 크고(1400줄 이상) 가장
 * 자주 손대는 화면이며, 점수·순위·추천이 모두 여기서 보인다. 다른 화면까지 조작을
 * 늘리지는 않는다 — 그 순간 이 파일은 스모크가 아니라 컴포넌트 테스트가 된다.
 */
describe('스모크 렌더 — 라운드 결과 패널', () => {
  let errors
  beforeEach(() => {
    setActivePinia(createPinia())
    errors = []
    const collect = (...a) => errors.push(a.map(x => (x && x.message) || String(x)).join(' '))
    vi.spyOn(console, 'error').mockImplementation(collect)
    vi.spyOn(console, 'warn').mockImplementation(collect)
  })
  afterEach(() => { vi.restoreAllMocks() })

  it('라운드를 고르면 결과 표까지 그려진다', async () => {
    const onRejection = (e) => errors.push(String(e?.reason?.message ?? e?.reason ?? e))
    process.on('unhandledRejection', onRejection)

    const mod = await all['../src/components/admin/RoundsTab.vue']()
    const wrapper = mount(mod.default, { global })
    await new Promise(r => setTimeout(r, 0))

    // ① 라운드를 고른다
    const card = wrapper.find('.cursor-pointer')
    expect(card.exists(), '라운드 카드가 없다 — 픽스처가 비었는지 확인하라').toBe(true)
    await card.trigger('click')
    await new Promise(r => setTimeout(r, 0))

    // ② [결과] 서브탭으로 넘어간다. 기본값은 [지원 현황]이라, 여기까지 오지 않으면
    //    결과 표(resultsByView·tieSet·univAutoButtonKeys)는 끝내 렌더되지 않는다.
    const resultsTab = wrapper.findAll('button').find(b => b.text() === '결과')
    expect(resultsTab, '[결과] 서브탭 버튼을 찾지 못했다').toBeTruthy()
    await resultsTab.trigger('click')
    await new Promise(r => setTimeout(r, 0))
    await new Promise(r => setTimeout(r, 0))
    process.off('unhandledRejection', onRejection)

    // 결과 표가 **정말로** 그려졌는지 확인한다. 이 단언이 없으면 표가 통째로 빠져도
    // "오류 0건"으로 초록이 뜬다 — 실제로 한 번 그렇게 속았다.
    // 표가 통째로 빠져도 "오류 0건"이면 초록이 뜬다 — 실제로 한 번 그렇게 속았다.
    // 그래서 파생값이 화면에 닿았다는 증거를 셋 다 확인한다.
    const text = wrapper.text()
    expect(text, '결과 행이 그려지지 않았다').toContain('학생01')            // groupBy*
    expect(text, '잔여석이 그려지지 않았다').toContain('잔여 4석')            // buildTrackQuotaMap
    expect(text, '자동 추천 버튼이 없다').toContain('전체 자동 추천')          // univAutoButtonKeys
    expect(errors.filter(e => FATAL.test(e)), '결과 패널 렌더 중 치명 오류').toEqual([])
    wrapper.unmount()
  })
})

describe('스모크 대상 목록이 낡지 않았다', () => {
  it('모든 .vue 가 스모크 대상이거나 명시적으로 제외되어 있다', () => {
    const uncovered = Object.keys(all)
      .filter(p => !p.endsWith('/App.vue'))
      .filter(p => !targets.includes(p) && !NEEDS_PROPS.has(p))
    expect(uncovered, '새 화면이 검사 밖에 있다').toEqual([])
  })

  it('NEEDS_PROPS 에 적힌 파일이 실제로 존재한다', () => {
    const gone = [...NEEDS_PROPS].filter(p => !(p in all))
    expect(gone, '지워진 컴포넌트가 제외 목록에 남아 있다').toEqual([])
  })
})
