import { describe, it, expect } from 'vitest'
import { formatScore, isKeyMatched } from './scorePreviewShared.js'

// 이 두 함수는 tools/oracle/front_check.mjs 가 BigInt 독립 오라클로 전수 대조한다.
// 여기서는 오라클이 다루지 않는 **경계·비정상 입력**을 고정한다 — 오라클은 정상
// 도메인(±10억, 소수 5자리)만 훑기 때문이다.

describe('formatScore', () => {
  it('값 없음은 계산 결과 0 이 아니라 - 로 구분한다', () => {
    // 0 점과 "아직 없음"이 같아 보이면 화면만 보고는 미입력을 알 수 없다.
    expect(formatScore(null)).toBe('-')
    expect(formatScore(undefined)).toBe('-')
  })

  it('유한하지 않은 값은 - 로 떨어뜨린다', () => {
    expect(formatScore(NaN)).toBe('-')
    expect(formatScore(Infinity)).toBe('-')
    expect(formatScore(-Infinity)).toBe('-')
    expect(formatScore('abc')).toBe('-')
  })

  // 변이 검사 기록(2026-09-21, 2026-09-21 정정):
  // `n % 1 === 0` 분기를 없애는 변이(FM-2)는 **점수 도메인 안에서만** 등가다.
  //
  // 처음에는 "정수면 언제나 등가"라고 적었는데 **거짓이었다.** `toFixed` 는
  // |n| >= 1e21 에서 지수 표기를 내고, 지수가 0 으로 끝나면 `/\.?0+$/` 가 그 0 을
  // 먹는다 — 1e30 -> "1e+3", 1e100 -> "1e+1", 1.5e30 -> "1.5e+3".
  // 당시 표본에 넣은 유일한 지수 범위 값이 하필 1e21 이었고, 지수가 "1" 로 끝나
  // 반례를 비켜 갔다. 즉 이 분기는 가독성용이 아니라 **지수 표기 절단을 막는
  // 장치**다.
  //
  // 그럼에도 아래에 |n| >= 1e21 단언을 넣지 않는 이유는 따로 있다: 점수는 ±10억
  // 이하로 제한되고(src/docs/12_verification.md), 표시에 오는 값이 1e21 을 넘으면
  // 그건 표기 문제가 아니라 데이터가 이미 망가진 것이다. 도메인 밖 동작을 테스트로
  // 고정하면 오히려 그 상태를 정상인 양 박제한다.
  //
  // **정리: 도메인 안에서는 등가라 FM-2 가 안 잡히는 것이 정상이고, 도메인 밖에서는
  // 등가가 아니므로 분기를 지우지 마라.**
  it('정수는 소수점을 붙이지 않는다', () => {
    expect(formatScore(0)).toBe('0')
    expect(formatScore(7)).toBe('7')
    expect(formatScore(-3)).toBe('-3')
    expect(formatScore(1000)).toBe('1000')
  })

  it('소수점 5자리까지 잘리지 않는다', () => {
    // 순위는 5자리로 갈리는데 표시가 2자리에서 끊기면
    // "점수 같은데 왜 순위가 다르지?"가 된다.
    expect(formatScore(0.00001)).toBe('0.00001')
    expect(formatScore(88.12345)).toBe('88.12345')
    expect(formatScore(88.12)).toBe('88.12')
    expect(formatScore(-0.00001)).toBe('-0.00001')
  })

  it('뒤따르는 0 은 떼어 값과 표시를 일치시킨다', () => {
    expect(formatScore(88.1)).toBe('88.1')
    expect(formatScore(88.10000)).toBe('88.1')
    expect(formatScore(0.5)).toBe('0.5')
  })

  it('문자열 숫자도 받는다 (API 가 문자열로 내려오는 경로)', () => {
    expect(formatScore('88.5')).toBe('88.5')
    expect(formatScore('12')).toBe('12')
  })
})

describe('isKeyMatched', () => {
  it('매칭 키가 없으면 어떤 행도 강조하지 않는다', () => {
    expect(isKeyMatched('NUMERIC', [], 1)).toBe(false)
    expect(isKeyMatched('NUMERIC', null, 1)).toBe(false)
    expect(isKeyMatched('NUMERIC', undefined, 1)).toBe(false)
    expect(isKeyMatched('CATEGORY', [], 'A')).toBe(false)
  })

  it('NUMERIC 은 부동소수점 오차만 흡수하고 다른 구간은 잡지 않는다', () => {
    expect(isKeyMatched('NUMERIC', [1.5], 1.5)).toBe(true)
    // 0.1+0.2 = 0.30000000000000004 — 같은 구간으로 봐야 한다
    expect(isKeyMatched('NUMERIC', [0.1 + 0.2], 0.3)).toBe(true)
    // 점수 최소 단위(1e-5)만큼 떨어진 구간은 별개다
    expect(isKeyMatched('NUMERIC', [1.50001], 1.5)).toBe(false)
    expect(isKeyMatched('NUMERIC', [2], 1.5)).toBe(false)
  })

  it('NUMERIC 허용 오차는 1e-9 경계에서 갈린다', () => {
    expect(isKeyMatched('NUMERIC', [1 + 1e-10], 1)).toBe(true)
    expect(isKeyMatched('NUMERIC', [1 + 1e-8], 1)).toBe(false)
  })

  it('NUMERIC 에 문자열 키가 섞여 오면 매칭하지 않는다', () => {
    // 근접 비교를 문자열에 적용하면 '1.5' 가 조용히 통과한다.
    expect(isKeyMatched('NUMERIC', ['1.5'], 1.5)).toBe(false)
    expect(isKeyMatched('NUMERIC', ['1.5', 1.5], 1.5)).toBe(true)
  })

  it('NUMERIC 이 아니면 문자열 정확 일치다', () => {
    expect(isKeyMatched('CATEGORY', ['A', 'B'], 'A')).toBe(true)
    expect(isKeyMatched('CATEGORY', ['A', 'B'], 'C')).toBe(false)
    // 부분 문자열로 번지지 않는다
    expect(isKeyMatched('CATEGORY', ['AB'], 'A')).toBe(false)
  })
})
