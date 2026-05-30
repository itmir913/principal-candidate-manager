<template>
  <div class="flex gap-4" style="height: calc(100vh - 210px)">
    <!-- ── 좌측: 학생 목록 ── -->
    <div class="w-52 flex-shrink-0 bg-white border rounded-lg flex flex-col overflow-hidden">
      <div class="px-3 py-2 border-b bg-gray-50 text-xs font-semibold text-gray-600">
        학생 목록 ({{ students.length }}명)
      </div>
      <div class="overflow-y-auto flex-1">
        <div
          v-for="s in students"
          :key="s.id"
          class="px-3 py-2.5 text-sm cursor-pointer border-b border-gray-100 hover:bg-blue-50 flex items-center justify-between"
          :class="selectedStudent?.id === s.id
            ? 'bg-blue-50 border-l-2 border-l-blue-500'
            : 'border-l-2 border-l-transparent'"
          @click="selectStudent(s)"
        >
          <span class="font-medium text-gray-800">{{ s.seq_no }}번 {{ s.name }}</span>
          <span
            v-if="getStudentAppCount(s.id) > 0"
            class="text-xs text-blue-600 font-semibold"
          >{{ getStudentAppCount(s.id) }}</span>
        </div>
      </div>
    </div>

    <!-- ── 우측: 지원 등록 영역 ── -->
    <div class="flex-1 overflow-y-auto">
      <!-- 학생 미선택 -->
      <div v-if="!selectedStudent" class="bg-white border rounded-lg p-10 text-center text-gray-400 text-sm">
        좌측에서 학생을 선택하세요.
      </div>

      <template v-else>
        <!-- 현재 라운드 지원 현황 -->
        <div class="bg-white border rounded-lg p-4 mb-4">
          <div class="flex items-center justify-between mb-3">
            <h2 class="text-sm font-semibold text-gray-700">
              {{ selectedStudent.name }} 학생의 {{ currentRound.id }}차 라운드 지원 현황
            </h2>
            <button
              v-if="!showForm"
              class="px-3 py-1.5 text-xs bg-blue-600 text-white rounded hover:bg-blue-700"
              @click="openNewForm"
            >+ 새 지원 추가</button>
          </div>

          <div v-if="studentApps.length === 0 && !showForm" class="text-sm text-gray-400">
            등록된 지원이 없습니다.
          </div>

          <div v-for="app in studentApps" :key="`${app.track_id}`" class="flex items-center gap-2 mb-1.5">
            <span class="text-sm text-gray-700">
              {{ app.univ_name }} — {{ app.track_name }}
              <span v-if="app.department_name" class="text-gray-500"> ({{ app.department_name }})</span>
            </span>
            <button
              class="text-xs px-1.5 py-0.5 border border-gray-300 text-gray-400 rounded hover:border-red-300 hover:text-red-400"
              :disabled="deletingApp === app.track_id"
              @click="deleteApp(app)"
            >취소</button>
          </div>
        </div>

        <!-- 새 지원 등록 폼 -->
        <div v-if="showForm" class="bg-white border rounded-lg p-4">
          <div class="flex items-center justify-between mb-4">
            <h3 class="text-sm font-semibold text-gray-700">새 지원 등록</h3>
            <button
              class="text-xs text-gray-400 hover:text-gray-600"
              @click="closeForm"
            >닫기</button>
          </div>

          <!-- 대학/모집단위/학과명 -->
          <div class="grid grid-cols-3 gap-3 mb-5">
            <div>
              <label class="block text-xs text-gray-500 mb-1">대학 <span class="text-red-500">*</span></label>
              <select
                v-model="form.univId"
                class="w-full border rounded px-2 py-1.5 text-sm"
                @change="onUnivChange"
              >
                <option value="">대학 선택</option>
                <option v-for="u in universities" :key="u.id" :value="u.id">{{ u.univ_name }}</option>
              </select>
            </div>
            <div>
              <label class="block text-xs text-gray-500 mb-1">모집단위 <span class="text-red-500">*</span></label>
              <select
                v-model="form.trackId"
                class="w-full border rounded px-2 py-1.5 text-sm"
                :disabled="!form.univId || tracksLoading"
                @change="onTrackChange"
              >
                <option value="">모집단위 선택</option>
                <option v-for="t in form.tracks" :key="t.id" :value="t.id">{{ t.track_name }}</option>
              </select>
            </div>
            <div>
              <label class="block text-xs text-gray-500 mb-1">학과명 <span class="text-red-500">*</span></label>
              <input
                v-model="form.departmentName"
                type="text"
                placeholder="예: 컴퓨터공학과"
                class="w-full border rounded px-2 py-1.5 text-sm"
                :disabled="!form.trackId"
              />
            </div>
          </div>

          <!-- 전형요소 섹션 -->
          <div v-if="contextLoading" class="text-sm text-gray-400 py-4 text-center">
            전형요소 로딩 중...
          </div>

          <div v-else-if="areaContext.length > 0">
            <div class="text-xs font-semibold text-gray-500 uppercase tracking-wide mb-3">전형요소</div>

            <div class="grid gap-3" :class="areaGridClass">
            <div
              v-for="area in areaContext"
              :key="area.area_id"
              class="border rounded-lg p-3"
              :class="area.teacher_editable ? 'border-gray-200' : 'border-gray-100 bg-gray-50'"
            >
              <!-- 전형요소 헤더 -->
              <div class="mb-2">
                <div class="flex items-center justify-between gap-2">
                  <div class="flex items-center gap-2 min-w-0">
                    <span class="text-sm font-medium text-gray-800 truncate">{{ area.area_name }}</span>
                    <span class="text-xs px-1.5 py-0.5 rounded bg-gray-100 text-gray-500 flex-shrink-0">{{ area.calc_type }}</span>
                  </div>
                  <span
                    v-if="!area.teacher_editable"
                    class="text-xs px-1.5 py-0.5 rounded bg-amber-100 text-amber-700 flex-shrink-0"
                  >관리자 입력 고정</span>
                </div>
                <div class="flex items-center justify-between mt-0.5">
                  <span class="text-xs text-gray-400">만점 {{ area.max_score }}</span>
                  <template v-if="scorePreview[area.area_id]">
                    <span v-if="scorePreview[area.area_id].error" class="text-xs text-red-500">
                      {{ scorePreview[area.area_id].error }}
                    </span>
                    <span
                      v-else-if="scorePreview[area.area_id].score !== null && scorePreview[area.area_id].score !== undefined"
                      class="text-xs text-blue-600 font-medium"
                    >
                      예상 {{ scorePreview[area.area_id].score }}점
                      <span v-if="scorePreview[area.area_id].warning" class="text-amber-600"> ⚠</span>
                    </span>
                  </template>
                </div>
              </div>

              <!-- 점수표 (위) -->
              <div
                v-if="area.table && area.table.length > 0"
                :ref="el => setTableRef(el, area.area_id)"
                class="border rounded overflow-hidden text-xs max-h-40 overflow-y-auto mb-2"
              >
                <table class="w-full">
                  <thead class="sticky top-0">
                    <tr class="bg-gray-50 border-b">
                      <th class="px-2 py-1 text-left text-gray-500 font-medium">
                        {{ area.calc_type === 'NUMERIC' ? '기준값' : '범주' }}
                      </th>
                      <th class="px-2 py-1 text-right text-gray-500 font-medium">점수</th>
                    </tr>
                  </thead>
                  <tbody>
                    <tr
                      v-for="row in area.table"
                      :key="row.key"
                      :data-highlighted="isHighlighted(area, row.key) || null"
                      class="border-b last:border-b-0 transition-colors duration-300"
                      :class="isHighlighted(area, row.key)
                        ? 'bg-yellow-100 font-semibold text-yellow-900'
                        : 'text-gray-600'"
                    >
                      <td class="px-2 py-1">{{ row.key }}</td>
                      <td class="px-2 py-1 text-right">{{ row.score }}</td>
                    </tr>
                  </tbody>
                </table>
              </div>

              <!-- 입력 영역 (아래) -->
              <div>
                <!-- NUMERIC -->
                <template v-if="area.calc_type === 'NUMERIC'">
                  <input
                    :value="areaValues[area.area_id] ?? ''"
                    type="number"
                    step="any"
                    class="w-full border rounded px-2 py-1.5 text-sm"
                    :class="area.teacher_editable ? '' : 'bg-gray-100 text-gray-500'"
                    :disabled="!area.teacher_editable"
                    :placeholder="area.teacher_editable ? '데이터 입력' : (area.current_values[0] ?? '데이터 없음')"
                    @input="onNumericInput(area, $event.target.value)"
                  />
                </template>

                <!-- CATEGORY 단일값 -->
                <template v-else-if="area.calc_type === 'CATEGORY' && !area.multi_value">
                  <select
                    :value="areaValues[area.area_id] ?? ''"
                    class="w-full border rounded px-2 py-1.5 text-sm"
                    :class="area.teacher_editable ? '' : 'bg-gray-100 text-gray-500'"
                    :disabled="!area.teacher_editable"
                    @change="onCategoryChange(area, $event.target.value)"
                  >
                    <option value="">선택하세요</option>
                    <option v-for="row in area.table" :key="row.key" :value="row.key">{{ row.key }}</option>
                  </select>
                </template>

                <!-- CATEGORY 복수값 -->
                <template v-else-if="area.calc_type === 'CATEGORY' && area.multi_value">
                  <div class="space-y-1">
                    <label
                      v-for="row in area.table"
                      :key="row.key"
                      class="flex items-center gap-2 text-sm"
                    >
                      <input
                        type="checkbox"
                        :value="row.key"
                        :checked="(areaMultiValues[area.area_id] || []).includes(row.key)"
                        :disabled="!area.teacher_editable"
                        class="rounded"
                        @change="onMultiValueChange(area, row.key, $event.target.checked)"
                      />
                      <span :class="area.teacher_editable ? 'text-gray-700' : 'text-gray-400'">
                        {{ row.key }}
                      </span>
                    </label>
                  </div>
                </template>

                <!-- MANUAL -->
                <template v-else>
                  <input
                    :value="areaValues[area.area_id] ?? ''"
                    type="number"
                    step="any"
                    class="w-full border rounded px-2 py-1.5 text-sm"
                    :class="area.teacher_editable ? '' : 'bg-gray-100 text-gray-500'"
                    :disabled="!area.teacher_editable"
                    :placeholder="area.teacher_editable ? '점수 직접 입력' : (area.current_values[0] ?? '데이터 없음')"
                    @input="onNumericInput(area, $event.target.value)"
                  />
                </template>

              </div>
            </div>
            </div>
          </div>

          <!-- 저장 버튼 -->
          <div class="flex items-center gap-3 mt-4 pt-4 border-t">
            <button
              class="px-5 py-2 text-sm bg-blue-600 text-white rounded hover:bg-blue-700 disabled:opacity-40"
              :disabled="!canSave || saving"
              @click="saveApplication"
            >{{ saving ? '등록 중...' : '저장' }}</button>
            <button
              class="px-4 py-2 text-sm border rounded text-gray-600 hover:bg-gray-50"
              @click="closeForm"
            >취소</button>
            <span v-if="saveError" class="text-xs text-red-500">{{ saveError }}</span>
          </div>
        </div>
      </template>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, reactive, watch } from 'vue'
