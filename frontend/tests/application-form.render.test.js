// @vitest-environment jsdom
/**
 * 담임 [지원 등록] 폼을 실제로 채워 본다 — 대학 → 모집단위 → 학과명 → 값 입력 → 저장.
 *
 * 지금까지 이 폼은 `canSaveApplication` 순수 함수와 "폼이 열리는가"(smoke-render)까지만
 * 검사됐다. 전형요소 입력칸은 한 번도 그려지지 않았고, 서버로 보내는 본문을 바꾸는
 * 변이는 전부 통과했다(2026-10-06 프론트 감사 커버리지 1위).
 *
 * 판별력의 소재(각 테스트 주석에 적는다): axios 를 이 파일에서 막고 응답을 테스트마다
 * 바꿔, 실패 경로와 늦은 응답을 직접 만든다.
 *
 * 아래 "저장·수정·취소 뒤" 묶음은 지원 상세 모달의 [수정]·[지원 취소] 와, 저장 뒤 목록
 * 재조회가 실패하는 경로를 누른다(2026-10-07 감사 B-1 담임 진입점).
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { settle } from './settle.js'
import { mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { dialogState, settleDialog } from '../src/components/common/dialog.js'

const OPEN_ROUND = { id: 3, status: 'OPEN', opened_at: '2026-03-21T00:00:00Z',
                     closed_at: null, finalized_at: null, needs_recalc: false }
const STUDENT = { id: 1, name: '학생01', student_code: '2026001',
                  grade: 3, class_no: 1, seq_no: 1, is_enrolled: true }
const AREA_CTX = { area_id: 1, area_name: '요소1', calc_type: 'NUMERIC', match_mode: 'UPPER',
                   lookup_scope: 'SIMPLE', multi_value: 0, teacher_editable: 1,
                   max_score: 10, current_values: [], table: [] }

/** 이미 등록된 지원 — 상세 모달·수정 경로에서 쓴다. */
const EXISTING = { student_id: 1, track_id: 10, round_id: 3, univ_id: 1, univ_name: '가대학',
                   track_name: '가모집단위', department_name: '컴퓨터공학과' }

/** 테스트마다 갈아 끼우는 응답. 함수는 (url, body) 를 받아 값 또는 Promise 를 돌려준다. */
let routes
/** POST 로 나간 요청 기록 — 저장 본문을 단언한다. */
let posts
/** DELETE 로 나간 URL 기록 */
let deletes

function defaultRoutes() {
  return {
    tracks:  () => [{ id: 10, track_name: '가모집단위' }, { id: 20, track_name: '나모집단위' }],
    context: () => [AREA_CTX],
    preview: () => ({ score: 5, matched_keys: [], warning: null, error: null }),
    save:    () => ({}),
    apps:    () => [],
  }
}

vi.mock('axios', () => {
  const ok = (data) => Promise.resolve({ data, headers: {} })
  const get = (url = '') => {
    const u = String(url)
    if (/rounds\/current/.test(u))                 return ok(OPEN_ROUND)
    if (/teacher\/students/.test(u))               return ok([STUDENT])
    if (/teacher\/universities\/\d+\/tracks/.test(u)) return Promise.resolve(routes.tracks(u)).then(ok)
    if (/teacher\/universities/.test(u))           return ok([{ id: 1, univ_name: '가대학' }])
    if (/teacher\/results/.test(u))                return ok({ rounds: [], results: [] })
    if (/teacher\/applications/.test(u))           return Promise.resolve(routes.apps(u)).then(ok)
    if (/confirm/.test(u))                         return ok({ confirmed: false, confirmed_at: null })
    if (/area-context/.test(u))                    return Promise.resolve(routes.context(u)).then(ok)
    return ok([])
  }
  const post = (url = '', body) => {
    const u = String(url)
    posts.push({ url: u, body })
    if (/area-score-preview/.test(u)) return Promise.resolve(routes.preview(u, body)).then(ok)
    if (/teacher\/applications/.test(u)) return Promise.resolve(routes.save(u, body)).then(ok)
    return ok({})
  }
  const del = (url = '') => {
    deletes.push(String(url))
    return ok({})
  }
  const axios = {
    get, post, put: () => ok({}), patch: () => ok({}), delete: del,
    interceptors: { request: { use: () => {} }, response: { use: () => {} } },
  }
  return { default: axios, ...axios }
})

