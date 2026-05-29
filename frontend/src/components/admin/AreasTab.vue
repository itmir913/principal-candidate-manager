<template>
  <div class="flex gap-4 min-h-0">
    <!-- ── 좌측: 전형 요소 목록 ── -->
    <div class="w-72 shrink-0">
      <div class="flex items-center justify-between mb-3">
        <h2 class="text-lg font-semibold text-gray-700">전형 요소 목록</h2>
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
            <span class="text-xs text-gray-400">{{ area.calc_type }} · {{ area.lookup_scope }}</span>
          </div>
          <button class="text-red-400 text-xs hover:text-red-600 ml-2 shrink-0"
            @click.stop="removeArea(area.id)">삭제</button>
        </li>
        <li v-if="areas.length === 0" class="text-gray-400 text-sm px-2">등록된 전형 요소 없음</li>
      </ul>

      <!-- 전형 요소 추가 폼 -->
      <div v-if="showAddForm" class="mt-3 p-3 border border-blue-200 rounded bg-blue-50 space-y-2 text-sm">
        <div>
          <label class="text-xs text-gray-500">전형 요소 이름</label>
          <input v-model="newArea.name" type="text" class="w-full border rounded px-2 py-1 mt-0.5" />
        </div>
        <div>
          <label class="text-xs text-gray-500">최고 점수(비율)</label>
          <input v-model.number="newArea.max_score_display" type="number" step="0.0001"
            class="w-full border rounded px-2 py-1 mt-0.5" />
        </div>
        <div class="flex gap-2">
          <div class="flex-1">
            <label class="text-xs text-gray-500">calc_type</label>
            <select v-model="newArea.calc_type" class="w-full border rounded px-2 py-1 mt-0.5">
              <option>RANGE</option><option>CATEGORY</option><option>MANUAL</option>
            </select>
          </div>
          <div class="flex-1">
            <label class="text-xs text-gray-500">lookup_scope</label>
            <select v-model="newArea.lookup_scope" class="w-full border rounded px-2 py-1 mt-0.5">
              <option>SIMPLE</option><option>COMPOSITE</option>
            </select>
          </div>
        </div>
        <div class="flex items-center gap-2">
          <input v-model="newArea.teacher_editable" type="checkbox" id="te" />
          <label for="te" class="text-xs text-gray-500">담임교사 수동 입력</label>
        </div>
        <div v-if="newArea.calc_type === 'RANGE'">
          <label class="text-xs text-gray-500">range_direction</label>
          <select v-model="newArea.range_direction" class="w-full border rounded px-2 py-1 mt-0.5">
            <option value="">없음</option><option>UPPER</option><option>LOWER</option>
          </select>
        </div>
        <div v-if="newArea.calc_type === 'CATEGORY'">
          <label class="text-xs text-gray-500">category_agg</label>
          <select v-model="newArea.category_agg" class="w-full border rounded px-2 py-1 mt-0.5">
            <option value="">없음</option><option>SUM</option><option>MAX</option>
          </select>
        </div>
        <div class="flex gap-1">
          <button class="px-2 py-1 bg-blue-600 text-white text-xs rounded" @click="addArea">저장</button>
          <button class="px-2 py-1 bg-gray-200 text-xs rounded" @click="showAddForm = false">취소</button>
        </div>
      </div>
    </div>

    <!-- ── 우측: 전형 요소 상세 (서브탭) ── -->
    <div class="flex-1 min-w-0" v-if="selected">
      <div class="flex items-center gap-2 mb-3">
        <h3 class="text-base font-semibold text-gray-800">{{ selected.name }}</h3>
        <span class="px-2 py-0.5 text-xs rounded bg-gray-100 text-gray-500">{{ selected.calc_type }}</span>
        <span class="px-2 py-0.5 text-xs rounded bg-gray-100 text-gray-500">{{ selected.lookup_scope }}</span>
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
        <p class="text-xs text-gray-500 mb-3">
          threshold·score는 ÷10000 표시값으로 작성 (예: 1.25, 30.5)
        </p>
        <ExcelPanel
          :area-id="selected.id"
          :calc-type="selected.calc_type"
          :area-name="selected.name"
          panel="score"
          @result="scoreResult = $event" />
        <ImportResultBox v-if="scoreResult" :result="scoreResult" class="mt-3" />
      </div>

      <!-- 기초 데이터 탭 -->
      <div v-if="activeTab === 'base'">
        <p class="text-xs text-gray-500 mb-3">
          student_code로 학생을 찾습니다. RANGE/MANUAL value는 ÷10000 표시값으로 작성.
        </p>
        <ExcelPanel
          :area-id="selected.id"
          :calc-type="selected.calc_type"
          :area-name="selected.name"
          panel="base"
          @result="baseResult = $event" />
        <ImportResultBox v-if="baseResult" :result="baseResult" class="mt-3" />
      </div>
    </div>

    <p v-else class="text-gray-400 text-sm mt-2">왼쪽에서 전형 요소를 선택하세요.</p>
  </div>
