// @vitest-environment jsdom
/**
 * 되돌리기 어려운 행위가 **2단계 확인**을 거치는지 — 버튼을 실제로 눌러 확인한다.
 *
 * **이 파일의 첫 판은 소스에서 `level: 'danger'` 문자열을 세는 것이었다. 뚫렸다.**
 * `level: 'danger',` 를 `// level: 'danger',` 로 주석 처리하면 담임 [추천 포기]가
 * 한 번 클릭으로 실행되는데 전 검증이 초록이었다(5차 감사 치-1).
 *
 * 이 저장소는 같은 실패를 **다섯 번** 겪었다 — 소스 정규식으로 배선을 지키려다
 * F-014 에서 한 번, F-013 에서 두 번, 오라클 3c 에서 한 번, 그리고 여기서 한 번.
 * **그래서 규칙을 세운다: 배선은 행동으로만 지킨다.** 소스 텍스트 검사는 "눈에 띄는
 * 금지 패턴을 일찍 알려 주는 보조"까지만 하고, 그 한계를 반드시 주석에 적는다.
 *
 * 여기서는 `dialogState` 를 직접 들여다본다. 버튼을 누르면 어떤 다이얼로그가 떴는지
 * 상태로 드러나므로, 주석·변수 추출·객체 스프레드·파일 이동 전부에 면역이다.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { dialogState, settleDialog } from '../src/components/common/dialog.js'

// [마감하기] 는 CLOSED 에서만, [포기 처리] 는 FINALIZED 에서만 보인다.
// 하나로는 둘 다 못 누르므로 두 라운드를 준다.
const ROUND_CLOSED = { id: 1, status: 'CLOSED', opened_at: '2026-03-02T00:00:00Z',
                       closed_at: '2026-03-10T00:00:00Z', finalized_at: null,
                       needs_recalc: false }
const ROUND = { id: 2, status: 'FINALIZED', opened_at: '2026-03-02T00:00:00Z',
                closed_at: '2026-03-10T00:00:00Z', finalized_at: '2026-03-20T00:00:00Z',
                needs_recalc: false }
const ROUNDS = [ROUND_CLOSED, ROUND]
const ROW = {
  // round_id 는 FINALIZED 라운드(2)에 맞춘다 — 담임 결과표와 [포기 처리] 가 거기 있다.
  student_id: 1, track_id: 1, round_id: 2, name: '학생01', student_code: '2026001',
  grade: 3, class_no: 1, seq_no: 1, is_enrolled: true,
  univ_name: '가대학', track_name: '가모집단위', department_name: '컴퓨터공학과',
  total_score: 5.31304, score_detail: { 1: 5.31304 },
  ranking: 1, track_rank: 1, recommended: true, excluded: false,
  excluded_reason: null, abandoned: false,
}
const UNIV = {
  id: 1, univ_id: 1, univ_name: '가대학', total_quota: 5, total_used: 1, unlimited: false,
  prioritize_enrolled: 0,
  tracks: [{ id: 10, univ_id: 1, track_name: '가모집단위', unit_quota: 2, unit_used: 1,
             unlimited: false, prioritize_enrolled: 0, by_round: [] }],
}
const AREA = { id: 1, area_id: 1, name: '요소1', calc_type: 'NUMERIC', match_mode: 'UPPER',
               category_agg: null, lookup_scope: 'SIMPLE', multi_value: 0,
               teacher_editable: 1, max_score: 10, current_values: [''], sort_order: 1 }
const STUDENT = { id: 1, name: '학생01', student_code: '2026001',
                  grade: 3, class_no: 1, seq_no: 1, is_enrolled: true }

// 쓰기 호출을 기록한다. **"취소를 눌렀는데 실행됐는가"** 를 보기 위해서다.
// 6차 감사 치-2: 다이얼로그가 떴는지만 보던 동안, `if (!(await confirm(...))) return` 에서
// `if` 만 걷어내는 변이(= 취소해도 삭제됨)가 전 검증을 통과했다. 간판만 보고 게이트를
// 안 본 것이다.
export const writes = []

vi.mock('axios', () => {
  const read = (url = '') => {
    const u = String(url)
    const data =
      /teacher\/results/.test(u) ? { rounds: [ROUND], results: [ROW] }
      : /quota-stats/.test(u)    ? { all_round_ids: [1], univs: [UNIV] }
      : /univ-tracks/.test(u)    ? UNIV.tracks
      : /universities/.test(u)   ? [UNIV]
      : /numeric-table\/list|category-map\/list|base-data\/list/.test(u)
                                 ? { rows: [], total: 0, page: 1, per_page: 50 }
      : /\/api\/areas/.test(u)   ? [AREA]
      : /\/api\/students/.test(u)? { rows: [STUDENT], total: 1, page: 1, per_page: 50, by_grade: { 3: [1] } }
      : /\/api\/classes/.test(u) ? [{ grade: 3, class_no: 1, teacher_name: '홍길동', has_password: true }]
      : /\/api\/applications/.test(u) ? [ROW]
      : /\/api\/rounds\/\d+\/results/.test(u) ? [ROW]
      : /\/api\/rounds/.test(u)  ? ROUNDS
      : []
    return Promise.resolve({ data, headers: {} })
  }
  const write = (method) => (url = '') => {
    writes.push(`${method} ${url}`)
    return read(url)
  }
  const axios = {
    get: read,
    post: write('POST'), put: write('PUT'), patch: write('PATCH'), delete: write('DELETE'),
    interceptors: { request: { use: () => {} }, response: { use: () => {} } },
  }
  return { default: axios, ...axios }
})

const tick = () => new Promise(r => setTimeout(r, 0))

/**
 * 파괴적 행위 **전부**. `열기` 는 그 버튼이 보이게 만드는 조작(없으면 마운트 직후 보인다),
 * `버튼` 은 누를 라벨, `호출` 은 확인을 **취소**했을 때 불리면 안 되는 axios 메서드다.
 *
 * 6차 감사 치-1: 앞 판은 8곳 중 **3곳만** 눌러 봤고, 나머지 5곳은 `level: 'danger',` 를
 * 주석 처리하면 그대로 통과했다. "목록이 낡으면 잡힌다"고 적어 둔 장치는 이 목록이
 * 아니라 **별도 하드코딩 객체**와 비교하고 있어서 3 vs 8 로 어긋난 채 아무 말도 없었다.
 * 이제 아래 `DANGER_SITES` 하나만 두고, 소스 개수와 **이 목록의 길이**를 대조한다.
 */
