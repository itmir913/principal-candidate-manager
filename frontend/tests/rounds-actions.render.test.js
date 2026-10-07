// @vitest-environment jsdom
/**
 * 관리자 [라운드] 화면의 **조작 버튼**을 실제로 누른다 — 추천 확정·취소, 미선발 처리·해제,
 * 자동 추천, 재계산, 학과명, 다시 열기·마감·종료. 그전까지 이 버튼들은 "그려지는가"
 * (smoke-render)와 "되돌리기 어려운 것은 2단계 확인을 거치는가"(destructive-confirm)까지만
 * 검사됐다. 어떤 요청이 나가는지, 성공 뒤 화면이 바뀌는지, 실패 때 무엇이 보이는지는 검사 밖이었다.
 *
 * P3 감사(2026-10-07)의 초안에서 시작했다. "수정 전 코드에서 실패했다" 로 표시한 테스트는
 * 수정 전 코드에 돌려 실제로 실패하는 것을 확인했다 — 픽스처가 그 경로에 닿는다는 뜻이다.
 *
 * 판별력의 소재: axios 를 **상태를 가진 가짜 서버**로 바꾼다. 쓰기 요청이 오면 서버 쪽 행을
 * 바꾸고, 조회는 그 행의 **복사본**을 준다 — 그래서 화면이 다시 조회하지 않으면 옛 값이
 * 그대로 남아 단언에 걸린다. 실패·지연은 `S.hooks` 로 요청마다 끼워 넣는다.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { settle } from './settle.js'
import { mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { dialogState, settleDialog } from '../src/components/common/dialog.js'

// ── 가짜 서버 ────────────────────────────────────────────────────
const ROUND = (id, status) => ({
  id, status, opened_at: '2026-03-02T00:00:00Z',
  closed_at: status === 'OPEN' ? null : '2026-03-10T00:00:00Z',
  finalized_at: status === 'FINALIZED' ? '2026-03-20T00:00:00Z' : null,
  needs_recalc: false,
})
// 순위를 서로 다르게 둔다 — 동점 표식(#fef3c7)이 끼면 행 색 단언이 흐려진다.
const ROW = (sid, name, over = {}) => ({
  student_id: sid, track_id: 1, round_id: 1, name, student_code: `C${sid}`,
  grade: 3, class_no: 1, seq_no: sid, is_enrolled: true,
  univ_name: '가대학', track_name: '가모집단위', department_name: '학과',
  total_score: 5 - sid / 10, score_detail: { 1: 5 }, ranking: sid, track_rank: sid,
  recommended: false, excluded: false, excluded_reason: null, abandoned: false,
  ...over,
})
// buildTrackQuotaMap 은 univ_id·track_id 를 읽는다. 이게 없으면 [가대학 전체 자동 추천] 이 안 그려진다.
const QUOTA = {
  all_round_ids: [1],
  univs: [{
    id: 1, univ_id: 1, univ_name: '가대학', total_quota: 5, total_used: 0, prioritize_enrolled: 0,
    tracks: [{ id: 1, track_id: 1, track_name: '가모집단위', unit_quota: 3, unit_used: 0,
               prioritize_enrolled: 0, by_round: [] }],
  }],
}

/** 서버 상태. 테스트마다 beforeEach 에서 갈아 끼운다. */
const S = { rounds: [], rows: [], calls: [], hooks: [], auto: null }

function ok(data) { return Promise.resolve({ data, headers: {}, status: 200 }) }
function noContent() { return Promise.resolve({ data: '', headers: {}, status: 204 }) }
/** axios 가 실패 응답에 만드는 모양. data 는 실제 axios 가 JSON 파싱을 마친 뒤의 값이다
 *  (백엔드는 422 JSON 을 text/plain 으로 보내지만 axios 의 forcedJSONParsing 이 객체로 바꾼다). */
function err(status, data) {
  return Promise.reject(Object.assign(new Error(`Request failed with status code ${status}`),
    { response: { status, data, headers: {} } }))
}
const rowsOf = (rid) => S.rows.filter(r => r.round_id === Number(rid)).map(r => ({ ...r }))
const findRow = (sid, tid, rid) =>
  S.rows.find(r => r.student_id === +sid && r.track_id === +tid && r.round_id === +rid)

function defaultRoute(method, u, body, config) {
  let m
  if (method === 'GET') {
    if (/\/api\/rounds$/.test(u)) return ok(S.rounds.map(r => ({ ...r })))
    if ((m = u.match(/\/api\/rounds\/(\d+)\/results$/))) return ok(rowsOf(m[1]))
    if (/\/api\/rounds\/\d+\/confirmation-status$/.test(u)) return ok({ classes: [] })
    if (/\/api\/applications$/.test(u)) {
      return ok(rowsOf(config.params.round_id).map(r => ({ ...r, univ_id: 1 })))
    }
    if (/quota-stats$/.test(u)) return ok(QUOTA)
    if (/\/api\/areas$/.test(u)) return ok([])
    return ok([])
  }
  if ((m = u.match(/\/api\/results\/(\d+)\/(\d+)\/(\d+)\/(recommend|unrecommend)$/))) {
    findRow(m[1], m[2], m[3]).recommended = m[4] === 'recommend'
    return noContent()
  }
  if ((m = u.match(/\/api\/applications\/(\d+)\/(\d+)\/(\d+)\/exclude$/))) {
    const r = findRow(m[1], m[2], m[3])
    r.excluded = method === 'PUT'
    r.excluded_reason = method === 'PUT' ? body.reason : null
    return noContent()
  }
  if ((m = u.match(/\/api\/applications\/(\d+)\/(\d+)\/(\d+)\/department$/))) {
    findRow(m[1], m[2], m[3]).department_name = body.department_name
    return noContent()
  }
  if ((m = u.match(/\/api\/rounds\/(\d+)\/calculate$/))) {
    S.rounds.find(r => r.id === +m[1]).needs_recalc = false
    return ok({ calculated: rowsOf(m[1]).length })
  }
  if ((m = u.match(/\/api\/rounds\/(\d+)\/auto-recommend(\/univ\/\d+)?$/))) {
    S.auto.apply?.()
    return ok(S.auto.response)
  }
  if ((m = u.match(/\/api\/rounds\/(\d+)\/(close|reopen|finalize)$/))) {
    const rnd = S.rounds.find(r => r.id === +m[1])
    if (m[2] === 'close') { rnd.status = 'CLOSED'; rnd.closed_at = '2026-03-10T00:00:00Z' }
    if (m[2] === 'finalize') { rnd.status = 'FINALIZED'; rnd.finalized_at = '2026-03-20T00:00:00Z' }
    if (m[2] === 'reopen') {
      // rounds.rs::reopen_round 와 같은 초기화 — 추천·순위·미선발(사유 포함)
      rnd.status = 'OPEN'; rnd.closed_at = null
      for (const r of S.rows.filter(r => r.round_id === rnd.id)) {
        r.recommended = false; r.ranking = null; r.excluded = false; r.excluded_reason = null
      }
    }
    return m[2] === 'close' ? ok({ calculated: rowsOf(rnd.id).length }) : noContent()
  }
  return ok({})
}

function handle(method, url, body, config = {}) {
  const u = String(url)
  S.calls.push({ method, url: u, body })
  for (const h of S.hooks) {
    if (h.method === method && h.re.test(u)) {
      const r = h.fn({ url: u, body, config })
      if (r !== undefined) return r
    }
  }
  return defaultRoute(method, u, body, config)
}

