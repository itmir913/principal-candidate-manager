import { describe, it, expect } from 'vitest'
import { totalMaxScore } from './areaTotals.js'

// 정밀도(부동소수점 합산이 표시와 어긋나지 않는가)는 tools/oracle/front_check.mjs §6 이
// BigInt 정확값과 대조한다 — 이제 그쪽도 **이 함수를 그대로 호출한다.**
// 여기서는 오라클이 다루지 않는 경계만 고정한다.
describe('totalMaxScore', () => {
  it('전형요소가 없으면 0 이다', () => {
    // '-' 나 undefined 가 되면 "총점 -점" 이 뜬다.
    expect(totalMaxScore([])).toBe(0)
  })

  it('만점을 모두 더한다', () => {
    expect(totalMaxScore([{ max_score: 40 }, { max_score: 35 }, { max_score: 25 }])).toBe(100)
  })

  it('소수 만점도 더한다', () => {
    expect(totalMaxScore([{ max_score: 12.5 }, { max_score: 7.25 }])).toBe(19.75)
  })

  it('0점짜리 요소를 빠뜨리지 않는다', () => {
    // CATEGORY 0점 강제 경로가 있어 0 은 정상 값이다.
    expect(totalMaxScore([{ max_score: 0 }, { max_score: 10 }])).toBe(10)
  })
})
