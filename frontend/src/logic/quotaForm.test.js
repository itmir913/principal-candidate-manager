import { describe, it, expect } from 'vitest'
import { isQuotaValid, isUnivFormValid, isTrackFormValid } from './quotaForm.js'

describe('isQuotaValid', () => {
  it('1 이상의 정수만 통과한다', () => {
    expect(isQuotaValid({ total_quota: 1, unlimited: false }, 'total_quota')).toBe(true)
    expect(isQuotaValid({ total_quota: 50, unlimited: false }, 'total_quota')).toBe(true)
  })

  it('0 과 음수는 막는다 — 1 로 고쳐 주지 않는다', () => {
    // F-013: 예전 `parseInt(v) || 1` 은 0 을 조용히 1 로 바꿨다.
    expect(isQuotaValid({ total_quota: 0, unlimited: false }, 'total_quota')).toBe(false)
    expect(isQuotaValid({ total_quota: -1, unlimited: false }, 'total_quota')).toBe(false)
  })

  it('소수·NaN·빈값은 막는다', () => {
    expect(isQuotaValid({ total_quota: 1.5, unlimited: false }, 'total_quota')).toBe(false)
    expect(isQuotaValid({ total_quota: NaN, unlimited: false }, 'total_quota')).toBe(false)
    expect(isQuotaValid({ total_quota: null, unlimited: false }, 'total_quota')).toBe(false)
    expect(isQuotaValid({ total_quota: undefined, unlimited: false }, 'total_quota')).toBe(false)
    expect(isQuotaValid({ total_quota: '5', unlimited: false }, 'total_quota')).toBe(false)
  })

  it('무제한이면 정원 값은 보지 않는다', () => {
    expect(isQuotaValid({ total_quota: null, unlimited: true }, 'total_quota')).toBe(true)
    expect(isQuotaValid({ total_quota: 0, unlimited: true }, 'total_quota')).toBe(true)
  })
})

describe('isUnivFormValid / isTrackFormValid', () => {
  it('이름과 정원이 둘 다 유효해야 저장 버튼이 열린다', () => {
    expect(isUnivFormValid({ univ_name: '가대학', total_quota: 5, unlimited: false })).toBe(true)
    expect(isTrackFormValid({ track_name: '가모집단위', unit_quota: 2, unlimited: false })).toBe(true)
  })

  it('이름이 비거나 공백뿐이면 막는다', () => {
    expect(isUnivFormValid({ univ_name: '', total_quota: 5, unlimited: false })).toBe(false)
    expect(isUnivFormValid({ univ_name: '  ', total_quota: 5, unlimited: false })).toBe(false)
    expect(isTrackFormValid({ track_name: '  ', unit_quota: 2, unlimited: false })).toBe(false)
  })

  it('이름이 멀쩡해도 정원이 틀리면 막는다', () => {
    expect(isUnivFormValid({ univ_name: '가대학', total_quota: 0, unlimited: false })).toBe(false)
    expect(isTrackFormValid({ track_name: '가모집단위', unit_quota: -3, unlimited: false })).toBe(false)
  })

  it('대학은 total_quota, 모집단위는 unit_quota 를 본다', () => {
    // 서로 다른 필드를 봐야 한다 — 바뀌면 한쪽 검증이 통째로 사라진다.
    expect(isUnivFormValid({ univ_name: '가', total_quota: 0, unit_quota: 5, unlimited: false })).toBe(false)
    expect(isTrackFormValid({ track_name: '가', total_quota: 0, unit_quota: 5, unlimited: false })).toBe(true)
  })
})