vi.mock('axios', () => {
  const axios = {
    get:    (url, config) => handle('GET', url, undefined, config),
    post:   (url, body, config) => handle('POST', url, body, config),
    put:    (url, body, config) => handle('PUT', url, body, config),
    patch:  (url, body, config) => handle('PATCH', url, body, config),
    delete: (url, config) => handle('DELETE', url, undefined, config),
    interceptors: { request: { use: () => {} }, response: { use: () => {} } },
  }
  return { default: axios, ...axios }
})

// ── 화면 조작 도우미 ─────────────────────────────────────────────
let refreshRoundSpy
const mountOpts = () => ({
  global: {
    // 미선발·미결정 모달은 Teleport 안에 있다 — 제자리에 그려야 찾을 수 있다
    stubs: { RouterLink: true, RouterView: true, teleport: true },
    mocks: { $route: { path: '/', params: {}, query: {} }, $router: { push: () => {}, replace: () => {} } },
    provide: { refreshRound: refreshRoundSpy },
  },
})

const writes = () => S.calls.filter(c => c.method !== 'GET').map(c => `${c.method} ${c.url}`)
const lastIndex = (pred) => { for (let i = S.calls.length - 1; i >= 0; i -= 1) if (pred(S.calls[i])) return i; return -1 }

function btnIn(scope, label) {
  const b = scope.findAll('button').find(x => x.text().trim() === label)
  expect(b, `[${label}] 버튼이 없다 — 픽스처나 상태를 확인하라`).toBeTruthy()
  return b
}
function rowOf(w, name) {
  const tr = w.findAll('tr').find(t => t.text().includes(name))
  expect(tr, `'${name}' 행이 없다`).toBeTruthy()
  return tr
}
async function click(el) { await el.trigger('click'); await settle() }
async function clickRound(w, id) {
  const card = w.findAll('.cursor-pointer').find(d => d.text().includes(`${id}차 라운드`))
  expect(card, `${id}차 라운드 카드가 없다`).toBeTruthy()
  await click(card)
}
/** 확인창이 떠 있는지 보고 답한다. */
async function answer(yes, { level } = {}) {
  expect(dialogState.open, '확인창이 뜨지 않았다').toBe(true)
  expect(dialogState.kind).toBe('confirm')
  if (level) expect(dialogState.level).toBe(level)
  settleDialog(yes)
  await settle()
}

/** 마운트 때는 성공하고, `on` 을 켠 뒤부터 실패하는 조회. 조작 뒤 재조회만 실패시킬 때 쓴다. */
function failLater(re, msg) {
  const sw = { on: false }
  S.hooks.push({ method: 'GET', re, fn: () => (sw.on ? err(500, msg) : undefined) })
  return sw
}

/**
 * 조작은 성공하고 재조회만 실패했을 때의 알림을 확인하고 닫는다. 제목을 **정확히** 본다 —
 * 조작과 재조회를 한 try 로 되돌리면 같은 자리에 '오류' 가 떠 여기서 걸린다(감사 B-1).
 */
async function expectReloadNotice(doneLabel, detail) {
  expect(dialogState.open, '재조회 실패를 알리지 않았다').toBe(true)
  expect(dialogState.kind).toBe('alert')
  expect(dialogState.title, '조작은 성공했는데 조작 실패처럼 알렸다').toBe(`${doneLabel} 완료 — 목록 새로고침 실패`)
  expect(dialogState.message).toContain('처리는 완료됐지만')
  expect(dialogState.message).toContain(detail)
  settleDialog(true)
  await settle()
}

async function openRoundTab({ roundId = 1, tab = '결과' } = {}) {
  const mod = await import('../src/components/admin/RoundsTab.vue')
  const w = mount(mod.default, mountOpts())
  await settle()
  await clickRound(w, roundId)
  if (tab) await click(btnIn(w, tab))
  return w
}

let rejections
const onRejection = (e) => rejections.push(String(e?.reason?.message ?? e?.reason ?? e))

beforeEach(() => {
  setActivePinia(createPinia())
  localStorage.clear()
  S.rounds = [ROUND(1, 'CLOSED')]
  S.rows = [ROW(1, '김갑돌'), ROW(2, '이을순')]
  S.calls = []
  S.hooks = []
  S.auto = null
  refreshRoundSpy = vi.fn()
  rejections = []
  process.on('unhandledRejection', onRejection)
  vi.spyOn(console, 'warn').mockImplementation(() => {})
})
afterEach(() => {
  if (dialogState.open) settleDialog(false)
  process.off('unhandledRejection', onRejection)
  vi.restoreAllMocks()
  expect(rejections, '미처리 거부').toEqual([])
})

