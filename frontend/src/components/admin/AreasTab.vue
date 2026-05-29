<template>
  <div class="flex gap-4 min-h-0">
    <!-- ── 좌측: 전형요소 목록 ── -->
    <div class="w-72 shrink-0">
      <div class="flex items-center justify-between mb-3">
        <h2 class="text-lg font-semibold text-gray-700">전형요소 목록</h2>
        <button class="px-2 py-1 bg-blue-600 text-white text-sm rounded hover:bg-blue-700"
          @click="openAddForm">+ 추가</button>
      </div>

      <p v-if="error" class="text-red-500 text-sm mb-2">{{ error }}</p>

      <ul class="space-y-1">
        <li v-for="area in areas" :key="area.id"
          class="flex items-center justify-between px-3 py-2 rounded border cursor-pointer"
          :class="selected?.id === area.id
            ? 'border-blue-500 bg-blue-50'
            : 'border-gray-200 hover:bg-gray-50'"
          @click="selectArea(area)">
          <div class="min-w-0">
            <span class="text-sm font-medium truncate block">{{ area.name }}</span>
            <span class="text-xs text-gray-400">{{ calcTypeLabel(area.calc_type) }} · {{ lookupScopeLabel(area.lookup_scope) }}</span>
          </div>
          <button class="text-red-400 text-xs hover:text-red-600 ml-2 shrink-0"
            @click.stop="removeArea(area.id)">삭제</button>
        </li>
        <li v-if="areas.length === 0" class="text-gray-400 text-sm px-2">등록된 전형요소 없음</li>
      </ul>

      <div v-if="areas.length > 0"
        class="mt-2 px-3 py-2 border-t border-gray-200 flex items-center justify-between text-sm font-semibold text-gray-700">
        <span>총점</span>
        <span>{{ displayScore(totalMaxScore) }}점</span>
      </div>

      <!-- 전형요소 추가 폼 -->
      <div v-if="showAddForm" class="mt-3 p-3 border border-blue-200 rounded bg-blue-50 space-y-2 text-sm">
        <div>
          <label class="text-xs text-gray-500">전형요소 이름</label>
          <input v-model="newArea.name" type="text" class="w-full border rounded px-2 py-1 mt-0.5" />
        </div>
        <div>
          <label class="text-xs text-gray-500">만점 (반영 비율)</label>
          <input v-model="newArea.max_score_display" type="number" step="0.00001"
            class="w-full border rounded px-2 py-1 mt-0.5" />
        </div>
        <div class="flex gap-2">
          <div class="flex-1">
            <label class="text-xs text-gray-500">점수 산출 방식</label>
            <select v-model="newArea.calc_type" class="w-full border rounded px-2 py-1 mt-0.5">
              <option value="NUMERIC">구간 조회</option>
              <option value="CATEGORY">범주 선택</option>
              <option value="MANUAL">수기 입력</option>
            </select>
          </div>
          <div class="flex-1">
            <label class="text-xs text-gray-500">데이터 조회 기준</label>
            <select v-model="newArea.lookup_scope" class="w-full border rounded px-2 py-1 mt-0.5">
              <option value="SIMPLE">기본 조회</option>
              <option value="COMPOSITE">대학별 환산점수 조회</option>
            </select>
          </div>
        </div>
        <div class="flex items-center gap-2">
          <input v-model="newArea.teacher_editable" type="checkbox" id="te" />
          <label for="te" class="text-xs text-gray-500">담임교사 입력 허용</label>
        </div>
        <div v-if="newArea.calc_type === 'NUMERIC'">
          <label class="text-xs text-gray-500">구간 탐색 방향 <span class="text-red-500">*</span></label>
          <select v-model="newArea.match_mode" class="w-full border rounded px-2 py-1 mt-0.5">
            <option value="">선택하세요</option>
            <option value="UPPER">해당 기준값 이상 ▲</option>
            <option value="LOWER">해당 기준값 이하 ▼</option>
            <option value="EXACT">정확히 일치</option>
          </select>
        </div>
        <div v-if="newArea.calc_type === 'CATEGORY'">
          <label class="text-xs text-gray-500">복수 활동 처리 방식 <span class="text-red-500">*</span></label>
          <select v-model="newArea.category_agg" class="w-full border rounded px-2 py-1 mt-0.5">
            <option value="">선택하세요</option>
            <option value="SUM">중복 선택 가능 (점수 합산)</option>
            <option value="MAX">최대 1개만 인정 (최고점 반영)</option>
          </select>
        </div>
        <p class="text-xs text-amber-700 bg-amber-50 border border-amber-200 rounded px-2 py-1 mt-2">
          전형요소는 등록 후 수정할 수 없습니다.
        </p>
        <div class="flex gap-1">
          <button class="px-2 py-1 bg-blue-600 text-white text-xs rounded" @click="addArea">저장</button>
          <button class="px-2 py-1 bg-gray-200 text-xs rounded" @click="showAddForm = false">취소</button>
        </div>
      </div>
    </div>

    <!-- ── 우측: 전형요소 상세 (서브탭) ── -->
    <div class="flex-1 min-w-0" v-if="selected">
      <div class="flex items-center gap-2 mb-3">
        <h3 class="text-base font-semibold text-gray-800">{{ selected.name }}</h3>
        <span class="px-2 py-0.5 text-xs rounded bg-gray-100 text-gray-500">{{ calcTypeLabel(selected.calc_type) }}</span>
        <span class="px-2 py-0.5 text-xs rounded bg-gray-100 text-gray-500">{{ lookupScopeLabel(selected.lookup_scope) }}</span>
      </div>

      <!-- 기본 정보 카드 -->
      <div class="mb-4 p-3 bg-gray-50 border border-gray-200 rounded text-sm">
        <div class="flex flex-wrap gap-x-6 gap-y-1 text-gray-700">
          <span><span class="text-gray-400 mr-1">만점</span>{{ displayScore(selected.max_score) }}점</span>
          <span><span class="text-gray-400 mr-1">조회 기준</span>{{ lookupScopeLabel(selected.lookup_scope) }}</span>
          <span><span class="text-gray-400 mr-1">계산 유형</span>{{ calcTypeLabel(selected.calc_type) }}</span>
          <span v-if="selected.calc_type === 'NUMERIC'"><span class="text-gray-400 mr-1">탐색 방향</span>{{ matchModeLabel(selected.match_mode) }}</span>
          <span v-if="selected.calc_type === 'CATEGORY'"><span class="text-gray-400 mr-1">범주 집계</span>{{ categoryAggLabel(selected.category_agg) }}</span>
          <span><span class="text-gray-400 mr-1">담임교사 입력</span>{{ selected.teacher_editable ? '허용' : '불가' }}</span>
        </div>
        <p class="text-xs text-amber-700 bg-amber-50 border border-amber-200 rounded px-2 py-1 mt-2">
          전형요소는 등록 후 수정할 수 없습니다. 설정을 변경하려면 삭제 후 재등록하세요.
        </p>
      </div>

      <!-- 서브탭 -->
      <div class="flex border-b mb-4">
        <button v-if="selected.calc_type !== 'MANUAL'"
          class="px-4 py-2 text-sm font-medium border-b-2 transition-colors"
          :class="activeTab === 'score'
            ? 'border-blue-600 text-blue-600'
            : 'border-transparent text-gray-500 hover:text-gray-700'"
          @click="activeTab = 'score'">
          점수 기준
        </button>
        <button
          class="px-4 py-2 text-sm font-medium border-b-2 transition-colors"
          :class="activeTab === 'base'
            ? 'border-blue-600 text-blue-600'
            : 'border-transparent text-gray-500 hover:text-gray-700'"
          @click="activeTab = 'base'">
          기초 데이터
        </button>
      </div>

      <!-- 점수 기준 탭 -->
      <div v-if="activeTab === 'score'">
        <div class="mb-4 p-3 bg-blue-50 border border-blue-100 rounded text-xs">
          <p class="font-medium text-blue-700 mb-2">양식 예시 — {{ scoreEx.desc }}</p>
          <table class="border-collapse text-gray-700">
            <thead>
              <tr>
                <th v-for="h in scoreEx.headers" :key="h"
                    class="border border-blue-200 bg-blue-100 px-3 py-1 text-left font-medium whitespace-nowrap">{{ h }}</th>
                <th class="bg-transparent px-3 py-1"></th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="(row, i) in scoreEx.rows" :key="i">
                <td v-for="(cell, j) in row" :key="j"
                    class="border border-blue-100 px-3 py-1 whitespace-nowrap">{{ cell }}</td>
                <td class="pl-4 py-1 text-gray-400 whitespace-nowrap">{{ scoreEx.rowDescs[i] }}</td>
              </tr>
            </tbody>
          </table>
        </div>
        <p class="text-xs text-gray-500 mb-3">
          기준값·점수는 실제 값으로 작성 (예: 1.25, 30.5 / 소수점 최대 5자리)
        </p>
        <ExcelPanel
          :area-id="selected.id"
          :calc-type="selected.calc_type"
          :area-name="selected.name"
          panel="score"
          @result="onScoreResult" />
        <ImportResultBox v-if="scoreResult" :result="scoreResult" class="mt-3" />

        <div class="mt-4 max-h-72 overflow-y-auto border border-gray-200 rounded">
          <p v-if="scoreRows.length === 0" class="text-gray-400 text-sm px-3 py-4 text-center">
            등록된 점수 기준 없음
          </p>
          <table v-else class="w-full text-sm border-collapse">
            <thead class="sticky top-0 bg-gray-50">
              <tr>
                <th class="border-b border-gray-200 px-3 py-2 text-left text-gray-600 font-medium">
                  {{ selected.calc_type === 'NUMERIC' ? '기준값' : '범주' }}
                </th>
                <th class="border-b border-gray-200 px-3 py-2 text-left text-gray-600 font-medium">점수</th>
                <template v-if="selected.lookup_scope === 'COMPOSITE'">
                  <th class="border-b border-gray-200 px-3 py-2 text-left text-gray-600 font-medium">대학명</th>
                  <th class="border-b border-gray-200 px-3 py-2 text-left text-gray-600 font-medium">모집단위명</th>
                </template>
              </tr>
            </thead>
            <tbody>
              <tr v-for="(row, i) in scoreRows" :key="i"
                  :class="i % 2 === 1 ? 'bg-gray-50' : ''">
                <td class="border-b border-gray-100 px-3 py-1.5 text-gray-700">
                  {{ selected.calc_type === 'NUMERIC' ? row.threshold : row.category }}
                </td>
                <td class="border-b border-gray-100 px-3 py-1.5 text-gray-700">{{ row.score }}</td>
                <template v-if="selected.lookup_scope === 'COMPOSITE'">
                  <td class="border-b border-gray-100 px-3 py-1.5 text-gray-700">{{ row.univ_name }}</td>
                  <td class="border-b border-gray-100 px-3 py-1.5 text-gray-700">{{ row.track_name }}</td>
                </template>
              </tr>
            </tbody>
          </table>
        </div>
      </div>

      <!-- 기초 데이터 탭 -->
      <div v-if="activeTab === 'base'">
        <div class="mb-4 p-3 bg-blue-50 border border-blue-100 rounded text-xs">
          <p class="font-medium text-blue-700 mb-2">양식 예시 — {{ baseEx.desc }}</p>
          <table class="border-collapse text-gray-700">
            <thead>
              <tr>
                <th v-for="h in baseEx.headers" :key="h"
                    class="border border-blue-200 bg-blue-100 px-3 py-1 text-left font-medium whitespace-nowrap">{{ h }}</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="(row, i) in baseEx.rows" :key="i">
                <td v-for="(cell, j) in row" :key="j"
                    class="border border-blue-100 px-3 py-1 whitespace-nowrap">{{ cell }}</td>
              </tr>
            </tbody>
          </table>
        </div>
        <p class="text-xs text-gray-500 mb-3">
          학생코드로 학생을 찾습니다. 구간 조회·수기 입력은 실제 값으로 작성 (소수점 최대 5자리).
        </p>
        <ExcelPanel
          :area-id="selected.id"
          :calc-type="selected.calc_type"
          :area-name="selected.name"
          panel="base"
          @result="onBaseResult" />
        <ImportResultBox v-if="baseResult" :result="baseResult" class="mt-3" />

        <div class="mt-4 max-h-72 overflow-y-auto border border-gray-200 rounded">
          <p v-if="baseRows.length === 0" class="text-gray-400 text-sm px-3 py-4 text-center">
            등록된 기초 데이터 없음
          </p>
          <table v-else class="w-full text-sm border-collapse">
            <thead class="sticky top-0 bg-gray-50">
              <tr>
                <th class="border-b border-gray-200 px-3 py-2 text-left text-gray-600 font-medium">학생코드</th>
                <th class="border-b border-gray-200 px-3 py-2 text-left text-gray-600 font-medium">이름</th>
                <th class="border-b border-gray-200 px-3 py-2 text-left text-gray-600 font-medium">값</th>
                <template v-if="selected.lookup_scope === 'COMPOSITE'">
                  <th class="border-b border-gray-200 px-3 py-2 text-left text-gray-600 font-medium">대학명</th>
                  <th class="border-b border-gray-200 px-3 py-2 text-left text-gray-600 font-medium">모집단위명</th>
                </template>
              </tr>
            </thead>
            <tbody>
              <tr v-for="(row, i) in baseRows" :key="i"
                  :class="i % 2 === 1 ? 'bg-gray-50' : ''">
                <td class="border-b border-gray-100 px-3 py-1.5 text-gray-700">{{ row.student_code }}</td>
                <td class="border-b border-gray-100 px-3 py-1.5 text-gray-700">{{ row.name }}</td>
                <td class="border-b border-gray-100 px-3 py-1.5 text-gray-700">{{ row.value }}</td>
                <template v-if="selected.lookup_scope === 'COMPOSITE'">
                  <td class="border-b border-gray-100 px-3 py-1.5 text-gray-700">{{ row.univ_name }}</td>
                  <td class="border-b border-gray-100 px-3 py-1.5 text-gray-700">{{ row.track_name }}</td>
                </template>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    </div>

    <p v-else class="text-gray-400 text-sm mt-2">왼쪽에서 전형요소를 선택하세요.</p>
  </div>
