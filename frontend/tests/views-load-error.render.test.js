// @vitest-environment jsdom
/**
 * 화면 틀(관리자·담임 사이드바, 로그인)이 조회 실패를 "없음"으로 위장하지 않는가.
 *
 * - 사이드바의 라운드 상태: 조회가 실패하면 예전엔 "진행 중인 라운드 없음"을 그렸다.
 *   TeacherView 의 주석은 정반대("위장하지 않도록")를 주장하고 있었다(2026-10-07 수정
 *   감사 B-2).
 * - 로그인 화면의 학급 목록: 실패하면 학년 목록이 빈 채로 남아, 담임은 왜 로그인할 수
 *   없는지 몰랐다(같은 감사 C-5).
 *
 * 판별력의 소재: 해당 조회만 실패시킨다. catch 가 다시 null/빈 상태로만 돌아가면
 * 오류 문구가 없고 "진행 중인 라운드 없음"이 그려진다.
 */
import { describe, it, expect, vi, beforeAll, beforeEach, afterEach } from 'vitest'
import { settle } from './settle.js'
import { mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'

/** 실패시킬 URL 패턴. 테스트마다 정한다. */
let failing = /$^/

vi.mock('axios', () => {
  // 빈 응답이면서 흔한 속성도 가진 값 — smoke-render 의 flexible() 과 같은 형태.
  // AdminView 는 기본 탭(OverviewTab)을 띄우고, 그 탭은 `all_time.total_rounds` 로 바로
  // 파고든다. 이 키가 없으면 비동기 탭 로드 타이밍에 따라 미처리 오류가 난다
  // (로컬은 통과, GitHub CI 는 실패 — 2026-10-07).
  const empty = Object.assign([], {
    univs: [], rows: [], items: [], tracks: [], areas: [], students: [], classes: [],
    logs: [], results: [], applications: [], rounds: [], all_round_ids: [], entries: [],
    total: 0, page: 1, per_page: 50, count: 0,
    all_time: { total_rounds: 0, total_applicants: 0, confirmed: 0, abandoned: 0 },
    round: null, graduated: null, enrolled: null,
    by_status: {}, by_univ: [], recent: [], summary: {}, by_grade: {}, grades: [],
  })
  const get = (url = '') => {
    const u = String(url)
    if (failing.test(u)) {
      return Promise.reject(Object.assign(new Error('서버 응답 없음'), { response: { data: '서버 응답 없음' } }))
    }
    if (/rounds\/current/.test(u)) return Promise.resolve({ data: null, headers: {} })
    return Promise.resolve({ data: empty, headers: {} })
  }
  const ok = () => Promise.resolve({ data: {}, headers: {} })
  const axios = {
    get, post: ok, put: ok, patch: ok, delete: ok,
    interceptors: { request: { use: () => {} }, response: { use: () => {} } },
  }
  return { default: axios, ...axios }
})

const global = {
  stubs: { RouterLink: true, RouterView: true },
  mocks: { $route: { path: '/', params: {}, query: {} }, $router: { push: () => {}, replace: () => {} } },
}

function signInAs(role) {
  localStorage.clear()
  localStorage.setItem('pcm_token', 'test-token')
  localStorage.setItem('pcm_role', role)
  localStorage.setItem('pcm_grade', '3')
  localStorage.setItem('pcm_class_no', '1')
  localStorage.setItem('pcm_teacher_name', '김담임')
}

describe('화면 틀 — 조회 실패를 빈 상태로 위장하지 않는다', () => {
  // AdminView·TeacherView 는 탭을 defineAsyncComponent 로 띄운다. 모듈을 미리 받아 두지
  // 않으면 탭 로드가 테스트가 끝난 뒤에 끝나, 탭 안의 오류가 다른 파일 실행 중에 터지거나
  // (GitHub CI) 아예 안 보인다(로컬). 미리 받아 두면 마운트 직후 settle 안에서 드러난다.
  // 판별력 확인(2026-10-07): 아래 mock 에서 all_time 을 빼면 이 파일이 실패한다.
  beforeAll(async () => {
    await Promise.all([
      import('../src/components/admin/OverviewTab.vue'),
      import('../src/components/teacher/ApplicationTab.vue'),
      import('../src/components/teacher/ClassTab.vue'),
      import('../src/components/teacher/ResultsTab.vue'),
    ])
  })

  let unhandled
  const onUnhandled = (e) => unhandled.push(String(e?.reason?.message ?? e?.reason ?? e?.message ?? e))
  beforeEach(() => {
    setActivePinia(createPinia())
    unhandled = []
    process.on('unhandledRejection', onUnhandled)
    process.on('uncaughtException', onUnhandled)
    failing = /$^/
    vi.spyOn(console, 'warn').mockImplementation(() => {})
    vi.spyOn(console, 'error').mockImplementation(() => {})
  })
  afterEach(async () => {
    // 비동기 탭 로드가 끝날 시간을 준 뒤, 그 사이 난 미처리 오류도 실패로 본다
    await settle()
    process.off('unhandledRejection', onUnhandled)
    process.off('uncaughtException', onUnhandled)
    vi.restoreAllMocks()
    expect(unhandled, '화면 틀 마운트 중 미처리 오류').toEqual([])
  })

  it.each([
    ['TeacherView', 'teacher'],
    ['AdminView', 'admin'],
  ])('%s — 현재 라운드 조회 실패는 "라운드 확인 실패"로 보인다', async (name, role) => {
    signInAs(role)
    failing = /rounds\/current/
    const mod = await import(`../src/views/${name}.vue`)
    const wrapper = mount(mod.default, { global })
    await settle()

    const t = wrapper.text()
    expect(t).toContain('라운드 확인 실패')
    expect(t, '실패를 "없음"으로 그렸다').not.toContain('진행 중인 라운드 없음')
    wrapper.unmount()
  })

  it.each([
    ['TeacherView', 'teacher'],
    ['AdminView', 'admin'],
  ])('%s — 조회가 성공하고 라운드가 없으면 "진행 중인 라운드 없음"', async (name, role) => {
    // 위 테스트의 짝 — 실패 표시가 정상 경로까지 덮으면 여기서 잡힌다
    signInAs(role)
    const mod = await import(`../src/views/${name}.vue`)
    const wrapper = mount(mod.default, { global })
    await settle()

    const t = wrapper.text()
    expect(t).toContain('진행 중인 라운드 없음')
    expect(t).not.toContain('라운드 확인 실패')
    wrapper.unmount()
  })

  it('LoginView — 학급 목록 조회 실패를 알린다', async () => {
    localStorage.clear()
    failing = /\/api\/classes/
    const mod = await import('../src/views/LoginView.vue')
    const wrapper = mount(mod.default, { global })
    await settle()

    expect(wrapper.text()).toContain('학급 목록을 불러오지 못했습니다')
    wrapper.unmount()
  })
})