// ════════════════════════════════════════════════════════════════
describe('결과 행 조작 — 추천 확정·취소, 미선발 처리·해제', () => {
  // 판별력: handleRecommend 에서 loadResults 를 빼면 '추천 확정됨' 이 안 보인다(서버는 바뀌었지만
  // 화면은 복사본을 쥐고 있다). URL 의 sid 를 2 로 고른 것은 행을 엉뚱하게 넘기는 변이를 잡기 위해서다.
  it('[추천 확정] — 그 행의 recommend 를 보내고, 다시 받아 "추천 확정됨" 으로 바꾼다', async () => {
    const w = await openRoundTab()
    await click(btnIn(rowOf(w, '이을순'), '추천 확정'))

    expect(writes()).toEqual(['PUT /api/results/2/1/1/recommend'])
    expect(dialogState.open, '성공했는데 알림이 떴다').toBe(false)
    expect(rowOf(w, '이을순').text()).toContain('추천 확정됨')
    expect(rowOf(w, '김갑돌').text()).not.toContain('추천 확정됨')
    w.unmount()
  })

  // 판별력: catch 를 지우면 미처리 거부(afterEach)와 알림 없음으로, finally 를 지우면 알림을 닫은 뒤에도
  // 버튼이 disabled 로 남아 걸린다.
  it('[추천 확정] 409 — 서버 문구를 "오류" 로 보이고, 닫으면 다시 누를 수 있다', async () => {
    const MSG = '모집단위 정원(3명)이 이미 찼습니다 (현재 추천 확정 3명)'
    S.hooks.push({ method: 'PUT', re: /\/recommend$/, fn: () => err(409, MSG) })
    const w = await openRoundTab()
    await click(btnIn(rowOf(w, '김갑돌'), '추천 확정'))

    expect(dialogState.open).toBe(true)
    expect(dialogState.kind).toBe('alert')
    expect(dialogState.title).toBe('오류')
    expect(dialogState.message).toBe(MSG)
    settleDialog(true)
    await settle()
    const b = btnIn(rowOf(w, '김갑돌'), '추천 확정')
    expect(b.attributes('disabled'), '실패 뒤 버튼이 잠긴 채 남았다').toBeUndefined()
    w.unmount()
  })

  // 판별력이 둘로 나뉜다 — ① `:disabled="resultActing"` 를 지우면 disabled 단언에서,
  // ② `if (resultActing.value) return` 을 지우면 disabled 를 강제로 풀고 누른 뒤의 PUT 횟수에서 걸린다.
  // 하나만 지워도 다른 하나가 중복을 막으므로 두 단언을 따로 둔다.
  it('[추천 확정] 진행 중 — 같은 행·다른 행을 다시 눌러도 요청은 하나다', async () => {
    let release
    const gate = new Promise(r => { release = r })
    S.hooks.push({ method: 'PUT', re: /\/recommend$/, fn: ({ url }) => gate.then(() => defaultRoute('PUT', url)) })
    const w = await openRoundTab()
    const first = btnIn(rowOf(w, '김갑돌'), '추천 확정')
    await click(first)

    const other = btnIn(rowOf(w, '이을순'), '추천 확정')
    expect(first.attributes('disabled'), '진행 중인데 버튼이 열려 있다').toBeDefined()
    expect(other.attributes('disabled'), '진행 중인데 다른 행 버튼이 열려 있다').toBeDefined()

    // 가드만 따로 본다 — disabled 를 DOM 에서 풀고 누른다
    for (const b of [first, other]) {
      b.element.disabled = false
      b.element.click()
    }
    await settle()
    expect(S.calls.filter(c => c.method === 'PUT').length, '진행 중에 두 번째 요청이 나갔다').toBe(1)

    release()
    await settle()
    expect(rowOf(w, '김갑돌').text()).toContain('추천 확정됨')
    w.unmount()
  })

  // ── U-1 (등급 B) — 수정 전 코드에서 실패했다 ─────────────────
  // 조작은 성공했고 다시 받기만 실패했다. 수정 전에는 같은 catch 가 '오류' 를 띄워 조작이
  // 실패한 것처럼 보였다 → 관리자가 다시 누르면 이미 추천된 행에 recommend 가 또 204(감사 로그
  // 중복)이거나, 정원 마지막 자리였으면 409 "정원이 찼습니다".
  // 판별력: 조작과 재조회를 한 try 로 되돌리면 알림 제목이 '오류' 가 되어 expectReloadNotice 에서 걸린다.
  it('[추천 확정] 성공 뒤 결과 재조회만 실패하면 "조작 실패" 로 알리지 않는다', async () => {
    const fail = failLater(/\/results$/, '결과 조회 실패 Q')
    const w = await openRoundTab()
    fail.on = true
    await click(btnIn(rowOf(w, '김갑돌'), '추천 확정'))

    expect(writes()).toEqual(['PUT /api/results/1/1/1/recommend'])
    await expectReloadNotice('추천 확정', '결과 조회 실패 Q')
    // 표가 낡았다는 사실은 알림을 닫은 뒤에도 남아야 한다(오류 상자)
    expect(w.text()).toContain('결과 조회 실패 Q')
    w.unmount()
  })

  // 판별력: [추천 취소]·[미선발 처리] 의 `:disabled="resultActing"` 를 지우면 걸린다(감사 C-4).
  // 위 [추천 확정] 진행 중 테스트는 [추천 확정] 버튼만 본다.
  it('[추천 확정] 진행 중 — [추천 취소]·[미선발 처리] 도 잠긴다', async () => {
    let release
    const gate = new Promise(r => { release = r })
    S.rows = [ROW(1, '김갑돌', { recommended: true }), ROW(2, '이을순')]
    S.hooks.push({ method: 'PUT', re: /\/recommend$/, fn: ({ url }) => gate.then(() => defaultRoute('PUT', url)) })
    const w = await openRoundTab()
    await click(btnIn(rowOf(w, '이을순'), '추천 확정'))

    expect(btnIn(rowOf(w, '김갑돌'), '추천 취소').attributes('disabled'), '진행 중인데 [추천 취소] 가 열려 있다').toBeDefined()
    expect(btnIn(rowOf(w, '김갑돌'), '미선발 처리').attributes('disabled'), '진행 중인데 [미선발 처리] 가 열려 있다').toBeDefined()
    expect(btnIn(rowOf(w, '이을순'), '미선발 처리').attributes('disabled')).toBeDefined()

    release()
    await settle()
    expect(btnIn(rowOf(w, '김갑돌'), '추천 취소').attributes('disabled'), '끝난 뒤에도 잠겨 있다').toBeUndefined()
    expect(btnIn(rowOf(w, '김갑돌'), '미선발 처리').attributes('disabled')).toBeUndefined()
    w.unmount()
  })

  // 판별력: 확인 결과를 보지 않으면(취소에도 진행) 첫 단언에서, URL 을 recommend 로 바꾸면 둘째에서 걸린다.
  it('[추천 취소] — 확인창(warn)을 거치고, 취소하면 아무것도 보내지 않는다', async () => {
    S.rows = [ROW(1, '김갑돌', { recommended: true }), ROW(2, '이을순')]
    const w = await openRoundTab()

    await click(btnIn(rowOf(w, '김갑돌'), '추천 취소'))
    await answer(false, { level: 'warn' })
    expect(writes(), '취소했는데 요청이 나갔다').toEqual([])

    await click(btnIn(rowOf(w, '김갑돌'), '추천 취소'))
    await answer(true, { level: 'warn' })
    expect(writes()).toEqual(['PUT /api/results/1/1/1/unrecommend'])
    expect(rowOf(w, '김갑돌').text()).not.toContain('추천 확정됨')
    btnIn(rowOf(w, '김갑돌'), '추천 확정')
    w.unmount()
  })

  // 판별력: trim 을 빼면 본문이 '  서류 미비  ' 가 되어, 빈 사유 disabled 를 지우면 첫 단언에서,
  // 성공 뒤 모달을 닫지 않으면 '미선발 확정' 버튼이 남아 걸린다.
  it('[미선발 처리] — 사유를 받아 그 행의 exclude 를 보내고, 표에 미선발로 보인다', async () => {
    const w = await openRoundTab()
    await click(btnIn(rowOf(w, '이을순'), '미선발 처리'))
    expect(w.text()).toContain('이을순 학생을 이번 라운드에서 미선발 처리합니다')

    const confirmBtn = btnIn(w, '미선발 확정')
    expect(confirmBtn.attributes('disabled'), '사유 없이 확정할 수 있다').toBeDefined()
    await w.find('input[placeholder="미선발 사유를 입력하세요"]').setValue('  서류 미비  ')
    await click(btnIn(w, '미선발 확정'))

    expect(S.calls.filter(c => c.method === 'PUT').map(c => [c.url, c.body]))
      .toEqual([['/api/applications/2/1/1/exclude', { reason: '서류 미비' }]])
    expect(w.findAll('button').some(b => b.text().trim() === '미선발 확정'), '성공했는데 모달이 남았다').toBe(false)
    btnIn(rowOf(w, '이을순'), '미선발 해제')
    w.unmount()
  })

  // 판별력: 실패해도 모달을 닫으면(showExcludeModal=false 를 try 앞으로) 입력한 사유가 사라져 걸린다.
  it('[미선발 처리] 409 — 알림을 띄우고 모달과 입력한 사유는 남긴다', async () => {
    const MSG = '이미 추천 확정된 지원은 미선발 처리할 수 없습니다. 추천을 먼저 취소한 후 미선발 처리하세요.'
    S.hooks.push({ method: 'PUT', re: /\/exclude$/, fn: () => err(409, MSG) })
    const w = await openRoundTab()
    await click(btnIn(rowOf(w, '김갑돌'), '미선발 처리'))
    await w.find('input[placeholder="미선발 사유를 입력하세요"]').setValue('정원 외')
    await click(btnIn(w, '미선발 확정'))

    expect(dialogState.title).toBe('오류')
    expect(dialogState.message).toBe(MSG)
    settleDialog(true)
    await settle()
    expect(w.find('input[placeholder="미선발 사유를 입력하세요"]').element.value).toBe('정원 외')
    w.unmount()
  })

  // 판별력: confirmExclude 의 조작과 재조회를 한 try 로 되돌리면 제목이 '오류' 로 걸린다.
  // 처리는 됐으므로 모달은 닫혀야 한다 — 남기면 다시 눌러 409 "이미 미선발" 을 만난다.
  it('[미선발 처리] 성공 뒤 결과 재조회만 실패하면 모달을 닫고 "완료" 로 알린다', async () => {
    const fail = failLater(/\/results$/, '결과 조회 실패 R')
    const w = await openRoundTab()
    await click(btnIn(rowOf(w, '이을순'), '미선발 처리'))
    await w.find('input[placeholder="미선발 사유를 입력하세요"]').setValue('서류 미비')
    fail.on = true
    await click(btnIn(w, '미선발 확정'))

    expect(writes()).toEqual(['PUT /api/applications/2/1/1/exclude'])
    await expectReloadNotice('미선발 처리', '결과 조회 실패 R')
    expect(w.findAll('button').some(b => b.text().trim() === '미선발 확정'), '처리됐는데 모달이 남았다').toBe(false)
    w.unmount()
  })

  // 판별력: saveDept 의 조작과 재조회를 한 try 로 되돌리면 제목이 '오류' 로 걸린다.
  // 지원 목록과 결과를 함께 다시 받는 경로의 대표다(포기 처리도 같은 모양이다).
  it('[학과명] 저장 뒤 지원 목록 재조회만 실패하면 편집기를 닫고 "완료" 로 알린다', async () => {
    const fail = failLater(/\/api\/applications$/, '지원 조회 실패 D')
    const w = await openRoundTab()
    const deptBtn = rowOf(w, '김갑돌').find('button[title="학과명 수정 (점수에 영향 없음)"]')
    expect(deptBtn.exists(), '학과명 버튼이 없다').toBe(true)
    await click(deptBtn)
    await rowOf(w, '김갑돌').find('input[placeholder="학과명"]').setValue('컴퓨터공학과')
    fail.on = true
    await click(btnIn(rowOf(w, '김갑돌'), '저장'))

    expect(S.calls.filter(c => c.method === 'PUT').map(c => [c.url, c.body]))
      .toEqual([['/api/applications/1/1/1/department', { department_name: '컴퓨터공학과' }]])
    await expectReloadNotice('학과명 수정', '지원 조회 실패 D')
    expect(w.find('input[placeholder="학과명"]').exists(), '저장됐는데 편집기가 남았다').toBe(false)
    // 결과 재조회는 성공했다 — 새 학과명이 결과 표에 보인다
    expect(rowOf(w, '김갑돌').text()).toContain('컴퓨터공학과')
    w.unmount()
  })

  // 판별력: 메서드를 PUT 으로 바꾸거나 확인 결과를 무시하면 걸린다.
  it('[미선발 해제] — 확인 뒤 DELETE 를 보내고, [미선발 처리] 버튼이 돌아온다', async () => {
    S.rows = [ROW(1, '김갑돌', { excluded: true, excluded_reason: '서류 미비' }), ROW(2, '이을순')]
    const w = await openRoundTab()
    await click(btnIn(rowOf(w, '김갑돌'), '미선발 해제'))
    await answer(false, { level: 'warn' })
    expect(writes()).toEqual([])

    await click(btnIn(rowOf(w, '김갑돌'), '미선발 해제'))
    await answer(true, { level: 'warn' })
    expect(writes()).toEqual(['DELETE /api/applications/1/1/1/exclude'])
    btnIn(rowOf(w, '김갑돌'), '미선발 처리')
    w.unmount()
  })
})

