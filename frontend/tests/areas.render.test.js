// @vitest-environment jsdom
/**
 * 전형요소 화면 — **요소를 골라 점수표까지** 그려 본다.
 *
 * 왜 필요한가: 이 화면은 오른쪽 패널(기본 정보·점수표·기초데이터)이 전부 `selected`
 * 뒤에 있어, 요소를 클릭하지 않으면 통째로 검사 밖이다. 스모크의 마운트 렌더로는
 * 거의 아무것도 그려지지 않던 화면이었다
 * (5차 감사 메타 판단: "테스트 수를 늘리지 말고 이 둘에 힘을 써라").
 *
 * 여기서 지키는 것: 요소 목록이 그려지는가, 고르면 점수표가 열리는가, 만점 합계가
 * 맞게 표시되는가, 탭을 옮기면 기초데이터가 나오는가.
 * **외관은 여전히 보지 않는다** — 그려졌는지와 값이 맞는지만 본다.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { settle } from './settle.js'
import { mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'

const AREAS = [
  { id: 1, name: '교과성적', calc_type: 'NUMERIC', match_mode: 'UPPER', category_agg: null,
    lookup_scope: 'SIMPLE', multi_value: 0, teacher_editable: 1, max_score: 40, sort_order: 1 },
  { id: 2, name: '출결', calc_type: 'CATEGORY', match_mode: null, category_agg: 'MAX',
    lookup_scope: 'SIMPLE', multi_value: 0, teacher_editable: 1, max_score: 10, sort_order: 2 },
  { id: 3, name: '교사추천', calc_type: 'MANUAL', match_mode: null, category_agg: null,
    lookup_scope: 'SIMPLE', multi_value: 0, teacher_editable: 1, max_score: 50, sort_order: 3 },
]
// getNumericTableList 의 응답. threshold·score 는 백엔드가 Score 로 직렬화해 내려준다.
const NUMERIC_ROWS = {
  rows: [
    { id: 11, area_id: 1, track_id: null, threshold: 1.5, score: 40 },
    { id: 12, area_id: 1, track_id: null, threshold: 2.0, score: 35 },
  ],
  total: 2, page: 1, per_page: 50,
}
const BASE_ROWS = { rows: [], total: 0, page: 1, per_page: 50 }

/** 기초데이터 import 응답 — **경고만 있고 오류는 없는** 경우. amber 분기를 태운다. */
const IMPORT_WITH_WARNING = {
  rows: 1,
  errors: [],
  warnings: ["2행: 3학년 1반 5번 이름 불일치 — 가져오기 완료됨 (파일: '이순신', DB: '홍길동')"],
}

vi.mock('axios', () => {
  const res = (url = '') => {
    const u = String(url)
    const data =
      /numeric-table\/list/.test(u) ? NUMERIC_ROWS
      : /category-map\/list/.test(u) ? { rows: [{ id: 21, area_id: 2, track_id: null, category: '무단결석 0회', score: 10 }], total: 1, page: 1, per_page: 50 }
      : /base-data\/list/.test(u)   ? BASE_ROWS
      : /\/api\/areas$/.test(u)     ? AREAS
      : /\/api\/rounds/.test(u)     ? []
      : []
    return Promise.resolve({ data, headers: {} })
  }
  const post = (url = '') =>
    /base-data\/import/.test(String(url))
      ? Promise.resolve({ data: IMPORT_WITH_WARNING, headers: {} })
      : res(url)
  const axios = { get: res, post, put: res, patch: res, delete: res,
    interceptors: { request: { use: () => {} }, response: { use: () => {} } } }
  return { default: axios, ...axios }
})

const load = () => import('../src/components/admin/AreasTab.vue')

/** 요소 목록에서 이름으로 하나를 고른다. */
async function pickArea(wrapper, name) {
  const row = wrapper.findAll('.cursor-pointer').find(d => d.text().includes(name))
  expect(row, `[${name}] 요소가 목록에 없다`).toBeTruthy()
  await row.trigger('click')
  await settle()
}

