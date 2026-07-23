/**
 * 점수표 행 하이라이팅 판정 유틸.
 * NUMERIC: threshold(f64)와 rowKey 근접 비교 (부동소수점 오차 허용).
 * 그 외: 문자열 includes 비교.
 */
export function isKeyMatched(calcType, matchedKeys, rowKey) {
  if (!matchedKeys?.length) return false
  if (calcType === 'NUMERIC') {
    return matchedKeys.some(mk => typeof mk === 'number' && Math.abs(mk - rowKey) < 1e-9)
  }
  return matchedKeys.includes(rowKey)
}

/**
 * 점수 표시 포맷 — 백엔드가 내려준 값을 **정밀도 손실 없이** 문자열로 만든다.
 *
 * 백엔드는 점수를 소수 5자리까지 허용한다(Score = ×100000 정수). `toFixed(2)`로
 * 2자리에서 끊으면, 순위는 5자리로 갈리는데 화면엔 두 학생이 같은 점수로 보여
 * "점수 같은데 왜 순위가 다르지?"가 된다. 정수면 정수로, 소수면 유효 자리까지만
 * 보이도록 뒤 0을 떼어 값과 표시를 일치시킨다. (AreasTab의 기존 패턴을 공용화.)
 *
 * 계산이 아니라 표시 전용 — CLAUDE.md 규칙 1(프론트에서 ÷100000 금지)과 무관하다.
 * 백엔드 Score가 이미 나눈 값을 f64로 내려주므로 여기서는 자릿수만 다듬는다.
 */
export function formatScore(v) {
  if (v === null || v === undefined) return '-'
  const n = Number(v)
  if (!Number.isFinite(n)) return '-'
  return n % 1 === 0 ? String(n) : n.toFixed(5).replace(/\.?0+$/, '')
}