// ════════════════════════════════════════════════════════════════
describe('자동 추천', () => {
  // 사유 문장은 백엔드가 쓴다(scoring.rs::run_auto_recommend). 화면은 **그대로** 보여야 한다 —
  // 해석·요약·번역을 하면 여기서 걸린다. 문장은 실제 형식을 채운 예다.
  const UNIV_REASON = '대학 전체 3위 동순위 — 동순위 지원자 가운데 누구를 먼저 추천하느냐에 따라 같은 모집단위의 다음 지원자(대학 순위가 같거나 더 좋음)가 들어올 수 있어 잔여 1석에 3명이 경합합니다 (대학 정원 5명, 이번 실행 포함 확정 4명, 잔여 1석 / 관리자 선택 필요)'
  const TRACK_REASON = '모집단위 2위 동점 — 잔여 1석에 2명 경합. 대학 정원이 찼습니다(이번 실행 포함 확정 5명 / 정원 5명) — 현재 상태에서는 이 동점에서 추천할 수 없습니다'
  const RESPONSE = {
    confirmed: [{ track_id: 1, univ_name: '가대학', track_name: '가모집단위', count: 2 }],
    manual: [
      { track_id: null, univ_name: '가대학', track_name: null, reason: UNIV_REASON },
      { track_id: 1, univ_name: '가대학', track_name: '가모집단위', reason: TRACK_REASON },
    ],
  }
  const applyAll = () => { for (const r of S.rows) r.recommended = true }

  // 판별력: URL·스코프 라벨·재조회·사유 원문 표시 중 하나를 바꾸면 해당 단언에서 걸린다.
  // 대학명과 모집단위명 사이 공백도 본다 — 공백을 `<template>` 첫 자식에 두면 Vue 공백 정리가
  // 지워 '가대학가모집단위' 로 붙는다. 수정 전 코드에서 이 단언으로 실패했다(감사 C-6).
  it('[자동 추천 확정] — 확인 뒤 라운드 전체를 보내고, 결과·수동 확인 목록을 그대로 보인다', async () => {
    S.auto = { response: RESPONSE, apply: applyAll }
    const w = await openRoundTab()
    await click(btnIn(w, '자동 추천 확정'))
    expect(dialogState.message).toContain('모든 대학')
    await answer(false)
    expect(writes(), '취소했는데 실행됐다').toEqual([])

    await click(btnIn(w, '자동 추천 확정'))
    await answer(true)
    expect(writes()).toEqual(['POST /api/rounds/1/auto-recommend'])
    const t = w.text()
    expect(t).toContain('처리 범위: 전체 대학')
    expect(t).toContain('1개 모집단위 2명 추천 확정')
    expect(t).toContain(`가대학 (대학 전체) — ${UNIV_REASON}`)
    expect(t).toContain(`가대학 가모집단위 — ${TRACK_REASON}`)
    // 다시 받았는가 — POST 뒤에 결과 GET 이 있어야 한다
    const post = lastIndex(c => c.method === 'POST')
    expect(lastIndex(c => c.method === 'GET' && /\/results$/.test(c.url))).toBeGreaterThan(post)
    expect(rowOf(w, '김갑돌').text()).toContain('추천 확정됨')
    w.unmount()
  })

  it('[가대학 전체 자동 추천] — 그 대학 id 로 보낸다', async () => {
    S.auto = { response: { confirmed: [], manual: [] } }
    const w = await openRoundTab()
    await click(btnIn(w, '가대학 전체 자동 추천'))
    await answer(true)
    expect(writes()).toEqual(['POST /api/rounds/1/auto-recommend/univ/1'])
    expect(w.text()).toContain('처리 범위: 가대학')
    expect(w.text()).toContain('자동 확정 대상 없음')
    w.unmount()
  })

  // 판별력: 두 버튼 중 하나의 `:disabled="autoRecommendActing"` 를 지우면 걸린다.
  it('진행 중에는 두 자동 추천 버튼이 모두 잠긴다', async () => {
    let release
    const gate = new Promise(r => { release = r })
    S.auto = { response: { confirmed: [], manual: [] } }
    S.hooks.push({ method: 'POST', re: /auto-recommend/, fn: ({ url }) => gate.then(() => defaultRoute('POST', url)) })
    const w = await openRoundTab()
    await click(btnIn(w, '자동 추천 확정'))
    await answer(true)
    expect(btnIn(w, '자동 추천 확정').attributes('disabled')).toBeDefined()
    expect(btnIn(w, '가대학 전체 자동 추천').attributes('disabled')).toBeDefined()
    release()
    await settle()
    expect(btnIn(w, '자동 추천 확정').attributes('disabled')).toBeUndefined()
    w.unmount()
  })

  // 판별력: runAutoRecommend 의 조작과 재조회를 한 try 로 되돌리면 제목이 '오류' 로 걸린다.
  // 결과 패널은 응답으로 그리므로 재조회 실패와 무관하게 보여야 한다.
  it('[자동 추천 확정] 성공 뒤 결과 재조회만 실패하면 패널을 보이고 "완료" 로 알린다', async () => {
    S.auto = { response: RESPONSE, apply: applyAll }
    const fail = failLater(/\/results$/, '결과 조회 실패 A')
    const w = await openRoundTab()
    fail.on = true
    await click(btnIn(w, '자동 추천 확정'))
    await answer(true)

    expect(writes()).toEqual(['POST /api/rounds/1/auto-recommend'])
    await expectReloadNotice('자동 추천 확정', '결과 조회 실패 A')
    expect(w.text()).toContain('처리 범위: 전체 대학')
    expect(w.text()).toContain('1개 모집단위 2명 추천 확정')
    w.unmount()
  })

  // ── 등급 C — 수정 전 코드에서 실패했다 ───────────────────────
  // 응답이 오기 전에 다른 라운드를 고르면, runAutoRecommend 가 새 라운드 화면에 옛 라운드의
  // 결과 패널을 띄웠다(selectRound 가 비운 뒤에 덮어쓴다). handleCalculate 는 같은 상황을
  // `selected.value?.id !== roundId` 로 막고 있었다.
  // 판별력: 라운드 비교를 빼면 '처리 범위' 가 2차 라운드 화면에 남는다.
  it('응답 전에 다른 라운드를 고르면 그 라운드에 자동 추천 결과가 뜨지 않는다', async () => {
    S.rounds = [ROUND(1, 'CLOSED'), ROUND(2, 'FINALIZED')]
    S.rows = [ROW(1, '김갑돌'), ROW(2, '이을순', { round_id: 2, recommended: true })]
    let release
    const gate = new Promise(r => { release = r })
    S.auto = { response: RESPONSE }
    S.hooks.push({ method: 'POST', re: /auto-recommend/, fn: ({ url }) => gate.then(() => defaultRoute('POST', url)) })
    const w = await openRoundTab({ roundId: 1 })
    await click(btnIn(w, '자동 추천 확정'))
    await answer(true)

    await clickRound(w, 2)      // 결과 탭은 그대로다(view 는 라운드를 바꿔도 유지된다)
    release()
    await settle()
    expect(w.text(), '1차 라운드의 자동 추천 결과가 2차 라운드 화면에 떴다').not.toContain('처리 범위')
    w.unmount()
  })
})

