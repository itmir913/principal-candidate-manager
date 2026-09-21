// @vitest-environment jsdom
/**
 * 스모크 렌더 — 모든 화면을 **실제로 마운트해 보고 터지지 않는지**만 본다.
 *
 * 왜 필요한가: `src/docs/13_frontend_pitfalls.md` §4 가 적어 둔 공백이다.
 * vite 는 함수 **본문 안의** 미정의 식별자를 컴파일 타임에 보지 않고, 이 저장소에는
 * eslint 도 타입 검사도 없다. 실제로 그 때문에 담임 [라운드 결과] 화면이 **아예 열리지
 * 않은** 사고가 있었다(`cancelDeptEdit` ↔ `cancelEdit` 오타, ReferenceError).
 * 백엔드 테스트·오라클·나머지 vitest 중 어느 것도 `.vue` 런타임을 실행하지 않으므로,
 * 그 유형은 사람이 화면을 열기 전까지 아무도 모른다.
 *
 * **이 파일이 하지 않는 것**: 외관·레이아웃·페인트는 단언하지 않는다. 그쪽은 여전히
 * 사람이 화면을 열어 확인하는 영역이다(13_frontend_pitfalls 의 나머지 항목 전부가
 * 그 종류다). 여기서 잡는 것은 "열리지도 않는다" 하나뿐이다.
 *
 * **한계**: 마운트와 첫 렌더에서 실행되는 코드만 지난다. 클릭해야 도달하는 핸들러
 * 안의 미정의 식별자는 여전히 잡히지 않는다.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { settle } from './settle.js'
import { mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { readdirSync, existsSync } from 'node:fs'
import { join, relative, sep } from 'node:path'

// 모든 화면은 axios 로 서버와 말한다. 여기 한 곳만 막으면 api 모듈(admin.js/teacher.js)은
// **실물이 그대로 실행된다** — 래퍼의 오타도 함께 지나간다.
//
// 응답 형태가 엔드포인트마다 달라(배열 / {rows,total} / {univs:[]} …) 하나로 맞출 수 없다.
// 그래서 **빈 배열이면서 흔히 쓰이는 속성도 가진** 값을 준다. 형태가 안 맞아 나는 오류는
// 이 테스트가 잡으려는 결함이 아니라 잡음이므로, 키를 채워 조용히 지나가게 한다.
// (모든 속성에 응답하는 Proxy 도 시도했으나 Vue 반응성이 무한 재귀에 빠졌다 — 쓰지 마라.)
const flexible = () => Object.assign([], {
  univs: [], rows: [], items: [], tracks: [], areas: [], students: [], classes: [],
  logs: [], results: [], applications: [], rounds: [], all_round_ids: [], entries: [],
  total: 0, page: 1, per_page: 50, count: 0,
  // 중첩 응답 — 옵셔널 체이닝 없이 바로 파고드는 곳이 있어 형태를 맞춰 준다.
  // (예: OverviewTab 의 `data.value.all_time.total_rounds`)
  // OverviewAllTime 의 필드는 total_rounds·total_applicants·confirmed·abandoned 다
  // (src/handlers/overview.rs:64). 이름을 틀렸더니 화면에 "undefined명" 이 그려졌고,
  // 새로 넣은 누출 검사가 바로 잡았다.
  all_time: { total_rounds: 0, total_applicants: 0, confirmed: 0, abandoned: 0 },
  round: null, graduated: null, enrolled: null,
  by_status: {}, by_univ: [], recent: [], summary: {}, by_grade: {}, grades: [],
})

// ── 최소 픽스처 ─────────────────────────────────────────────────
// 빈 응답만 주면 화면 대부분이 `v-if` 뒤에서 렌더되지 않는다. 실제로 그 상태에서는
// "템플릿이 없는 이름을 참조" 같은 결함을 하나도 잡지 못했다(변이로 확인).
// 그래서 **목록이 하나씩은 있는** 상태를 만든다 — 표·카드·행이 실제로 그려진다.
// 필드는 백엔드 응답에서 가져왔다(results 행은 tools/oracle/actual.json 실측 형태).
const ROUND = { id: 1, status: 'CLOSED', opened_at: '2026-03-02T00:00:00Z',
                closed_at: '2026-03-10T00:00:00Z', finalized_at: null, needs_recalc: false }
/**
 * `/api/rounds/current` 는 백엔드에서 **`status = 'OPEN'` 인 행만** 돌려준다
 * (src/handlers/rounds.rs:95). 여기에 CLOSED 를 주면 백엔드가 만들 수 없는 상태라,
 * `currentRound.status === 'OPEN'` 뒤에 있는 화면 분기가 **한 번도 렌더되지 않는다** —
 * 커버리지처럼 보이는 공백이었다(6차 감사 미결 항목).
 * 목록(`/api/rounds`)에는 CLOSED 와 함께 둔다. 실제로 가능한 상태다.
 */
