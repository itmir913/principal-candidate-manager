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
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { settle } from './settle.js'
import { mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'

/** 실패시킬 URL 패턴. 테스트마다 정한다. */
let failing = /$^/

vi.mock('axios', () => {
  const empty = Object.assign([], { rows: [], total: 0, page: 1, per_page: 50, rounds: [], results: [], univs: [] })
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
  beforeEach(() => {
    setActivePinia(createPinia())
    failing = /$^/
    vi.spyOn(console, 'warn').mockImplementation(() => {})
    vi.spyOn(console, 'error').mockImplementation(() => {})
  })
  afterEach(() => { vi.restoreAllMocks() })

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
