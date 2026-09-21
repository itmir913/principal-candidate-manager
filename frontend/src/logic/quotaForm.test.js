import { describe, it, expect } from 'vitest'
import { isQuotaValid, isUnivFormValid, isTrackFormValid, parseQuotaInput } from './quotaForm.js'

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

describe('parseQuotaInput — F-013 의 본체', () => {
  it('정수 문자열은 숫자로 바꾼다', () => {
    expect(parseQuotaInput('5')).toBe(5)
    expect(parseQuotaInput(' 12 ')).toBe(12)
  })

  it('0 을 1 로 바꾸지 않는다', () => {
    // 예전 `parseInt(v) || 1` 은 0 을 조용히 1 로 만들었다. 관리자는 모른다.
    // 0 은 그대로 0 으로 올려보내고, 저장은 isQuotaValid 가 막는다.
    expect(parseQuotaInput('0')).toBe(0)
    expect(isQuotaValid({ total_quota: parseQuotaInput('0'), unlimited: false }, 'total_quota'))
      .toBe(false)
  })

  it('음수도 그대로 올려보내고 저장만 막는다', () => {
    // `|| 1` 은 음수를 통과시켰다(-3 은 truthy 다).
    expect(parseQuotaInput('-3')).toBe(-3)
    expect(isQuotaValid({ total_quota: -3, unlimited: false }, 'total_quota')).toBe(false)
  })

  it('소수는 잘라내지 않고 거부한다', () => {
    // parseInt 였다면 "5.7" -> 5 로 **조용히 절단**된다. type=number 입력칸은
    // 소수를 유효한 입력으로 받으므로 반드시 막아야 한다.
    expect(parseQuotaInput('5.7')).toBeNull()
    expect(parseQuotaInput('1.0')).toBe(1)   // 값이 정수와 같으면 받는다
  })

  it('지수 표기는 값 그대로 읽는다 — 절단하지 않는다', () => {
    // 여기가 parseInt 대신 Number 를 쓰는 이유다.
    // parseInt('2e3') 은 **2** 를 준다(관리자는 2000 을 의도했다).
    expect(parseQuotaInput('2e3')).toBe(2000)
  })

  it('빈 입력과 공백은 null 이다', () => {
    // `Number('')` 은 0 이다 — 지우는 중인 칸이 0 으로 확정되면 안 된다.
    expect(parseQuotaInput('')).toBeNull()
    expect(parseQuotaInput('   ')).toBeNull()
    expect(parseQuotaInput(null)).toBeNull()
    expect(parseQuotaInput(undefined)).toBeNull()
  })

  it('i64 를 넘는 값은 거부한다', () => {
    // Number.isInteger(1e21) 은 참이다. 그대로 통과시키면 프론트가 "유효"라고 말한
    // 값을 백엔드 serde 가 거부한다 — 관리자는 이유 없이 저장에 실패한다.
    expect(parseQuotaInput('1e21')).toBeNull()
    expect(parseQuotaInput('9007199254740993')).toBeNull()   // 2^53 초과
    expect(parseQuotaInput('9007199254740991')).toBe(9007199254740991)
  })

  it('숫자가 아닌 것은 null 이다', () => {
    expect(parseQuotaInput('abc')).toBeNull()
    expect(parseQuotaInput('5명')).toBeNull()
    expect(parseQuotaInput('Infinity')).toBeNull()
    expect(parseQuotaInput('NaN')).toBeNull()
  })

  it('입력 -> 저장 가능 판정까지 이어 본다', () => {
    // 두 함수가 따로 옳아도 이어 붙였을 때 어긋나면 화면은 틀린다.
    const cases = [['3', true], ['0', false], ['-1', false], ['5.7', false], ['', false]]
    for (const [typed, canSave] of cases) {
      const form = { univ_name: '가대학', total_quota: parseQuotaInput(typed), unlimited: false }
      expect(isUnivFormValid(form), `입력 "${typed}"`).toBe(canSave)
    }
  })
})
