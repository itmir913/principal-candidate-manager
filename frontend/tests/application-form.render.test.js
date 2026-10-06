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
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { settle } from './settle.js'
import { mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'

const OPEN_ROUND = { id: 3, status: 'OPEN', opened_at: '2026-03-21T00:00:00Z',
                     closed_at: null, finalized_at: null, needs_recalc: false }
const STUDENT = { id: 1, name: '학생01', student_code: '2026001',
                  grade: 3, class_no: 1, seq_no: 1, is_enrolled: true }
const AREA_CTX = { area_id: 1, area_name: '요소1', calc_type: 'NUMERIC', match_mode: 'UPPER',
                   lookup_scope: 'SIMPLE', multi_value: 0, teacher_editable: 1,
                   max_score: 10, current_values: [], table: [] }

/** 테스트마다 갈아 끼우는 응답. 함수는 (url, body) 를 받아 값 또는 Promise 를 돌려준다. */
let routes
/** POST 로 나간 요청 기록 — 저장 본문을 단언한다. */
let posts

function defaultRoutes() {
  return {
    tracks:  () => [{ id: 10, track_name: '가모집단위' }, { id: 20, track_name: '나모집단위' }],
    context: () => [AREA_CTX],
    preview: () => ({ score: 5, matched_keys: [], warning: null, error: null }),
    save:    () => ({}),
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
    if (/teacher\/applications/.test(u))           return ok([])
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
  const axios = {
    get, post, put: () => ok({}), patch: () => ok({}), delete: () => ok({}),
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

/** 학생을 고르고 [+ 새 지원 추가] 로 폼을 연다. */
async function openForm() {
  const mod = await import('../src/components/teacher/ApplicationTab.vue')
  const wrapper = mount(mod.default, { global })
  await settle()
  const row = wrapper.findAll('.cursor-pointer').find(d => d.text().includes('학생01'))
  expect(row, '학생 행이 없다').toBeTruthy()
  await row.trigger('click')
  await settle()
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
    rejections = []
    process.on('unhandledRejection', onRejection)
    vi.spyOn(console, 'warn').mockImplementation(() => {})
  })
  afterEach(() => {
    process.off('unhandledRejection', onRejection)
    vi.restoreAllMocks()
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
})
