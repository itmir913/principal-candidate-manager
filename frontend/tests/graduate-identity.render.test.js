// @vitest-environment jsdom
/**
 * 이슈 #49 — 졸업생 담당(grade 0·class_no 0)이 "null학년 null반 담임"·"번 이름"으로 보였다.
 *
 * 사양: 백엔드가 0/0 을 주면 졸업생 담당, 양의 정수면 일반 담임, 그 외(null 포함)에서는
 * 일반 담임 문자열을 만들지 않고 "다시 로그인" 안내만 보인다.
 *
 * 판별력의 소재:
 * - axios mock 이 스토어가 `interceptors.response.use` 에 넘긴 에러 핸들러를 붙잡는다.
 *   401 은 이 핸들러를 **실제로** 지나게 해, 손으로 상태를 null 로 만드는 대체물이 아니다.
 * - 스토어는 모듈 전역으로 인터셉터를 한 번만 등록하고, 그 핸들러는 처음 만든 스토어
 *   인스턴스를 붙잡는다. 그래서 이 파일은 pinia 를 **하나만** 만들고 테스트마다 상태를
 *   다시 쓴다. 저장소 복원 경로는 pinia 를 새로 만들어야 하므로
 *   graduate-identity.restore.test.js 에 따로 둔다.
 * - 담임 이름 픽스처에 "담임"을 넣지 않는다 — `학년 … 반 담임` 단언과 섞이지 않게.
 */
import { describe, it, expect, vi, beforeAll, beforeEach, afterEach } from 'vitest'
import { settle } from './settle.js'
import { mount } from '@vue/test-utils'
import { h } from 'vue'
import { createPinia, setActivePinia } from 'pinia'
import { createRouter, createMemoryHistory, RouterView } from 'vue-router'
import { dialogState, settleDialog } from '../src/components/common/dialog.js'

const OPEN_ROUND = { id: 3, status: 'OPEN', opened_at: '2026-03-21T00:00:00Z',
                     closed_at: null, finalized_at: null, needs_recalc: false }
const GRAD_STUDENT = { id: 1, name: '학생01', student_code: '2020001',
                       grade: null, class_no: null, seq_no: null, is_enrolled: false }
const AREA_CTX = { area_id: 1, area_name: '요소1', calc_type: 'NUMERIC', match_mode: 'UPPER',
                   lookup_scope: 'SIMPLE', multi_value: 0, teacher_editable: 1,
                   max_score: 10, current_values: [], table: [] }

/** 스토어가 등록한 응답 에러 핸들러 */
const captured = vi.hoisted(() => ({ onError: null }))
/** 로그인 POST 응답. 테스트마다 바꾼다. */
let loginReply

vi.mock('axios', () => {
  const ok = (data) => Promise.resolve({ data, headers: {} })
  // axios 처럼 실패 응답을 인터셉터에 통과시킨다.
  const fail = (status, data) => {
    const e = Object.assign(new Error(String(data)), { response: { status, data } })
    return captured.onError ? captured.onError(e) : Promise.reject(e)
  }
  const get = (url = '') => {
    const u = String(url)
    if (/rounds\/current/.test(u))                     return ok(OPEN_ROUND)
    if (/teacher\/students/.test(u))                   return ok([GRAD_STUDENT])
    if (/teacher\/universities\/\d+\/tracks/.test(u))  return ok([{ id: 10, track_name: '가모집단위' }])
    if (/teacher\/universities/.test(u))               return ok([{ id: 1, univ_name: '가대학' }])
    if (/teacher\/results/.test(u))                    return ok({ rounds: [], results: [] })
    if (/teacher\/applications/.test(u))               return ok([])
    if (/confirm/.test(u))                             return ok({ confirmed: false, confirmed_at: null })
    if (/area-context/.test(u))                        return ok([AREA_CTX])
    if (/\/api\/classes/.test(u))                      return ok([{ grade: 0, class_no: 0 }, { grade: 3, class_no: 1 }])
    return ok([])
  }
  const post = (url = '', body) => {
    const u = String(url)
    if (/auth\/teacher/.test(u)) {
      const r = loginReply(body)
      return r.status ? fail(r.status, r.data) : ok(r.data)
    }
    if (/area-score-preview/.test(u)) return ok({ score: 5, matched_keys: [], warning: null, error: null })
    return ok({})
  }
  const axios = {
    get, post, put: () => ok({}), patch: () => ok({}), delete: () => ok({}),
    interceptors: {
      request: { use: () => {} },
      response: { use: (_ok, onError) => { captured.onError = onError } },
    },
  }
  return { default: axios, ...axios }
})

import { useAuthStore, setRouter } from '../src/stores/auth.js'

