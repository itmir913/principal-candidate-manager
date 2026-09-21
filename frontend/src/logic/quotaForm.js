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

/**
 * 정원 입력칸의 문자열을 값으로 바꾼다. **F-013 의 본체가 여기다.**
 *
 * 값을 보정하지 않는다 — 유효하지 않으면 `null` 을 올려보내 저장 버튼이 잠기게 한다.
 * 예전 `parseInt(v) || 1` 은 두 가지를 조용히 저질렀다:
 *   - `0` 을 `1` 로 바꿨다(관리자가 모른다)
 *   - 음수를 그대로 통과시켰다
 * `parseInt` 는 `"5.7"` → 5, `"2e3"` → **2** 로 잘라낸다. `type=number` 입력칸은 이 둘을
 * 유효한 입력으로 받으므로 조용한 절단이 된다. 그래서 `Number` 로 통째 해석한다 —
 * `"2e3"` 은 2000 이 되고(의도한 값), `"5.7"` 은 정수가 아니므로 거부된다.
 *
 * 1 이상인지는 여기서 보지 않는다 — 그건 `isQuotaValid` 의 몫이고, 입력 중간 상태
 * (`0` 을 지우고 다시 치는 중)까지 막으면 타이핑이 불가능해진다. 대신 정수가 아닌 것은
 * 여기서 `null` 로 떨어뜨려 저장을 막는다.
 *
 * 이 함수가 컴포넌트 안에 있는 동안에는 어떤 테스트도 닿지 않았고,
 * `Number(s) || 1` 로 되돌리는 변이가 전 검증을 통과했다(2026-09-21 감사 중-10).
 *
 * @param {string} raw 입력칸의 원본 문자열
 * @returns {number|null}
 */
export function parseQuotaInput(raw) {
  const s = String(raw ?? '').trim()
  const n = s === '' ? NaN : Number(s)
  if (!Number.isInteger(n)) return null
  // 백엔드 정원은 i64 다. `1e21` 같은 값은 `Number.isInteger` 를 통과하지만 serde 가
  // 거부하므로, 프론트가 "유효"라고 말해 놓고 저장이 실패한다. 여기서 먼저 막는다.
  if (!Number.isSafeInteger(n)) return null
  return n
}

export function isQuotaValid(form, key) {
  return form.unlimited || (Number.isInteger(form[key]) && form[key] >= 1)
}

export function isUnivFormValid(form) {
  return form.univ_name.trim() !== '' && isQuotaValid(form, 'total_quota')
}

export function isTrackFormValid(form) {
  return form.track_name.trim() !== '' && isQuotaValid(form, 'unit_quota')
}