const DESTRUCTIVE = [
  { 이름: '담임 — 추천 포기',      파일: 'teacher/ResultsTab.vue',     버튼: '추천 포기' },
  { 이름: '관리자 — 학급 삭제',     파일: 'admin/ClassesTab.vue',       버튼: '삭제' },
  { 이름: '관리자 — 대학 삭제',     파일: 'admin/UniversitiesTab.vue',  버튼: '삭제' },
  { 이름: '관리자 — 전형요소 삭제',  파일: 'admin/AreasTab.vue',         버튼: '삭제' },
  { 이름: '관리자 — 학생 삭제',     파일: 'admin/StudentsTab.vue',      버튼: '삭제' },
  {
    이름: '관리자 — 모집단위 삭제', 파일: 'admin/UniversitiesTab.vue', 버튼: '삭제',
    // 대학 카드의 [삭제]와 라벨이 같다. 표 안(td)에 있는 쪽이 모집단위 것이다.
    고르기: (w) => w.findAll('td button').filter(b => b.text() === '삭제')[0],
  },
  {
    이름: '관리자 — 라운드 마감',   파일: 'admin/RoundsTab.vue',        버튼: '마감하기',
    // 마감 버튼은 CLOSED("종료") 라운드에만 있다.
    열기: async (w, tick) => { await pickRound(w, tick, '종료') },
  },
  {
    // 화면 버튼은 '포기하기' 다 — '포기 처리' 는 다이얼로그의 confirmText 다.
    이름: '관리자 — 지원 포기 처리', 파일: 'admin/RoundsTab.vue',       버튼: '포기하기',
    // 포기 처리는 FINALIZED("마감") 라운드의 추천 확정된 지원에만 있다.
    열기: async (w, tick) => { await pickRound(w, tick, '마감') },
  },
]

/** 상태 표기로 라운드 카드를 골라 상세 패널을 연다. */
async function pickRound(wrapper, tick, 상태) {
  const card = wrapper.findAll('.cursor-pointer').find(d => d.text().includes(상태))
  expect(card, `[${상태}] 라운드 카드가 없다`).toBeTruthy()
  await card.trigger('click')
  await tick()
}

const load = (p) => import(/* @vite-ignore */ p)

