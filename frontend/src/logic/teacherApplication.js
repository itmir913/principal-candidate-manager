/**
 * 담임 지원 등록 폼의 저장 가능 조건 — 순수 함수.
 *
 * ApplicationTab.vue 의 `canSave` computed 를 로직 변경 없이 옮겼다.
 * 저장 버튼의 잠금 조건이므로, 조건이 느슨해지면 **미완성 지원이 서버로 간다.**
 * `every` 를 `some` 으로 바꾸는 종류의 실수를 테스트로 막는다.
 *
 * 호출 규약: computed 안에서 `ref.value` / reactive 필드를 읽어 넘긴다.
 */

/**
 * @param {object}  p
 * @param {boolean} p.hasStudent        선택된 학생이 있는가
 * @param {*}       p.trackId           선택된 모집단위
 * @param {boolean} p.hasRound          현재 라운드가 있는가
 * @param {string}  p.departmentName    학과명 (공백만이면 안 된다)
 * @param {Array}   p.areaContext       전형요소 목록
 * @param {object}  p.areaValues        area_id → 단일 값
 * @param {object}  p.areaMultiValues   area_id → 값 배열
 */
export function canSaveApplication({
  hasStudent, trackId, hasRound, departmentName,
  areaContext, areaValues, areaMultiValues,
}) {
  if (!hasStudent || !trackId || !hasRound || !departmentName?.trim()) return false
  // 전형요소가 하나도 없으면 점수를 낼 수 없다 — 저장할 것이 없다.
  if (areaContext.length === 0) return false
  return areaContext.every(area => {
    if (area.teacher_editable) {
      if (area.multi_value) {
        return (areaMultiValues[area.area_id] || []).length > 0
      }
      const v = areaValues[area.area_id]
      return v !== undefined && v !== ''
    }
    // 관리자 입력 고정: 서버에서 받은 current_values 가 있어야 한다.
    return area.current_values.some(v => v !== '')
  })
}