const RESP_401 = () => Object.assign(new Error('401'), { response: { status: 401, data: '인증 만료' } })
/** 붙잡은 401 핸들러를 실제로 부른다(핸들러는 거부를 돌려준다). */
async function fire401() {
  expect(captured.onError, '스토어가 응답 인터셉터를 등록하지 않았다').toBeTypeOf('function')
  await captured.onError(RESP_401()).catch(() => {})
  await settle()
}

let auth
function setIdentity({ role = 'teacher', grade, classNo, name = '김선생' }) {
  auth.token = 'test-token'
  auth.role = role
  auth.grade = grade
  auth.classNo = classNo
  auth.teacherName = name
}

const HOMEROOM_LABEL = /\d*\s*학년\s*\S*\s*반\s*담임/
const NULLISH = /\b(?:null|undefined|NaN)\b/

const routerStub = () => ({ push: vi.fn(() => Promise.resolve()), replace: vi.fn(), currentRoute: { value: { path: '/teacher' } } })

async function mountTeacherView() {
  const mod = await import('../src/views/TeacherView.vue')
  const stub = routerStub()
  setRouter(stub)
  const w = mount(mod.default, {
    global: {
      stubs: { RouterLink: true, RouterView: true },
      mocks: { $route: { path: '/teacher', params: {}, query: {} }, $router: stub },
    },
  })
  await settle()
  return { w, stub }
}

/** 사이드바를 접어 아바타를 그리게 한다. */
async function avatarText(w) {
  await w.find('[data-testid="sidebar-toggle"]').trigger('click')
  await settle()
  // 접힘이 실제로 일어났는지 — 펼친 정보 카드의 [로그아웃] 이 사라져야 한다.
  // 이게 없으면 아바타 없음('') 단언이 접히지 않은 채로 헛통과한다.
  expect(w.findAll('button').some(b => b.text().includes('로그아웃')), '사이드바가 접히지 않았다').toBe(false)
  const av = w.findAll('div.rounded-full.font-bold')
  return av.map(a => a.text()).join('')
}

