import { describe, it, expect } from 'vitest'
import {
  filterByTrack, computeTieSet, groupByTrack, groupByUniv, sortGroups,
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