// ════════════════════════════════════════════════════════════════
describe('라운드 상태 전이', () => {
  // ── U-3 (소유자 결정) — 수정 전 코드에서 실패했다 ────────────
  // 재개는 추천만이 아니라 미선발(사유 포함)도 지운다(rounds.rs 의 reopen_round).
  // 판별력: 문구를 옛 "확정한 추천 표시가 모두 초기화" 로 되돌리면 toBe 에서 걸린다.
  it('[다시 열기] 확인창이 추천과 미선발이 모두 초기화된다고 말한다', async () => {
    const w = await openRoundTab({ tab: null })
    await click(btnIn(w, '다시 열기'))
    expect(dialogState.message).toBe('라운드를 다시 여시겠습니까?\n지금까지 한 추천·미선발 결정이 모두 초기화됩니다.')
    await answer(false, { level: 'warn' })
    expect(writes()).toEqual([])
    w.unmount()
  })

  // ── U-2 (등급 B) — 수정 전 코드에서 실패했다 ─────────────────
  // 재개는 서버에서 추천·순위·미선발을 지운다. 수정 전 화면은 다시 받지 않아, 방금
  // "초기화됩니다" 에 동의한 관리자 앞에 '추천 확정됨'·'미선발'·자동 추천 패널이 그대로 남았다.
  // 판별력: 재개 성공 뒤 selectRound 를 빼면 '추천 확정됨' 이 남는다. 결과만 다시 받게 바꾸면
  // 표는 맞아도 자동 추천 패널('처리 범위')이 남아 걸린다.
  it('[다시 열기] 성공 뒤 결과 표·자동 추천 패널이 초기화 상태로 바뀐다', async () => {
    S.rows = [ROW(1, '김갑돌'), ROW(2, '이을순', { excluded: true, excluded_reason: '서류 미비' })]
    S.auto = { response: { confirmed: [{ track_id: 1, univ_name: '가대학', track_name: '가모집단위', count: 1 }], manual: [] },
               apply: () => { S.rows[0].recommended = true } }
    const w = await openRoundTab()
    await click(btnIn(w, '자동 추천 확정'))
    await answer(true)
    expect(rowOf(w, '김갑돌').text()).toContain('추천 확정됨')
    expect(rowOf(w, '이을순').text()).toContain('미선발')
    expect(w.text()).toContain('처리 범위')

    await click(btnIn(w, '다시 열기'))
    await answer(true, { level: 'warn' })
    expect(writes()).toContain('PUT /api/rounds/1/reopen')
    const t = w.text()
    expect(t).toContain('진행중')
    expect(rowOf(w, '김갑돌').text(), '재개 뒤에도 추천 표시가 남았다').not.toContain('추천 확정됨')
    expect(rowOf(w, '이을순').text(), '재개 뒤에도 미선발 표시가 남았다').not.toContain('미선발')
    expect(t, '재개 뒤에도 자동 추천 결과 패널이 남았다').not.toContain('처리 범위')
    expect(refreshRoundSpy).toHaveBeenCalled()
    w.unmount()
  })

  // ── 등급 B — 수정 전 코드에서 실패했다 ───────────────────────
  // 수정 전 마감 성공 경로는 라운드 목록만 다시 받았다. [지원 현황] 표의 '추천' 칸은 app.recommended
  // 를 보는데, 지원 목록은 라운드를 고를 때 한 번 받은 것이다(추천 조작은 결과만 다시 받는다).
  // 그래서 이 화면에서 추천한 학생이 마감 직후 [지원 현황] 에서 '미선발' 로 보이고 [포기하기]
  // 버튼도 없다. 판별력: 마감 성공 뒤 지원 목록 재조회를 빼면 '미선발' 이 보인다.
  it('[마감하기] 성공 뒤 [지원 현황] 이 이 화면에서 한 추천을 반영한다', async () => {
    const w = await openRoundTab()
    await click(btnIn(rowOf(w, '김갑돌'), '추천 확정'))
    await click(btnIn(rowOf(w, '이을순'), '미선발 처리'))
    await w.find('input[placeholder="미선발 사유를 입력하세요"]').setValue('정원 외')
    await click(btnIn(w, '미선발 확정'))

    await click(btnIn(w, '마감하기'))
    expect(dialogState.level).toBe('danger')
    settleDialog(true)          // DialogHost 가 없으므로 2단계 확인은 바로 통과시킨다
    await settle()
    expect(writes()).toContain('PUT /api/rounds/1/finalize')
    // '마감' 글자는 [마감하기] 버튼에도 있어 판별력이 없다 — 버튼이 사라졌는지로 본다
    expect(w.findAll('button').some(b => b.text().trim() === '마감하기'), '마감 뒤에도 CLOSED 화면이다').toBe(false)

    await click(btnIn(w, '지원 현황'))
    const row = rowOf(w, '김갑돌')
    expect(row.text(), '추천한 학생이 마감 직후 미선발로 보인다').toContain('추천 확정')
    expect(row.text()).not.toContain('미선발')
    btnIn(row, '포기하기')
    w.unmount()
  })

  // 판별력: undecided 분기를 지우면 JSON 이 문자열로 알림에 뜨거나("[object Object]") 모달이 안 뜬다.
  // 모달은 서버의 error 문장을 쓰지 않고 자기 안내문을 쓴다 — 그 문장이 화면에 보이면 걸린다.
  // (백엔드 문장의 용어는 tests/handler_rounds.rs 가 따로 본다.)
  const UNDECIDED_ERR = '추천 또는 미선발이 결정되지 않은 지원자가 있어 라운드를 마감할 수 없습니다'
  it('[마감하기] 422 미결정 — 명단 모달을 띄우고, 서버의 error 문장은 보이지 않는다', async () => {
    S.hooks.push({ method: 'PUT', re: /\/finalize$/, fn: () => err(422, {
      error: UNDECIDED_ERR,
      undecided: [{ student_code: 'C2', student_name: '이을순', grade: 3, class_no: 1,
                    univ_name: '가대학', track_name: '가모집단위' }],
    }) })
    const w = await openRoundTab()
    await click(btnIn(w, '마감하기'))
    settleDialog(true)
    await settle()

    expect(dialogState.open, '미결정은 알림이 아니라 모달로 보여야 한다').toBe(false)
    const t = w.text()
    expect(t).toContain('아래 지원자는 추천도 미선발도 결정되지 않았습니다')
    expect(t).toContain('총 1명')
    expect(t).toContain('3학년 1반')
    expect(t).toContain('이을순')
    expect(t, '서버의 error 문장이 모달에 섞였다').not.toContain(UNDECIDED_ERR)
    await click(btnIn(w, '닫기'))
    expect(w.text()).not.toContain('아래 지원자는 추천도 미선발도 결정되지 않았습니다')
    btnIn(w, '마감하기')        // 여전히 CLOSED 화면이다
    w.unmount()
  })

  // ── 등급 B — 수정 전 코드에서 실패했다 ───────────────────────
  // 졸업생은 students.grade/class_no 가 NULL 이다(migrations/v1/002-students.sql CHECK).
  // 수정 전 백엔드 UndecidedApplication 은 i64 라 sqlx-sqlite 가 NULL 을 0 으로 읽어 "0학년 0반"
  // 이 됐다. 수정 뒤 계약(grade: null)을 픽스처로 둔다 — 백엔드 쪽은 tests/handler_rounds.rs 가 본다.
  // 판별력: 모달의 학년/반 칸을 되돌리면 '졸업생' 이 없다.
  it('[마감하기] 422 미결정 명단의 졸업생은 학년/반 대신 졸업생으로 보인다', async () => {
    S.hooks.push({ method: 'PUT', re: /\/finalize$/, fn: () => err(422, {
      error: '…',
      undecided: [{ student_code: 'G9', student_name: '박졸업', grade: null, class_no: null,
                    univ_name: '가대학', track_name: '가모집단위' }],
    }) })
    const w = await openRoundTab()
    await click(btnIn(w, '마감하기'))
    settleDialog(true)
    await settle()
    const row = rowOf(w, '박졸업')
    expect(row.text()).toContain('졸업생')
    expect(row.text()).not.toMatch(/학년/)
    w.unmount()
  })

  // 판별력: finalizeErrMsg 의 줄 형식·필드 이름이 백엔드 TrackOverQuota/UnivOverQuota 와 어긋나면 걸린다.
  it('[마감하기] 422 정원 초과 — 위반 목록을 사람이 읽는 줄로 펼친다', async () => {
    S.hooks.push({ method: 'PUT', re: /\/finalize$/, fn: () => err(422, {
      error: '정원 초과로 라운드를 확정할 수 없습니다',
      track_violations: [{ track_name: '가모집단위', univ_name: '가대학', unit_quota: 1, total_recommended: 2 }],
      univ_violations: [{ univ_name: '가대학', total_quota: 1, total_recommended: 2 }],
    }) })
    const w = await openRoundTab()
    await click(btnIn(w, '마감하기'))
    settleDialog(true)
    await settle()
    expect(dialogState.title).toBe('마감할 수 없습니다')
    expect(dialogState.message).toBe([
      '정원 초과로 라운드를 확정할 수 없습니다',
      '- 가대학 가모집단위: 모집단위 정원 1명, 추천 확정 2명',
      '- 가대학 (대학 전체): 정원 1명, 추천 확정 2명',
    ].join('\n'))
    w.unmount()
  })

  // 판별력: close 실패를 삼키거나 상태를 낙관적으로 바꾸면 걸린다.
  it('[종료하기] 422 기초데이터 누락 — 서버 문장을 그대로 보이고 라운드는 진행중으로 남는다', async () => {
    const MSG = "기초데이터 누락으로 라운드를 종료할 수 없습니다:\n전형요소 '요소1': 가대학 가모집단위 지원자 김갑돌 (C1)"
    S.rounds = [ROUND(1, 'OPEN')]
    S.hooks.push({ method: 'PUT', re: /\/close$/, fn: () => err(422, MSG) })
    const w = await openRoundTab({ tab: null })
    await click(btnIn(w, '종료하기'))
    await answer(true, { level: 'warn' })
    expect(dialogState.title).toBe('오류')
    expect(dialogState.message).toBe(MSG)
    settleDialog(true)
    await settle()
    expect(w.text()).toContain('진행중')
    btnIn(w, '종료하기')
    w.unmount()
  })

  // ── U-1 의 종료 경로 (등급 B) — 수정 전 코드에서 실패했다 ─────
  // 수정 전에는 종료가 성공했는데 결과 재조회가 실패하면, 같은 try 라 '오류' 가 뜨고 사이드바
  // 갱신(refreshRound)이 건너뛰어졌다 — 사이드바는 계속 "진행 중" 라운드를 보였다.
  // 지금은 selectRound 가 실패를 상세 오류 상자에 적고 던지지 않는다(알림은 띄우지 않는다).
  // 판별력: 조작과 재조회를 한 try 로 되돌리고 재조회가 던지게 하면 '오류' 와 spy 단언에서 걸린다.
  it('[종료하기] 성공 뒤 결과 재조회가 실패해도 사이드바는 갱신되고 "오류" 로 알리지 않는다', async () => {
    S.rounds = [ROUND(1, 'OPEN')]
    const fail = failLater(/\/results$/, '결과 조회 실패 Q')
    const w = await openRoundTab({ tab: null })
    fail.on = true
    await click(btnIn(w, '종료하기'))
    await answer(true, { level: 'warn' })

    expect(writes()).toContain('PUT /api/rounds/1/close')
    // 상태 전이 뒤 재조회 실패는 화면 자리(상세 오류 상자)에 적을 뿐 알림을 띄우지 않는다
    expect(dialogState.open, '종료는 성공했는데 알림이 떴다').toBe(false)
    expect(refreshRoundSpy, '사이드바 라운드 상태를 갱신하지 않았다').toHaveBeenCalled()
    btnIn(w, '마감하기')        // CLOSED 화면으로 바뀌었다('종료' 글자는 [종료하기] 에도 있어 쓰지 않는다)
    expect(w.text(), '재조회 실패가 화면 어디에도 없다').toContain('결과 조회 실패 Q')
    w.unmount()
  })

  // ── 마감 감사 C-2 — 수정 전 코드에서 실패했다 ─────────────────
  // 전이가 성공했으면 서버 상태는 확정이다. 수정 전 reloadAfterTransition 은 라운드 목록 재조회가
  // 실패하면(rounds 가 빈다) updated 를 못 찾아 selectRound 를 건너뛰었다 — 상세 머리글·버튼이
  // 옛 상태(마감했는데 CLOSED 와 [마감하기])로 남고, [지원 현황] 의 추천 칸도 옛 지원 목록을
  // 봤다. 또 rounds 가 비어 hasOpenRound 가 false 가 되어 [+ 라운드 열기] 가 열렸다(서버는 409 로
  // 막지만 화면이 거짓이다).
  // 판별력: ① 목록을 못 받았을 때의 폴백(전이 뒤 상태로 selectRound)을 빼면 [마감하기] 가 남아
  // 첫 버튼 단언에서 걸린다. ② [+ 라운드 열기] 의 roundsLoadError 잠금을 빼면 disabled 단언에서 걸린다.
  it('[마감하기] 성공 뒤 라운드 목록 재조회가 실패해도 상세는 마감 상태이고 [+ 라운드 열기] 는 잠긴다', async () => {
    S.rows = [ROW(1, '김갑돌', { recommended: true }), ROW(2, '이을순', { excluded: true, excluded_reason: '정원 외' })]
    const fail = failLater(/\/api\/rounds$/, '라운드 목록 조회 실패 F')
    const w = await openRoundTab()
    fail.on = true
    await click(btnIn(w, '마감하기'))
    settleDialog(true)
    await settle()

    expect(writes()).toContain('PUT /api/rounds/1/finalize')
    expect(dialogState.open, '마감은 성공했는데 알림이 떴다').toBe(false)
    expect(w.findAll('button').some(b => b.text().trim() === '마감하기'), '목록을 못 받았다고 상세가 CLOSED 로 남았다').toBe(false)
    expect(w.text()).toContain('라운드 목록을 불러오지 못했습니다')
    expect(w.text()).toContain('라운드 목록 조회 실패 F')
    expect(btnIn(w, '+ 라운드 열기').attributes('disabled'), '라운드 목록을 모르는데 [+ 라운드 열기] 가 열렸다').toBeDefined()
    expect(refreshRoundSpy).toHaveBeenCalled()
    // 마감 시각은 서버 값이 와야 안다 — 목록이 없는 동안 지어내지 않는다
    expect(w.text()).not.toContain('최종 마감')

    await click(btnIn(w, '지원 현황'))
    const row = rowOf(w, '김갑돌')
    expect(row.text(), '마감 상태인데 [지원 현황] 의 추천 칸이 비었다').toContain('추천 확정')
    btnIn(row, '포기하기')
    expect(rowOf(w, '이을순').text()).toContain('미선발')
    w.unmount()
  })

  // 재개 — 서버는 추천·순위·미선발을 지우고 closed_at 을 NULL 로 되돌린다(rounds.rs 의 reopen_round).
  // needs_recalc 는 서버 판정식(rounds.rs 의 needs_recalc_expr)이 CLOSED 에서만 1 이라 OPEN 에서는 0 이다.
  // 목록을 못 받아도 상세는 진행중이어야 하고 결과 표의 옛 추천 표시·"입력 종료" 시각·"최신이 아닙니다"
  // 경고가 남지 않아야 한다.
  // 판별력: 폴백의 `closed_at: null` 을 빼면 "입력 종료" 단언에서, `needs_recalc: false` 를 빼면
  // "최신이 아닙니다" 단언에서 걸린다.
  it('[다시 열기] 성공 뒤 라운드 목록 재조회가 실패해도 상세는 진행중이고 결과 표가 초기화된다', async () => {
    S.rounds = [{ ...ROUND(1, 'CLOSED'), needs_recalc: true }]
    S.rows = [ROW(1, '김갑돌', { recommended: true }), ROW(2, '이을순')]
    const fail = failLater(/\/api\/rounds$/, '라운드 목록 조회 실패 R')
    const w = await openRoundTab()
    expect(rowOf(w, '김갑돌').text()).toContain('추천 확정됨')
    expect(w.text()).toContain('입력 종료')
    expect(w.text(), '픽스처가 재계산 필요 경고에 닿지 않는다').toContain('표시된 총점·순위가 최신이 아닙니다')
    fail.on = true
    await click(btnIn(w, '다시 열기'))
    await answer(true, { level: 'warn' })

    expect(writes()).toContain('PUT /api/rounds/1/reopen')
    expect(dialogState.open).toBe(false)
    btnIn(w, '종료하기')        // OPEN 화면이다
    expect(w.findAll('button').some(b => b.text().trim() === '다시 열기'), '재개했는데 CLOSED 화면이 남았다').toBe(false)
    expect(rowOf(w, '김갑돌').text(), '재개 뒤에도 추천 표시가 남았다').not.toContain('추천 확정됨')
    expect(w.text(), '재개했는데 입력 종료 시각이 남았다').not.toContain('입력 종료')
    expect(w.text(), '재개했는데 옛 needs_recalc 로 재계산 필요 경고가 남았다').not.toContain('표시된 총점·순위가 최신이 아닙니다')
    expect(w.text()).toContain('라운드 목록 조회 실패 R')
    expect(btnIn(w, '+ 라운드 열기').attributes('disabled')).toBeDefined()
    w.unmount()
  })

  it('[종료하기] 성공 뒤 라운드 목록 재조회가 실패해도 상세는 종료 상태다', async () => {
    S.rounds = [ROUND(1, 'OPEN')]
    const fail = failLater(/\/api\/rounds$/, '라운드 목록 조회 실패 C')
    const w = await openRoundTab({ tab: null })
    fail.on = true
    await click(btnIn(w, '종료하기'))
    await answer(true, { level: 'warn' })

    expect(writes()).toContain('PUT /api/rounds/1/close')
    expect(dialogState.open).toBe(false)
    btnIn(w, '마감하기')        // CLOSED 화면이다
    expect(w.findAll('button').some(b => b.text().trim() === '종료하기'), '종료했는데 OPEN 화면이 남았다').toBe(false)
    expect(w.text()).toContain('라운드 목록 조회 실패 C')
    expect(btnIn(w, '+ 라운드 열기').attributes('disabled')).toBeDefined()
    w.unmount()
  })

  // 첫 화면 로드부터 목록을 못 받으면 진행 중 라운드가 있는지 알 수 없다 — 같은 잠금이 적용된다.
  // 빈 상태 도움말("첫 라운드 열기 전 확인하세요")도 뜨지 않아야 한다 — rounds 가 빈 것은 오류이지
  // 라운드가 없다는 뜻이 아니다. 판별력: 잠금을 hasOpenRound 만으로 되돌리면 rounds 가 비어 버튼이
  // 열리고, 도움말의 `!roundsLoadError` 조건을 빼면 제목 단언에서 걸린다.
  it('첫 화면에서 라운드 목록을 못 받으면 [+ 라운드 열기] 는 잠기고 빈 상태 도움말은 뜨지 않는다', async () => {
    S.hooks.push({ method: 'GET', re: /\/api\/rounds$/, fn: () => err(500, '라운드 목록 조회 실패 M') })
    const mod = await import('../src/components/admin/RoundsTab.vue')
    const w = mount(mod.default, mountOpts())
    await settle()
    expect(w.text()).toContain('라운드 목록 조회 실패 M')
    expect(btnIn(w, '+ 라운드 열기').attributes('disabled')).toBeDefined()
    expect(w.text(), '목록 오류인데 빈 상태 도움말이 떴다').not.toContain('첫 라운드 열기 전 확인하세요')
    w.unmount()
  })

  // 짝: 진짜 빈 목록(오류 없음)에서는 도움말이 뜬다 — 위 조건을 `false` 로 넓히는 변이를 막는다.
  // 수정 전에도 통과하는 테스트다(이 묶음의 "수정 전 코드에서 실패했다" 표지와 다르다) — 변이 방어용.
  it('라운드가 정말 없으면 빈 상태 도움말이 뜬다', async () => {
    S.rounds = []
    const mod = await import('../src/components/admin/RoundsTab.vue')
    const w = mount(mod.default, mountOpts())
    await settle()
    expect(w.text()).toContain('첫 라운드 열기 전 확인하세요')
    expect(w.text()).toContain('라운드 없음')
    w.unmount()
  })
})