</template>

<script setup>
import { ref, watch, onMounted, defineComponent, h, computed } from 'vue'
import {
  getAreas, createArea, deleteArea,
  downloadNumericTableTemplate, exportNumericTable, importNumericTable,
  downloadCategoryMapTemplate, exportCategoryMap, importCategoryMap,
  downloadBaseDataTemplate, exportBaseData, importBaseData,
  getNumericTableList, getCategoryMapList, getBaseDataList,
} from '../../api/admin.js'
import { getScoreExample, getBaseExample } from '../../data/areaSamples.js'

// ── 상태 ──────────────────────────────────────────────────────
const areas    = ref([])
const selected = ref(null)
const error    = ref('')
const activeTab   = ref('score')
const scoreResult = ref(null)
const baseResult  = ref(null)
const scoreRows   = ref([])
const baseRows    = ref([])

const showAddForm = ref(false)
const newArea = ref(defaultNewArea())

function defaultNewArea() {
  return { name: '', max_score_display: '', calc_type: 'NUMERIC',
           lookup_scope: 'SIMPLE', teacher_editable: true,
           match_mode: '', category_agg: '' }
}

function parseDisplayScore(raw) {
  const s = String(raw).trim()
  if (!s) return null
  const f = parseFloat(s)
  if (isNaN(f) || f < 0) return null
  const dot = s.indexOf('.')
  if (dot !== -1 && s.slice(dot + 1).replace(/0+$/, '').length > 5) return null
  return Math.round(f * 100000)
}

