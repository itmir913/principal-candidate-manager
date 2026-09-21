import { flushPromises } from '@vue/test-utils'

/**
 * 비동기 렌더가 **가라앉을 때까지** 흘려보낸다.
 *
 * 왜 헬퍼인가: 이 저장소의 화면은 `onMounted` 안에서 axios 를 여러 겹 연쇄로 기다린다
 * (라운드 조회 → 그 id 로 결과 조회 → 확정 현황 조회 …). `await new Promise(r => setTimeout(r, 0))`
 * 를 몇 번 적을지 세는 방식은 연쇄가 한 겹만 깊어져도 **조용히 부족해진다** —
 * 렌더가 덜 끝난 상태로 단언하면 그 뒤에 날 오류를 못 보고 초록이 뜬다.
 *
 * `flushPromises()` 는 매크로태스크 한 번 + 그 사이 마이크로태스크 전부를 흘린다.
 * 넉넉히 반복해 두면 겹이 늘어도 테스트를 고칠 필요가 없다.
 */
export const settle = async (rounds = 5) => {
  for (let i = 0; i < rounds; i += 1) await flushPromises()
}
