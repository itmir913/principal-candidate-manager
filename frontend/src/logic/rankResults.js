/**
 * 결과 표(관리자 [라운드] 탭)의 파생 계산 — Vue 에 기대지 않는 순수 함수.
 *
 * RoundsTab.vue 의 computed 안에 있던 것을 옮겼다. 태그 `0.2.21` 시점 원본과 줄 단위로
 * 대조해 동치임을 확인했다(2026-09-21 코드 감사). 옮긴 이유는
 * `.vue` 안에 있는 동안에는 테스트가 불가능해, `tools/oracle/front_check.mjs` 가
 * 같은 코드를 **손으로 복사**해 대조해야 했기 때문이다. 복사본은 원본이 바뀌어도
 * 초록이라 조용히 낡는다(실제로 낡았다).
 *
 * **`?? null` / `?? 0` 폴백에 대하여**: 아래 그룹 함수들이 쓰는 폴백은 원본 컴포넌트에서
 * 그대로 옮겨 온 것이고, 이 파일에서 새로 판단한 것이 아니다. 같은 커밋이 `seq_no ?? 999`
 * 는 지웠는데 여기는 남긴 것이 비일관으로 보일 수 있어 근거를 적어 둔다 —
 * `seq_no` 는 **정렬 순서를 조용히 틀리게** 만들지만, 여기 폴백은 정원 정보가 아직 안
 * 온 그룹을 "정원 없음(null)"으로 표시할 뿐 순위·추천에 쓰이지 않는다. 프론트 전체에
 * 같은 꼴이 19곳 있으므로, 정리한다면 이 세 곳만이 아니라 한 번에 봐야 한다(별건).
 *
 * 호출 규약 — 반응성은 호출하는 쪽 책임이다:
 *   - 반드시 computed **안에서** `ref.value` 를 읽어 넘긴다. ref 자체를 넘기면
 *     의존 추적이 끊기는데, 순수 함수 테스트는 그대로 통과하므로 드러나지 않는다.
 *   - `computeTieSet` 에는 **필터 이전 전체 배열**을 넘긴다. 자세한 이유는 아래.
 */

/**
 * 표시용 모집단위 필터.
 * `results` 는 항상 라운드 전체이고, 좁히는 것은 여기뿐이다.
 */
export function filterByTrack(rows, trackId) {
  if (!trackId) return rows
  const tid = Number(trackId)
  return rows.filter(r => r.track_id === tid)
}

/**
 * 동점 표식 집합 — `${student_id}-${track_id}` 의 Set.
 *
 * **반드시 필터 이전 전체 배열로 계산한다(F-014).** 대학 순위 보기의 동점 판정은
 * 같은 대학의 **다른 모집단위 지원자**까지 봐야 하는데, 걸러진 배열을 넘기면 그
 * 상대가 없어 동점 표식이 사라진다. 서버에 필터를 넘기지 않는 것도 같은 이유다
 * (RoundsTab.loadResults).
 *
 * @param {Array}  rows  라운드 전체 결과 (visibleResults 아님)
 * @param {'track'|'univ'} view
 */
export function computeTieSet(rows, view) {
  const set = new Set()
  const counts = {}
  for (const r of rows) {
    // 순위가 없는 행(미선발 등)은 동점 판정 대상이 아니다.
    const rank = view === 'track' ? r.track_rank : r.ranking
    if (rank == null) continue
    // 라운드가 다르면 같은 순위여도 동점이 아니다.
    const scope = view === 'track' ? r.track_id : r.univ_name
    const k = `${scope}-${r.round_id}-${rank}`
    if (!counts[k]) counts[k] = []
    counts[k].push(r)
  }
  for (const rows_ of Object.values(counts)) {
    if (rows_.length > 1) for (const r of rows_) set.add(`${r.student_id}-${r.track_id}`)
  }
  return set
}

/** ranking / track_rank 오름차순, 값 없는 행은 뒤로. */
function byRank(key) {
  return (a, b) => {
    if (a[key] == null && b[key] == null) return 0
    if (a[key] == null) return 1
    if (b[key] == null) return -1
    return a[key] - b[key]
  }
}

/**
 * 모집단위별 보기 — `${대학} ${모집단위}` 키로 묶는다.
 * 표시하는 순위 숫자는 `track_rank` 다. `ranking`(대학 전체 순위) 순서를 그대로
 * 쓰면 대학과 모집단위의 재학생우선 설정이 다를 때 표시 번호가 3,1,2 로 어긋난다.
 */
export function groupByTrack(rows, quotaMap) {
  const map = {}
  for (const r of rows) {
    const key = `${r.univ_name} ${r.track_name}`
    if (!map[key]) {
      const q = quotaMap[r.track_id]
      const unitQuota = q?.unitQuota ?? null
      const totalQuota = q?.totalQuota ?? null
      map[key] = {
        univId: q?.univId ?? null,
        univName: r.univ_name,
        unitQuota,
        totalQuota,
        remaining: unitQuota != null ? Math.max(0, unitQuota - (q?.unitUsed ?? 0)) : null,
        univRemaining: totalQuota != null ? Math.max(0, totalQuota - (q?.totalUsed ?? 0)) : null,
        results: [],
      }
    }
    map[key].results.push(r)
  }
  for (const g of Object.values(map)) g.results.sort(byRank('track_rank'))
  return map
}