describe('전형요소 화면 — 요소를 골라 점수표까지', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.spyOn(console, 'warn').mockImplementation(() => {})
  })
  afterEach(() => { vi.restoreAllMocks() })

  it('요소 목록과 만점 합계가 그려진다', async () => {
    const wrapper = mount((await load()).default)
    await settle()

    const t = wrapper.text()
    for (const a of AREAS) expect(t, `[${a.name}] 이 목록에 없다`).toContain(a.name)
    // 40 + 10 + 50 = 100. totalMaxScore(logic/areaTotals.js)가 화면에 닿는 유일한 지점이다.
    expect(t, '만점 합계가 틀리거나 안 그려졌다').toContain('100점')
    wrapper.unmount()
  })

  it('NUMERIC 요소를 고르면 점수표가 열린다', async () => {
    const wrapper = mount((await load()).default)
    await settle()
    await pickArea(wrapper, '교과성적')

    const t = wrapper.text()
    expect(t, '기본 정보가 안 보인다').toContain('교과성적')
    // 점수표 행이 실제로 그려져야 한다 — "등록된 점수 기준 없음" 이면 안 된다.
    expect(t, '점수 기준이 비어 있다고 나온다 — 픽스처가 닿지 않았다')
      .not.toContain('등록된 점수 기준 없음')
    // **'40' 은 쓰지 않는다** — 만점 표기·양식 예시·설명문에도 있어 점수 열을 비워도
    // 통과했다(6차 감사 중-1). 점수표에만 나오는 값으로 고른다.
    expect(t, '점수표 기준값이 안 보인다').toContain('1.5')
    expect(t, '점수표 두 번째 행이 안 보인다').toContain('2')
    expect(t, '점수 열이 비어 있다').toContain('35')
    wrapper.unmount()
  })

  it('CATEGORY 요소는 범주 목록을 보여 준다', async () => {
    // 같은 패널이 calc_type 에 따라 다른 표를 그린다. 하나만 보면 나머지가 깨져도 모른다.
    const wrapper = mount((await load()).default)
    await settle()
    await pickArea(wrapper, '출결')

    const t = wrapper.text()
    expect(t, '범주 값이 안 보인다').toContain('무단결석 0회')
    // `/최대|MAX/` 는 점수 기준 탭의 고정 안내문 "소수점 최대 5자리" 에 항상 걸린다 —
    // 범주 집계 span 을 통째로 지워도 통과했다. 라벨과 값을 함께 본다.
    expect(t, '범주 집계 라벨이 없다').toContain('범주 집계')
    wrapper.unmount()
  })

  it('MANUAL 요소는 점수표 대신 기초데이터 탭으로 연다', async () => {
    // selectArea 가 calc_type 으로 첫 탭을 고른다(AreasTab.vue:945).
    // 이 분기가 뒤집히면 MANUAL 요소에서 빈 점수표가 열린다.
    const wrapper = mount((await load()).default)
    await settle()
    await pickArea(wrapper, '교사추천')

    // 부정문 하나만 두면 **오른쪽 패널이 통째로 비어도** 통과한다(6차 감사 중-1).
    // 무엇이 열렸는지 긍정으로 단언한다.
    const t = wrapper.text()
    expect(t, 'MANUAL 인데 점수 기준 화면이 열렸다').not.toContain('등록된 점수 기준 없음')
    expect(t, '오른쪽 패널이 비었다 — 기초 데이터 탭이 열려야 한다').toContain('교사추천')
    expect(t, '기초 데이터 탭이 아니다').toContain('기초 데이터')
    wrapper.unmount()
  })

  it('요소를 바꾸면 표도 함께 바뀐다', async () => {
    // 선택만 바뀌고 표가 안 따라오면 **이전 요소의 점수표를 보면서 편집**하게 된다.
    const wrapper = mount((await load()).default)
    await settle()

    await pickArea(wrapper, '교과성적')
    expect(wrapper.text()).toContain('1.5')

    await pickArea(wrapper, '출결')
    const t = wrapper.text()
    expect(t, '요소를 바꿨는데 이전 점수표가 남아 있다').not.toContain('1.5')
    expect(t).toContain('무단결석 0회')
    wrapper.unmount()
  })
})

