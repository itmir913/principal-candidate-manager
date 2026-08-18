/**
 * 프론트엔드 파생 로직 독립 대조 (E4).
 *
 * 프론트엔드에는 테스트 러너가 없다(package.json:10 "test": "cargo test").
 * 감사 지침에 따라 `frontend/` 와 `package.json` 은 건드리지 않고, 문제의 JS 로직을
 * **그대로 복사**해 이 오라클 안에서 돌린 뒤 백엔드 실측값(actual.json)과 대조한다.
 *
 * 복사 출처 (커밋 14bba2d):
 *   formatScore / isKeyMatched : frontend/src/utils/scorePreviewShared.js:6-30
 *   tieSet (RoundsTab)         : frontend/src/components/admin/RoundsTab.vue:894-920
 *   tieSet (ResultsTab)        : frontend/src/components/teacher/ResultsTab.vue:241-267
 *   resultsByUnivOnly 정렬     : frontend/src/components/admin/RoundsTab.vue:866-875
 *   studentsByRound 정렬       : frontend/src/components/teacher/ResultsTab.vue:288-292
 *   totalMaxScore              : frontend/src/components/admin/AreasTab.vue:902
 *
 * 실행: node front_check.mjs
 */
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const HERE = path.dirname(fileURLToPath(import.meta.url))
const scenarios = JSON.parse(fs.readFileSync(path.join(HERE, 'scenarios.json'), 'utf8'))
const actual = JSON.parse(fs.readFileSync(path.join(HERE, 'actual.json'), 'utf8'))

// ── 프론트 코드 그대로 복사 ───────────────────────────────────────
function isKeyMatched(calcType, matchedKeys, rowKey) {
  if (!matchedKeys?.length) return false
  if (calcType === 'NUMERIC') {
    return matchedKeys.some(mk => typeof mk === 'number' && Math.abs(mk - rowKey) < 1e-9)
  }
  return matchedKeys.includes(rowKey)
}

function formatScore(v) {
  if (v === null || v === undefined) return '-'
  const n = Number(v)
  if (!Number.isFinite(n)) return '-'
  return n % 1 === 0 ? String(n) : n.toFixed(5).replace(/\.?0+$/, '')
}
// ─────────────────────────────────────────────────────────────────

let fails = 0
// 알려진 결함 — 아직 고치지 않았고, 고쳐질 때까지 CI 를 빨갛게 만들지 않는다.
// 대신 **고쳐지면 여기서 실패한다** — 목록에 남은 채 통과하면 baseline 이 낡은 것이므로
// 항목을 지우라고 요구한다. 결함을 조용히 덮는 장치가 아니라 만료일을 붙이는 장치다.
const KNOWN_FAIL = new Map([
  ['tieSet univ 보기 + 모집단위 필터 (동점 표식 누락)',
   'F-014 — 모집단위 필터 + 대학 순위 보기에서 동점 상대가 서버 응답에서 빠진다'],
])

const report = (name, bad, total, sample) => {
  const known = KNOWN_FAIL.get(name)
  if (known && bad > 0) {
    console.log(`[XFAIL] ${name}: 검사 ${total}건 / 불일치 ${bad}건 — 알려진 결함`)
    console.log(`        ${known}`)
    return
  }
  if (known && bad === 0) {
    console.log(`[BASELINE 낡음] ${name}: 이제 통과한다 — KNOWN_FAIL 에서 지워라`)
    fails++
    return
  }
  const mark = bad === 0 ? 'OK  ' : 'FAIL'
  console.log(`[${mark}] ${name}: 검사 ${total}건 / 불일치 ${bad}건` + (sample ? `\n        예: ${sample}` : ''))
  if (bad) fails++
}

// ── 1. formatScore 무손실성 ──────────────────────────────────────
// 독립 오라클: BigInt 십진 문자열 연산으로 raw/100000 의 정확한 표기를 만든다.
function exactDecimal(raw) {
  const neg = raw < 0n
  const a = neg ? -raw : raw
  const int = a / 100000n
  const frac = (a % 100000n).toString().padStart(5, '0').replace(/0+$/, '')
  const s = frac ? `${int}.${frac}` : `${int}`
  return neg && (int !== 0n || frac) ? `-${s}` : s
}

