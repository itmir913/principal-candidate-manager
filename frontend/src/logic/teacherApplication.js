/**
 * 담임 지원 등록 폼의 저장 가능 조건 — 순수 함수.
 *
 * ApplicationTab.vue 의 `canSave` computed 를 옮겼다. **한 곳이 달라졌다** —
 * `departmentName` 의 옵셔널 체이닝(아래 해당 줄 주석). 나머지는 원본과 동치다.
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
  // `?.` 는 원본(`form.departmentName.trim()`)에 없던 것이다 — 엄밀히 말해 이동이
  // 아니라 동작 변경이다(undefined 에서 throw → false). 호출부의 `form` 이
  // `reactive({ departmentName: '' })` 라 도달할 수 없는 경로지만, 저장 버튼 잠금
  // 조건이 예외로 터지면 화면이 통째로 멈추므로 여기서는 막는 쪽을 택했다.
  // 값을 지어내는 폴백이 아니라 "조건 미충족"으로 떨어뜨리는 것이라 규칙 2 와 어긋나지
  // 않는다. (감사 지적 — 커밋 f1ae466 의 "로직 변경 없이 옮겼다" 는 이 한 곳에서 부정확했다.)
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
