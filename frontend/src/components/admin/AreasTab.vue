<template>
  <div class="flex gap-4 h-full">
    <!-- 왼쪽: 영역 목록 -->
    <div class="w-72 shrink-0">
      <div class="flex items-center justify-between mb-3">
        <h2 class="text-lg font-semibold text-gray-700">영역 목록</h2>
        <button class="px-2 py-1 bg-blue-600 text-white text-sm rounded hover:bg-blue-700" @click="openAddArea">+</button>
      </div>

      <p v-if="error" class="text-red-500 text-sm mb-2">{{ error }}</p>

      <ul class="space-y-1">
        <li v-for="area in areas" :key="area.id"
          class="flex items-center justify-between px-3 py-2 rounded border cursor-pointer"
          :class="selected?.id === area.id ? 'border-blue-500 bg-blue-50' : 'border-gray-200 hover:bg-gray-50'"
          @click="selectArea(area)">
          <div>
            <span class="text-sm font-medium">{{ area.name }}</span>
            <span class="ml-2 text-xs text-gray-400">{{ area.calc_type }}</span>
          </div>
          <button class="text-red-400 text-xs hover:text-red-600 ml-2" @click.stop="removeArea(area.id)">삭제</button>
        </li>
        <li v-if="areas.length === 0" class="text-gray-400 text-sm px-2">영역 없음</li>
      </ul>

      <!-- 영역 추가 폼 -->
      <div v-if="showAddArea" class="mt-3 p-3 border border-blue-200 rounded bg-blue-50 space-y-2">
        <div>
          <label class="text-xs text-gray-500">이름</label>
          <input v-model="newArea.name" type="text" class="w-full border rounded px-2 py-1 text-sm mt-0.5" />
        </div>
        <div>
          <label class="text-xs text-gray-500">최대점수 (표시값)</label>
          <input v-model.number="newArea.max_score_display" type="number" step="0.0001"
            class="w-full border rounded px-2 py-1 text-sm mt-0.5" />
        </div>
        <div>
          <label class="text-xs text-gray-500">calc_type</label>
          <select v-model="newArea.calc_type" class="w-full border rounded px-2 py-1 text-sm mt-0.5">
            <option>RANGE</option><option>CATEGORY</option><option>MANUAL</option>
          </select>
        </div>
        <div>
          <label class="text-xs text-gray-500">lookup_scope</label>
          <select v-model="newArea.lookup_scope" class="w-full border rounded px-2 py-1 text-sm mt-0.5">
            <option>SIMPLE</option><option>COMPOSITE</option>
          </select>
        </div>
        <div class="flex items-center gap-2">
          <label class="text-xs text-gray-500">교사 편집 가능</label>
          <input v-model="newArea.teacher_editable" type="checkbox" />
        </div>
        <div v-if="newArea.calc_type === 'RANGE'">
          <label class="text-xs text-gray-500">range_direction</label>
          <select v-model="newArea.range_direction" class="w-full border rounded px-2 py-1 text-sm mt-0.5">
            <option value="">없음</option><option>UPPER</option><option>LOWER</option>
          </select>
        </div>
        <div v-if="newArea.calc_type === 'CATEGORY'">
          <label class="text-xs text-gray-500">category_agg</label>
          <select v-model="newArea.category_agg" class="w-full border rounded px-2 py-1 text-sm mt-0.5">
            <option value="">없음</option><option>SUM</option><option>MAX</option>
          </select>
        </div>
        <div class="flex gap-1">
          <button class="px-2 py-1 bg-blue-600 text-white text-xs rounded" @click="addArea">저장</button>
          <button class="px-2 py-1 bg-gray-200 text-xs rounded" @click="showAddArea = false">취소</button>
        </div>
      </div>
    </div>

    <!-- 오른쪽: 구간표/범주표 편집 -->
    <div class="flex-1 min-w-0">
      <template v-if="selected">
        <h3 class="text-base font-semibold text-gray-700 mb-3">
          {{ selected.name }} —
          <span class="text-sm text-gray-500">{{ selected.calc_type }}</span>
        </h3>

        <!-- RANGE 구간표 -->
        <template v-if="selected.calc_type === 'RANGE'">
          <p class="text-xs text-gray-500 mb-2">threshold·score 모두 ÷10000 표시, 저장 시 ×10000 변환</p>
          <table class="w-full text-sm border-collapse mb-2">
            <thead>
              <tr class="bg-gray-100 text-left">
                <th class="px-3 py-2 border-b">threshold</th>
                <th class="px-3 py-2 border-b">score</th>
                <th class="px-3 py-2 border-b w-12"></th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="(row, i) in rangeRows" :key="i">
                <td class="px-3 py-1 border-b">
                  <input v-model.number="row.threshold_display" type="number" step="0.0001"
                    class="w-28 border rounded px-2 py-0.5 text-sm" />
                </td>
                <td class="px-3 py-1 border-b">
                  <input v-model.number="row.score_display" type="number" step="0.0001"
                    class="w-28 border rounded px-2 py-0.5 text-sm" />
                </td>
                <td class="px-3 py-1 border-b">
                  <button class="text-red-400 text-xs hover:text-red-600" @click="rangeRows.splice(i, 1)">삭제</button>
                </td>
              </tr>
            </tbody>
          </table>
          <div class="flex gap-2">
            <button class="px-2 py-1 bg-gray-200 text-xs rounded hover:bg-gray-300" @click="rangeRows.push({ threshold_display: 0, score_display: 0 })">+ 행 추가</button>
            <button class="px-3 py-1 bg-blue-600 text-white text-xs rounded hover:bg-blue-700" @click="saveRangeTable">저장</button>
          </div>
        </template>

        <!-- CATEGORY 범주표 -->
        <template v-else-if="selected.calc_type === 'CATEGORY'">
          <p class="text-xs text-gray-500 mb-2">score ÷10000 표시, 저장 시 ×10000 변환</p>
          <table class="w-full text-sm border-collapse mb-2">
            <thead>
              <tr class="bg-gray-100 text-left">
                <th class="px-3 py-2 border-b">범주</th>
                <th class="px-3 py-2 border-b">score</th>
                <th class="px-3 py-2 border-b w-12"></th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="(row, i) in categoryRows" :key="i">
                <td class="px-3 py-1 border-b">
                  <input v-model="row.category" type="text"
                    class="w-28 border rounded px-2 py-0.5 text-sm" />
                </td>
                <td class="px-3 py-1 border-b">
                  <input v-model.number="row.score_display" type="number" step="0.0001"
                    class="w-28 border rounded px-2 py-0.5 text-sm" />
                </td>
                <td class="px-3 py-1 border-b">
                  <button class="text-red-400 text-xs hover:text-red-600" @click="categoryRows.splice(i, 1)">삭제</button>
                </td>
              </tr>
            </tbody>
          </table>
          <div class="flex gap-2">
            <button class="px-2 py-1 bg-gray-200 text-xs rounded hover:bg-gray-300" @click="categoryRows.push({ category: '', score_display: 0 })">+ 행 추가</button>
            <button class="px-3 py-1 bg-blue-600 text-white text-xs rounded hover:bg-blue-700" @click="saveCategoryMap">저장</button>
          </div>
        </template>

        <template v-else>
          <p class="text-gray-400 text-sm">MANUAL 영역은 별도 테이블이 없습니다.</p>
        </template>

        <p v-if="tableError" class="text-red-500 text-sm mt-2">{{ tableError }}</p>
      </template>
      <p v-else class="text-gray-400 text-sm">왼쪽에서 영역을 선택하세요.</p>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import {
  getAreas, createArea, deleteArea,
  getRangeTable, putRangeTable,
  getCategoryMap, putCategoryMap,
} from '../../api/admin.js'