const global = {
  stubs: { RouterLink: true, RouterView: true },
  mocks: {
    $route: { path: '/', params: {}, query: {} },
    $router: { push: () => {}, replace: () => {} },
  },
}

const reject = (msg) => () => Promise.reject(Object.assign(new Error(msg), { response: { data: msg } }))
const wait = (ms) => new Promise(r => setTimeout(r, ms))

/** 마운트하고 학생을 고른다. */
async function openStudent() {
  const mod = await import('../src/components/teacher/ApplicationTab.vue')
  const wrapper = mount(mod.default, { global })
  await settle()
  const row = wrapper.findAll('.cursor-pointer').find(d => d.text().includes('학생01'))
  expect(row, '학생 행이 없다').toBeTruthy()
  await row.trigger('click')
  await settle()
  return wrapper
}

/** 학생을 고르고 [+ 새 지원 추가] 로 폼을 연다. */
async function openForm() {
  const wrapper = await openStudent()
  const add = wrapper.findAll('button').find(b => b.text().includes('새 지원 추가'))
  expect(add, '[+ 새 지원 추가] 가 없다').toBeTruthy()
  await add.trigger('click')
  await settle()
  return wrapper
}

const selects = (w) => w.findAll('select')

async function pickUniv(w) {
  await selects(w)[0].setValue('1')
  await settle()
}

async function pickTrack(w, id = '10') {
  await selects(w)[1].setValue(id)
  await settle()
}

/** 전형요소 숫자 입력칸에 값을 넣고 미리보기 지연(400ms)을 지나 보낸다. */
async function typeValue(w, value) {
  const input = w.find('input[type="number"]')
  expect(input.exists(), '전형요소 입력칸이 그려지지 않았다').toBe(true)
  await input.setValue(value)
  await wait(450)
  await settle()
}