/**
 * 가져오기 결과 상자 — **경고만 있을 때 눈에 띄는가**.
 *
 * 이름 불일치 경고는 "행이 한 칸 밀린 파일"의 신호라 관리자가 반드시 봐야 한다.
 * 그런데 `ImportResultBox` 는 오류가 없으면 무조건 초록 성공 상자였고, 경고 글자도
 * 초록이라 "완료" 옆에 묻혔다. 색을 소스에서 grep 하는 방식으로는 이 배선을 지킬 수
 * 없다(13_frontend_pitfalls "소스 텍스트로 배선을 지키려 하지 마라") — 실제로 파일을
 * 올려 결과 상자가 그려지는 것까지 본다.
 */
describe('가져오기 결과 — 경고만 있는 경우', () => {
  beforeEach(() => { setActivePinia(createPinia()); localStorage.clear() })
  afterEach(() => { vi.restoreAllMocks() })

  /** 숨겨진 파일 입력에 파일을 물려 change 를 쏜다. `files` 는 읽기 전용이라 정의해 준다. */
  async function uploadFile(wrapper) {
    const input = wrapper.findAll('input[type="file"]')
      .find(i => (i.attributes('accept') ?? '').includes('.csv'))
    expect(input, '기초데이터 업로드 입력칸이 없다').toBeTruthy()
    const file = new File(['학년,반,번호,이름,값\n3,1,5,이순신,4.5\n'], 'base.csv',
                          { type: 'text/csv' })
    Object.defineProperty(input.element, 'files', { value: [file], configurable: true })
    await input.trigger('change')
    await settle()
    return wrapper
  }

  it('경고가 화면에 그려지고, 성공 초록이 아니라 주의 색으로 뜬다', async () => {
    const wrapper = mount((await load()).default)
    await settle()
    await pickArea(wrapper, '교사추천')   // MANUAL — 기초 데이터 탭이 바로 열린다
    await settle()
    await uploadFile(wrapper)

    // ① 경고 문구가 실제로 화면에 있다. 백엔드가 보낸 것이 그대로 닿아야 한다.
    const t = wrapper.text()
    expect(t, '경고가 화면에 안 보인다 — 응답은 왔는데 그려지지 않았다').toContain('이름 불일치')
    expect(t, '어느 학생인지 알 수 있어야 한다').toContain('이순신')
    expect(t, '가져오기 자체는 완료됐다고 알려야 한다').toContain('완료')

    // ② 성공 초록이 아니라 주의 색이다. 색을 직접 보는 이유는, 문구만 보면
    //    "초록 상자 안의 초록 글씨"로 되돌아가도 통과하기 때문이다.
    const box = wrapper.findAll('div').find(d => d.text().includes('이름 불일치')
      && (d.attributes('style') ?? '').includes('border-radius: 12px'))
    expect(box, '결과 상자를 찾지 못했다').toBeTruthy()
    // jsdom 은 style 속성의 hex 를 rgb() 로 정규화한다. 소스의 `#fffbeb` 를 그대로
    // 찾으면 영영 못 찾고, 그 실패는 "색이 틀렸다"처럼 보인다. 변환값으로 단언한다.
    const style = box.attributes('style') ?? ''
    expect(style, `경고만 있는데 성공 초록(#f0fdf4)으로 그려졌다: ${style}`)
      .not.toContain('rgb(240, 253, 244)')
    expect(style, `주의 색(#fffbeb)이 아니다: ${style}`).toContain('rgb(255, 251, 235)')
    wrapper.unmount()
  })
})