const areas = ref([])
const selected = ref(null)
const error = ref('')
const tableError = ref('')
const rangeRows = ref([])
const categoryRows = ref([])

const showAddArea = ref(false)
const newArea = ref(defaultNewArea())

function defaultNewArea() {
  return { name: '', max_score_display: 0, calc_type: 'RANGE', lookup_scope: 'SIMPLE', teacher_editable: true, range_direction: '', category_agg: '' }
}

async function load() {
  try { areas.value = await getAreas() } catch (e) { error.value = e.response?.data ?? e.message }
}

async function selectArea(area) {
  selected.value = area
  tableError.value = ''
  rangeRows.value = []
  categoryRows.value = []
  try {
    if (area.calc_type === 'RANGE') {
      const rows = await getRangeTable(area.id)
      rangeRows.value = rows.map(r => ({ threshold_display: r.threshold / 10000, score_display: r.score / 10000 }))
    } else if (area.calc_type === 'CATEGORY') {
      const rows = await getCategoryMap(area.id)
      categoryRows.value = rows.map(r => ({ category: r.category, score_display: r.score / 10000 }))
    }
  } catch (e) { tableError.value = e.response?.data ?? e.message }
}

async function saveRangeTable() {
  try {
    const rows = rangeRows.value.map(r => ({
      threshold: Math.round(r.threshold_display * 10000),
      score: Math.round(r.score_display * 10000),
    }))
    await putRangeTable(selected.value.id, rows)
  } catch (e) { tableError.value = e.response?.data ?? e.message }
}

async function saveCategoryMap() {
  try {
    const rows = categoryRows.value.map(r => ({
      category: r.category,
      score: Math.round(r.score_display * 10000),
    }))
    await putCategoryMap(selected.value.id, rows)
  } catch (e) { tableError.value = e.response?.data ?? e.message }
}

function openAddArea() {
  newArea.value = defaultNewArea()
  showAddArea.value = true
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
    showAddArea.value = false
    await load()
  } catch (e) { error.value = e.response?.data ?? e.message }
}

async function removeArea(id) {
  if (!confirm('영역을 삭제하면 구간표·범주표도 함께 삭제됩니다. 계속하시겠습니까?')) return
  try {
    await deleteArea(id)
    if (selected.value?.id === id) selected.value = null
    await load()
  } catch (e) { error.value = e.response?.data ?? e.message }
}

onMounted(load)
</script>