const CALC_TYPE_LABELS    = { NUMERIC: '구간 조회', CATEGORY: '범주 선택', MANUAL: '수기 입력' }
const LOOKUP_SCOPE_LABELS = { SIMPLE: '기본 조회', COMPOSITE: '대학별 환산점수 조회' }
const MATCH_MODE_LABELS   = { UPPER: '이상 ▲', LOWER: '이하 ▼', EXACT: '정확히 일치' }
const CATEGORY_AGG_LABELS = { SUM: '중복 선택 (합산)', MAX: '최대 1개 (최고점)' }
function calcTypeLabel(v)    { return CALC_TYPE_LABELS[v]    ?? v }
function lookupScopeLabel(v) { return LOOKUP_SCOPE_LABELS[v] ?? v }
function matchModeLabel(v)   { return v ? (MATCH_MODE_LABELS[v]   ?? v) : '—' }
function categoryAggLabel(v) { return v ? (CATEGORY_AGG_LABELS[v] ?? v) : '—' }
function displayScore(v) {
  const f = v / 100000
  return f % 1 === 0 ? String(f) : f.toFixed(5).replace(/\.?0+$/, '')
}

const totalMaxScore = computed(() => areas.value.reduce((sum, a) => sum + a.max_score, 0))

const scoreEx = computed(() => selected.value ? getScoreExample(selected.value) : null)
const baseEx  = computed(() => selected.value ? getBaseExample(selected.value) : null)