const OPEN_ROUND = { id: 3, status: 'OPEN', opened_at: '2026-03-21T00:00:00Z',
                     closed_at: null, finalized_at: null, needs_recalc: false }

const RESULT = {
  student_id: 1, track_id: 1, round_id: 1, name: '학생01', student_code: '2026001',
  grade: 3, class_no: 1, seq_no: 1, is_enrolled: true,
  univ_name: '가대학', track_name: '가모집단위', department_name: '컴퓨터공학과',
  total_score: 5.31304, score_detail: { 1: 5.31304 },
  ranking: 1, track_rank: 1, recommended: false, excluded: false,
  excluded_reason: null, abandoned: false,
}
// 같은 대학의 **다른 모집단위**에서 같은 대학 순위(1위) — 대학 전체 보기에서 동점이다.
// F-014 의 본질이 여기 있다: 모집단위 필터를 걸어도 이 동점 표식이 남아야 한다.
const RESULT2 = {
  student_id: 2, track_id: 2, round_id: 1, name: '학생02', student_code: '2026002',
  grade: 3, class_no: 1, seq_no: 2, is_enrolled: true,
  univ_name: '가대학', track_name: '나모집단위', department_name: '전자공학과',
  total_score: 5.31304, score_detail: { 1: 5.31304 },
  ranking: 1, track_rank: 1, recommended: false, excluded: false,
  excluded_reason: null, abandoned: false,
}

/**
 * 담임 화면 엔드포인트. 없으면 전부 flexible() 로 떨어져 빈 껍데기만 렌더된다(감사 치-3).
 *
 * **라운드는 FINALIZED 여야 한다.** 담임 [라운드 결과] 화면은 CLOSED 면
 * "관리자가 결과를 확정하는 중입니다"만 그리고 결과 표 전체(학과명 편집·포기 버튼 포함)를
 * 건너뛴다. 엔드포인트만 넣고 status 를 CLOSED 로 두었더니, §4 사고가 난 바로 그 화면의
 * 결과 표가 여전히 한 번도 안 그려졌다(감사 치-3 재지적).
 *
 * 공용 ROUND 를 FINALIZED 로 바꾸면 안 된다 — 관리자 RoundsTab 이 FINALIZED 에서 행
 * 배경을 추천/미선발 색으로 칠해 F-014 의 동점 표식(#fef3c7) 단언이 무너진다.
 * 그래서 담임 쪽에만 따로 둔다.
 */
const FINAL_ROUND = { ...ROUND, id: 2, status: 'FINALIZED',
                      finalized_at: '2026-03-20T00:00:00Z' }

function TEACHER_FIXTURE(u) {
  // teacherGetResults 는 배열이 아니라 `{ rounds, results }` 를 준다
  // (ResultsTab.vue:469). 배열로 주면 `rounds.value` 가 undefined 가 되어 렌더가 터진다.
  // **한 행은 추천 확정, 한 행은 미선발**로 둔다. 둘 다 미선발이면 "추천 확정"·
  // "포기됨" 분기와 [추천 포기] 버튼이 영영 렌더되지 않는다 — 되돌리기 어려운 행위의
  // 버튼이 검사 밖에 있었다(4차 감사 놓친 항목 4).
  if (/results/.test(u)) return {
    rounds: [FINAL_ROUND],
    results: [
      { ...RESULT,  round_id: 2, recommended: true },
      { ...RESULT2, round_id: 2 },
    ],
  }
  // StudentRow 의 기본키는 `id` 다(src/handlers/students.rs:40). `student_id` 로 주면
  // `v-for :key="s.id"` 와 `selectedStudent?.id` 가 전부 undefined 가 되어,
  // 목록은 그려지는데 **선택이 되지 않는다.**
  if (/students/.test(u))     return [{ id: 1, name: '학생01', student_code: '2026001',
                                        grade: 3, class_no: 1, seq_no: 1, is_enrolled: true }]
  if (/applications/.test(u)) return [RESULT]
  if (/universities/.test(u)) return [UNIV]
  // teacher_areas 의 필드는 area_name·table 이다(src/handlers/teacher_areas.rs).
  if (/area/.test(u))         return [{ ...AREA, area_name: '요소1', table: [] }]
  if (/confirm/.test(u))      return { confirmed: false, confirmed_at: null }
  if (/rounds/.test(u))       return [FINAL_ROUND]
  return flexible()
}

const AREA = {
  id: 1, area_id: 1, name: '요소1', calc_type: 'NUMERIC', match_mode: 'UPPER',
  category_agg: null, lookup_scope: 'SIMPLE', multi_value: 0, teacher_editable: 1,
  max_score: 10, current_values: [''], sort_order: 1,
}
const UNIV = {
  id: 1, univ_id: 1, univ_name: '가대학', total_quota: 5, total_used: 1,
  prioritize_enrolled: 0,
  tracks: [{ id: 1, track_id: 1, track_name: '가모집단위', unit_quota: 2, unit_used: 1,
             prioritize_enrolled: 0, by_round: [] }],
}