describe('담임 지원 등록 폼 — 실제로 채운다', () => {
  let rejections
  const onRejection = (e) => rejections.push(String(e?.reason?.message ?? e?.reason ?? e))

  beforeEach(() => {
    setActivePinia(createPinia())
    localStorage.clear()
    routes = defaultRoutes()
    posts = []
    deletes = []
    rejections = []
    process.on('unhandledRejection', onRejection)
    vi.spyOn(console, 'warn').mockImplementation(() => {})
    // jsdom 에는 scrollIntoView 가 없다. 저장 성공 경로가 폼을 닫은 뒤 부르므로, 없으면
    // TypeError 가 미처리 오류로 샌다(실제 브라우저에는 있다).
    Element.prototype.scrollIntoView = () => {}
  })
  // 미처리 거부는 아래 묶음처럼 여기서 한 번에 본다 — 테스트 안의 단언은 일부에만 있었다.
  afterEach(() => {
    if (dialogState.open) settleDialog(false)
    process.off('unhandledRejection', onRejection)
    vi.restoreAllMocks()
    delete Element.prototype.scrollIntoView
    expect(rejections, '미처리 거부').toEqual([])
  })

  // 판별력: 본문 필드 하나라도 빠지거나 값이 바뀌면 toEqual 이 깨진다.
  it('저장하면 고른 학생·모집단위·라운드·학과명·입력값이 그대로 서버로 간다', async () => {
    const w = await openForm()
    await pickUniv(w)
    await pickTrack(w)
    await w.find('input[type="text"][placeholder="예: 컴퓨터공학과"]').setValue('컴퓨터공학과')
    await typeValue(w, '3.5')

    const save = w.findAll('button').find(b => b.text() === '저장')
    expect(save.attributes('disabled'), '모든 칸을 채웠는데 저장이 잠겨 있다').toBeUndefined()
    await save.trigger('click')
    await settle()

    const sent = posts.filter(p => /teacher\/applications$/.test(p.url))
    expect(sent).toHaveLength(1)
    expect(sent[0].body).toEqual({
      student_id: 1,
      track_id: 10,
      round_id: 3,
      department_name: '컴퓨터공학과',
      base_data_entries: [{ area_id: 1, values: ['3.5'] }],
    })
    expect(rejections).toEqual([])
    w.unmount()
  })

  // 판별력: onUnivChange 의 catch 를 지우면 오류 문구가 없고 미처리 거부가 남는다.
  it('모집단위 목록을 못 불러오면 빈 목록으로 두지 않고 알린다', async () => {
    routes.tracks = reject('서버 오류 X')
    const w = await openForm()
    await pickUniv(w)

    expect(w.text()).toContain('모집단위를 불러오지 못했습니다')
    expect(w.text()).toContain('서버 오류 X')
    expect(rejections).toEqual([])
    w.unmount()
  })

  // 판별력: onTrackChange 의 catch 를 지우면 오류 문구가 없다.
  it('전형요소 정보를 못 불러오면 알린다', async () => {
    routes.context = reject('컨텍스트 오류 Y')
    const w = await openForm()
    await pickUniv(w)
    await pickTrack(w)

    expect(w.text()).toContain('전형요소 정보를 불러오지 못했습니다')
    expect(w.text()).toContain('컨텍스트 오류 Y')
    expect(rejections).toEqual([])
    w.unmount()
  })

  // 판별력: 경고 span 이 예전처럼 "⚠" 만 그리면 문구 단언이 깨진다.
  it('미리보기 경고는 기호만이 아니라 문구로 보인다', async () => {
    routes.preview = () => ({
      score: 10, matched_keys: [], error: null,
      warning: '계산된 점수가 만점을 초과하여 만점으로 처리됩니다',
    })
    const w = await openForm()
    await pickUniv(w)
    await pickTrack(w)
    await typeValue(w, '99')

    expect(w.text()).toContain('계산된 점수가 만점을 초과하여 만점으로 처리됩니다')
    w.unmount()
  })

  // 판별력: fetchScorePreview 의 seq 비교를 지우면, 모집단위를 바꾼 뒤 도착한
  // 옛 모집단위의 미리보기(77점)가 새 모집단위 칸에 그려진다.
  it('모집단위를 바꾼 뒤 늦게 온 이전 모집단위의 미리보기는 버린다', async () => {
    let release
    const late = new Promise(r => { release = r })
    routes.preview = (_u, body) =>
      body.track_id === 10 ? late.then(() => ({ score: 77, matched_keys: [], warning: null, error: null }))
                           : { score: 5, matched_keys: [], warning: null, error: null }
    const w = await openForm()
    await pickUniv(w)
    await pickTrack(w, '10')
    await typeValue(w, '3')          // 모집단위 10 의 미리보기 요청 — 아직 응답 없음
    await pickTrack(w, '20')         // 모집단위를 바꾼다
    release()                        // 이제서야 10 의 응답이 도착한다
    await settle()

    expect(w.text()).not.toContain('예상 77')
    w.unmount()
  })

  // 판별력: fetchScorePreview 의 전형요소별 순번 비교를 지우면, 체크박스를 빠르게 두 개
  // 눌렀을 때 늦게 온 첫 요청(값 1개, 6점)의 응답이 두 번째(값 2개, 10점) 응답을 덮어쓴다.
  // 2026-10-07 브라우저 실측에서 실제로 "예상 6점" 이 남았다(백엔드는 10점을 돌려줬다).
  it('같은 전형요소의 미리보기가 겹치면 마지막 요청의 응답만 남는다', async () => {
    const CAT = { area_id: 2, area_name: '수상실적', calc_type: 'CATEGORY', match_mode: null,
                  lookup_scope: 'SIMPLE', multi_value: 1, teacher_editable: 1, max_score: 10,
                  current_values: [],
                  table: [{ key: '교내 대상', score: 6 }, { key: '교외 우수상', score: 7 }] }
    routes.context = () => [CAT]
    let releaseFirst
    const firstHeld = new Promise(r => { releaseFirst = r })
    routes.preview = (_u, body) => body.values.length === 1
      ? firstHeld.then(() => ({ score: 6, matched_keys: ['교내 대상'], warning: null, error: null }))
      : { score: 10, matched_keys: body.values, warning: '계산된 점수가 만점을 초과하여 만점으로 처리됩니다', error: null }

    const w = await openForm()
    await pickUniv(w)
    await pickTrack(w)
    const boxes = w.findAll('input[type="checkbox"]')
    expect(boxes.length).toBe(2)
    await boxes[0].setValue(true)   // 요청 1 (값 1개) — 붙잡힌다
    await boxes[1].setValue(true)   // 요청 2 (값 2개) — 바로 온다
    await settle()
    expect(w.text()).toContain('예상 10점')

    releaseFirst()                  // 요청 1 의 응답이 이제서야 도착한다
    await settle()
    const t = w.text()
    expect(t, '늦게 온 이전 요청의 응답이 예상 점수를 덮어썼다').toContain('예상 10점')
    expect(t).not.toContain('예상 6점')
    w.unmount()
  })

  // 판별력: clearPreview 가 순번을 올리지 않으면, 체크를 모두 푼 뒤 도착한 이전 응답이
  // 지운 미리보기를 되살린다.
  it('선택을 모두 풀면 늦게 온 응답이 미리보기를 되살리지 못한다', async () => {
    const CAT = { area_id: 2, area_name: '수상실적', calc_type: 'CATEGORY', match_mode: null,
                  lookup_scope: 'SIMPLE', multi_value: 1, teacher_editable: 1, max_score: 10,
                  current_values: [],
                  table: [{ key: '교내 대상', score: 6 }, { key: '교외 우수상', score: 7 }] }
    routes.context = () => [CAT]
    let release
    const held = new Promise(r => { release = r })
    routes.preview = () => held.then(() => ({ score: 6, matched_keys: ['교내 대상'], warning: null, error: null }))

    const w = await openForm()
    await pickUniv(w)
    await pickTrack(w)
    const box = w.findAll('input[type="checkbox"]')[0]
    await box.setValue(true)    // 요청 — 붙잡힌다
    await box.setValue(false)   // 선택을 모두 푼다 → 미리보기 지움
    await settle()
    release()
    await settle()
    expect(w.text(), '지운 미리보기가 늦은 응답으로 되살아났다').not.toContain('예상 6점')
    w.unmount()
  })
})

