import { describe, it, expect } from 'vitest'
import { sortStudents } from './studentOrder.js'

const s = (seq_no, student_code) => ({ seq_no, student_code, name: `학생${seq_no}` })

describe('sortStudents', () => {
  it('일반 담임은 번호순으로 본다', () => {
    const out = sortStudents([s(3, 'c'), s(1, 'a'), s(2, 'b')], false)
    expect(out.map(x => x.seq_no)).toEqual([1, 2, 3])
  })

  it('특수 계정(grade=0)은 학번순으로 본다', () => {
    // 여러 학급이 섞여 오므로 번호는 의미가 없다.
    const out = sortStudents([s(1, '2026003'), s(1, '2026001'), s(2, '2026002')], true)
    expect(out.map(x => x.student_code)).toEqual(['2026001', '2026002', '2026003'])
  })

  it('번호 없는 학생은 항상 뒤로 — 999 로 지어내지 않는다', () => {
    // `seq_no ?? 999` 였을 때는 실제 999번 학생과 섞였다.
    const out = sortStudents([s(null, 'x'), s(999, 'y'), s(1, 'z')], false)
    expect(out.map(x => x.seq_no)).toEqual([1, 999, null])
  })

  // 위 단건은 **입력 순서에 따라 우연히 통과할 수 있다**(변이 FM-12 로 실증됐다 —
  // 비교 함수가 모순돼도 3원소 삽입 정렬이 같은 결과를 냈다). 불변식은 "어떤 순서로
  // 들어와도 null 은 뒤"이므로 순열 전수로 고정한다.
  it('입력 순서가 어떻든 번호 없는 학생은 맨 뒤에 모인다', () => {
    const permutations = (xs) => xs.length <= 1 ? [xs] : xs.flatMap(
      (x, i) => permutations([...xs.slice(0, i), ...xs.slice(i + 1)]).map(p => [x, ...p]))

    for (const input of permutations([s(1, 'a'), s(999, 'b'), s(null, 'c'), s(null, 'd')])) {
      const got = sortStudents(input, false).map(x => x.seq_no)
      const firstNull = got.indexOf(null)
      expect(got.slice(0, firstNull), JSON.stringify(got)).toEqual([1, 999])
      expect(got.slice(firstNull), JSON.stringify(got)).toEqual([null, null])
    }
  })

  it('번호가 전부 없어도 순서를 뒤집지 않는다', () => {
    const out = sortStudents([s(null, 'a'), s(null, 'b')], false)
    expect(out.map(x => x.student_code)).toEqual(['a', 'b'])
  })

  // 학번 누락에 대한 테스트는 두지 않았다. 코드는 `?? ''` 로 뭉개지 않지만(규칙 2),
  // 실제로 터지는지는 **비교 함수의 인자 순서에 달려 있다** — 누락된 쪽이 b 로 오면
  // `'2026001'.localeCompare(undefined)` 가 "undefined" 로 강제 변환돼 통과한다.
  // 엔진의 정렬 구현에 기대는 단언이 되므로, 초록을 내면서 아무것도 막지 못한다.
  // 학번 NOT NULL 은 스키마(002-students.sql)와 백엔드가 지키는 몫이다.

  it('원본 배열을 바꾸지 않는다', () => {
    // computed 안에서 부르므로 입력을 제자리 정렬하면 반응성이 꼬인다.
    const input = [s(2, 'b'), s(1, 'a')]
    sortStudents(input, false)
    expect(input.map(x => x.seq_no)).toEqual([2, 1])
  })
})