import { useAuthStore } from '../../stores/auth.js'
import {
  getCurrentRound,
  teacherGetStudents,
  teacherGetApplications,
  teacherGetUniversities,
  teacherGetUnivTracks,
  teacherGetAreaContext,
  teacherAreaScorePreview,
  teacherCreateApplication,
  teacherDeleteApplication,
} from '../../api/teacher.js'

const auth = useAuthStore()

// ── 페이지 상태 ───────────────────────────────────────────────────
const currentRound  = ref(null)
const students      = ref([])
const applications  = ref([])
const universities  = ref([])

const selectedStudent = ref(null)
const showForm        = ref(false)
const saving          = ref(false)
const saveError       = ref('')
const deletingApp     = ref(null)

// ── 폼 상태 ───────────────────────────────────────────────────────
const form = reactive({
  univId:         '',
  tracks:         [],
  trackId:        '',
  departmentName: '',
})
const tracksLoading = ref(false)
const contextLoading = ref(false)

// areaValues: { [area_id]: string }  (NUMERIC, MANUAL, CATEGORY 단일)
const areaValues      = ref({})
// areaMultiValues: { [area_id]: string[] }  (CATEGORY 복수)
const areaMultiValues = ref({})
// scorePreview: { [area_id]: { score, matched_keys, warning, error } }
const scorePreview    = ref({})
const areaContext     = ref([])