// ════════════════════════════════════════════════════════════════
describe('담임 지원 — 저장·수정·취소 뒤', () => {
  let rejections
  const onRejection = (e) => rejections.push(String(e?.reason?.message ?? e?.reason ?? e))

  beforeEach(() => {
    setActivePinia(createPinia())
    localStorage.clear()
    routes = defaultRoutes()
    routes.apps = () => [EXISTING]
    posts = []
    deletes = []
    rejections = []
    process.on('unhandledRejection', onRejection)
    vi.spyOn(console, 'warn').mockImplementation(() => {})
    Element.prototype.scrollIntoView = () => {}   // 위 묶음과 같은 이유
  })
  afterEach(() => {
    if (dialogState.open) settleDialog(false)
    process.off('unhandledRejection', onRejection)
    vi.restoreAllMocks()
    delete Element.prototype.scrollIntoView
    expect(rejections, '미처리 거부').toEqual([])
  })

  const btn = (w, label) => {
    const b = w.findAll('button').find(x => x.text().trim() === label)
    expect(b, `[${label}] 버튼이 없다`).toBeTruthy()
    return b
  }
  async function click(el) { await el.trigger('click'); await settle() }

  /** 학생을 고르고 등록된 지원을 눌러 상세 모달을 연다. */
  async function openDetail() {
    const w = await openStudent()
    const app = w.findAll('.cursor-pointer').find(d => d.text().includes('가대학 — 가모집단위'))
    expect(app, '등록된 지원 행이 없다').toBeTruthy()
    await click(app)
    expect(w.text(), '상세 모달이 열리지 않았다').toContain('학생01 — 가대학 가모집단위')
    return w
  }

  // (a) 판별력: onModalDeleted 의 catch 를 지우면 알림이 없고 미처리 거부가 남는다(afterEach).
  // 제목·본문을 정확히 보므로 "오류" 류 알림으로 바꿔도 걸린다.
  it('[지원 취소] 뒤 목록 재조회만 실패하면 "삭제는 완료됐지만" 으로 알린다', async () => {
    const w = await openDetail()
    routes.apps = reject('목록 조회 실패 L')
    await click(btn(w, '지원 취소'))
    expect(dialogState.open).toBe(true)
    expect(dialogState.kind).toBe('confirm')
    settleDialog(true)
    await settle()

    expect(deletes).toEqual(['/api/teacher/applications/1/10/3'])
    expect(dialogState.open, '재조회 실패를 알리지 않았다').toBe(true)
    expect(dialogState.kind).toBe('alert')
    expect(dialogState.title).toBe('목록 새로고침 실패')
    expect(dialogState.message).toContain('삭제는 완료됐지만')
    expect(dialogState.message).toContain('목록 조회 실패 L')
    settleDialog(true)
    await settle()
    w.unmount()
  })

  // (b) 판별력: onModalEdit 의 catch 를 지우면 문구가 없고 미처리 거부가 남는다. 저장 잠금은
  // canSave 가 맡는다 — 실패 뒤 모집단위나 전형요소가 빈 채로 저장이 열리면 걸린다.
  // 전형요소 실패는 모달을 연 **뒤에** 건다 — 모달도 같은 area-context 를 부르기 때문이다.
  it.each([
    ['모집단위 목록', () => { routes.tracks = reject('트랙 오류 T') }, '트랙 오류 T'],
    ['전형요소 정보', () => { routes.context = reject('컨텍스트 오류 K') }, '컨텍스트 오류 K'],
  ])('[수정] — %s 를 못 불러오면 알리고 저장을 잠근다', async (_name, breakIt, msg) => {
    const w = await openDetail()
    breakIt()
    await click(btn(w, '수정'))

    const t = w.text()
    expect(t).toContain('지원 수정 — 가대학 가모집단위')
    expect(t).toContain('수정할 지원 정보를 불러오지 못했습니다')
    expect(t).toContain(msg)
    expect(btn(w, '저장').attributes('disabled'), '불러오지 못했는데 저장이 열려 있다').toBeDefined()
    w.unmount()
  })

  // (c) 판별력: onModalEdit 가 기존 모집단위를 고르지 않거나(form.trackId) 저장 본문에
  // prev_track_id 를 빠뜨리면 걸린다. prev_track_id 가 없으면 서버는 새 지원으로 받는다.
  it('[수정] 성공 경로 — 기존 모집단위·값이 채워지고, 저장 본문에 prev_track_id 가 간다', async () => {
    routes.context = () => [{ ...AREA_CTX, current_values: ['3.5'] }]
    const w = await openDetail()
    await click(btn(w, '수정'))

    expect(selects(w)[1].element.value, '기존 모집단위가 선택되지 않았다').toBe('10')
    expect(w.find('input[type="text"][placeholder="예: 컴퓨터공학과"]').element.value).toBe('컴퓨터공학과')
    expect(w.find('input[type="number"]').element.value).toBe('3.5')
    const save = btn(w, '저장')
    expect(save.attributes('disabled'), '값이 다 있는데 저장이 잠겨 있다').toBeUndefined()
    await click(save)

    const sent = posts.filter(p => /teacher\/applications$/.test(p.url))
    expect(sent).toHaveLength(1)
    expect(sent[0].body).toEqual({
      student_id: 1,
      track_id: 10,
      round_id: 3,
      department_name: '컴퓨터공학과',
      base_data_entries: [{ area_id: 1, values: ['3.5'] }],
      prev_track_id: 10,
    })
    w.unmount()
  })

  // (d) 판별력: saveApplication 의 저장과 재조회를 한 try 로 되돌리면(수정 전 모양) 알림이
  // 없고, 재조회 오류가 saveError 에 떠 폼이 열린 채 남는다 — 다시 누르면 모집단위를 바꾼
  // 수정이었을 때 409 "이미 해당 모집단위에 지원되어 있습니다" 를 만난다.
  it('저장 뒤 목록 재조회만 실패하면 저장 실패로 보이지 않고 폼을 닫는다', async () => {
    routes.apps = () => []
    const w = await openForm()
    await pickUniv(w)
    await pickTrack(w)
    await w.find('input[type="text"][placeholder="예: 컴퓨터공학과"]').setValue('컴퓨터공학과')
    await typeValue(w, '3.5')
    routes.apps = reject('목록 조회 실패 S')
    await click(btn(w, '저장'))

    expect(posts.filter(p => /teacher\/applications$/.test(p.url))).toHaveLength(1)
    expect(dialogState.open, '재조회 실패를 알리지 않았다').toBe(true)
    expect(dialogState.title).toBe('목록 새로고침 실패')
    expect(dialogState.message).toContain('저장은 완료됐지만')
    settleDialog(true)
    await settle()
    expect(w.findAll('button').some(b => b.text().trim() === '저장'), '저장됐는데 폼이 열려 있다').toBe(false)
    expect(w.text(), '재조회 오류가 저장 실패 자리에 떴다').not.toContain('목록 조회 실패 S')
    w.unmount()
  })
})
