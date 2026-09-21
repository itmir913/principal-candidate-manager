/**
 * 결과 표(관리자 [라운드] 탭)의 파생 계산 — Vue 에 기대지 않는 순수 함수.
 *
 * RoundsTab.vue 의 computed 안에 있던 것을 **로직 변경 없이** 옮겼다. 옮긴 이유는
 * `.vue` 안에 있는 동안에는 테스트가 불가능해, `tools/oracle/front_check.mjs` 가
 * 같은 코드를 **손으로 복사**해 대조해야 했기 때문이다. 복사본은 원본이 바뀌어도
 * 초록이라 조용히 낡는다(실제로 낡았다).
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
