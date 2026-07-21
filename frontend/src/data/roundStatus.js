/**
 * 라운드 상태의 한글 표기 — 단일 출처.
 *
 * 화면마다 다르게 적으면 같은 상태를 두고 관리자와 담임이 서로 다른 말을 보게
 * 되고("종료" vs "집계중"), 매뉴얼도 어느 쪽에 맞출지 정할 수 없다.
 * 실제로 네 화면이 제각각이던 것을 이 파일로 모았다.
 *
 * 매뉴얼(`frontend/public/sections/*.html`)도 이 표기를 따른다 — 바꾸면
 * 매뉴얼도 함께 고쳐야 한다.
 */
export const ROUND_STATUS_LABELS = {
  OPEN:      '진행중',
  CLOSED:    '종료',
  FINALIZED: '마감',
}

/**
 * 모르는 상태값은 감추지 않고 원문을 그대로 보여준다.
 * 조용히 빈칸이 되면 화면만 보고는 원인을 추적할 수 없다.
 */
export function roundStatusLabel(status) {
  return ROUND_STATUS_LABELS[status] ?? status
}
