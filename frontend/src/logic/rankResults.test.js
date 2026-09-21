import { describe, it, expect } from 'vitest'
import {
  filterByTrack, computeTieSet, groupByTrack, groupByUniv, sortGroups,
  univAutoButtonKeys, buildResultsView,
} from './rankResults.js'

// 결과 행 한 건. 필요한 필드만 채운다.
const row = (o) => ({
  student_id: 1, track_id: 10, round_id: 1,
  univ_name: '가대학', track_name: '가모집단위',
  ranking: 1, track_rank: 1, ...o,
})

describe('filterByTrack', () => {
  const rows = [row({ track_id: 10 }), row({ student_id: 2, track_id: 20 })]

  it('필터가 없으면 전체를 그대로 준다', () => {
    expect(filterByTrack(rows, null)).toBe(rows)
    expect(filterByTrack(rows, '')).toBe(rows)
  })

  it('모집단위로 좁힌다 — 드롭다운이 주는 문자열 id 도 받는다', () => {
    expect(filterByTrack(rows, 20).map(r => r.student_id)).toEqual([2])
    expect(filterByTrack(rows, '20').map(r => r.student_id)).toEqual([2])
  })
})

describe('computeTieSet', () => {
  it('동점이 없으면 빈 집합이다', () => {
    const rows = [
      row({ student_id: 1, ranking: 1, track_rank: 1 }),
      row({ student_id: 2, ranking: 2, track_rank: 2 }),
    ]
    expect(computeTieSet(rows, 'univ').size).toBe(0)
    expect(computeTieSet(rows, 'track').size).toBe(0)
  })

  it('같은 순위 2인은 양쪽 다 표시한다', () => {
    const rows = [
      row({ student_id: 1, ranking: 1 }),
      row({ student_id: 2, ranking: 1 }),
      row({ student_id: 3, ranking: 3 }),
    ]
    const tie = computeTieSet(rows, 'univ')
    expect([...tie].sort()).toEqual(['1-10', '2-10'])
  })

  it('3인 동점도 전원 표시한다', () => {
    const rows = [1, 2, 3].map(id => row({ student_id: id, ranking: 1 }))
    expect(computeTieSet(rows, 'univ').size).toBe(3)
  })

  it('순위가 없는 행은 동점으로 치지 않는다', () => {
    // 미선발 등으로 순위가 비어 있는 행끼리 "동점"이 되면 안 된다.
    const rows = [
      row({ student_id: 1, ranking: null }),
      row({ student_id: 2, ranking: null }),
    ]
    expect(computeTieSet(rows, 'univ').size).toBe(0)
  })

  it('라운드가 다르면 같은 순위여도 동점이 아니다', () => {
    const rows = [
      row({ student_id: 1, round_id: 1, ranking: 1 }),
      row({ student_id: 2, round_id: 2, ranking: 1 }),
    ]
    expect(computeTieSet(rows, 'univ').size).toBe(0)
  })

  it('대학이 다르면 같은 순위여도 동점이 아니다', () => {
    const rows = [
      row({ student_id: 1, univ_name: '가대학', ranking: 1 }),
      row({ student_id: 2, univ_name: '나대학', ranking: 1 }),
    ]
    expect(computeTieSet(rows, 'univ').size).toBe(0)
  })

  it('track 보기는 모집단위 안에서만 동점을 본다', () => {
    const rows = [
      row({ student_id: 1, track_id: 10, track_rank: 1 }),
      row({ student_id: 2, track_id: 20, track_rank: 1 }),
    ]
    expect(computeTieSet(rows, 'track').size).toBe(0)
    expect(computeTieSet([
      row({ student_id: 1, track_id: 10, track_rank: 1 }),
      row({ student_id: 2, track_id: 10, track_rank: 1 }),
    ], 'track').size).toBe(2)
  })

  it('보기에 따라 다른 순위 필드를 본다', () => {
    // univ 보기는 ranking, track 보기는 track_rank.
    const rows = [
      row({ student_id: 1, track_id: 10, ranking: 1, track_rank: 1 }),
      row({ student_id: 2, track_id: 20, ranking: 1, track_rank: 2 }),
    ]
    expect(computeTieSet(rows, 'univ').size).toBe(2)   // 같은 대학, 같은 ranking
    expect(computeTieSet(rows, 'track').size).toBe(0)  // 모집단위가 다르다
  })

  // F-014 의 본질 — 이것이 깨지면 필터를 걸 때 동점 표식이 사라진다.
  it('필터 이전 전체로 계산해야 다른 모집단위의 동점 상대가 살아 있다', () => {
    const all = [
      row({ student_id: 1, track_id: 10, ranking: 1 }),
      row({ student_id: 2, track_id: 20, ranking: 1 }),  // 같은 대학, 다른 모집단위
    ]
    expect(computeTieSet(all, 'univ').has('1-10')).toBe(true)
    // 걸러진 배열을 넘기면 상대가 없어 동점이 사라진다 — 그래서 컴포넌트는
    // results(전체)를 넘겨야 한다.
    expect(computeTieSet(filterByTrack(all, 10), 'univ').has('1-10')).toBe(false)
  })
})