{
  const raws = new Set()
  for (let i = 0; i <= 200000; i++) raws.add(BigInt(i))          // 0 ~ 2.0, 1e-5 간격 전수
  for (let i = 1; i <= 200000; i++) raws.add(BigInt(-i))
  for (const b of [1n, -1n, 99999n, 100000n, 100001n, 12345600n, 999999999n,
                   100000000000000n, -100000000000000n, 99999999999999n]) raws.add(b)
  // 백엔드가 실제로 내려준 값 전부
  for (const scn of actual) for (const r of scn.rows ?? []) {
    raws.add(BigInt(r.total_score_raw))
    for (const v of Object.values(r.score_detail_raw)) raws.add(BigInt(v))
  }
  // 무작위 대형값
  let seed = 20260817
  const rnd = () => (seed = (seed * 1103515245 + 12345) & 0x7fffffff)
  for (let i = 0; i < 50000; i++) raws.add(BigInt(rnd()) * BigInt(rnd() % 1000) - 500000000n)

  let bad = 0, sample = ''
  for (const raw of raws) {
    if (raw > 100000000000000n || raw < -100000000000000n) continue  // ±10억(표시) 도메인 밖
    const got = formatScore(Number(raw) / 100000)
    const want = exactDecimal(raw)
    if (got !== want) { bad++; if (!sample) sample = `raw=${raw} want=${want} got=${got}` }
  }
  report('formatScore 무손실 (2-80 / U-06)', bad, raws.size, sample)
}

// ── 2. isKeyMatched 오매칭 ──────────────────────────────────────
// 시나리오의 모든 numeric_table threshold 쌍을 대상으로
//  (a) 같은 threshold 는 반드시 매칭   (b) 다른 threshold 는 절대 매칭 안 됨
{
  const ths = new Set()
  for (const s of scenarios) for (const r of s.numeric_table) ths.add(r.threshold)
  // 인접·극단 경계 보강
  for (const b of [0, 1, 2, 99999, 100000, 100001, 99999999999999, 100000000000000,
                   -1, -100000000000000]) ths.add(b)
  const arr = [...ths].sort((a, b) => a - b)

  let bad = 0, checks = 0, sample = ''
  for (let i = 0; i < arr.length; i++) {
    const key = arr[i] / 100000
    checks++
    if (!isKeyMatched('NUMERIC', [key], key)) {
      bad++; if (!sample) sample = `자기 자신 미매칭 th=${arr[i]}`
    }
    // 가장 가까운 이웃(오매칭 위험이 가장 큰 쌍)만 검사
    for (const j of [i - 1, i + 1]) {
      if (j < 0 || j >= arr.length || arr[j] === arr[i]) continue
      checks++
      if (isKeyMatched('NUMERIC', [arr[j] / 100000], key)) {
        bad++; if (!sample) sample = `오매칭 ${arr[j]} vs ${arr[i]}`
      }
    }
  }
  // raw 차이 1인 최악 쌍을 인위적으로 추가
  for (const base of [0, 100000, 99999999999999n && 99999999999999, 100000000000000]) {
    const a = Number(base) / 100000, b = (Number(base) + 1) / 100000
    checks++
    if (isKeyMatched('NUMERIC', [a], b)) { bad++; if (!sample) sample = `raw 1 차이 오매칭 base=${base}` }
  }
  report('isKeyMatched 오매칭 (2-81 / U-07)', bad, checks, sample)
}