// 입력 디바운스 타이머
const previewTimers = {}

// 점수표 컨테이너 ref (area_id → DOM element)
const tableRefs = {}
function setTableRef(el, areaId) {
  if (el) tableRefs[areaId] = el
  else delete tableRefs[areaId]
}

watch(scorePreview, () => {
  for (const areaId of Object.keys(tableRefs)) {
    const container = tableRefs[areaId]
    if (!container) continue
    const highlighted = container.querySelector('[data-highlighted]')
    if (!highlighted) continue

    const theadHeight = container.querySelector('thead')?.offsetHeight ?? 0
    const rowTop    = highlighted.offsetTop
    const rowBottom = rowTop + highlighted.offsetHeight
    const visTop    = container.scrollTop + theadHeight
    const visBottom = container.scrollTop + container.clientHeight

    if (rowTop < visTop) {
      container.scrollTo({ top: rowTop - theadHeight, behavior: 'smooth' })
    } else if (rowBottom > visBottom) {
      container.scrollTo({ top: rowBottom - container.clientHeight, behavior: 'smooth' })
    }
  }
}, { deep: true, flush: 'post' })

// ── Computed ──────────────────────────────────────────────────────
const studentApps = computed(() =>
  applications.value.filter(a => a.student_id === selectedStudent.value?.id)
)