describe('groupByUniv / groupByTrack', () => {
  const quotaMap = {
    10: { univId: 1, univName: '가대학', unitQuota: 2, unitUsed: 1, totalQuota: 5, totalUsed: 3 },
    20: { univId: 1, univName: '가대학', unitQuota: null, unitUsed: 0, totalQuota: 5, totalUsed: 3 },
  }

  it('대학 보기는 대학 이름으로 묶고 ranking 오름차순으로 세운다', () => {
    const rows = [
      row({ student_id: 3, ranking: 3 }),
      row({ student_id: 1, ranking: 1 }),
      row({ student_id: 2, ranking: 2 }),
    ]
    const g = groupByUniv(rows, quotaMap)
    expect(g['가대학'].results.map(r => r.ranking)).toEqual([1, 2, 3])
  })

  it('순위 없는 행은 항상 뒤로 간다', () => {
    const rows = [
      row({ student_id: 1, ranking: null }),
      row({ student_id: 2, ranking: 2 }),
    ]
    expect(groupByUniv(rows, quotaMap)['가대학'].results.map(r => r.student_id))
      .toEqual([2, 1])
  })

  it('모집단위 보기는 track_rank 로 세운다', () => {
    // ranking 순서를 쓰면 재학생우선 설정이 다를 때 표시 번호가 3,1,2 로 어긋난다.
    const rows = [
      row({ student_id: 1, ranking: 1, track_rank: 2 }),
      row({ student_id: 2, ranking: 2, track_rank: 1 }),
    ]
    const g = groupByTrack(rows, quotaMap)
    expect(g['가대학 가모집단위'].results.map(r => r.student_id)).toEqual([2, 1])
  })

  it('잔여석은 음수로 내려가지 않는다', () => {
    const over = { 10: { univId: 1, unitQuota: 1, unitUsed: 5, totalQuota: 2, totalUsed: 9 } }
    const g = groupByTrack([row({})], over)
    expect(g['가대학 가모집단위'].remaining).toBe(0)
    expect(g['가대학 가모집단위'].univRemaining).toBe(0)
  })

  it('정원이 무제한(null)이면 잔여석도 null 이다', () => {
    // 0 으로 내리면 "자리 없음"으로 보인다 — 무제한과 만석은 다르다.
    const g = groupByTrack([row({ track_id: 20, track_name: '나모집단위' })], quotaMap)
    expect(g['가대학 나모집단위'].remaining).toBeNull()
  })

  it('정원 정보가 없는 모집단위도 떨어뜨리지 않는다', () => {
    const g = groupByUniv([row({ track_id: 99 })], quotaMap)
    expect(g['가대학'].univId).toBeNull()
    expect(g['가대학'].results).toHaveLength(1)
  })
})

describe('sortGroups', () => {
  it('키를 한글 가나다순으로 다시 담는다', () => {
    // v-for 는 객체의 삽입 순서를 그대로 쓴다.
    const m = { '다대학': 1, '가대학': 2, '나대학': 3 }
    expect(Object.keys(sortGroups(m))).toEqual(['가대학', '나대학', '다대학'])
  })

  it('값은 건드리지 않는다', () => {
    const m = { '나': { results: [1] }, '가': { results: [2] } }
    expect(sortGroups(m)['가'].results).toEqual([2])
  })
})