describe('졸업생 담당 신원 표시 (이슈 #49)', () => {
  beforeAll(async () => {
    setActivePinia(createPinia())
    auth = useAuthStore()
    await Promise.all([
      import('../src/components/teacher/ApplicationTab.vue'),
      import('../src/components/teacher/ClassTab.vue'),
      import('../src/components/teacher/ResultsTab.vue'),
    ])
  })

  let rejections
  const onRejection = (e) => rejections.push(String(e?.reason?.message ?? e?.reason ?? e))
  beforeEach(() => {
    localStorage.clear()
    auth.logout()
    loginReply = () => ({ data: { token: 't', grade: 0, class_no: 0, teacher_name: '졸업생' } })
    rejections = []
    process.on('unhandledRejection', onRejection)
    vi.spyOn(console, 'warn').mockImplementation(() => {})
    Element.prototype.scrollIntoView = () => {}
  })
  afterEach(() => {
    if (dialogState.open) settleDialog(false)
    process.off('unhandledRejection', onRejection)
    vi.restoreAllMocks()
    delete Element.prototype.scrollIntoView
    expect(rejections, '미처리 거부').toEqual([])
  })

  // A. 판별력: 백엔드 0/0 응답 → 졸업생 표시. 일반 담임 분기로 떨어지면 라벨·버튼이 바뀐다.
  it('A. 졸업생 로그인 응답(0/0)이면 졸업생 담당으로 그린다', async () => {
    await auth.loginTeacher(0, 0, 'pw')
    expect(auth.teacherKind).toBe('graduate')
    const { w } = await mountTeacherView()
    const text = w.text()
    expect(text).toContain('졸업생 담당')
    expect(text).not.toMatch(HOMEROOM_LABEL)
    expect(text).not.toMatch(NULLISH)
    expect(text).not.toContain('비번변경')
    expect(w.find('[data-testid="identity-invalid"]').exists()).toBe(false)
    expect(await avatarText(w)).toBe('졸')
    w.unmount()
  })

  // D. 대조군 — 일반 담임은 그대로 일반 담임이다(관문이 정상 계정을 막지 않는다).
  it('D. 담임(3/1) 은 일반 담임으로 그린다', async () => {
    setIdentity({ grade: 3, classNo: 1 })
    const { w } = await mountTeacherView()
    expect(w.text()).toContain('3학년 1반 담임')
    expect(w.text()).toContain('비번변경')
    expect(await avatarText(w)).toBe('담')
    w.unmount()
  })

  // E. 인공 상태: 복원 검증이 반쪽 저장소를 지우므로 저장소로는 만들 수 없어 직접 대입한다.
  //    현실 경로(401 뒤 role 까지 null — J)와 다르다. 판별력: teacherKind 가 null 을
  //    일반 담임으로 떨어뜨리거나, 화면이 auth.grade === 0 으로 되돌아가면 일반 담임 문자열·
  //    '담' 아바타·비번변경 버튼이 나온다.
  it('E. role=teacher 인데 학년·반이 null 이면 안내만 보이고 일반 담임으로 떨어지지 않는다', async () => {
    setIdentity({ grade: 3, classNo: 1 })
    auth.grade = null
    auth.classNo = null
    expect(auth.teacherKind).toBe('invalid')
    const { w } = await mountTeacherView()
    const text = w.text()
    expect(w.find('[data-testid="identity-invalid"]').exists()).toBe(true)
    expect(text).not.toMatch(HOMEROOM_LABEL)
    expect(text).not.toMatch(NULLISH)
    expect(text).not.toContain('비번변경')
    expect(text).not.toContain('학급 현황')
    expect(await avatarText(w)).toBe('')
    w.unmount()
  })

  // F. 판별력: 붙잡은 핸들러를 실제로 부른다. teacherName 정리가 빠지면 이름이 남는다.
  it('F. 401 인터셉터는 인증 상태 전부와 저장소를 지우고 /login 으로 보낸다', async () => {
    await auth.loginTeacher(0, 0, 'pw')
    const stub = routerStub()
    setRouter(stub)
    await fire401()
    expect([auth.token, auth.role, auth.grade, auth.classNo, auth.teacherName]).toEqual([null, null, null, null, null])
    for (const k of ['pcm_token', 'pcm_role', 'pcm_grade', 'pcm_class_no', 'pcm_teacher_name']) {
      expect(localStorage.getItem(k), k).toBeNull()
    }
    expect(stub.push).toHaveBeenCalledWith('/login')
  })

  // J. 이동이 끝나지 않는 401 구간 — push 를 stub 해 TeacherView 가 마운트된 채로 둔다.
  //    판별력: 관문이 'invalid' 만 막으면(401 뒤 teacherKind 는 null) 탭과 일반 담임
  //    문자열이 그대로 그려진다. 이슈가 본 "null학년 null반 담임" 이 이 구간이다.
  it.each([
    ['졸업생 0/0', 0, 0],
    ['담임 3/1', 3, 1],
  ])('J. %s — 401 뒤 이동이 끝나기 전에도 일반 담임 문자열을 그리지 않는다', async (_n, g, c) => {
    setIdentity({ grade: g, classNo: c })
    const { w } = await mountTeacherView()
    setRouter(routerStub())  // 이동은 일어나지 않는다
    await fire401()
    const text = w.text()
    expect(w.find('[data-testid="identity-invalid"]').exists()).toBe(true)
    expect(text).not.toMatch(HOMEROOM_LABEL)
    expect(text).not.toMatch(NULLISH)
    expect(text).not.toContain('비번변경')
    expect(await avatarText(w)).toBe('')
    w.unmount()
  })

  // G. 판별력의 소재는 "확인 창이 뜨지 않았다"와 "경로가 /login" 이다. 이동이 끝나면
  //    TeacherView 가 내려가므로 화면 문자열 단언은 자명해서 넣지 않는다.
  //    onBeforeRouteLeave 가 등록되도록 실제 라우터의 router-view 안에 마운트한다.
  it('G. 지원 폼 입력 중 401 이 와도 확인 창에 막히지 않고 /login 으로 간다', async () => {
    setIdentity({ grade: 0, classNo: 0 })
    const ApplicationTab = (await import('../src/components/teacher/ApplicationTab.vue')).default
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [
        { path: '/teacher', component: ApplicationTab },
        { path: '/login', component: { render: () => h('p', '로그인 화면') } },
      ],
    })
    router.push('/teacher')
    await router.isReady()
    setRouter(router)
    const w = mount({ render: () => h(RouterView) }, { global: { plugins: [router] } })
    await settle()

    // 학생 → [+ 새 지원 추가] → 대학 → 모집단위 : form.trackId 가 생겨 dirty
    await w.findAll('.cursor-pointer').find(d => d.text().includes('학생01')).trigger('click')
    await settle()
    // 졸업생 학생 목록은 "학번 이름" 이다 — "번 이름" 이 아니다.
    expect(w.text()).toContain('2020001 학생01')
    expect(w.text()).not.toMatch(/번 학생01/)
    await w.findAll('button').find(b => b.text().includes('새 지원 추가')).trigger('click')
    await settle()
    await w.findAll('select')[0].setValue('1')
    await settle()
    await w.findAll('select')[1].setValue('10')
    await settle()

    // 대조: 인증이 살아 있으면 leave 가드가 확인 창을 띄운다(가드가 dirty 를 실제로 본다).
    const nav = router.push('/login')
    await settle()
    expect(dialogState.open, '대조: dirty 폼에서 이동하면 확인 창이 떠야 한다').toBe(true)
    settleDialog(false)
    await nav
    await settle()
    expect(router.currentRoute.value.path).toBe('/teacher')

    await fire401()
    expect(dialogState.open, '401 강제 로그아웃에 확인 창이 떴다').toBe(false)
    expect(router.currentRoute.value.path).toBe('/login')
    w.unmount()
  })

  // K. 로그아웃 버튼은 인증을 지우기 **전에** 저장하지 않은 입력을 확인한다.
  //    판별력: 순서를 되돌리면(인증 먼저 지움) leave 가드가 확인 없이 통과해 창이 뜨지 않고,
  //    취소해도 인증이 지워진 채 남는다.
  it('K. 폼 입력 중 [로그아웃] — 확인 창, 취소하면 인증·화면 유지, 나가기면 로그아웃', async () => {
    setIdentity({ grade: 0, classNo: 0 })
    const TeacherView = (await import('../src/views/TeacherView.vue')).default
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [
        { path: '/teacher', component: TeacherView },
        { path: '/login', component: { render: () => h('p', '로그인 화면') } },
      ],
    })
    router.push('/teacher')
    await router.isReady()
    setRouter(router)
    const w = mount({ render: () => h(RouterView) }, { global: { plugins: [router] } })
    await settle()
    await w.findAll('button, a, div').find(e => e.text().trim() === '지원자 등록').trigger('click')
    await settle()
    await w.findAll('.cursor-pointer').find(d => d.text().includes('학생01')).trigger('click')
    await settle()
    await w.findAll('button').find(b => b.text().includes('새 지원 추가')).trigger('click')
    await settle()
    await w.findAll('select')[0].setValue('1')
    await settle()
    await w.findAll('select')[1].setValue('10')
    await settle()

    const logoutBtn = () => w.findAll('button').find(b => b.text().includes('로그아웃'))
    await logoutBtn().trigger('click')
    await settle()
    expect(dialogState.open, '입력 중 로그아웃인데 확인 창이 없다').toBe(true)
    settleDialog(false)
    await settle()
    expect(auth.teacherKind).toBe('graduate')
    expect(router.currentRoute.value.path).toBe('/teacher')
    expect(w.text()).toContain('졸업생 담당')

    await logoutBtn().trigger('click')
    await settle()
    expect(dialogState.open).toBe(true)
    settleDialog(true)
    await settle()
    expect(dialogState.open, '확인을 마친 로그아웃에 확인 창이 또 떴다').toBe(false)
    expect(auth.role).toBeNull()
    expect(router.currentRoute.value.path).toBe('/login')
    w.unmount()
  })

  // H. 로그인 실패 401 도 같은 인터셉터를 지난다 — logout() 으로 모은 뒤에도 오류가 보인다.
  it('H. 로그인 화면의 비밀번호 오류(401)는 그대로 표시된다', async () => {
    loginReply = () => ({ status: 401, data: '비밀번호가 틀렸습니다' })
    setRouter(routerStub())
    const LoginView = (await import('../src/views/LoginView.vue')).default
    const w = mount(LoginView, {
      global: { stubs: { RouterLink: true }, mocks: { $router: routerStub() } },
    })
    await settle()
    await w.findAll('select')[0].setValue('3')
    await settle()
    await w.findAll('select')[1].setValue('1')
    await w.find('input[type="password"]').setValue('wrong')
    await w.find('form').trigger('submit')
    await settle()
    expect(w.text()).toContain('비밀번호가 틀렸습니다')
    expect(auth.role).toBeNull()
    w.unmount()
  })

  // I. 판별력: `Number(getItem) || ''` 로 되돌리면 저장된 0 이 '' 가 되어 졸업생 선택이 풀리고,
  //    비밀번호 라벨의 "(관리자 비밀번호)" 가 사라진다.
  it('I. 저장된 학년 선택 0(졸업생)이 로그인 화면에서 살아남는다', async () => {
    localStorage.setItem('login_teacher_grade', '0')
    localStorage.setItem('login_teacher_class', '0')
    const LoginView = (await import('../src/views/LoginView.vue')).default
    const w = mount(LoginView, {
      global: { stubs: { RouterLink: true }, mocks: { $router: routerStub() } },
    })
    await settle()
    expect(w.text()).toContain('(관리자 비밀번호)')
    expect(w.findAll('select')).toHaveLength(1)  // 반 선택이 없다
    w.unmount()
  })
})
