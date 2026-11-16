// @vitest-environment jsdom
/**
 * 이슈 #49 — 새로고침 뒤 저장소에서 담임 신원을 복원하는 경로.
 *
 * 판별력의 소재: localStorage 를 직접 써 두고 pinia 를 새로 만들어 스토어 초기화를
 * 실제로 다시 돌린다. 인터셉터는 쓰지 않는다(graduate-identity.render.test.js 참고).
 */
import { describe, it, expect, vi, beforeAll, beforeEach } from 'vitest'
import { settle } from './settle.js'
import { mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'

vi.mock('axios', () => {
  const ok = (data) => Promise.resolve({ data, headers: {} })
  const get = (url = '') => {
    if (/rounds\/current/.test(String(url))) return ok(null)
    if (/teacher\/results/.test(String(url))) return ok({ rounds: [], results: [] })
    return ok([])
  }
  const axios = {
    get, post: () => ok({}), put: () => ok({}), patch: () => ok({}), delete: () => ok({}),
    interceptors: { request: { use: () => {} }, response: { use: () => {} } },
  }
  return { default: axios, ...axios }
})

import { useAuthStore } from '../src/stores/auth.js'

function store(entries) {
  localStorage.clear()
  for (const [k, v] of Object.entries(entries)) localStorage.setItem(k, v)
  setActivePinia(createPinia())
  return useAuthStore()
}

async function mountTeacherView() {
  const mod = await import('../src/views/TeacherView.vue')
  const w = mount(mod.default, {
    global: {
      stubs: { RouterLink: true, RouterView: true },
      mocks: { $route: { path: '/teacher', params: {}, query: {} }, $router: { push: () => {}, replace: () => {} } },
    },
  })
  await settle()
  return w
}

describe('담임 신원 복원 (이슈 #49)', () => {
  beforeAll(async () => {
    await import('../src/components/teacher/ClassTab.vue')
  })
  beforeEach(() => { vi.spyOn(console, 'warn').mockImplementation(() => {}) })

  // B. 판별력: 저장값 '0' 이 복원 과정에서 null·'' 로 바뀌면 졸업생 표시가 사라진다.
  it('B. 저장된 졸업생 0/0 을 복원하면 졸업생 담당으로 그린다', async () => {
    const auth = store({ pcm_token: 't', pcm_role: 'teacher', pcm_grade: '0', pcm_class_no: '0', pcm_teacher_name: '졸업생' })
    expect(auth.teacherKind).toBe('graduate')
    const w = await mountTeacherView()
    expect(w.text()).toContain('졸업생 담당')
    expect(w.text()).toContain('졸업생 현황')
    expect(w.text()).not.toMatch(/학년\s*\S*\s*반\s*담임/)
    w.unmount()
  })

  // C. 판별력: 복원 검증이 없으면 role=teacher·grade=null 반쪽 상태가 그대로 남는다.
  it('C. 반쪽 저장소(학년·반 없음)는 인증 상태를 전부 지운다', () => {
    const auth = store({ pcm_token: 't', pcm_role: 'teacher', pcm_teacher_name: '김선생' })
    expect([auth.token, auth.role, auth.grade, auth.classNo, auth.teacherName]).toEqual([null, null, null, null, null])
    expect(localStorage.getItem('pcm_token')).toBeNull()
    expect(localStorage.getItem('pcm_role')).toBeNull()
  })

  it('C-대조. 관리자 저장소는 복원 검증에 지워지지 않는다', () => {
    const auth = store({ pcm_token: 't', pcm_role: 'admin' })
    expect(auth.role).toBe('admin')
    expect(auth.teacherKind).toBeNull()
  })
})
