/**
 * 대학·모집단위 폼의 정원 유효성 — 순수 함수.
 *
 * UniversitiesTab.vue 에 있던 것을 로직 변경 없이 옮겼다.
 *
 * **F-013 의 교훈**: 예전 입력 처리는 `parseInt(v) || 1` 이라 0 을 조용히 1 로
 * 바꾸고 음수는 그대로 통과시켰다. UI 는 값을 고쳐 주는 대신 **저장을 막아야 한다**
 * (CLAUDE.md 규칙 2 Fail-Fast). 그래서 여기서는 값을 보정하지 않고 참/거짓만 답한다.
 *
 * 기준은 백엔드 `validate_quota` 와 같다 — 1 이상의 정수, 또는 무제한.
 */

export function isQuotaValid(form, key) {
  return form.unlimited || (Number.isInteger(form[key]) && form[key] >= 1)
}

export function isUnivFormValid(form) {
  return form.univ_name.trim() !== '' && isQuotaValid(form, 'total_quota')
}

export function isTrackFormValid(form) {
  return form.track_name.trim() !== '' && isQuotaValid(form, 'unit_quota')
}