/**
 * URL(+ 쿼리)로 응답을 고른다. 못 찾으면 위의 느슨한 기본값.
 *
 * **서버가 거르는 흉내를 낸다.** `track_id` 파라미터가 오면 그 모집단위만 돌려준다 —
 * 실제 백엔드가 그렇게 동작하기 때문이다. 이걸 빼면 "필터를 서버에 넘기는" 회귀가
 * 목 단계에서 무력화돼 테스트가 못 잡는다(감사 치-1 의 B-1 이 정확히 그랬다).
 */
function fixtureFor(url = '', config) {
  const u = String(url)
  const trackId = config?.params?.track_id
  if (/\/api\/teacher\//.test(u))                 return TEACHER_FIXTURE(u)
  if (/\/api\/rounds\/current/.test(u))          return OPEN_ROUND
  if (/\/api\/rounds\/\d+\/results/.test(u)) {
    const rows = [RESULT, RESULT2]
    return trackId ? rows.filter(r => r.track_id === Number(trackId)) : rows
  }
  if (/\/api\/rounds$/.test(u))                  return [ROUND, OPEN_ROUND]
  if (/\/api\/rounds\/\d+\/confirmation/.test(u)) return { total: 1, confirmed: 1, pending: [] }
  // 두 건을 준다 — 대학별 묶기(appsByUniv)와 재학생 우선 정렬이 한 건으로는 안 돈다.
  if (/\/api\/applications/.test(u))             return [RESULT, RESULT2]
  if (/quota-stats/.test(u))                      return { all_round_ids: [1], univs: [UNIV] }
  if (/\/api\/universities/.test(u))             return [UNIV]
  if (/\/api\/areas\/\d+\/(numeric-table|category-map|base-data)\/list/.test(u))
    return { rows: [], total: 0, page: 1, per_page: 50 }
  if (/\/api\/areas/.test(u))                    return [AREA]
  if (/\/api\/students/.test(u))                 return { rows: [RESULT], total: 1, page: 1, per_page: 50, by_grade: { 3: [1] } }
  if (/\/api\/classes/.test(u))                  return [{ grade: 3, class_no: 1, teacher_name: '홍길동', has_password: true }]
  if (/\/api\/audit-logs/.test(u))               return { rows: [], total: 0, page: 1, per_page: 50 }
  if (/\/api\/app-info/.test(u))                 return { version: '0.2.21', server_addr: '127.0.0.1:8080' }
  return flexible()
}

vi.mock('axios', () => {
  const res = (url, a, b) => {
    // axios 의 config 위치가 메서드마다 다르다: get/delete 는 두 번째, post/put/patch 는 세 번째.
    const config = (a && (a.params || a.responseType)) ? a : b
    return Promise.resolve({ data: fixtureFor(url, config), headers: {} })
  }
  const axios = {
    get: res, post: res, put: res, patch: res, delete: res,
    interceptors: { request: { use: () => {} }, response: { use: () => {} } },
  }
  return { default: axios, ...axios }
})

// 일부 화면은 `useRoute()`/`useRouter()` 를 쓴다(옵션 API 의 $route 목으로는 안 닿는다).
vi.mock('vue-router', async (orig) => ({
  ...(await orig()),
  useRoute:  () => ({ path: '/', params: {}, query: {}, name: 'x', fullPath: '/' }),
  useRouter: () => ({ push: () => {}, replace: () => {}, go: () => {}, back: () => {} }),
}))

// import 는 vi.mock 이후에 평가된다(vitest 가 호이스팅한다).
// **재귀 글롭**이어야 한다. 디렉터리별로 네 줄을 적어 두면 새 디렉터리에 만든 화면이
// 목록에도, 아래 `모든 .vue 가 스모크 대상이거나…` 대조에도 **동시에** 안 잡힌다 —
// 대조가 자기 자신의 글롭 결과를 보고 있으니 순환이라 영원히 초록이다(6차 감사 지적).
const all = import.meta.glob('../src/**/*.vue')

/**
 * props 를 요구하는 컴포넌트는 부모가 늘 값을 주므로 단독 마운트 대상이 아니다.
 * 목록을 여기 적어 두는 이유: 새 컴포넌트가 조용히 검사 밖으로 빠지지 않게 하려는 것이다
 * (아래 `모든 .vue 가 스모크 대상이거나 명시적으로 제외되어 있다` 테스트가 대조한다).
 */
const NEEDS_PROPS = new Set([
  '../src/components/admin/MiniPie.vue',            // value/총합을 부모가 준다
  '../src/components/admin/ScoreDemoCard.vue',      // 전형요소 한 건을 받는다
  '../src/components/common/HelpBox.vue',           // 안내 문구를 받는다
  '../src/components/common/ProductInfoCard.vue',   // 버전 정보를 받는다
  '../src/components/teacher/ApplicationDetailModal.vue', // 아래에서 props 를 주고 따로 마운트
])

/**
 * props 를 요구해 자동 마운트에서 빠지는 것 중, **혼자 화면을 채우는 것**은 따로 연다.
 * 상세 모달은 부모의 `v-if="detailApp"` 뒤라 클릭 테스트로도 안 열려, 212줄이 통째로
 * 검증 밖이었다(4차 감사 중-C).
 */
const PROPPED = {
  '../src/components/teacher/ApplicationDetailModal.vue': {
    props: {
      app: { student_id: 1, track_id: 1, round_id: 1, univ_name: '가대학',
             track_name: '가모집단위', department_name: '컴퓨터공학과',
             recommended: false, abandoned: false },
      studentName: '학생01',
    },
    evidence: '가대학',
  },
}

/** 라우터를 쓰는 화면이 있으므로 최소 스텁을 끼운다. */
const global = {
  stubs: { RouterLink: true, RouterView: true },
  mocks: {
    $route: { path: '/', params: {}, query: {} },
    $router: { push: () => {}, replace: () => {} },
  },
}

/**
 * 로그인 상태를 만든다. 인증 없이 마운트하면 `auth.grade` 가 null 이라
 * TeacherView 가 **"선생님null학년 null반 담임"** 을 그린다 — 실제 화면에서는
 * 라우터 가드가 막고 백엔드가 `grade: i64`(non-optional)를 주므로 일어나지 않는
 * **테스트 인공물**이다. 그 상태로 두면 아래 "null 노출" 검사가 늘 빨개진다.
 */
function signIn() {
  localStorage.clear()
  localStorage.setItem('pcm_token', 'test-token')
  localStorage.setItem('pcm_role', 'teacher')
  localStorage.setItem('pcm_grade', '3')
  localStorage.setItem('pcm_class_no', '1')
  localStorage.setItem('pcm_teacher_name', '김담임')
}

const targets = Object.keys(all).filter(p => !NEEDS_PROPS.has(p) && !p.endsWith('/App.vue'))

/**
 * `it.each` 가 **실제로 마운트한** 경로. 아래 커버리지 대조가 이걸 본다.
 *
 * `targets` 와 대조하면 의미가 없다 — `targets` 자체가 `all` 에서 파생되므로
 * (all − NEEDS_PROPS − App) 을 all 에서 다시 빼는 꼴이고, 결과는 **항상 빈 집합**이다.
 * 실행 기록만이 글롭·목록과 독립된 증거다.
 */
const MOUNTED = new Set()

/**
 * 치명으로 볼 신호. **없는 이름을 쓴 것**만 고른다 —
 * prop 형태 경고나 개발 편의 경고까지 잡으면 잡음에 묻혀 아무도 안 본다.
 */
const FATAL = new RegExp([
  'ReferenceError',
  'is not defined',                              // ReferenceError 본문 + Vue 경고 양쪽
  'is not a function',
  'was accessed during render',                  // 템플릿이 없는 이름을 읽었다
  'Failed to resolve component',                 // 컴포넌트 이름 오타
  'Invalid vnode type',
  // 렌더 중 TypeError. 템플릿에서 `is not defined` 다음으로 흔한데 처음엔 빠져 있었다 —
  // `{{ row.nope.teacher_name }}` 변이가 23/23 초록으로 빠져나갔다(감사 치-2).
  'Cannot read propert',
  'of undefined',
  'of null',
  'Unhandled error during execution',
].join('|'))

describe('스모크 렌더', () => {
  let errors

  beforeEach(() => {
    setActivePinia(createPinia())
    signIn()
    errors = []
    // error 와 warn 을 **둘 다** 모은다. 미정의 식별자는 예외가 아니라 경고로 나오는
    // 경우가 많다 — Vue 는 템플릿에서 없는 이름을 읽으면
    // "Property X was accessed during render but is not defined" 를 warn 으로 흘린다.
    // 처음에 warn 을 버렸더니 바로 그 유형(템플릿이 제거된 식별자를 참조)을 놓쳤다.
    const collect = (...a) => errors.push(a.map(x => (x && x.message) || String(x)).join(' '))
    vi.spyOn(console, 'error').mockImplementation(collect)
    vi.spyOn(console, 'warn').mockImplementation(collect)
  })

  afterEach(() => { vi.restoreAllMocks() })

  it('마운트 대상이 비어 있지 않다', () => {
    // glob 경로가 어긋나면 0개를 돌면서 초록이 뜬다 — 그 침묵부터 막는다.
    expect(targets.length).toBeGreaterThanOrEqual(15)
  })

  it.each(targets)('%s 이(가) 열린다', async (path) => {
    // 비동기 렌더(computed 가 로드 뒤에 처음 평가되는 경우)에서 터지면 console 이 아니라
    // 처리되지 않은 rejection 으로 샌다. 그 상태로 두면 vitest 가 "어느 화면인지"를
    // 알려 주지 못한다 — 실제로 AreasTab 의 import 누락이 그렇게 보고됐다.
    const onRejection = (e) => errors.push(String(e?.reason?.message ?? e?.reason ?? e))
    process.on('unhandledRejection', onRejection)

    const mod = await all[path]()
    const wrapper = mount(mod.default, { global })
    MOUNTED.add(path)
    // onMounted 의 비동기 로드까지 흘려보낸다 — 사고가 났던 지점이 거기였다.
    await settle()
    process.off('unhandledRejection', onRejection)

    const fatal = errors.filter(e => FATAL.test(e))
    expect(fatal, `${path} 렌더 중 치명 오류`).toEqual([])

    // **화면에 렌더된 글자도 본다.** 이 저장소의 로더는 전부
    // `try { … } catch (e) { error.value = e.message }` 라, 안에서 난
    // ReferenceError 가 console 로 새지 않고 **오류 문구로 화면에 그려진다.**
    // 그래서 console 만 보던 판정은 `getClasses` -> `getClassesTypo` 변이를
    // 통째로 놓쳤다(감사 치-2). 사고가 났던 §4 도 정확히 이 모양이었다.
    const shown = wrapper.text()
    expect(FATAL.test(shown), `${path} 화면에 오류 문구가 그려졌다: ${shown.slice(0, 160)}`)
      .toBe(false)

    // **`null`/`undefined`/`NaN` 이 글자로 새어 나오는지.** 오류는 아니지만 사용자에게
    // 보이는 결함이다(4차 감사 경-7).
    //
    // 새는 길은 둘뿐이다 — Vue 의 `{{ }}` 는 null·undefined 를 **빈 문자열**로 그리므로
    // 보간만으로는 안 샌다(`NaN` 은 예외로 "NaN" 이 찍힌다).
    //   ① 스크립트의 템플릿 리터럴·문자열 결합: `${auth.grade}학년` → "null학년"
    //   ② 계산 결과가 NaN: "총점 NaN"
    const LEAK = /\b(?:null|undefined|NaN)\b/
    const at = shown.search(LEAK)
    expect(at, `${path} 화면에 null/undefined/NaN 이 그대로 그려졌다: ` +
      `…${shown.slice(Math.max(0, at - 40), at + 40)}…`).toBe(-1)

    expect(wrapper.html()).toBeTruthy()

    // **픽스처가 화면에 닿았다는 증거.** 없으면 픽스처가 조용히 안 맞게 돼도
    // "오류 0건"으로 초록이 뜬다 — 담임 화면 셋이 실제로 그렇게 빈 껍데기만
    // 그리면서 통과했다(감사 치-3).
    const evidence = RENDER_EVIDENCE[path]
    if (evidence) {
      expect(shown, `${path} 가 픽스처를 그리지 않았다 (렌더 길이 ${shown.length})`)
        .toContain(evidence)
    }

    wrapper.unmount()
  })
})

/**
 * 화면별 "여기까지 그려졌다"는 증거 한 조각.
 * 목록에 없는 화면은 마운트만 확인한다 — 전부 채우면 픽스처 유지비가 화면 수만큼 는다.
 * 대신 **사고가 났거나 위험한 화면**은 반드시 넣는다.
 */
const RENDER_EVIDENCE = {
  // §4 사고가 난 화면. FINALIZED 결과 표가 그려져야 한다.
  '../src/components/teacher/ResultsTab.vue': '학생01',
  '../src/components/teacher/ClassTab.vue': '학생01',
  // '담임' 은 표 **헤더**("담임명")에도 있어 픽스처가 비어도 통과했다(5차 감사 중-5).
  // 데이터에서만 나오는 값을 쓴다.
  '../src/components/admin/ClassesTab.vue': '홍길동',
  '../src/components/admin/UniversitiesTab.vue': '가대학',
}

/**
 * 결과 패널은 라운드를 **클릭해야** 열린다. 마운트만으로는 `v-if="!selected"` 뒤에
 * 가려져 있어, 그 안의 결함(템플릿이 없는 이름을 참조 / computed 가 없는 함수를 호출)을
 * 하나도 잡지 못했다 — 변이로 확인했다.
 *
 * RoundsTab 만 따로 한 단계 더 밟는 이유: 이 저장소에서 가장 크고(1400줄 이상) 가장
 * 자주 손대는 화면이며, 점수·순위·추천이 모두 여기서 보인다. 다른 화면까지 조작을
 * 늘리지는 않는다 — 그 순간 이 파일은 스모크가 아니라 컴포넌트 테스트가 된다.
 */
describe('스모크 렌더 — 라운드 결과 패널', () => {
  let errors
  beforeEach(() => {
    setActivePinia(createPinia())
    errors = []
    const collect = (...a) => errors.push(a.map(x => (x && x.message) || String(x)).join(' '))
    vi.spyOn(console, 'error').mockImplementation(collect)
    vi.spyOn(console, 'warn').mockImplementation(collect)
  })
  afterEach(() => { vi.restoreAllMocks() })

  it('라운드를 고르면 결과 표까지 그려진다', async () => {
    const onRejection = (e) => errors.push(String(e?.reason?.message ?? e?.reason ?? e))
    process.on('unhandledRejection', onRejection)

    const mod = await all['../src/components/admin/RoundsTab.vue']()
    const wrapper = mount(mod.default, { global })
    await settle()

    // ⓪ 먼저 [지원 현황] 탭(기본 탭)이 실제로 그려지는지 본다. 결과 탭만 보던 동안
    //    이쪽은 "오류 0건"만 통과했다 — 학과명 편집·미선발 처리가 다 여기 있다.
    // ① 라운드를 고른다
    const card = wrapper.find('.cursor-pointer')
    expect(card.exists(), '라운드 카드가 없다 — 픽스처가 비었는지 확인하라').toBe(true)
    await card.trigger('click')
    await settle()

    // 지원 현황 표가 그려졌는지 — 픽스처가 닿았다는 증거.
    const appsText = wrapper.text()
    expect(appsText, '지원 현황 표에 학생이 없다').toContain('학생01')
    expect(appsText, '학과명이 그려지지 않았다').toContain('컴퓨터공학과')
    expect(appsText, '지원 건수가 안 보인다').toContain('총 2건')

    // ② [결과] 서브탭으로 넘어간다. 기본값은 [지원 현황]이라, 여기까지 오지 않으면
    //    결과 표(resultsByView·tieSet·univAutoButtonKeys)는 끝내 렌더되지 않는다.
    const resultsTab = wrapper.findAll('button').find(b => b.text() === '결과')
    expect(resultsTab, '[결과] 서브탭 버튼을 찾지 못했다').toBeTruthy()
    await resultsTab.trigger('click')
    await settle()
    process.off('unhandledRejection', onRejection)

    // 결과 표가 **정말로** 그려졌는지 확인한다. 이 단언이 없으면 표가 통째로 빠져도
    // "오류 0건"으로 초록이 뜬다 — 실제로 한 번 그렇게 속았다.
    // 표가 통째로 빠져도 "오류 0건"이면 초록이 뜬다 — 실제로 한 번 그렇게 속았다.
    // 그래서 파생값이 화면에 닿았다는 증거를 셋 다 확인한다.
    const text = wrapper.text()
    expect(text, '결과 행이 그려지지 않았다').toContain('학생01')            // groupBy*
    expect(text, '잔여석이 그려지지 않았다').toContain('잔여 4석')            // buildTrackQuotaMap
    expect(text, '자동 추천 버튼이 없다').toContain('전체 자동 추천')          // univAutoButtonKeys

    // ③ F-014 — **여기가 방어선이다.**
    //
    // 픽스처의 두 학생은 같은 대학의 **다른 모집단위**에서 대학 순위가 똑같이 1위다.
    // 즉 대학 전체 보기에서 동점이고, 동점 행은 배경색 #fef3c7 로 표시된다.
    // 모집단위 필터를 걸면 화면에는 한 명만 남지만 **동점 표식은 그대로여야 한다.**
    //
    // 이 단언이 없으면 호출부 회귀를 아무도 잡지 못한다. 실제로 그랬다 —
    // `getResults(id, selectedTrackId)` 로 서버에 필터를 넘기거나
    // `rows` 에 걸러진 배열을 넘기는 변이가 전 검증을 통과했다(감사 치-1).
    // computeTieSet 자체는 어떤 배열을 받아도 옳게 동작하므로 순수 함수 테스트로는
    // 원리적으로 잡을 수 없다.
    // `#fef3c7` 는 "재계산 필요" 배지 색이기도 하다(RoundsTab.vue:64). 행 배경 변수까지
    // 붙여 그 배지와 섞이지 않게 한다 — 픽스처를 한 글자 바꾸면 방어선 셋이 전부
    // 거짓 초록이 될 수 있었다.
    const TIE = '--row-bg:#fef3c7'
    const tieMark = () => wrapper.html().replace(/\s/g, '')
    expect(tieMark(), '필터 전에 동점 표식이 없다 — 픽스처를 확인하라').toContain(TIE)

    const select = wrapper.find('select')
    expect(select.exists(), '모집단위 필터를 찾지 못했다').toBe(true)
    await select.setValue('1')
    await settle()

    const filtered = wrapper.text()
    expect(filtered, '필터가 표시를 좁히지 않았다').not.toContain('학생02')
    expect(tieMark(),
      '모집단위 필터를 걸자 동점 표식이 사라졌다 — tieSet 에 걸러진 배열이 넘어갔다(F-014)')
      .toContain(TIE)

    // ④ 필터를 건 채 **재조회**한다. 여기까지 와야 "서버에 필터를 넘기는" 회귀가
    //    드러난다 — loadResults 는 라운드를 고를 때 한 번 돌고, 그때는 필터가 비어 있어
    //    잘못된 인자도 무해하게 지나간다. 실제 사용자는 필터를 걸어 둔 채 [새로고침]을
    //    누르거나 추천을 확정해 재조회를 일으킨다. 그 순간 같은 대학 다른 모집단위의
    //    동점 상대가 응답에서 빠져 표식이 사라진다.
    //    (목 axios 도 `track_id` 파라미터가 오면 서버처럼 걸러 준다.)
    const refresh = wrapper.findAll('button').find(b => b.text() === '새로고침')
    expect(refresh, '[새로고침] 버튼을 찾지 못했다').toBeTruthy()
    await refresh.trigger('click')
    await settle()

    expect(tieMark(),
      '필터를 건 채 재조회하자 동점 표식이 사라졌다 — loadResults 가 서버에 필터를 ' +
      '넘기고 있다. 라운드 전체를 받아 표시 단계에서만 걸러야 한다(F-014)')
      .toContain(TIE)

    expect(errors.filter(e => FATAL.test(e)), '결과 패널 렌더 중 치명 오류').toEqual([])
    // 첫 블록에만 있던 화면 글자 판정을 여기에도 건다 — 가장 복잡한 화면인데 빠져 있었다.
    expect(FATAL.test(wrapper.text()), '결과 패널에 오류 문구가 그려졌다').toBe(false)
    wrapper.unmount()
  })
})

/**
 * 담임 [지원 등록] — 학생을 골라야 오른쪽 패널이 열린다.
 * 마운트만으로는 "좌측에서 학생을 선택하세요"만 그려져, 그 패널 안의 결함을 하나도
 * 잡지 못했다(감사 치-3). RoundsTab 과 같은 이유로 한 단계 더 밟는다.
 */
describe('스모크 렌더 — 담임 지원 등록 패널', () => {
  let errors
  beforeEach(() => {
    setActivePinia(createPinia())
    localStorage.clear()
    errors = []
    const collect = (...a) => errors.push(a.map(x => (x && x.message) || String(x)).join(' '))
    vi.spyOn(console, 'error').mockImplementation(collect)
    vi.spyOn(console, 'warn').mockImplementation(collect)
  })
  afterEach(() => { vi.restoreAllMocks() })

  it('학생을 고르면 지원 입력 패널이 열린다', async () => {
    const onRejection = (e) => errors.push(String(e?.reason?.message ?? e?.reason ?? e))
    process.on('unhandledRejection', onRejection)

    const mod = await all['../src/components/teacher/ApplicationTab.vue']()
    const wrapper = mount(mod.default, { global })
    await settle()

    const row = wrapper.findAll('.cursor-pointer').find(d => d.text().includes('학생01'))
    expect(row, '학생 목록 행을 찾지 못했다 — 픽스처의 id 필드를 확인하라').toBeTruthy()
    await row.trigger('click')
    await settle()
    process.off('unhandledRejection', onRejection)

    const shown = wrapper.text()
    expect(shown, '학생을 골랐는데 패널이 열리지 않았다')
      .not.toContain('좌측에서 학생을 선택하세요')

    // 지원 입력 폼까지 연다. 여기가 담임이 실제로 점수를 넣는 화면인데,
    // 학생 선택까지만 밟던 동안 통째로 검사 밖이었다(4차 감사 놓친 항목 1).
    //
    // **`if (addBtn)` 으로 감싸지 않는다.** 그렇게 두었더니 버튼의 `@click` 을 통째로
    // 지워도(= 담임이 지원을 아무것도 등록할 수 없다) 26/26 초록이었다 —
    // 자기 자신을 끄는 블록은 초록 발생기다(5차 감사 중-3).
    const addBtn = wrapper.findAll('button').find(b => b.text().includes('새 지원 추가'))
    expect(addBtn, '[+ 새 지원 추가] 버튼이 없다').toBeTruthy()
    await addBtn.trigger('click')
    await settle()

    const afterForm = wrapper.text()
    expect(afterForm, '[+ 새 지원 추가] 를 눌렀는데 등록 폼이 열리지 않았다')
      .toContain('새 지원 등록')
    expect(errors.filter(e => FATAL.test(e)), '지원 입력 폼 렌더 중 치명 오류').toEqual([])
    expect(FATAL.test(afterForm), '지원 입력 폼에 오류 문구가 그려졌다').toBe(false)
    expect(errors.filter(e => FATAL.test(e)), '지원 패널 렌더 중 치명 오류').toEqual([])
    expect(FATAL.test(shown), `화면에 오류 문구가 그려졌다: ${shown.slice(0, 160)}`).toBe(false)
    wrapper.unmount()
  })
})

describe('스모크 렌더 — props 를 받는 화면', () => {
  let errors
  beforeEach(() => {
    setActivePinia(createPinia())
    errors = []
    const collect = (...a) => errors.push(a.map(x => (x && x.message) || String(x)).join(' '))
    vi.spyOn(console, 'error').mockImplementation(collect)
    vi.spyOn(console, 'warn').mockImplementation(collect)
  })
  afterEach(() => { vi.restoreAllMocks() })

  it.each(Object.keys(PROPPED))('%s 이(가) 열린다', async (path) => {
    const onRejection = (e) => errors.push(String(e?.reason?.message ?? e?.reason ?? e))
    process.on('unhandledRejection', onRejection)

    const { props, evidence } = PROPPED[path]
    const mod = await all[path]()
    const wrapper = mount(mod.default, { props, global })
    await settle()
    process.off('unhandledRejection', onRejection)

    const shown = wrapper.text()
    expect(errors.filter(e => FATAL.test(e)), `${path} 렌더 중 치명 오류`).toEqual([])
    expect(FATAL.test(shown), `${path} 화면에 오류 문구가 그려졌다`).toBe(false)
    expect(shown, `${path} 가 props 를 그리지 않았다`).toContain(evidence)
    wrapper.unmount()
  })
})

describe('스모크 대상 목록이 낡지 않았다', () => {
  /**
   * 디스크를 **글롭과 도립적으로** 센다.
   *
   * 이전 판은 `targets` 를 `all` 에서 파생시켜 놓고 다시 `all` 과 비교했다 —
   * 집합으로 쓰면 (all − NEEDS_PROPS − App) 을 all 에서 다시 빼는 꼴이라
   * 결과가 **항상 빈 집합**이다. 즉 어떤 경우에도 실패하지 않는 항진명제였다.
   * 글롭 패턴이 파일을 놓치는 상황은 그 글롭 자신으로는 볼 수 없다.
   */
  const vueFilesOnDisk = () => {
    // vitest 가 변환한 모듈에서는 import.meta.url 이 file: 스킴이 아니다.
    // vite 의 root(= frontend/)가 곧 cwd 이므로 거기서 잡는다.
    const root = join(process.cwd(), 'src')
    if (!existsSync(root)) throw new Error(`src 를 찾지 못했다: ${root}`)
    const walk = (d) => readdirSync(d, { withFileTypes: true }).flatMap(e =>
      e.isDirectory() ? walk(join(d, e.name))
        : (e.name.endsWith('.vue') ? [join(d, e.name)] : []))
    return walk(root)
      .map(f => '../src/' + relative(root, f).split(sep).join('/'))
      .sort()
  }

  it('글롭이 디스크의 .vue 를 하나도 빼뜨리지 않는다', () => {
    expect(Object.keys(all).sort()).toEqual(vueFilesOnDisk())
  })

  it('디스크의 모든 .vue 가 실제로 마운트됐거나 명시적으로 제외되어 있다', () => {
    // 이 검사는 위 `it.each` 가 전부 돈 뒤에만 의미가 있다. `-t` 로 걸러 돌리면
    // 기록이 비므로, 조용히 통과하는 대신 여기서 먼저 멈춘다.
    expect(MOUNTED.size, '마운트 기록이 비었다 — 이 검사는 파일 전체를 돌려야 한다').toBeGreaterThan(0)

    const uncovered = vueFilesOnDisk()
      .filter(p => !p.endsWith('/App.vue'))
      .filter(p => !MOUNTED.has(p) && !NEEDS_PROPS.has(p))
    expect(uncovered, '새 화면이 검사 밖에 있다').toEqual([])
  })

  it('PROPPED 에 적힌 파일이 NEEDS_PROPS 안에 있다', () => {
    // 둘이 어긋나면 어느 쪽도 안 도는 화면이 생긴다.
    const orphan = Object.keys(PROPPED).filter(p => !NEEDS_PROPS.has(p))
    expect(orphan, 'PROPPED 에만 있고 NEEDS_PROPS 에 없다').toEqual([])
  })

  it('NEEDS_PROPS 에 적힌 파일이 실제로 존재한다', () => {
    const gone = [...NEEDS_PROPS].filter(p => !(p in all))
    expect(gone, '지워진 컴포넌트가 제외 목록에 남아 있다').toEqual([])
  })
})
