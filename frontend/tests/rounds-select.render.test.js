// @vitest-environment jsdom
/**
 * 관리자 [라운드] — 라운드를 바꿀 때 **이전 라운드의 표가 새 라운드 머리글 아래 남지 않는가**.
 *
 * `selectRound` 는 지원·결과를 다시 불러오기만 하고 비우지도, 실패를 잡지도 않았다.
 * 조회가 실패하면 옛 행이 그대로 남았고, 그 행의 [추천]·[미선발] 버튼은 행 자신의
 * round_id — 즉 **옛 라운드**에 대해 동작한다. 빠르게 두 라운드를 누르면 먼저 누른
 * 쪽의 늦은 응답이 나중 것을 덮어썼다(2026-10-06 프론트 감사 F-4).
 *
 * 판별력의 소재: 라운드별 응답을 이 파일에서 직접 만든다 — 한 라운드는 실패시키고,
 * 한 라운드는 응답을 붙잡아 두었다가 다른 라운드를 고른 뒤에 풀어 준다.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { settle } from './settle.js'
import { mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'

const R = (id) => ({ id, status: 'CLOSED', opened_at: '2026-03-02T00:00:00Z',
                     closed_at: '2026-03-10T00:00:00Z', finalized_at: null, needs_recalc: false })
const ROW = (rid, sid, name) => ({
  student_id: sid, track_id: 1, round_id: rid, name, student_code: `C${sid}`,
  grade: 3, class_no: 1, seq_no: sid, is_enrolled: true,
  univ_name: '가대학', track_name: '가모집단위', department_name: '학과',
  total_score: 5, score_detail: { 1: 5 }, ranking: 1, track_rank: 1,
  recommended: false, excluded: false, excluded_reason: null, abandoned: false,
})

/** 라운드 id → 지원 목록 응답(값 또는 Promise). 테스트마다 갈아 끼운다. */
let appsByRound

vi.mock('axios', () => {
  const ok = (data) => Promise.resolve({ data, headers: {} })
  const get = (url = '', config = {}) => {
    const u = String(url)
    if (/\/api\/rounds$/.test(u)) return ok([R(1), R(2)])
    if (/\/api\/applications/.test(u)) {
      return Promise.resolve(appsByRound[config.params.round_id]()).then(ok)
    }
    const m = u.match(/\/api\/rounds\/(\d+)\/results/)
    if (m) return Promise.resolve(appsByRound[Number(m[1])]()).then(ok)
    if (/quota-stats/.test(u)) return ok({ all_round_ids: [1, 2], univs: [] })
    if (/\/api\/areas/.test(u)) return ok([])
    return ok([])
  }
  const axios = {
    get, post: () => ok({}), put: () => ok({}), patch: () => ok({}), delete: () => ok({}),
    interceptors: { request: { use: () => {} }, response: { use: () => {} } },
  }
  return { default: axios, ...axios }
})

const global = {
  stubs: { RouterLink: true, RouterView: true },
  mocks: { $route: { path: '/', params: {}, query: {} }, $router: { push: () => {}, replace: () => {} } },
}

async function clickRound(wrapper, id) {
  const card = wrapper.findAll('.cursor-pointer').find(d => d.text().includes(`${id}차 라운드`))
  expect(card, `${id}차 라운드 카드가 없다`).toBeTruthy()
  await card.trigger('click')
  await settle()
}

describe('라운드를 바꾸면 이전 라운드의 표가 남지 않는다', () => {
  let rejections
  const onRejection = (e) => rejections.push(String(e?.reason?.message ?? e?.reason ?? e))

  beforeEach(() => {
    setActivePinia(createPinia())
    localStorage.clear()
    rejections = []
    process.on('unhandledRejection', onRejection)
    vi.spyOn(console, 'warn').mockImplementation(() => {})
  })
  afterEach(() => {
    process.off('unhandledRejection', onRejection)
    vi.restoreAllMocks()
  })

  // 판별력: selectRound 가 표를 비우지 않거나 catch 가 없으면, 실패한 2차 라운드
  // 머리글 아래 1차 라운드의 '김갑돌' 가 남고 오류 문구가 없다.
  it('새 라운드 조회가 실패하면 옛 행을 지우고 오류를 보인다', async () => {
    appsByRound = {
      1: () => [ROW(1, 1, '김갑돌')],
      2: () => Promise.reject(Object.assign(new Error('조회 실패 Z'), { response: { data: '조회 실패 Z' } })),
    }
    const mod = await import('../src/components/admin/RoundsTab.vue')
    const wrapper = mount(mod.default, { global })
    await settle()

    await clickRound(wrapper, 1)
    expect(wrapper.text()).toContain('김갑돌')

    await clickRound(wrapper, 2)
    const t = wrapper.text()
    expect(t, '실패한 라운드 아래 이전 라운드의 행이 남았다').not.toContain('김갑돌')
    expect(t).toContain('이 라운드의 지원·결과를 불러오지 못했습니다')
    expect(t).toContain('조회 실패 Z')
    expect(rejections).toEqual([])
    wrapper.unmount()
  })

  // 판별력: loadApps·loadResults 의 라운드 비교를 지우면, 늦게 도착한 1차 라운드
  // 응답('김갑돌')이 2차 라운드('이을순') 표를 덮어쓴다. 두 비교를 하나씩 지워 각각
  // 잡히는 것을 확인했다(2026-10-06) — 결과 쪽은 [결과] 탭을 열어야 보인다.
  it('먼저 누른 라운드의 늦은 응답이 나중 라운드의 표를 덮어쓰지 않는다', async () => {
    let release
    const late = new Promise(r => { release = r })
    appsByRound = {
      1: () => late.then(() => [ROW(1, 1, '김갑돌')]),
      2: () => [ROW(2, 2, '이을순')],
    }
    const mod = await import('../src/components/admin/RoundsTab.vue')
    const wrapper = mount(mod.default, { global })
    await settle()

    await clickRound(wrapper, 1)   // 응답이 붙잡혀 있다
    await clickRound(wrapper, 2)
    expect(wrapper.text()).toContain('이을순')

    release()                      // 이제서야 1차 라운드 응답이 도착한다
    await settle()
    const t = wrapper.text()
    expect(t, '늦게 온 1차 라운드 응답이 2차 라운드 표를 덮어썼다').not.toContain('김갑돌')
    expect(t).toContain('이을순')

    // 결과 표는 [결과] 탭에 있다 — 지원 현황 탭만 보면 결과 쪽 덮어쓰기를 못 본다
    const tab = wrapper.findAll('button').find(b => b.text() === '결과')
    expect(tab, '[결과] 탭 버튼이 없다').toBeTruthy()
    await tab.trigger('click')
    await settle()
    const r = wrapper.text()
    expect(r, '늦게 온 1차 라운드 결과가 2차 라운드 결과 표를 덮어썼다').not.toContain('김갑돌')
    expect(r, '결과 표가 그려지지 않았다').toContain('이을순')
    wrapper.unmount()
  })
})
