/**
 * 담임 화면의 "저장하지 않은 입력" 확인을 탭 밖(로그아웃 버튼)에서도 쓰게 한다.
 *
 * 로그아웃은 인증을 지운 뒤 이동하는데, 인증이 지워진 뒤엔 ApplicationTab 의
 * onBeforeRouteLeave 가 확인 없이 통과한다(이슈 #49 — 막으면 학년·반 null 화면에 남는다).
 * 그래서 로그아웃 버튼은 인증을 지우기 **전에** 이 확인을 먼저 부른다.
 */
let current = null

/** 탭이 마운트될 때 확인 함수를 등록하고, 내려갈 때 해제 함수를 부른다. */
export function registerUnsavedGuard(fn) {
  current = fn
  return () => { if (current === fn) current = null }
}

/** 등록된 확인이 없으면 true. 있으면 그 결과(떠나도 되면 true). */
export async function confirmDiscardUnsaved() {
  return current ? await current() : true
}