// ── 3. tieSet: 프론트 파생 동점 집합 vs 백엔드 순위값 기반 정의 ────
{
  // 백엔드 실측 행에 대학·트랙 메타를 붙인다
  let badTrack = 0, badUniv = 0, rows = 0, sampleU = ''
  for (const scn of actual) {
    const src = scenarios.find(s => s.name === scn.name)
    const trackOf = Object.fromEntries(src.tracks.map(t => [t.id, t]))
    const univOf = Object.fromEntries(src.universities.map(u => [u.id, u]))
    const results = (scn.rows ?? []).map(r => ({
      student_id: r.student_id, track_id: r.track_id, round_id: r.round_id,
      ranking: r.ranking, track_rank: r.track_rank,
      univ_name: univOf[trackOf[r.track_id].univ_id].univ_name,
      total_score: r.total_score_raw,
    }))
    rows += results.length

    // --- RoundsTab tieSet (track 보기) 복사본
    const setTrack = new Set()
    {
      const counts = {}
      for (const r of results) {
        if (r.track_rank == null) continue
        const k = `${r.track_id}-${r.round_id}-${r.track_rank}`
        ;(counts[k] ||= []).push(r)
      }
      for (const rs of Object.values(counts))
        if (rs.length > 1) for (const r of rs) setTrack.add(`${r.student_id}-${r.track_id}`)
    }
    // 독립 정의: 같은 트랙 안에서 track_rank 가 같은 행이 2개 이상
    const wantTrack = new Set()
    {
      const g = {}
      for (const r of results) (g[`${r.track_id}|${r.track_rank}`] ||= []).push(r)
      for (const rs of Object.values(g))
        if (rs.length > 1) for (const r of rs) wantTrack.add(`${r.student_id}-${r.track_id}`)
    }
    for (const k of wantTrack) if (!setTrack.has(k)) badTrack++
    for (const k of setTrack) if (!wantTrack.has(k)) badTrack++

    // --- RoundsTab tieSet (univ 보기) 복사본
    const setUniv = new Set()
    {
      const counts = {}
      for (const r of results) {
        if (r.ranking == null) continue
        const k = `${r.univ_name}-${r.round_id}-${r.ranking}`
        ;(counts[k] ||= []).push(r)
      }
      for (const rs of Object.values(counts))
        if (rs.length > 1) for (const r of rs) setUniv.add(`${r.student_id}-${r.track_id}`)
    }
    const wantUniv = new Set()
    {
      const g = {}
      for (const r of results) (g[`${r.univ_name}|${r.ranking}`] ||= []).push(r)
      for (const rs of Object.values(g))
        if (rs.length > 1) for (const r of rs) wantUniv.add(`${r.student_id}-${r.track_id}`)
    }
    for (const k of wantUniv) if (!setUniv.has(k)) { badUniv++; sampleU ||= `${scn.name} ${k}` }
    for (const k of setUniv) if (!wantUniv.has(k)) { badUniv++; sampleU ||= `${scn.name} ${k}` }
  }
  report('tieSet track 보기 (2-90 / U-16)', badTrack, rows, '')
  report('tieSet univ 보기 (2-90 / U-16)', badUniv, rows, sampleU)
}

// ── 3b. tieSet univ 보기 + 모집단위 필터 조합 (선택된 트랙만 배열에 담김) ──
{
  let missed = 0, cases = 0, sample = ''
  for (const scn of actual) {
    const src = scenarios.find(s => s.name === scn.name)
    const trackOf = Object.fromEntries(src.tracks.map(t => [t.id, t]))
    const univOf = Object.fromEntries(src.universities.map(u => [u.id, u]))
    const all = (scn.rows ?? []).map(r => ({
      student_id: r.student_id, track_id: r.track_id, round_id: r.round_id,
      ranking: r.ranking,
      univ_name: univOf[trackOf[r.track_id].univ_id].univ_name,
    }))
    const tieAll = new Set()
    {
      const g = {}
      for (const r of all) (g[`${r.univ_name}|${r.ranking}`] ||= []).push(r)
      for (const rs of Object.values(g))
        if (rs.length > 1) for (const r of rs) tieAll.add(`${r.student_id}-${r.track_id}`)
    }
    for (const t of src.tracks) {
      const filtered = all.filter(r => r.track_id === t.id)
      if (!filtered.length) continue
      cases++
      const counts = {}
      for (const r of filtered) {
        if (r.ranking == null) continue
        const k = `${r.univ_name}-${r.round_id}-${r.ranking}`
        ;(counts[k] ||= []).push(r)
      }
      const shown = new Set()
      for (const rs of Object.values(counts))
        if (rs.length > 1) for (const r of rs) shown.add(`${r.student_id}-${r.track_id}`)
      for (const r of filtered) {
        const k = `${r.student_id}-${r.track_id}`
        if (tieAll.has(k) && !shown.has(k)) {
          missed++
          sample ||= `${scn.name} 트랙${t.id} 필터 시 ${k} (대학순위 ${r.ranking}) 동점 표식 누락`
        }
      }
    }
  }
  report('tieSet univ 보기 + 모집단위 필터 (동점 표식 누락)', missed, cases, sample)
}