describe('파괴적 행위는 2단계로 확인한다 — 버튼을 실제로 누른다', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    localStorage.clear()
    localStorage.setItem('pcm_token', 't')
    localStorage.setItem('pcm_role', 'admin')
    localStorage.setItem('pcm_grade', '3')
    localStorage.setItem('pcm_class_no', '1')
    writes.length = 0
    vi.spyOn(console, 'warn').mockImplementation(() => {})
    vi.spyOn(console, 'error').mockImplementation(() => {})
  })
  afterEach(() => {
    if (dialogState.open) settleDialog(false)
    vi.restoreAllMocks()
  })

  it.each(DESTRUCTIVE)('$이름 — 확인을 거치고, 취소하면 실행되지 않는다',
    async ({ 파일, 버튼, 열기, 고르기 }) => {
      const mod = await load(`../src/components/${파일}`)
      const wrapper = mount(mod.default, {
        global: { stubs: { RouterLink: true, RouterView: true } },
      })
      await tick()
      if (열기) await 열기(wrapper, tick)

      const target = 고르기
        ? 고르기(wrapper)
        : wrapper.findAll('button').find(b => b.text() === 버튼)
      expect(target, `[${버튼}] 버튼이 화면에 없다 — 픽스처나 열기 단계를 확인하라`)
        .toBeTruthy()

      writes.length = 0
      await target.trigger('click')
      await tick()

      // ① 확인 없이 바로 실행되지 않는다
      expect(dialogState.open, '확인 없이 바로 실행된다').toBe(true)
      expect(dialogState.level,
        '되돌리기 어려운 행위인데 danger 가 아니다 — 한 번 누르면 실행된다').toBe('danger')
      expect(dialogState.dangerNotice,
        '2단계 화면에 보여 줄 문구가 없다').toBeTruthy()
      expect(dialogState.step, '처음부터 2단계로 열렸다').toBe(1)
      expect(writes, '다이얼로그가 뜨기도 전에 서버를 불렀다').toEqual([])

      // ② **취소하면 실제로 막힌다.** 여기가 게이트다 — ①은 간판일 뿐이다.
      settleDialog(false)
      await tick()
      expect(writes,
        '취소를 눌렀는데 실행됐다 — confirm 결과를 보지 않고 있다').toEqual([])

      wrapper.unmount()
    })
})

/**
 * 위 행동 테스트는 **내가 아는 버튼**만 누른다. 새 파괴적 행위가 생겼는데 목록에
 * 안 적으면 조용히 검사 밖이 된다. 소스에서 `danger` 를 쓰는 곳을 세어 목록과 대조한다.
 *
 * **이것은 보조 장치다.** 주석 처리·변수 추출로 우회된다 — 그래서 위 행동 테스트가
 * 본체이고, 여기는 "목록이 낡았는지"만 본다.
 */
describe('danger 를 쓰는 곳이 목록과 어긋나지 않는다 (보조)', () => {
  it('소스의 danger 개수가 알려진 수와 같다', async () => {
    const [{ default: fs }, { default: path }, { fileURLToPath }] =
      await Promise.all([import('node:fs'), import('node:path'), import('node:url')])
    const SRC = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', 'src')

    const hits = []
    const walk = (dir) => {
      for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
        const p = path.join(dir, e.name)
        if (e.isDirectory()) walk(p)
        else if (/\.(vue|js)$/.test(e.name)) {
          const n = (fs.readFileSync(p, 'utf8').match(/level:\s*'danger'/g) ?? []).length
          if (n) hits.push([path.relative(SRC, p).replace(/\\/g, '/'), n])
        }
      }
    }
    walk(SRC)

    // 되돌리기 어려운 행위 8곳. 늘거나 줄면 목록과 위 DESTRUCTIVE 를 함께 고쳐라.
    expect(Object.fromEntries(hits)).toEqual({
      'components/admin/AreasTab.vue': 1,          // 전형요소 삭제
      'components/admin/ClassesTab.vue': 1,        // 학급 삭제
      'components/admin/RoundsTab.vue': 2,         // 라운드 마감 / 지원 포기 처리
      'components/admin/StudentsTab.vue': 1,       // 학생 삭제
      'components/admin/UniversitiesTab.vue': 2,   // 대학 삭제 / 모집단위 삭제
      'components/teacher/ResultsTab.vue': 1,      // 추천 포기
    })
  })
})
