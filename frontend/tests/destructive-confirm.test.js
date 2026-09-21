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

const ROUND = { id: 1, status: 'FINALIZED', opened_at: '2026-03-02T00:00:00Z',
                closed_at: '2026-03-10T00:00:00Z', finalized_at: '2026-03-20T00:00:00Z',
                needs_recalc: false }
const ROW = {
  student_id: 1, track_id: 1, round_id: 1, name: '학생01', student_code: '2026001',
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

vi.mock('axios', () => {
  const res = (url = '') => {
    const u = String(url)
    const data =
      /teacher\/results/.test(u) ? { rounds: [ROUND], results: [ROW] }
      : /quota-stats/.test(u)    ? { all_round_ids: [1], univs: [UNIV] }
      : /univ-tracks/.test(u)    ? UNIV.tracks
      : /universities/.test(u)   ? [UNIV]
      : /\/api\/areas/.test(u)   ? [AREA]
      : /\/api\/students/.test(u)? { rows: [STUDENT], total: 1, page: 1, per_page: 50, by_grade: { 3: [1] } }
      : /\/api\/classes/.test(u) ? [{ grade: 3, class_no: 1, teacher_name: '홍길동', has_password: true }]
      : /\/api\/rounds/.test(u)  ? [ROUND]
      : []
    return Promise.resolve({ data, headers: {} })
  }
  const axios = { get: res, post: res, put: res, patch: res, delete: res,
    interceptors: { request: { use: () => {} }, response: { use: () => {} } } }
  return { default: axios, ...axios }
})

const tick = () => new Promise(r => setTimeout(r, 0))

/**
 * 파괴적 행위 버튼. `열기` 가 화면을 눌러 그 버튼이 보이게 만들고, `버튼` 이 그것을 고른다.
 * 새 파괴적 행위가 생기면 여기에 한 줄을 더해야 한다 — 빠뜨리면 아래
 * "danger 를 쓰는 곳이 목록과 같다" 검사가 실패한다.
 */
const DESTRUCTIVE = [
  {
    이름: '담임 — 추천 포기',
    파일: '../src/components/teacher/ResultsTab.vue',
    버튼: '추천 포기',
  },
  {
    이름: '관리자 — 학급 삭제',
    파일: '../src/components/admin/ClassesTab.vue',
    버튼: '삭제',
  },
  {
    이름: '관리자 — 대학 삭제',
    파일: '../src/components/admin/UniversitiesTab.vue',
    버튼: '삭제',
  },
]

const load = (p) => import(/* @vite-ignore */ p)

describe('파괴적 행위는 2단계로 확인한다 — 버튼을 실제로 누른다', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    localStorage.clear()
    localStorage.setItem('pcm_token', 't')
    localStorage.setItem('pcm_role', 'teacher')
    localStorage.setItem('pcm_grade', '3')
    localStorage.setItem('pcm_class_no', '1')
    vi.spyOn(console, 'warn').mockImplementation(() => {})
    vi.spyOn(console, 'error').mockImplementation(() => {})
  })
  afterEach(() => {
    if (dialogState.open) settleDialog(false)
    vi.restoreAllMocks()
  })

  it.each(DESTRUCTIVE)('$이름 — 누르면 danger 확인이 뜬다', async ({ 파일, 버튼 }) => {
    const mod = await load(파일)
    const wrapper = mount(mod.default, {
      global: { stubs: { RouterLink: true, RouterView: true } },
    })
    await tick(); await tick()

    const target = wrapper.findAll('button').find(b => b.text() === 버튼)
    expect(target, `[${버튼}] 버튼이 화면에 없다 — 픽스처가 그 상태를 만들었는지 확인하라`)
      .toBeTruthy()
    await target.trigger('click')
    await tick()

    // **상태로 본다.** 소스에 'danger' 라는 글자가 있는지가 아니라,
    // 실제로 danger 다이얼로그가 떴는지.
    expect(dialogState.open, '확인 없이 바로 실행된다').toBe(true)
    expect(dialogState.level,
      `되돌리기 어려운 행위인데 danger 가 아니다 — 한 번 누르면 실행된다`).toBe('danger')
    expect(dialogState.dangerNotice,
      '2단계 화면에 보여 줄 문구가 없다 — 관리자는 무엇을 확인하는지 모른다').toBeTruthy()
    expect(dialogState.step, '처음부터 2단계로 열렸다').toBe(1)

    settleDialog(false)
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