// ── 전형요소 목록 ─────────────────────────────────────────────────
async function load() {
  try { areas.value = await getAreas() }
  catch (e) { error.value = e.response?.data ?? e.message }
}

function selectArea(area) {
  selected.value = area
  activeTab.value = area.calc_type === 'MANUAL' ? 'base' : 'score'

  scoreResult.value = null
  baseResult.value  = null
  loadScoreRows()
  loadBaseRows()
}

async function loadScoreRows() {
  const area = selected.value
  if (!area || area.calc_type === 'MANUAL') { scoreRows.value = []; return }
  try {
    scoreRows.value = area.calc_type === 'CATEGORY'
      ? await getCategoryMapList(area.id)
      : await getNumericTableList(area.id)
  } catch { scoreRows.value = [] }
}

async function loadBaseRows() {
  if (!selected.value) { baseRows.value = []; return }
  try { baseRows.value = await getBaseDataList(selected.value.id) }
  catch { baseRows.value = [] }
}

function onScoreResult(evt) { scoreResult.value = evt; loadScoreRows() }
function onBaseResult(evt)  { baseResult.value = evt;  loadBaseRows()  }

async function addArea() {
  const maxScore = parseDisplayScore(newArea.value.max_score_display)
  if (maxScore === null) {
    error.value = '만점: 양수이고 소수점 최대 5자리까지 입력하세요'
    return
  }
  const body = {
    name: newArea.value.name,
    max_score: maxScore,
    calc_type: newArea.value.calc_type,
    lookup_scope: newArea.value.lookup_scope,
    teacher_editable: newArea.value.teacher_editable ? 1 : 0,
    match_mode: newArea.value.match_mode || null,
    category_agg: newArea.value.category_agg || null,
  }
  try {
    await createArea(body)
    showAddForm.value = false
    await load()
  } catch (e) { error.value = e.response?.data ?? e.message }
}