/** 대학 전체 순위 보기 — 대학 이름으로 묶고 `ranking` 으로 세운다. */
export function groupByUniv(rows, quotaMap) {
  const map = {}
  for (const r of rows) {
    const key = r.univ_name
    if (!map[key]) {
      const q = quotaMap[r.track_id]
      const totalQuota = q?.totalQuota ?? null
      map[key] = {
        univId: q?.univId ?? null,
        univName: r.univ_name,
        totalQuota,
        univRemaining: totalQuota != null ? Math.max(0, totalQuota - (q?.totalUsed ?? 0)) : null,
        results: [],
      }
    }
    map[key].results.push(r)
  }
  for (const g of Object.values(map)) g.results.sort(byRank('ranking'))
  return map
}

/**
 * 대학 가나다 → (모집단위별 보기에서는) 모집단위 가나다.
 * v-for 는 객체 키의 삽입 순서를 그대로 쓰므로, 백엔드 ORDER BY 에 기대지 않고
 * 여기서 키 순서를 확정한다.
 */
export function sortGroups(map) {
  return Object.fromEntries(
    Object.entries(map).sort(([a], [b]) => a.localeCompare(b, 'ko'))
  )
}

/**
 * 대학별 자동 추천 버튼을 띄울 그룹 키.
 *
 * 자동 추천은 **대학 단위** 동작이다. 모집단위별 보기에서는 그룹이 모집단위마다 나뉘므로
 * 각 대학의 첫 그룹에만 노출한다 — 같은 버튼이 모집단위 수만큼 반복되면
 * "이 모집단위만 처리"로 오해된다. 정원 정보가 없어 `univId` 를 모르는 그룹은 제외한다
 * (어느 대학인지 모르는 채로 대학 전체를 확정할 수는 없다).
 */
export function univAutoButtonKeys(groups) {
  const seen = new Set()
  const keys = new Set()
  for (const [key, g] of Object.entries(groups)) {
    if (g.univId == null || seen.has(g.univId)) continue
    seen.add(g.univId)
    keys.add(key)
  }
  return keys
}

/**
 * `GET /api/universities/quota-stats` 응답을 `groupBy*` 가 쓰는 형태로 바꾼다.
 *
 * 응답은 대학 → 모집단위 중첩인데, 그룹 함수는 `track_id` 로 바로 찾는다.
 * 잔여석 표시 전체가 이 매핑 하나에 달려 있다 — 여기서 대학 정원을 모집단위 정원
 * 자리에 넣거나 키를 잘못 잡으면 "남은 자리"가 통째로 틀리는데, 화면은 멀쩡해 보인다.
 *
 * 응답이 아직 없으면(첫 렌더) 빈 map 을 준다. 그룹 함수는 정원 정보가 없는
 * 모집단위를 "정원 없음"으로 표시하고 행은 떨어뜨리지 않는다.
 */
export function buildTrackQuotaMap(quotaStats) {
  const map = {}
  if (!quotaStats) return map
  for (const u of quotaStats.univs) {
    for (const t of u.tracks) {
      map[t.track_id] = {
        univId: u.univ_id,
        univName: u.univ_name,
        unitQuota: t.unit_quota,
        unitUsed: t.unit_used,
        // 대학 단위 값 — 모집단위 값과 섞이면 잔여석이 조용히 틀린다.
        totalQuota: u.total_quota,
        totalUsed: u.total_used,
      }
    }
  }
  return map
}

/**
 * 결과 탭이 화면에 쓰는 값 **전부**를 한 번에 만든다.
 *
 * 왜 하나로 묶었나 — 여기서 잡으려는 회귀는 개별 함수의 버그가 아니라 **배선**이다.
 * `computeTieSet` 은 어떤 배열을 받아도 옳게 동작하므로, "걸러진 배열을 넘겼다"는
 * 잘못은 그 함수의 테스트로는 영원히 잡히지 않는다. 예전에는 `.vue` 본문을 정규식으로
 * 훑는 소스 가드로 막으려 했으나(F-014 대응), 감사에서 `.filter()` 체이닝과
 * `loadResults` 안에서의 필터링을 둘 다 놓치고 줄바꿈 하나에는 오탐으로 터지는 것이
 * 드러났다. 배선을 함수 안으로 넣으면 **행동으로** 단언할 수 있다:
 * "trackId 를 줘도 tieSet 은 줄지 않는다."
 *
 * @param {object} p
 * @param {Array}  p.rows      라운드 **전체** 결과. 여기서 걸러 넣지 마라.
 * @param {*}      p.trackId   표시용 모집단위 필터 (없으면 전체)
 * @param {'track'|'univ'} p.view
 * @param {object} p.quotaMap  track_id → 정원 정보
 * @returns {{ groups: object, tieSet: Set<string>, autoButtonKeys: Set<string> }}
 */
export function buildResultsView({ rows, trackId, view, quotaMap }) {
  const visible = filterByTrack(rows, trackId)
  const grouped = view === 'track'
    ? groupByTrack(visible, quotaMap)
    : groupByUniv(visible, quotaMap)
  const groups = sortGroups(grouped)

  return {
    groups,
    // 동점 판정만 rows(필터 이전)를 쓴다 — 같은 대학 다른 모집단위의 동점 상대가
    // 걸러지면 표식이 사라진다(F-014).
    tieSet: computeTieSet(rows, view),
    autoButtonKeys: univAutoButtonKeys(groups),
  }
}
