/**
 * 전형요소 조합별 양식 예시 데이터
 * calc_type × match_mode/category_agg × lookup_scope 조합에 따라 예시 반환
 *
 * 수정 시 이 파일만 편집하면 됩니다.
 */

const COMPOSITE_COLS = ['대학명', '모집단위명']
const COMPOSITE_VALS = ['서울대', '일반']

// ── 점수 기준 예시 (numeric_table / category_map) ──────────────

const SCORE_EXAMPLES = {
  NUMERIC_UPPER: {
    desc: '기준값 이상이면 해당 점수 부여',
    keyCol: '기준값',
    rows: [
      [40, 2],
      [30, 1],
      [0,  0],
    ],
    rowDescs: [
      '40점 이상이면 2점',
      '30점 이상이면 1점',
      '0점 이상이면 0점',
    ],
  },
  NUMERIC_LOWER: {
    desc: '기준값 이하이면 해당 점수 부여',
    keyCol: '기준값',
    rows: [
      [0, 10],
      [1,  9],
      [2,  8],
      [3,  7],
      [4,  6],
      [5,  5],
    ],
    rowDescs: [
      '0점 이하이면 10점',
      '1점 이하이면 9점',
      '2점 이하이면 8점',
      '3점 이하이면 7점',
      '4점 이하이면 6점',
      '5점 이하이면 5점',
    ],
  },
  NUMERIC_EXACT: {
    desc: '기준값과 정확히 일치하면 해당 점수 부여',
    keyCol: '기준값',
    rows: [
      [1, 10],
      [2,  8],
      [3,  6],
      [4,  4],
    ],
    rowDescs: [
      '정확히 1이면 10점',
      '정확히 2이면 8점',
      '정확히 3이면 6점',
      '정확히 4이면 4점',
    ],
  },
  CATEGORY_MAX: {
    desc: '해당하는 범주 중 최고점 1개만 반영',
    keyCol: '범주',
    rows: [
      ['총학생자치회장',    5],
      ['총학생자치회부회장', 5],
      ['총학생자치회부·차장', 4],
      ['학급자치회장',      4],
      ['학급자치회부회장',  3],
      ['학급자치회부장',    2],
      ['학급자치회부원',    2],
    ],
    rowDescs: [
      "'총학생자치회장' 해당 시 5점",
      "'총학생자치회부회장' 해당 시 5점",
      "'총학생자치회부·차장' 해당 시 4점",
      "'학급자치회장' 해당 시 4점",
      "'학급자치회부회장' 해당 시 3점",
      "'학급자치회부장' 해당 시 2점",
      "'학급자치회부원' 해당 시 2점",
    ],
  },
  CATEGORY_SUM: {
    desc: '해당하는 모든 범주 점수 합산',
    keyCol: '범주',
    rows: [
      ['교내수상', 5],
      ['교외수상', 3],
      ['자격증',   2],
    ],
    rowDescs: [
      "'교내수상' 해당 시 5점 합산",
      "'교외수상' 해당 시 3점 합산",
      "'자격증' 해당 시 2점 합산",
    ],
  },
}

// ── 기초 데이터 예시 (base_data) ──────────────────────────────

const BASE_EXAMPLES = {
  NUMERIC: {
    desc: '수치 입력 — 점수 기준표와 비교하여 점수 산출',
    rows: [
      ['20250001', '홍길동', 42],
      ['20250002', '김철수',  0],
      ['20250003', '이영희', 35],
    ],
  },
  CATEGORY: {
    desc: '범주 텍스트 입력 — 점수 기준표의 범주명과 정확히 일치해야 함',
    rows: [
      ['20250001', '홍길동', '총학생자치회장'],
      ['20250002', '김철수', '학급자치회부원'],
      ['20250003', '이영희', '학급자치회장'],
    ],
  },
  MANUAL: {
    desc: '점수 직접 입력 (소수점 최대 5자리)',
    rows: [
      ['20250001', '홍길동', 9.5],
      ['20250002', '김철수', 7.0],
      ['20250003', '이영희', 8.5],
    ],
  },
}

// ── 공개 API ──────────────────────────────────────────────────

/**
 * 점수 기준 양식 예시 반환
 * @param {object} area - AreaRow (calc_type, match_mode, category_agg, lookup_scope)
 * @returns {{ desc: string, headers: string[], rows: any[][], rowDescs: string[] }}
 */
export function getScoreExample(area) {
  let key
  if (area.calc_type === 'NUMERIC') {
    key = `NUMERIC_${area.match_mode ?? 'UPPER'}`
  } else {
    key = `CATEGORY_${area.category_agg ?? 'MAX'}`
  }

  const ex = SCORE_EXAMPLES[key]
  const composite = area.lookup_scope === 'COMPOSITE'
  const headers = [ex.keyCol, '점수', ...(composite ? COMPOSITE_COLS : [])]
  const rows = composite
    ? ex.rows.map(r => [...r, ...COMPOSITE_VALS])
    : ex.rows.map(r => [...r])

  return { desc: ex.desc, headers, rows, rowDescs: ex.rowDescs }
}

/**
 * 기초 데이터 양식 예시 반환
 * @param {object} area - AreaRow (calc_type, lookup_scope)
 * @returns {{ desc: string, headers: string[], rows: any[][] }}
 */
export function getBaseExample(area) {
  const ex = BASE_EXAMPLES[area.calc_type] ?? BASE_EXAMPLES.MANUAL
  const composite = area.lookup_scope === 'COMPOSITE'
  const headers = ['학생코드', '이름', '값', ...(composite ? COMPOSITE_COLS : [])]
  const rows = composite
    ? ex.rows.map(r => [...r, ...COMPOSITE_VALS])
    : ex.rows.map(r => [...r])

  return { desc: ex.desc, headers, rows }
}
