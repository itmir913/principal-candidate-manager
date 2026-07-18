<template>
  <div class="rounded-xl" style="padding: 18px 20px; background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04);">
    <p class="text-base font-semibold" style="color: #1e293b; margin: 0 0 4px;">점수 계산 테스트</p>
    <p class="text-base" style="color: #94a3b8; margin: 0 0 14px; line-height: 1.5;">
      담임교사가 값을 입력했을 때 계산될 점수를 미리 확인합니다. 저장되지 않습니다.
    </p>

    <!-- COMPOSITE 전형요소: 트랙 선택 -->
    <div v-if="isComposite" class="mb-4">
      <label class="block text-base font-medium mb-1.5" style="color: #64748b;">대학 / 모집단위</label>
      <select v-model="demoTrackId" class="text-base w-full"
        style="border: 1px solid #e2e8f0; border-radius: 8px; padding: 9px 12px; background: white; box-sizing: border-box;"
        @change="onTrackChange">
        <option :value="0">전역 기준</option>
        <option v-for="t in distinctTracks" :key="t.trackId" :value="t.trackId">{{ t.label }}</option>
      </select>
    </div>

    <!-- NUMERIC -->
    <div v-if="area.calc_type === 'NUMERIC'" class="mb-3">
      <label class="block text-base font-medium mb-1.5" style="color: #64748b;">기준값</label>
      <input v-model="numericInput" type="number" step="any"
        :placeholder="area.unit ? `데이터 입력 (${area.unit})` : '데이터 입력'"
        class="w-full text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
        style="border: 1px solid #e2e8f0; border-radius: 8px; padding: 9px 12px; box-sizing: border-box;"
        @input="schedulePreview" />
    </div>

    <!-- CATEGORY 단일 -->
    <div v-else-if="area.calc_type === 'CATEGORY' && !isMultiValue" class="mb-3">
      <label class="block text-base font-medium mb-1.5" style="color: #64748b;">범주 선택</label>
      <select v-model="categoryValue" class="text-base w-full"
        style="border: 1px solid #e2e8f0; border-radius: 8px; padding: 9px 12px; background: white; box-sizing: border-box;"
        @change="onSingleCategoryChange">
        <option value="">— 선택 —</option>
        <option v-for="c in currentCategories" :key="c" :value="c">{{ c }}</option>
      </select>
    </div>

    <!-- CATEGORY 복수(합산) -->
    <div v-else-if="area.calc_type === 'CATEGORY' && isMultiValue" class="mb-3">
      <label class="block text-base font-medium mb-2" style="color: #64748b;">범주 선택 (복수 가능)</label>
      <div v-if="currentCategories.length === 0" class="text-base" style="color: #94a3b8;">등록된 범주 없음</div>
      <div class="flex flex-col gap-1.5">
        <label v-for="c in currentCategories" :key="c"
          class="flex items-center gap-2 text-base cursor-pointer" style="color: #1e293b;">
          <input type="checkbox" :value="c" v-model="multiValues"
            class="accent-blue-600 w-4 h-4"
            @change="onMultiChange" />
          {{ c }}
        </label>
      </div>
    </div>

    <!-- MANUAL -->
    <div v-else-if="area.calc_type === 'MANUAL'" class="mb-3">
      <label class="block text-base font-medium mb-1.5" style="color: #64748b;">점수</label>
      <input v-model="numericInput" type="number" step="any"
        placeholder="점수 직접 입력 (점)"
        class="w-full text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
        style="border: 1px solid #e2e8f0; border-radius: 8px; padding: 9px 12px; box-sizing: border-box;"
        @input="schedulePreview" />
    </div>

    <!-- 결과 -->
    <div v-if="result !== null" style="margin-top: 4px;">
      <span v-if="result.error" class="text-base" style="color: #ef4444;">{{ result.error }}</span>
      <template v-else-if="result.score !== null && result.score !== undefined">
        <span class="text-base font-medium" style="color: #2563eb;">예상 점수: {{ Number(result.score).toFixed(2) }}점</span>
        <span v-if="result.warning" class="text-base ml-2" style="color: #d97706;">⚠ {{ result.warning }}</span>
      </template>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, watch } from 'vue'