async function removeArea(id) {
  if (!confirm('전형요소를 삭제하면 구간표·범주표·기초데이터도 함께 삭제됩니다. 계속할까요?')) return
  try {
    await deleteArea(id)
    if (selected.value?.id === id) selected.value = null
    await load()
  } catch (e) { error.value = e.response?.data ?? e.message }
}

function openAddForm() {
  newArea.value = defaultNewArea()
  showAddForm.value = true
}

onMounted(load)

// ── 다운로드 헬퍼 ─────────────────────────────────────────────
function saveBlob(response, filename) {
  const url = URL.createObjectURL(new Blob([response.data]))
  const a = document.createElement('a')
  a.href = url; a.download = filename; a.click()
  URL.revokeObjectURL(url)
}

// ── ExcelPanel (인라인 컴포넌트) ──────────────────────────────
const ExcelPanel = defineComponent({
  props: {
    areaId:   { type: Number, required: true },
    calcType: { type: String, required: true },
    areaName: { type: String, required: true },
    panel:    { type: String, required: true }, // 'score' | 'base'
  },
  emits: ['result'],
  setup(props, { emit }) {
    const err = ref('')
    const uploading = ref(false)

    async function dlTemplate() {
      err.value = ''
      try {
        if (props.panel === 'score') {
          const res = props.calcType === 'CATEGORY'
            ? await downloadCategoryMapTemplate(props.areaId)
            : await downloadNumericTableTemplate(props.areaId)
          saveBlob(res, props.calcType === 'CATEGORY'
            ? `${props.areaName}_category_map_template.xlsx`
            : `${props.areaName}_numeric_table_template.xlsx`)
        } else {
          const res = await downloadBaseDataTemplate(props.areaId)
          saveBlob(res, `${props.areaName}_base_data_template.xlsx`)
        }
      } catch (e) { err.value = e.response?.data ?? e.message }
    }

    async function dlExport() {
      err.value = ''
      try {
        if (props.panel === 'score') {
          const res = props.calcType === 'CATEGORY'
            ? await exportCategoryMap(props.areaId)
            : await exportNumericTable(props.areaId)
          saveBlob(res, props.calcType === 'CATEGORY'
            ? `${props.areaName}_category_map.xlsx`
            : `${props.areaName}_numeric_table.xlsx`)
        } else {
          const res = await exportBaseData(props.areaId)
          saveBlob(res, `${props.areaName}_base_data.xlsx`)
        }
      } catch (e) { err.value = e.response?.data ?? e.message }
    }

    async function onFile(evt) {
      const file = evt.target.files?.[0]
      if (!file) return
      err.value = ''
      uploading.value = true
      try {
        let result
        if (props.panel === 'score') {
          result = props.calcType === 'CATEGORY'
            ? await importCategoryMap(props.areaId, file)
            : await importNumericTable(props.areaId, file)
        } else {
          result = await importBaseData(props.areaId, file)
        }
        emit('result', result)
      } catch (e) {
        const d = e.response?.data
        if (d != null && typeof d === 'object' && Array.isArray(d.errors)) {
          emit('result', d)
        } else {
          err.value = typeof d === 'string' ? d : (e.message ?? '오류가 발생했습니다')
        }
      }
      finally { uploading.value = false; evt.target.value = '' }
    }

    return () => h('div', { class: 'space-y-2' }, [
      h('div', { class: 'flex flex-wrap gap-2' }, [
        h('button', {
          class: 'px-3 py-1.5 border border-gray-300 text-gray-700 text-sm rounded hover:bg-gray-50',
          onClick: dlTemplate,
        }, '양식 다운로드'),
        h('button', {
          class: 'px-3 py-1.5 border border-gray-300 text-gray-700 text-sm rounded hover:bg-gray-50',
          onClick: dlExport,
        }, '목록 내보내기'),
        h('label', {
          class: `px-3 py-1.5 text-sm rounded cursor-pointer ${uploading.value ? 'bg-gray-400 text-white' : 'bg-blue-600 text-white hover:bg-blue-700'}`,
        }, [
          uploading.value ? '가져오는 중…' : '가져오기',
          h('input', { type: 'file', accept: '.xlsx,.csv', class: 'hidden', onChange: onFile }),
        ]),
      ]),
      err.value ? h('p', { class: 'text-red-500 text-sm' }, err.value) : null,
    ])
  },
})

// ── ImportResultBox (인라인 컴포넌트) ─────────────────────────
const ImportResultBox = defineComponent({
  props: { result: Object },
  setup(props) {
    return () => {
      const r = props.result
      const hasErrors = r.errors?.length > 0
      const hasWarnings = r.warnings?.length > 0
      return h('div', {
        class: `p-3 rounded border text-sm ${hasErrors ? 'border-yellow-400 bg-yellow-50' : 'border-green-400 bg-green-50'}`,
      }, [
        h('p', { class: 'font-medium mb-1' },
          hasErrors
            ? '오류 발생 — 가져오기 실패'
            : `완료 — ${r.rows != null ? `${r.rows}건` : r.inserted != null ? `신규 ${r.inserted}명, 수정 ${r.updated}명` : ''} 처리됨`),
        hasWarnings
          ? h('ul', { class: 'list-disc list-inside text-blue-700 text-xs mb-1' },
              r.warnings.map((w, i) => h('li', { key: i }, w)))
          : null,
        hasErrors
          ? h('ul', { class: 'list-disc list-inside text-yellow-700 text-xs' },
              r.errors.map((e, i) => h('li', { key: i }, e)))
          : null,
      ])
    }
  },
})
</script>