</template>

<script setup>
import { ref, watch, onMounted, defineComponent, h } from 'vue'
import {
  getAreas, createArea, deleteArea,
  downloadRangeTableTemplate, exportRangeTable, importRangeTable,
  downloadCategoryMapTemplate, exportCategoryMap, importCategoryMap,
  downloadBaseDataTemplate, exportBaseData, importBaseData,
} from '../../api/admin.js'

// ── 상태 ──────────────────────────────────────────────────────
const areas    = ref([])
const selected = ref(null)
const error    = ref('')
const activeTab   = ref('score')
const scoreResult = ref(null)
const baseResult  = ref(null)

const showAddForm = ref(false)
const newArea = ref(defaultNewArea())

function defaultNewArea() {
  return { name: '', max_score_display: 0, calc_type: 'RANGE',
           lookup_scope: 'SIMPLE', teacher_editable: true,
           range_direction: '', category_agg: '' }
}

// ── 전형 요소 목록 ─────────────────────────────────────────────────
async function load() {
  try { areas.value = await getAreas() }
  catch (e) { error.value = e.response?.data ?? e.message }
}

function selectArea(area) {
  selected.value = area
  activeTab.value = area.calc_type === 'MANUAL' ? 'base' : 'score'
  scoreResult.value = null
  baseResult.value  = null
}

async function addArea() {
  const body = {
    name: newArea.value.name,
    max_score: Math.round(newArea.value.max_score_display * 10000),
    calc_type: newArea.value.calc_type,
    lookup_scope: newArea.value.lookup_scope,
    teacher_editable: newArea.value.teacher_editable ? 1 : 0,
    range_direction: newArea.value.range_direction || null,
    category_agg: newArea.value.category_agg || null,
  }
  try {
    await createArea(body)
    showAddForm.value = false
    await load()
  } catch (e) { error.value = e.response?.data ?? e.message }
}

async function removeArea(id) {
  if (!confirm('전형 요소를 삭제하면 구간표·범주표·기초데이터도 함께 삭제됩니다. 계속할까요?')) return
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
            : await downloadRangeTableTemplate(props.areaId)
          saveBlob(res, props.calcType === 'CATEGORY'
            ? `${props.areaName}_category_map_template.xlsx`
            : `${props.areaName}_range_table_template.xlsx`)
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
            : await exportRangeTable(props.areaId)
          saveBlob(res, props.calcType === 'CATEGORY'
            ? `${props.areaName}_category_map.xlsx`
            : `${props.areaName}_range_table.xlsx`)
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
            : await importRangeTable(props.areaId, file)
        } else {
          result = await importBaseData(props.areaId, file)
        }
        emit('result', result)
      } catch (e) { err.value = e.response?.data ?? e.message }
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
          `업로드 완료 — ${r.rows ?? (r.inserted != null ? `신규 ${r.inserted}명, 수정 ${r.updated}명` : '')} 처리됨`),
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
