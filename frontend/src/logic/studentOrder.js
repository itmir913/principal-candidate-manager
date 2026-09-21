/**
 * 담임 화면의 학생 나열 순서 — 순수 함수.
 *
 * ResultsTab.vue 의 computed 안에 있던 것을 옮기면서 **`seq_no ?? 999` 를 없앴다.**
 * 그 표현은 번호가 없는 학생을 조용히 999번으로 취급한다 — 실제 999번 학생이 있으면
 * 순서가 섞이고, 화면만 보고는 원인을 알 수 없다. Fail-Fast(CLAUDE.md 규칙 2)의
 * 금지 대상인 silent fallback 이며, `src/docs/silent_fallback_allowed.md` 의 허용
 * 목록에도 없다(그 목록에 프론트 항목은 하나도 없다).
 *
 * 대신 **번호 없는 학생은 항상 뒤로** 보낸다. 값을 지어내지 않으므로 999번 학생과
 * 섞이지 않고, 뒤에 모여 있으니 누락이 눈에 띈다.
 *
 * 참고: 재학생은 스키마상 `seq_no` 가 NOT NULL 이라(002-students.sql) 정상 경로에서는
 * null 이 오지 않는다. `tools/oracle/front_check.mjs` §5 가 이를 계속 확인한다.
 * 그래도 방어를 남기는 이유는, 값이 없을 때 **틀린 순서를 조용히 만들지 않기** 위해서다.
 */

/**
 * @param {Array}   students        { student_code, seq_no, ... }
 * @param {boolean} byStudentCode   특수 계정(grade=0)은 학번순, 일반 담임은 번호순
 */
export function sortStudents(students, byStudentCode) {
  return [...students].sort((a, b) => {
    if (byStudentCode) {
      // `?? ''` 를 붙이지 않는다 — 학번은 students 스키마상 NOT NULL 이고,
      // 없는데도 빈 문자열로 뭉개면 그 학생이 조용히 맨 앞에 선다. 없으면 터뜨려서
      // 데이터가 잘못됐음을 알리는 편이 낫다(규칙 2).
      return a.student_code.localeCompare(b.student_code)
    }
    // 번호 없는 학생은 뒤로 — 임의의 번호를 지어내지 않는다.
    if (a.seq_no == null && b.seq_no == null) return 0
    if (a.seq_no == null) return 1
    if (b.seq_no == null) return -1
    return a.seq_no - b.seq_no
  })
}