function getStudentAppCount(sid) {
  return applications.value.filter(a => a.student_id === sid).length
}

const areaGridClass = computed(() => {
  const n = areaContext.value.length
  if (n >= 5 && n <= 6) return 'grid-cols-1 md:grid-cols-2 xl:grid-cols-3'
  return 'grid-cols-1 md:grid-cols-2 xl:grid-cols-4'
})

const canSave = computed(() =>
  !!selectedStudent.value && !!form.trackId && !!currentRound.value && !!form.departmentName.trim()
)

// ── 초기 로드 ─────────────────────────────────────────────────────
async function loadAll() {
  const [round, sts, univs] = await Promise.all([
    getCurrentRound(),
    teacherGetStudents(),
    teacherGetUniversities(),
  ])
  currentRound.value = round
  students.value = sts
  universities.value = univs

  if (round) {
    applications.value = await teacherGetApplications(round.id)
  }
}

// ── 학생 선택 ─────────────────────────────────────────────────────
function selectStudent(s) {
  selectedStudent.value = s
  closeForm()
}

// ── 폼 열기/닫기 ─────────────────────────────────────────────────
function openNewForm() {
  showForm.value = true
  form.univId = ''
  form.tracks = []
  form.trackId = ''
  form.departmentName = ''
  areaContext.value = []
  areaValues.value = {}
  areaMultiValues.value = {}
  scorePreview.value = {}
  saveError.value = ''
}

function closeForm() {
  showForm.value = false
  saveError.value = ''
}

// ── 대학 선택 → 모집단위 로드 ─────────────────────────────────────
async function onUnivChange() {
  form.trackId = ''
  form.tracks = []
  areaContext.value = []
  areaValues.value = {}
  areaMultiValues.value = {}
  scorePreview.value = {}

  if (!form.univId) return
  tracksLoading.value = true
  try {
    form.tracks = await teacherGetUnivTracks(form.univId)
  } finally {
    tracksLoading.value = false
  }
}

// ── 모집단위 선택 → area-context 로드 ────────────────────────────
async function onTrackChange() {
  areaContext.value = []
  areaValues.value = {}
  areaMultiValues.value = {}
  scorePreview.value = {}

  if (!form.trackId || !selectedStudent.value) return
  contextLoading.value = true
  try {
    const ctx = await teacherGetAreaContext(selectedStudent.value.id, form.trackId)
    areaContext.value = ctx
    initAreaValues(ctx)
    // 기저장 값이 있는 항목에 대해 즉시 점수 계산
    await triggerInitialPreviews(ctx)
  } finally {
    contextLoading.value = false
  }
}

function initAreaValues(context) {
  const vals = {}
  const multiVals = {}
  for (const area of context) {
    if (!area.teacher_editable) continue
    if (area.multi_value) {
      multiVals[area.area_id] = [...area.current_values]
    } else {
      vals[area.area_id] = area.current_values[0] ?? ''
    }
  }
  areaValues.value = vals
  areaMultiValues.value = multiVals
}

