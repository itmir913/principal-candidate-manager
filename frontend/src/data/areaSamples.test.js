import { describe, it, expect } from 'vitest'
import { getScoreExample, getBaseExample } from './areaSamples.js'

// src/enums.rs 의 변형을 그대로 옮긴 것. 백엔드에 변형이 늘면 이 목록도 늘려야 하고,
// 늘리는 순간 대응 예시가 없으면 아래 전수 검사가 실패한다.
const CALC_TYPES    = ['NUMERIC', 'CATEGORY', 'MANUAL']
const MATCH_MODES   = ['UPPER', 'LOWER', 'EXACT']
const CATEGORY_AGGS = ['SUM', 'MAX']
const SCOPES        = ['SIMPLE', 'COMPOSITE']

describe('getScoreExample', () => {
  // SCORE_EXAMPLES 에 키가 없으면 `ex.desc` 에서 TypeError 로 터진다 —
  // 폴백이 없는 함수라 "undefined 를 반환하지 않는가"가 아니라
  // "터지지 않는가"를 물어야 실제로 무언가를 막는다.
  it('가능한 모든 전형요소 조합에서 예시를 만든다', () => {
    const missing = []
    for (const calc_type of CALC_TYPES) {
      for (const lookup_scope of SCOPES) {
        for (const match_mode of [...MATCH_MODES, null]) {
          for (const category_agg of [...CATEGORY_AGGS, null]) {
            const area = { calc_type, lookup_scope, match_mode, category_agg }
            try {
              const ex = getScoreExample(area)
              if (!ex?.desc || !Array.isArray(ex.headers) || !Array.isArray(ex.rows)) {
                missing.push(`${JSON.stringify(area)} → 빈 예시`)
              }
            } catch (e) {
              missing.push(`${JSON.stringify(area)} → ${e.message}`)
            }
          }
        }
      }
    }
    expect(missing).toEqual([])
  })

  it('match_mode 가 없으면 UPPER 로 본다', () => {
    const withNull = getScoreExample({ calc_type: 'NUMERIC', lookup_scope: 'SIMPLE', match_mode: null })
    const upper    = getScoreExample({ calc_type: 'NUMERIC', lookup_scope: 'SIMPLE', match_mode: 'UPPER' })
    expect(withNull.desc).toBe(upper.desc)
  })

  it('category_agg 가 없으면 MAX 로 본다', () => {
    const withNull = getScoreExample({ calc_type: 'CATEGORY', lookup_scope: 'SIMPLE', category_agg: null })
    const max      = getScoreExample({ calc_type: 'CATEGORY', lookup_scope: 'SIMPLE', category_agg: 'MAX' })
    expect(withNull.desc).toBe(max.desc)
  })

  it('COMPOSITE 는 SIMPLE 과 다른 예시를 준다', () => {
    // 같은 예시가 나오면 조합형 안내가 사실상 없는 것이다.
    for (const calc_type of CALC_TYPES) {
      const simple    = getScoreExample({ calc_type, lookup_scope: 'SIMPLE' })
      const composite = getScoreExample({ calc_type, lookup_scope: 'COMPOSITE' })
      expect(composite.desc).not.toBe(simple.desc)
    }
  })
})

describe('getBaseExample', () => {
  it('재학생·졸업생 모든 조합에서 예시를 만든다', () => {
    for (const calc_type of [...CALC_TYPES, null]) {
      for (const lookup_scope of SCOPES) {
        for (const studentType of ['enrolled', 'graduated']) {
          const ex = getBaseExample({ calc_type, lookup_scope }, studentType)
          expect(ex.desc, `${calc_type}/${lookup_scope}/${studentType}`).toBeTruthy()
          expect(Array.isArray(ex.headers)).toBe(true)
        }
      }
    }
  })

  it('studentType 기본값은 졸업생이다', () => {
    const area = { calc_type: 'NUMERIC', lookup_scope: 'SIMPLE' }
    expect(getBaseExample(area).headers).toEqual(getBaseExample(area, 'graduated').headers)
  })

  it('재학생과 졸업생의 양식 열이 다르다', () => {
    // 재학생 양식에는 학년·반·번호가 들어간다 — 같은 열이면 업로드 안내가 틀린다.
    const area = { calc_type: 'NUMERIC', lookup_scope: 'SIMPLE' }
    expect(getBaseExample(area, 'enrolled').headers)
      .not.toEqual(getBaseExample(area, 'graduated').headers)
  })
})
