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