// ── 배선 ─────────────────────────────────────────────────────────────────
// 여기부터가 F-014 의 진짜 방어선이다. computeTieSet 은 어떤 배열을 받아도 옳게
// 동작하므로, "걸러진 배열을 넘겼다"는 잘못은 그 함수의 테스트로 잡을 수 없다.
// 예전에는 .vue 를 정규식으로 훑는 소스 가드가 맡았는데, 감사에서 `.filter()`
// 체이닝과 loadResults 안의 필터링을 둘 다 놓치고 줄바꿈 하나에는 오탐으로
// 터지는 것이 드러났다. 이제 배선이 함수 안에 있으므로 행동으로 단언한다.
describe('buildResultsView', () => {
  const quotaMap = {
    10: { univId: 1, unitQuota: 2, unitUsed: 0, totalQuota: 5, totalUsed: 0 },
    20: { univId: 1, unitQuota: 2, unitUsed: 0, totalQuota: 5, totalUsed: 0 },
    30: { univId: 2, unitQuota: 1, unitUsed: 0, totalQuota: 1, totalUsed: 0 },
  }
  // 같은 대학(가대학) 안에서 모집단위가 다른 두 학생이 대학 순위 1위로 동점이다.
  const rows = [
    row({ student_id: 1, track_id: 10, univ_name: '가대학', track_name: '가', ranking: 1, track_rank: 1 }),
    row({ student_id: 2, track_id: 20, univ_name: '가대학', track_name: '나', ranking: 1, track_rank: 1 }),
    row({ student_id: 3, track_id: 30, univ_name: '나대학', track_name: '다', ranking: 1, track_rank: 1 }),
  ]

  it('모집단위 필터를 걸어도 대학 전체 동점 표식이 유지된다 (F-014)', () => {
    const all      = buildResultsView({ rows, trackId: null, view: 'univ', quotaMap })
    const filtered = buildResultsView({ rows, trackId: 10,   view: 'univ', quotaMap })

    // 표시는 줄어든다
    expect(Object.keys(filtered.groups)).toEqual(['가대학'])
    expect(filtered.groups['가대학'].results.map(r => r.student_id)).toEqual([1])
    // 동점 표식은 줄지 않는다 — 이것이 F-014 의 본질이다
    expect(filtered.tieSet).toEqual(all.tieSet)
    expect(filtered.tieSet.has('1-10')).toBe(true)
    expect(filtered.tieSet.has('2-20')).toBe(true)
  })

  it('어떤 모집단위로 걸러도 tieSet 은 전체와 같다', () => {
    const all = buildResultsView({ rows, trackId: null, view: 'univ', quotaMap })
    for (const tid of [10, 20, 30, '10', '20']) {
      const one = buildResultsView({ rows, trackId: tid, view: 'univ', quotaMap })
      expect(one.tieSet, `trackId=${tid}`).toEqual(all.tieSet)
    }
  })

  it('track 보기에서도 필터가 동점 표식을 줄이지 않는다', () => {
    const all      = buildResultsView({ rows, trackId: null, view: 'track', quotaMap })
    const filtered = buildResultsView({ rows, trackId: 10,   view: 'track', quotaMap })
    expect(filtered.tieSet).toEqual(all.tieSet)
  })

  // 위와 짝을 이루는 반대 방향 단언. tieSet 만 보면 "표시도 안 좁히는" 변이가
  // 빠져나간다(실측: 그룹을 rows 로 만들어도 tieSet 테스트는 전부 통과했다).
  it.each(['univ', 'track'])('%s 보기에서 필터는 표시를 실제로 좁힌다', (view) => {
    const all      = buildResultsView({ rows, trackId: null, view, quotaMap })
    const filtered = buildResultsView({ rows, trackId: 10,   view, quotaMap })

    const count = (v) => Object.values(v.groups).reduce((n, g) => n + g.results.length, 0)
    expect(count(all)).toBe(3)
    expect(count(filtered)).toBe(1)
    expect(Object.values(filtered.groups).flatMap(g => g.results).map(r => r.track_id))
      .toEqual([10])
  })

  it('보기에 따라 그룹 키가 달라진다', () => {
    const univ  = buildResultsView({ rows, trackId: null, view: 'univ',  quotaMap })
    const track = buildResultsView({ rows, trackId: null, view: 'track', quotaMap })
    expect(Object.keys(univ.groups)).toEqual(['가대학', '나대학'])
    expect(Object.keys(track.groups)).toEqual(['가대학 가', '가대학 나', '나대학 다'])
  })

  it('그룹 키는 가나다순이다', () => {
    const shuffled = [rows[2], rows[1], rows[0]]
    const v = buildResultsView({ rows: shuffled, trackId: null, view: 'univ', quotaMap })
    expect(Object.keys(v.groups)).toEqual(['가대학', '나대학'])
  })
})