// ════════════════════════════════════════════════════════════════
describe('점수 재계산', () => {
  // 판별력: handleCalculate 의 조작과 재조회를 한 try 로 되돌리면(수정 전 모양) ① 알림 없이
  // calcMsg 가 실패 문구로 덮이고 ② selected 갱신이 건너뛰어져 [결과] 의 "최신이 아닙니다"
  // 경고가 남는다. 첫 단언(expectReloadNotice)에서 먼저 걸린다. 재조회 뒤 selected 갱신만
  // 빼는 변이는 마지막 단언이 따로 잡는다.
  it('성공 뒤 결과 재조회만 실패해도 성공 문구와 재계산 필요 해제가 남는다', async () => {
    S.rounds = [{ ...ROUND(1, 'CLOSED'), needs_recalc: true }]
    const fail = failLater(/\/results$/, '결과 조회 실패 C')
    const w = await openRoundTab({ tab: null })
    fail.on = true
    await click(btnIn(w, '점수 전체 재계산'))

    expect(writes()).toEqual(['POST /api/rounds/1/calculate'])
    await expectReloadNotice('점수 재계산', '결과 조회 실패 C')
    expect(w.text(), '재계산은 성공했는데 실패 문구로 덮였다').toContain('점수 재계산 완료: 2건')
    await click(btnIn(w, '결과'))
    expect(w.text(), '재계산 뒤에도 "최신이 아닙니다" 경고가 남았다').not.toContain('표시된 총점·순위가 최신이 아닙니다')
    w.unmount()
  })

  // ── 최종 감사 F3 C-1 ──────────────────────────────────────────
  // 재계산 성공 → 결과 재조회는 성공, **라운드 목록 재조회만** 실패. loadRounds 는 던지지 않고
  // rounds 를 비우므로 reloadAfter 는 알림을 띄우지 않고, handleCalculate 는 `fresh` 를 못 찾는다.
  // 그 경우 selected 를 그대로 두면 needs_recalc 가 옛 true 로 남아 "점수 재계산 완료" 와
  // "표시된 총점·순위가 최신이 아닙니다 … 차단됩니다" 가 한 화면에 같이 보였다(모순).
  // 판별력: handleCalculate 의 `?? { ...selected.value, needs_recalc: false }` 폴백을 지우면 마지막
  // 단언에서 걸린다. 픽스처가 경고에 닿는지는 재계산 전 단언이 따로 확인한다.
  it('성공 뒤 라운드 목록 재조회만 실패해도 "재계산 필요" 경고가 남지 않는다', async () => {
    S.rounds = [{ ...ROUND(1, 'CLOSED'), needs_recalc: true }]
    const fail = failLater(/\/api\/rounds$/, '라운드 목록 조회 실패 K')
    const w = await openRoundTab()
    expect(w.text(), '픽스처가 재계산 필요 경고에 닿지 않는다').toContain('표시된 총점·순위가 최신이 아닙니다')
    await click(btnIn(w, '지원 현황'))
    fail.on = true
    await click(btnIn(w, '점수 전체 재계산'))

    expect(writes()).toEqual(['POST /api/rounds/1/calculate'])
    // loadRounds 는 던지지 않는다 — 알림이 없어야 한다(뜨면 reloadAfter 의 성격이 바뀐 것)
    expect(dialogState.open, '목록 재조회 실패에 알림이 떴다').toBe(false)
    expect(w.text()).toContain('점수 재계산 완료: 2건')
    expect(w.text()).toContain('라운드 목록 조회 실패 K')
    await click(btnIn(w, '결과'))
    expect(w.text(), '재계산은 성공했는데 옛 needs_recalc 로 "최신이 아닙니다" 경고가 남았다')
      .not.toContain('표시된 총점·순위가 최신이 아닙니다')
    w.unmount()
  })

  // ── 마감 감사 C-1 — 수정 전 코드에서 실패했다 ─────────────────
  // 재계산 성공 → 재조회(결과·라운드 목록)가 끝나기 전에 관리자가 2차 라운드를 고른다.
  // 수정 전 handleCalculate 는 재조회 뒤 `if (fresh) selected.value = fresh` 를 라운드 비교 없이
  // 실행해 selected 가 1차로 되돌아갔다 — 머리글은 원래 라운드인데 표는 다른 라운드의 것(응답
  // 순서에 따라 빈 표: 2차 조회가 늦으면 늦은 응답 가드 `selected.value?.id !== rid` 가 버린다).
  // 이 픽스처는 clickRound 의 settle 이 2차 조회를 끝낸 뒤 release 하므로 머리글 단언에서 걸린다.
  // 판별력: selected 갱신의 라운드 비교(`selected.value?.id === roundId`)를 빼면 머리글 단언에서 걸린다.
  it('재계산 뒤 재조회 중에 다른 라운드를 고르면 머리글이 1차로 되돌아가지 않는다', async () => {
    S.rounds = [{ ...ROUND(1, 'CLOSED'), needs_recalc: true }, ROUND(2, 'FINALIZED')]
    S.rows = [ROW(1, '김갑돌'), ROW(2, '이을순', { round_id: 2, recommended: true })]
    // 재계산 뒤 첫 라운드 목록 재조회만 붙잡는다 — 마운트 때의 조회는 그대로 둔다
    let release
    const gate = new Promise(r => { release = r })
    let armed = false
    S.hooks.push({ method: 'GET', re: /\/api\/rounds$/, fn: ({ url }) => (armed ? gate.then(() => defaultRoute('GET', url)) : undefined) })
    const w = await openRoundTab({ tab: null })
    armed = true
    await click(btnIn(w, '점수 전체 재계산'))      // POST 성공 → 재조회 시작(라운드 목록은 붙잡힘)

    await clickRound(w, 2)                          // 재조회가 끝나기 전에 2차를 고른다
    release()
    await settle()

    // 머리글(상세 카드)의 라운드 번호 — 2차여야 한다
    const header = w.find('.text-xl.font-bold')
    expect(header.exists()).toBe(true)
    expect(header.text(), '재계산 재조회의 늦은 완료가 선택 라운드를 1차로 되돌렸다').toBe('2차 라운드')
    // 2차 라운드의 지원 목록이 보여야 한다(빈 표가 아니라)
    expect(w.text()).toContain('이을순')
    w.unmount()
  })

  // 같은 상황에서 POST 자체가 실패하면 — 실패 문구는 원래 라운드 것이다. 2차 라운드 화면에
  // 1차의 "재계산 실패" 를 띄우지 않는다(selectRound 가 calcMsg 를 비운 뒤에 덮어쓰게 된다).
  // 판별력: 실패 분기의 라운드 비교를 빼면 2차 화면(CLOSED 가 아니라 문구 자리가 없다)에서는
  // 안 보이므로, 2차도 CLOSED 로 두어 문구 자리가 있게 한다.
  it('응답 전에 다른 라운드를 고르면 재계산 실패 문구가 그 라운드에 뜨지 않는다', async () => {
    S.rounds = [ROUND(1, 'CLOSED'), ROUND(2, 'CLOSED')]
    S.rows = [ROW(1, '김갑돌'), ROW(2, '이을순', { round_id: 2 })]
    let release
    const gate = new Promise(r => { release = r })
    S.hooks.push({ method: 'POST', re: /\/calculate$/, fn: () => gate.then(() => err(500, '재계산 실패 Z')) })
    const w = await openRoundTab({ tab: null })
    await click(btnIn(w, '점수 전체 재계산'))
    await clickRound(w, 2)
    release()
    await settle()
    expect(w.find('.text-xl.font-bold').text()).toBe('2차 라운드')
    expect(w.text(), '1차 라운드의 재계산 실패 문구가 2차 화면에 떴다').not.toContain('재계산 실패 Z')
    w.unmount()
  })
})