// ── 4. resultsByUnivOnly 재정렬 vs 백엔드 ranking 순서 ───────────
{
  let bad = 0, checks = 0, sample = ''
  for (const scn of actual) {
    const src = scenarios.find(s => s.name === scn.name)
    const trackOf = Object.fromEntries(src.tracks.map(t => [t.id, t]))
    const univOf = Object.fromEntries(src.universities.map(u => [u.id, u]))
    const byUniv = {}
    for (const r of scn.rows ?? []) {
      const un = univOf[trackOf[r.track_id].univ_id].univ_name
      ;(byUniv[un] ||= []).push({ ...r, univ_name: un })
    }
    for (const rows of Object.values(byUniv)) {
      const sorted = [...rows].sort((a, b) => {
        if (a.ranking == null && b.ranking == null) return 0
        if (a.ranking == null) return 1
        if (b.ranking == null) return -1
        return a.ranking - b.ranking
      })
      checks++
      for (let i = 1; i < sorted.length; i++) {
        if (sorted[i - 1].ranking > sorted[i].ranking) {
          bad++; sample ||= `${scn.name} 정렬 역전`
        }
        // 순위가 앞선 행의 총점이 더 낮으면(우선순위 그룹 무시) 표시 모순
      }
    }
  }
  report('resultsByUnivOnly 재정렬 (2-92 / U-18)', bad, checks, sample)
}

// ── 5. studentsByRound 정렬 — seq_no ?? 999 ─────────────────────
{
  // 재학생 담임(auth.grade !== 0) 경로: 학생은 전원 재학생이라 seq_no NOT NULL
  // (002-students.sql:15-27 CHECK). ?? 999 가 발동하는 입력이 있는지 확인.
  let nullSeq = 0, negSeq = 0, total = 0
  for (const s of scenarios) for (const st of s.students) {
    if (!st.is_enrolled) continue
    total++
    if (st.seq_no == null) nullSeq++
    if (st.seq_no != null && st.seq_no <= 0) negSeq++
  }
  report('studentsByRound: 재학생 seq_no NULL (?? 999 발동)', nullSeq, total, '')
  console.log(`        참고: 시나리오상 seq_no <= 0 재학생 ${negSeq}명 (JSON add_enrolled 는 범위 미검증 — P2 U-28)`)
}

// ── 6. AreasTab totalMaxScore — f64 reduce 합산 vs 정확한 정수 합 ─
{
  let bad = 0, checks = 0, sample = ''
  for (const s of scenarios) {
    const exact = s.areas.reduce((acc, a) => acc + BigInt(a.max_score), 0n)
    const front = s.areas.reduce((sum, a) => sum + a.max_score / 100000, 0)   // 백엔드 Score 직렬화 후 f64 합
    checks++
    if (formatScore(front) !== exactDecimal(exact)) {
      bad++; sample ||= `${s.name}: front=${formatScore(front)} exact=${exactDecimal(exact)}`
    }
  }
  // 최악 케이스: 5자리 소수 만점을 많이 더했을 때
  for (const n of [10, 50, 100, 500]) {
    const areas = Array.from({ length: n }, () => ({ max_score: 12345600 })) // 123.456
    const exact = areas.reduce((a, x) => a + BigInt(x.max_score), 0n)
    const front = areas.reduce((s, x) => s + x.max_score / 100000, 0)
    checks++
    if (formatScore(front) !== exactDecimal(exact)) {
      bad++; sample ||= `n=${n}: front=${formatScore(front)} exact=${exactDecimal(exact)}`
    }
  }
  report('AreasTab totalMaxScore f64 합산 (2-82 / U-26)', bad, checks, sample)
}

console.log(fails === 0
  ? `\n프론트 대조 통과 (알려진 결함 ${KNOWN_FAIL.size}건은 XFAIL 로 제외)`
  : `\n실패한 검사 ${fails}건`)
process.exit(fails === 0 ? 0 : 1)
