import { describe, it, expect } from 'vitest'
import { canSaveApplication } from './teacherApplication.js'

// 담임이 입력하는 요소 하나 + 관리자 고정 요소 하나로 이루어진 기본 상황.
const base = () => ({
  hasStudent: true,
  trackId: 10,
  hasRound: true,
  departmentName: '컴퓨터공학과',
  areaContext: [
    { area_id: 1, teacher_editable: true,  multi_value: false, current_values: [] },
    { area_id: 2, teacher_editable: false, multi_value: false, current_values: ['3.2'] },
  ],
  areaValues: { 1: '88' },
  areaMultiValues: {},
})

describe('canSaveApplication', () => {
  it('모든 조건이 갖춰지면 저장할 수 있다', () => {
    expect(canSaveApplication(base())).toBe(true)
  })

  it.each([
    ['학생 미선택',   { hasStudent: false }],
    ['모집단위 미선택', { trackId: null }],
    ['라운드 없음',   { hasRound: false }],
    ['학과명 없음',   { departmentName: '' }],
    ['학과명 공백만', { departmentName: '   ' }],
  ])('%s 이면 저장할 수 없다', (_, patch) => {
    expect(canSaveApplication({ ...base(), ...patch })).toBe(false)
  })

  it('학과명이 undefined 여도 터지지 않는다', () => {
    expect(canSaveApplication({ ...base(), departmentName: undefined })).toBe(false)
  })

  it('전형요소가 하나도 없으면 저장할 수 없다', () => {
    // 점수를 낼 수 없는 지원이 서버로 가면 안 된다.
    expect(canSaveApplication({ ...base(), areaContext: [] })).toBe(false)
  })

  it('담임 입력 요소가 하나라도 비면 저장할 수 없다', () => {
    // every 를 some 으로 바꾸면 여기서 걸린다 — 부분 입력이 통과하면 안 된다.
    expect(canSaveApplication({ ...base(), areaValues: {} })).toBe(false)
    expect(canSaveApplication({ ...base(), areaValues: { 1: '' } })).toBe(false)
  })

  it('0 은 비어 있는 값이 아니다', () => {
    // 0점을 미입력으로 보면 정당한 지원이 막힌다.
    expect(canSaveApplication({ ...base(), areaValues: { 1: 0 } })).toBe(true)
    expect(canSaveApplication({ ...base(), areaValues: { 1: '0' } })).toBe(true)
  })

  it('다중값 요소는 최소 한 개가 필요하다', () => {
    const multi = {
      ...base(),
      areaContext: [{ area_id: 1, teacher_editable: true, multi_value: true, current_values: [] }],
      areaValues: {},
      areaMultiValues: {},
    }
    expect(canSaveApplication(multi)).toBe(false)
    expect(canSaveApplication({ ...multi, areaMultiValues: { 1: [] } })).toBe(false)
    expect(canSaveApplication({ ...multi, areaMultiValues: { 1: ['A'] } })).toBe(true)
  })

  it('관리자 고정 요소는 서버가 준 값이 있어야 한다', () => {
    const p = base()
    p.areaContext[1].current_values = []
    expect(canSaveApplication(p)).toBe(false)
    p.areaContext[1].current_values = ['']
    expect(canSaveApplication(p)).toBe(false)
  })
})