describe('univAutoButtonKeys', () => {
  // 자동 추천은 되돌리기 어려운 행위다. 버튼이 사라지면 관리자가 자동추천을 못 쓰고,
  // 모집단위마다 반복되면 "이 모집단위만 처리"로 오해한다.
  it('대학마다 첫 그룹에만 버튼을 준다', () => {
    const groups = {
      '가대학 가': { univId: 1 },
      '가대학 나': { univId: 1 },
      '나대학 다': { univId: 2 },
    }
    expect([...univAutoButtonKeys(groups)]).toEqual(['가대학 가', '나대학 다'])
  })

  it('어느 대학인지 모르는 그룹은 제외한다', () => {
    // univId 를 모르는 채로 "대학 전체 확정"을 누르게 하면 안 된다.
    expect([...univAutoButtonKeys({ '가대학': { univId: null } })]).toEqual([])
  })

  it('대학이 하나면 키도 하나다', () => {
    const v = buildResultsView({
      rows: [row({ track_id: 10 }), row({ student_id: 2, track_id: 20, track_name: '나' })],
      trackId: null, view: 'track',
      quotaMap: { 10: { univId: 1 }, 20: { univId: 1 } },
    })
    expect(v.autoButtonKeys.size).toBe(1)
  })
})

describe('byRank 의 비교 함수가 모순되지 않는다', () => {
  // FM-12 의 교훈: 단건 배열은 비교 함수가 모순돼도 우연히 통과한다.
  // studentOrder 에는 순열 전수를 붙였으면서 같은 종류인 여기에는 빠져 있었다.
  const permutations = (xs) => xs.length <= 1 ? [xs] : xs.flatMap(
    (x, i) => permutations([...xs.slice(0, i), ...xs.slice(i + 1)]).map(p => [x, ...p]))

  it('어떤 입력 순서에서도 순위 없는 행이 뒤에 모인다', () => {
    const input = [
      row({ student_id: 1, ranking: 2 }),
      row({ student_id: 2, ranking: 1 }),
      row({ student_id: 3, ranking: null }),
      row({ student_id: 4, ranking: null }),
    ]
    for (const perm of permutations(input)) {
      const got = groupByUniv(perm, {})['가대학'].results.map(r => r.ranking)
      expect(got.slice(0, 2), JSON.stringify(got)).toEqual([1, 2])
      expect(got.slice(2), JSON.stringify(got)).toEqual([null, null])
    }
  })
})

describe('groupBy* 가 대학 이름을 잃지 않는다', () => {
  it('univName 은 확인 대화상자 문구에 쓰이므로 비면 안 된다', () => {
    // handleAutoRecommendUniv 가 `${group.univName}의 모집단위만…` 으로 묻는다.
    // 비어 버리면 되돌리기 어려운 행위를 이름 없이 확인받게 된다.
    const g = groupByUniv([row({ univ_name: '가대학' })], {})
    expect(g['가대학'].univName).toBe('가대학')
    const t = groupByTrack([row({ univ_name: '가대학', track_name: '가' })], {})
    expect(t['가대학 가'].univName).toBe('가대학')
  })
})
