import { describe, it, expect } from 'vitest'
import { ROUND_STATUS_LABELS, roundStatusLabel } from './roundStatus.js'

describe('roundStatusLabel', () => {
  it('세 상태의 표기는 단일 출처에서 온다', () => {
    // 화면마다 다르게 적으면 관리자와 담임이 같은 상태를 두고 다른 말을 본다.
    expect(roundStatusLabel('OPEN')).toBe('진행중')
    expect(roundStatusLabel('CLOSED')).toBe('종료')
    expect(roundStatusLabel('FINALIZED')).toBe('마감')
  })

  it('백엔드 RoundStatus 세 변형을 빠짐없이 덮는다', () => {
    // src/enums.rs 의 RoundStatus = Open | Closed | Finalized.
    //
    // 주의: 이 테스트는 **Rust 를 읽지 않는다.** 여기 적힌 세 값은 손으로 옮긴
    // 것이라, `src/enums.rs` 에 변형이 늘어도 이 단언은 조용히 통과한다.
    // 백엔드와의 실제 대조는 tests/round_status_labels.rs 가 한다 —
    // 그쪽이 `include_str!` 로 두 파일을 함께 읽는다.
    // 여기서 고정하는 것은 "세 키가 사라지지 않았다"까지다.
    expect(Object.keys(ROUND_STATUS_LABELS).sort())
      .toEqual(['CLOSED', 'FINALIZED', 'OPEN'])
  })

  it('모르는 상태값은 감추지 않고 원문을 보여준다', () => {
    // 조용히 빈칸이 되면 화면만 보고는 원인을 추적할 수 없다(Fail-Fast).
    expect(roundStatusLabel('ARCHIVED')).toBe('ARCHIVED')
    expect(roundStatusLabel('open')).toBe('open')
  })

  it('값이 없을 때도 빈 문자열로 뭉개지 않는다', () => {
    expect(roundStatusLabel(undefined)).toBeUndefined()
    expect(roundStatusLabel(null)).toBeNull()
  })
})