async function triggerInitialPreviews(context) {
  for (const area of context) {
    const vals = area.teacher_editable
      ? getAreaInputValues(area)
      : area.current_values.filter(v => v !== '')
    if (vals.length > 0) {
      await fetchScorePreview(area, vals)
    }
  }
}

// ── 입력 이벤트 ───────────────────────────────────────────────────
function onNumericInput(area, value) {
  areaValues.value = { ...areaValues.value, [area.area_id]: value }
  schedulePreview(area)
}

function onCategoryChange(area, value) {
  areaValues.value = { ...areaValues.value, [area.area_id]: value }
  if (value) fetchScorePreview(area, [value])
  else scorePreview.value = { ...scorePreview.value, [area.area_id]: null }
}

function onMultiValueChange(area, key, checked) {
  const current = [...(areaMultiValues.value[area.area_id] || [])]
  if (checked) {
    if (!current.includes(key)) current.push(key)
  } else {
    const idx = current.indexOf(key)
    if (idx !== -1) current.splice(idx, 1)
  }
  areaMultiValues.value = { ...areaMultiValues.value, [area.area_id]: current }
  if (current.length > 0) fetchScorePreview(area, current)
  else scorePreview.value = { ...scorePreview.value, [area.area_id]: null }
}

function schedulePreview(area, delay = 400) {
  clearTimeout(previewTimers[area.area_id])
  previewTimers[area.area_id] = setTimeout(() => {
    const vals = getAreaInputValues(area)
    if (vals.length > 0 && vals[0] !== '') {
      fetchScorePreview(area, vals)
    } else {
      scorePreview.value = { ...scorePreview.value, [area.area_id]: null }
    }
  }, delay)
}

function getAreaInputValues(area) {
  if (area.multi_value) {
    return areaMultiValues.value[area.area_id] || []
  }
  const v = areaValues.value[area.area_id]
  return v !== undefined && v !== '' ? [String(v)] : []
}

async function fetchScorePreview(area, values) {
  try {
    const result = await teacherAreaScorePreview(area.area_id, Number(form.trackId), values)
    scorePreview.value = { ...scorePreview.value, [area.area_id]: result }
  } catch (e) {
    scorePreview.value = {
      ...scorePreview.value,
      [area.area_id]: { score: null, matched_keys: [], warning: null, error: e.response?.data || e.message },
    }
  }
}

// ── 점수표 하이라이팅 ─────────────────────────────────────────────
function isHighlighted(area, rowKey) {
  const preview = scorePreview.value[area.area_id]
  if (!preview?.matched_keys?.length) return false
  if (area.calc_type === 'NUMERIC') {
    return preview.matched_keys.some(
      mk => typeof mk === 'number' && Math.abs(mk - rowKey) < 1e-9
    )
  }
  return preview.matched_keys.includes(rowKey)
}

// ── 저장 ──────────────────────────────────────────────────────────
async function saveApplication() {
  if (!canSave.value) return
  saving.value = true
  saveError.value = ''

  const baseDataEntries = areaContext.value
    .filter(a => a.teacher_editable)
    .map(a => {
      const values = a.multi_value
        ? (areaMultiValues.value[a.area_id] || [])
        : [String(areaValues.value[a.area_id] ?? '')].filter(v => v !== '')
      return { area_id: a.area_id, values }
    })
    .filter(e => e.values.length > 0)

  try {
    await teacherCreateApplication({
      student_id:          selectedStudent.value.id,
      track_id:            Number(form.trackId),
      round_id:            currentRound.value.id,
      department_name:     form.departmentName,
      base_data_entries:   baseDataEntries,
    })
    applications.value = await teacherGetApplications(currentRound.value.id)
    closeForm()
  } catch (e) {
    saveError.value = e.response?.data || e.message
  } finally {
    saving.value = false
  }
}

// ── 삭제 ──────────────────────────────────────────────────────────
async function deleteApp(app) {
  if (!confirm(`${app.univ_name} ${app.track_name} 지원을 취소하시겠습니까?`)) return
  deletingApp.value = app.track_id
  try {
    await teacherDeleteApplication(app.student_id, app.track_id, app.round_id)
    applications.value = await teacherGetApplications(currentRound.value.id)
  } catch (e) {
    alert(e.response?.data || e.message)
  } finally {
    deletingApp.value = null
  }
}

onMounted(loadAll)
</script>
