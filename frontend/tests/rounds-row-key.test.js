/**
 * 라운드 표의 행 키가 **학생+모집단위 복합**인지 — 실제 데이터로 드러난 결함이다.
 *
 * 브라우저에서 실측하다 콘솔에 이게 떴다(2026-09-22, 6차 라운드):
 *   `[Vue warn]: Duplicate keys found during update: 169  at <RoundsTab>`
 *
 * 원인: 한 학생이 **한 대학의 두 모집단위**에 지원하면, 대학별로 묶는 화면에서 그 학생의
 * 두 행이 같은 묶음에 들어간다. 키가 `student_id` 뿐이면 두 행의 키가 같아진다.
 * 중복 키는 Vue 가 갱신 때 DOM 노드를 잘못 재사용하게 만든다.
 * 이 결함은 리팩터가 만든 것이 아니다 — 태그 `0.2.21` 에도 같은 키였다.
 *
 * ## 이것은 **보조 검사**다. 한계를 정직하게 적는다.
 *
 * 행동 테스트를 먼저 시도했고 **실패했다.** RoundsTab 을 마운트해 같은 학생을 한 대학의
 * 두 모집단위에 두고, 라운드 재선택·[새로고침]·뷰 전환까지 눌러 봤지만 jsdom 에서는
 * Vue 가 **경고를 하나도 내지 않았고**(행은 2개 다 정상 렌더) 눈에 보이는 부작용도
 * 재현되지 않았다. 브라우저에서는 뜬다 — 그 경로가 실제 patch 로 가기 때문이다.
 * 그래서 키를 `student_id` 로 되돌리는 변이를 행동 테스트가 **잡지 못했다**.
 *
 * 판별하지 못하는 테스트를 남기면 방어선이 있다고 착각하게 된다. 그래서 그 테스트는
 * 버리고, 여기서는 **소스에서 키의 모양만** 본다.
 *
 * **한계를 정확히 적는다**: 키를 헬퍼나 변수로 빼면 이 검사는 통과가 아니라 **실패**한다
 * (캡처한 문자열에 두 이름이 없어지므로). 즉 조용히 뚫리는 쪽이 아니라 시끄럽게 틀리는
 * 쪽이다. 리팩터로 실패하면 이 주석을 읽고 검사를 함께 고쳐라 — 소스 텍스트 검사를
 * 쓰는 대가다. 반대로 두 이름을 모두 포함한 계산식이면 내용과 무관하게 통과한다.
 * (13_frontend_pitfalls "소스 텍스트로 배선을 지키려 하지 마라" 의 보조 계층.)
 *
 * ## 고침은 브라우저에서 전/후로 확인했다
 *
 * 같은 데이터(6차 라운드, 같은 학생이 경기대 두 모집단위에 지원)로 **라운드 카드 →
 * [결과] 탭** 경로를 밟았을 때:
 *   - 고치기 전: `Duplicate keys found during update: 169` 가 떴다.
 *   - 고친 뒤:   같은 경로에서 경고가 나지 않았다.
 * jsdom 이 아니라 실제 브라우저라야 이 경로가 patch 로 간다.
 */
import { describe, it, expect } from 'vitest'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'

const 읽기 = (rel) => readFileSync(join(process.cwd(), rel), 'utf8')

/**
 * 한 목록 안에서 **같은 학생이 두 번 나올 수 있는** 행 v-for 들.
 *
 * 두 가지 이유로 그렇게 된다:
 *   ① 대학별로 묶으면 한 학생의 여러 모집단위 지원이 한 묶음에 들어온다(RoundsTab).
 *   ② 라운드를 한정하지 않고 받아 오면 같은 모집단위 재지원이 겹친다(ClassTab —
 *      `teacherGetApplications()` 를 인자 없이 부르고, applications PK 는
 *      (student_id, track_id, round_id) 다).
 *
 * ①만 막고 ②를 놓쳤다가 감사에서 지적받았다. 쓰기 지점을 전수로 세는 것이 이 목록의 일이다.
 */
const 행_VFOR = [
  { 이름: 'RoundsTab 지원 현황 표', 파일: 'src/components/admin/RoundsTab.vue',
    패턴: /v-for="app in group"[\s\S]{0,400}?:key="([^"]+)"/s, 필요: ['student_id', 'track_id'] },
  { 이름: 'RoundsTab 결과 표', 파일: 'src/components/admin/RoundsTab.vue',
    패턴: /v-for="r in group\.results"[\s\S]{0,400}?:key="([^"]+)"/s, 필요: ['student_id', 'track_id'] },
  { 이름: 'ClassTab 학생별 지원 목록', 파일: 'src/components/teacher/ClassTab.vue',
    패턴: /v-for="app in getStudentApps\(s\.id\)"[\s\S]{0,400}?:key="([^"]+)"/s, 필요: ['round_id', 'track_id'] },
]

describe('같은 학생이 두 번 나올 수 있는 목록의 행 키는 복합이어야 한다 (보조)', () => {
  it.each(행_VFOR)('$이름', ({ 이름, 파일, 패턴, 필요 }) => {
    const m = 읽기(파일).match(패턴)
    expect(m, `${이름}의 v-for/:key 를 찾지 못했다 — 구조가 바뀌었다면 이 검사도 고쳐라`)
      .toBeTruthy()
    const key = m[1]
    for (const 조각 of 필요) {
      expect(key, `${이름}의 키에 ${조각} 이 없다: ${key}` +
        ' — 이 목록에서는 같은 학생이 두 번 나올 수 있어 단일 id 로는 키가 중복된다.')
        .toMatch(new RegExp(조각))
    }
  })
})