import { adminAreaScorePreview } from '../../api/admin.js'

const props = defineProps({
  area: { type: Object, required: true },
  rows: { type: Array, required: true },
})

const emit = defineEmits(['highlight'])

const isComposite  = computed(() => props.area.lookup_scope === 'COMPOSITE')
const isMultiValue = computed(() => props.area.calc_type === 'CATEGORY' && props.area.category_agg === 'SUM')

const demoTrackId   = ref(0)
const numericInput  = ref('')
const categoryValue = ref('')
const multiValues   = ref([])
const result        = ref(null)
let previewTimer = null

// COMPOSITE: 로드된 rows에서 중복 없는 트랙 목록
const distinctTracks = computed(() => {
  const seen = new Set()
  const out  = []
  for (const row of props.rows) {
    if (row.track_id != null && !seen.has(row.track_id)) {
      seen.add(row.track_id)
      out.push({ trackId: row.track_id, label: `${row.univ_name} — ${row.track_name}` })
    }
  }
  return out
})

// CATEGORY: 현재 선택된 트랙에 해당하는 범주 목록
const currentCategories = computed(() => {
  if (props.area.calc_type !== 'CATEGORY') return []
  let filtered
  if (isComposite.value) {
    filtered = demoTrackId.value === 0
      ? props.rows.filter(r => r.track_id == null)
      : props.rows.filter(r => r.track_id === demoTrackId.value)
  } else {
    filtered = props.rows
  }
  return filtered.map(r => r.category)
})

function onTrackChange() {
  numericInput.value  = ''
  categoryValue.value = ''
  multiValues.value   = []
  result.value        = null
  clearTimeout(previewTimer)
  emit('highlight', { matchedKeys: [], trackId: null })
}

function schedulePreview() {
  clearTimeout(previewTimer)
  previewTimer = setTimeout(() => {
    const val = numericInput.value
    if (val !== '' && val != null) {
      callPreview([String(val)])
    } else {
      result.value = null
      emit('highlight', { matchedKeys: [], trackId: null })
    }
  }, 400)
}

function onSingleCategoryChange() {
  if (categoryValue.value) {
    callPreview([categoryValue.value])
  } else {
    result.value = null
    emit('highlight', { matchedKeys: [], trackId: null })
  }
}

function onMultiChange() {
  if (multiValues.value.length > 0) {
    callPreview([...multiValues.value])
  } else {
    result.value = null
    emit('highlight', { matchedKeys: [], trackId: null })
  }
}

async function callPreview(values) {
  try {
    const res = await adminAreaScorePreview(props.area.id, demoTrackId.value, values)
    result.value = res
    emit('highlight', { matchedKeys: res.matched_keys ?? [], trackId: demoTrackId.value })
  } catch (e) {
    result.value = { score: null, error: e.response?.data || e.message }
    emit('highlight', { matchedKeys: [], trackId: null })
  }
}

function resetDemo() {
  demoTrackId.value   = 0
  numericInput.value  = ''
  categoryValue.value = ''
  multiValues.value   = []
  result.value        = null
  clearTimeout(previewTimer)
  emit('highlight', { matchedKeys: [], trackId: null })
}

// 전형요소 변경 시 전체 초기화
watch(() => props.area.id, resetDemo)
// 점수 기준 목록은 페이지네이션 — 페이지가 바뀌면 트랙·범주 선택지의 파생 원본(rows)이
// 바뀌므로 데모를 초기화한다 (이전 페이지 트랙이 남아 하이라이팅이 오판되는 것 방지)
watch(() => props.rows, resetDemo)
</script>
