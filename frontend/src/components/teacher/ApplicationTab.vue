<template>
  <!-- 전체 레이아웃: 세로 flex, 뷰포트 전체 높이 -->
  <div class="flex flex-col">

    <!-- 페이지 헤더 -->
    <div class="flex-shrink-0 pt-8 pb-5 px-4 sm:px-10">
      <p class="text-base mb-1" style="color: #94a3b8;">담임 교사</p>
      <h1 class="text-2xl font-semibold" style="color: #1e293b; margin: 0;">지원자 등록</h1>
    </div>

    <!-- 두 열 레이아웃 (남은 높이 전체 차지) -->
    <div class="flex flex-col lg:flex-row gap-6 px-4 sm:px-10 pb-8">

      <!-- ── 좌측: 학생 목록 ── -->
      <div
        class="flex flex-col w-full lg:flex-shrink-0 lg:w-[220px] lg:overflow-hidden rounded-xl"
        style="background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04);"
      >
        <div class="flex-shrink-0 text-base font-semibold" style="padding: 14px 16px; border-bottom: 1px solid #e2e8f0; color: #475569;">
          학생 목록 ({{ students.length }}명)
        </div>
        <div>
          <div
            v-for="s in students"
            :key="s.id"
            class="flex items-center justify-between cursor-pointer text-base"
            style="padding: 12px 16px; border-bottom: 1px solid #f1f5f9; border-left: 3px solid transparent; transition: background 0.1s;"
            :style="{
              background: selectedStudent?.id === s.id ? '#eff6ff' : 'transparent',
              borderLeftColor: selectedStudent?.id === s.id ? '#2563eb' : 'transparent',
            }"
            @click="selectStudent(s)"
          >
            <span class="font-medium" style="color: #1e293b;">
              <template v-if="auth.grade === 0">{{ s.student_code }} {{ s.name }}</template>
              <template v-else>{{ s.seq_no }}번 {{ s.name }}</template>
            </span>
            <span
              v-if="getStudentAppCount(s.id) > 0"
              class="text-base font-semibold"
              style="color: #2563eb;"
            >{{ getStudentAppCount(s.id) }}</span>
          </div>
        </div>
      </div>

      <!-- ── 우측: 지원 등록 영역 ── -->
      <div class="lg:flex-1 min-w-0 @container">

        <!-- 학생 미선택 -->
        <div
          v-if="!selectedStudent"
          class="rounded-xl flex items-center justify-center"
          style="background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04); height: 240px;"
        >
          <p class="text-base" style="color: #94a3b8;">좌측에서 학생을 선택하세요.</p>
        </div>

        <template v-else>
          <!-- 현재 라운드 지원 현황 -->
          <div
            class="rounded-xl mb-4"
            style="background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04); padding: 1.25rem 1.5rem;"
          >
            <div class="flex items-center justify-between">
              <h2 class="text-base font-semibold" style="color: #1e293b; margin: 0;">
                {{ selectedStudent.name }} 학생의 {{ currentRound.id }}차 라운드 지원 현황
              </h2>
              <button
                class="text-base font-semibold"
                style="padding: 8px 18px; border: none; background: #2563eb; color: white; border-radius: 8px; cursor: pointer;"
                :style="{ visibility: showForm ? 'hidden' : 'visible' }"
                @click="openNewForm"
              >+ 새 지원 추가</button>
            </div>

            <div v-if="studentApps.length === 0 && !showForm" class="text-base" style="color: #94a3b8; margin-top: 4px;">
              등록된 지원이 없습니다.
            </div>

            <div v-for="app in studentApps" :key="`${app.track_id}`" class="flex items-center gap-2 mb-2">
              <span class="text-base" style="color: #1e293b;">
                {{ app.univ_name }} — {{ app.track_name }}
                <span v-if="app.department_name" style="color: #64748b;"> ({{ app.department_name }})</span>
              </span>
              <button
                class="text-base"
                style="padding: 4px 12px; border: 1px solid #fca5a5; border-radius: 6px; background: white; color: #ef4444; cursor: pointer;"
                :disabled="deletingApp === app.track_id"
                @click="deleteApp(app)"
              >취소</button>
            </div>
          </div>

          <!-- 새 지원 등록 폼 -->
          <div
            v-if="showForm"
            class="rounded-xl"
            style="background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04); padding: 1.5rem;"
          >
            <div class="flex items-center justify-between mb-5">
              <h3 class="text-base font-semibold" style="color: #1e293b; margin: 0;">새 지원 등록</h3>
              <button
                class="text-base"
                style="background: none; border: none; cursor: pointer; color: #94a3b8;"
                @click="closeForm"
              >닫기</button>
            </div>

            <!-- 대학 / 모집단위 / 학과명 -->
            <div class="grid grid-cols-1 @4xl:grid-cols-3 gap-4 mb-6">
              <div>
                <label class="block text-base font-medium mb-1.5" style="color: #64748b;">
                  대학 <span style="color: #ef4444;">*</span>
                </label>
                <select
                  v-model="form.univId"
                  class="w-full text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
                  style="border: 1px solid #e2e8f0; border-radius: 8px; padding: 10px 12px; box-sizing: border-box;"
                  @change="onUnivChange"
                >
                  <option value="">대학 선택</option>
                  <option v-for="u in universities" :key="u.id" :value="u.id">{{ u.univ_name }}</option>
                </select>
              </div>
              <div>
                <label class="block text-base font-medium mb-1.5" style="color: #64748b;">
                  모집단위 <span style="color: #ef4444;">*</span>
                </label>
                <select
                  v-model="form.trackId"
                  class="w-full text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
                  style="border: 1px solid #e2e8f0; border-radius: 8px; padding: 10px 12px; box-sizing: border-box;"
                  :disabled="!form.univId || tracksLoading"
                  @change="onTrackChange"
                >
                  <option value="">모집단위 선택</option>
                  <option v-for="t in form.tracks" :key="t.id" :value="t.id">{{ t.track_name }}</option>
                </select>
              </div>
              <div>
                <label class="block text-base font-medium mb-1.5" style="color: #64748b;">
                  학과명 <span style="color: #ef4444;">*</span>
                </label>
                <input
                  v-model="form.departmentName"
                  type="text"
                  placeholder="예: 컴퓨터공학과"
                  class="w-full text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
                  style="border: 1px solid #e2e8f0; border-radius: 8px; padding: 10px 12px; box-sizing: border-box;"
                  :disabled="!form.trackId"
                />
              </div>
            </div>

            <!-- 전형요소 섹션 -->
            <div v-if="contextLoading" class="text-base text-center" style="color: #94a3b8; padding: 2rem 0;">
              전형요소 로딩 중...
            </div>

            <div v-else-if="areaContext.length > 0">
              <p class="text-base font-semibold mb-4" style="color: #475569; text-transform: uppercase; letter-spacing: 0.05em;">
                전형요소
              </p>

              <div class="grid gap-4" style="grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));">
                <div
                  v-for="area in areaContext"
                  :key="area.area_id"
                  class="rounded-xl"
                  style="padding: 1rem 1.125rem;"
                  :style="{
                    border: area.teacher_editable ? '1px solid #e2e8f0' : '1px solid #f1f5f9',
                    background: area.teacher_editable ? 'white' : '#f8fafc',
                  }"
                >
                  <!-- 전형요소 헤더 -->
                  <div class="mb-3">
                    <div class="flex items-center justify-between gap-2">
                      <div class="flex items-center gap-2 min-w-0">
                        <span class="text-base font-semibold truncate" style="color: #1e293b;">{{ area.area_name }}</span>
                        <span
                          class="text-base flex-shrink-0"
                          style="padding: 2px 8px; border-radius: 6px; background: #f1f5f9; color: #64748b;"
                        >{{ area.calc_type }}</span>
                      </div>
                      <span
                        v-if="!area.teacher_editable"
                        class="text-base flex-shrink-0"
                        style="padding: 2px 8px; border-radius: 6px; background: #fffbeb; color: #92400e;"
                      >관리자 입력 고정</span>
                    </div>
                    <div class="flex items-center justify-between mt-1">
                      <span class="text-base" style="color: #94a3b8;">만점 {{ area.max_score }}</span>
                      <template v-if="scorePreview[area.area_id]">
                        <span v-if="scorePreview[area.area_id].error" class="text-base" style="color: #ef4444;">
                          {{ scorePreview[area.area_id].error }}
                        </span>
                        <span
                          v-else-if="scorePreview[area.area_id].score !== null && scorePreview[area.area_id].score !== undefined"
                          class="text-base font-medium"
                          style="color: #2563eb;"
                        >
                          예상 {{ scorePreview[area.area_id].score }}점
                          <span v-if="scorePreview[area.area_id].warning" style="color: #d97706;"> ⚠</span>
                        </span>
                      </template>
                    </div>
                  </div>

                  <!-- 점수표 -->
                  <div
                    v-if="area.table && area.table.length > 0"
                    :ref="el => setTableRef(el, area.area_id)"
                    class="rounded-lg overflow-hidden overflow-y-auto mb-3"
                    style="border: 1px solid #e2e8f0; max-height: 240px;"
                  >
                    <table class="w-full" style="border-collapse: collapse;">
                      <thead class="sticky top-0">
                      <tr style="background: #f8fafc; border-bottom: 1px solid #e2e8f0;">
                        <th class="text-base font-semibold text-left" style="padding: 8px 12px; color: #475569;">
                          {{ area.calc_type === 'NUMERIC' ? '기준값' : '범주' }}
                        </th>

                        <th class="text-base font-semibold text-right whitespace-nowrap w-12" style="padding: 8px 12px; color: #475569;">
                          점수
                        </th>
                      </tr>
                      </thead>
                      <tbody>
                        <tr
                            v-for="row in area.table"
                            :key="row.key"
                            :data-highlighted="isHighlighted(area, row.key) || null"
                            style="border-bottom: 1px solid #f1f5f9; transition: background 0.3s;"
                            :style="{
                              background: isHighlighted(area, row.key) ? '#fefce8' : 'transparent'
                            }"
                        >
                          <td
                              class="text-base text-left break-keep word-break-keep-all min-w-0"
                              style="padding: 8px 12px;"
                              :style="{
                                color: isHighlighted(area, row.key) ? '#5c320a' : '#475569',
                                fontWeight: isHighlighted(area, row.key) ? '600' : '400'
                              }"
                          >
                            {{ row.key }}
                          </td>

                          <td
                              class="text-base text-right whitespace-nowrap w-12"
                              style="padding: 8px 12px;"
                              :style="{
                                color: isHighlighted(area, row.key) ? '#5c320a' : '#64748b',
                                fontWeight: isHighlighted(area, row.key) ? '600' : '400'
                              }"
                          >
                            {{ row.score }}
                          </td>
                        </tr>
                      </tbody>
                    </table>
                  </div>

                  <!-- 입력 영역 -->
                  <div>
                    <!-- NUMERIC -->
                    <template v-if="area.calc_type === 'NUMERIC'">
                      <input
                        :value="areaValues[area.area_id] ?? ''"
                        type="number"
                        step="any"
                        class="w-full text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
                        style="border: 1px solid #e2e8f0; border-radius: 8px; padding: 9px 12px; box-sizing: border-box;"
                        :style="{ background: area.teacher_editable ? 'white' : '#f1f5f9', color: area.teacher_editable ? '#1e293b' : '#94a3b8' }"
                        :disabled="!area.teacher_editable"
                        :placeholder="area.teacher_editable ? '데이터 입력' : (area.current_values[0] ?? '데이터 없음')"
                        @input="onNumericInput(area, $event.target.value)"
                      />
                    </template>

                    <!-- CATEGORY 단일값 -->
                    <template v-else-if="area.calc_type === 'CATEGORY' && !area.multi_value">
                      <select
                        :value="areaValues[area.area_id] ?? ''"
                        class="w-full text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
                        style="border: 1px solid #e2e8f0; border-radius: 8px; padding: 9px 12px; box-sizing: border-box;"
                        :style="{ background: area.teacher_editable ? 'white' : '#f1f5f9', color: area.teacher_editable ? '#1e293b' : '#94a3b8' }"
                        :disabled="!area.teacher_editable"
                        @change="onCategoryChange(area, $event.target.value)"
                      >
                        <option value="">선택하세요</option>
                        <option v-for="row in area.table" :key="row.key" :value="row.key">{{ row.key }}</option>
                      </select>
                    </template>

                    <!-- CATEGORY 복수값 -->
                    <template v-else-if="area.calc_type === 'CATEGORY' && area.multi_value">
                      <div class="flex flex-col gap-2">
                        <label
                          v-for="row in area.table"
                          :key="row.key"
                          class="flex items-center gap-2 text-base cursor-pointer"
                          :style="{ color: area.teacher_editable ? '#1e293b' : '#94a3b8' }"
                        >
                          <input
                            type="checkbox"
                            :value="row.key"
                            :checked="(areaMultiValues[area.area_id] || []).includes(row.key)"
                            :disabled="!area.teacher_editable"
                            class="accent-blue-600"
                            @change="onMultiValueChange(area, row.key, $event.target.checked)"
                          />
                          {{ row.key }}
                        </label>
                      </div>
                    </template>

                    <!-- MANUAL -->
                    <template v-else>
                      <input
                        :value="areaValues[area.area_id] ?? ''"
                        type="number"
                        step="any"
                        class="w-full text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
                        style="border: 1px solid #e2e8f0; border-radius: 8px; padding: 9px 12px; box-sizing: border-box;"
                        :style="{ background: area.teacher_editable ? 'white' : '#f1f5f9', color: area.teacher_editable ? '#1e293b' : '#94a3b8' }"
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
            <div class="flex items-center gap-3 mt-6 pt-5" style="border-top: 1px solid #f1f5f9;">
              <button
                class="text-base font-semibold disabled:opacity-40"
                style="padding: 10px 24px; border: none; background: #2563eb; color: white; border-radius: 8px; cursor: pointer;"
                :disabled="!canSave || saving"
                @click="saveApplication"
              >{{ saving ? '등록 중...' : '저장' }}</button>
              <button
                class="text-base"
                style="padding: 10px 20px; border: 1px solid #e2e8f0; background: white; color: #475569; border-radius: 8px; cursor: pointer;"
                @click="closeForm"
              >취소</button>
              <span v-if="saveError" class="text-base" style="color: #ef4444;">{{ saveError }}</span>
            </div>
          </div>
        </template>
      </div>
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
const tracksLoading  = ref(false)
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

const canSave = computed(() => {
  if (!selectedStudent.value || !form.trackId || !currentRound.value || !form.departmentName.trim()) return false
  if (areaContext.value.length === 0) return false
  return areaContext.value.every(area => {
    if (area.teacher_editable) {
      if (area.multi_value) {
        return (areaMultiValues.value[area.area_id] || []).length > 0
      }
      const v = areaValues.value[area.area_id]
      return v !== undefined && v !== ''
    }
    // 관리자 입력 고정: 서버에서 받은 current_values가 있어야 함
    return area.current_values.some(v => v !== '')
  })
})

// ── 초기 로드 ─────────────────────────────────────────────────────
async function loadAll() {
  const [round, sts, univs] = await Promise.all([
    getCurrentRound(),
    teacherGetStudents(),
    teacherGetUniversities(),
  ])
  currentRound.value = round
  students.value     = sts
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
  form.univId         = ''
  form.tracks         = []
  form.trackId        = ''
  form.departmentName = ''
  areaContext.value   = []
  areaValues.value    = {}
  areaMultiValues.value = {}
  scorePreview.value  = {}
  saveError.value     = ''
}

function closeForm() {
  showForm.value  = false
  saveError.value = ''
}

// ── 대학 선택 → 모집단위 로드 ─────────────────────────────────────
async function onUnivChange() {
  form.trackId        = ''
  form.tracks         = []
  areaContext.value   = []
  areaValues.value    = {}
  areaMultiValues.value = {}
  scorePreview.value  = {}

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
  areaContext.value   = []
  areaValues.value    = {}
  areaMultiValues.value = {}
  scorePreview.value  = {}

  if (!form.trackId || !selectedStudent.value) return
  contextLoading.value = true
  try {
    const ctx = await teacherGetAreaContext(selectedStudent.value.id, form.trackId)
    areaContext.value    = ctx
    initAreaValues(ctx)
    contextLoading.value = false
    // 기저장 값이 있는 항목에 대해 즉시 점수 계산 (테이블 렌더링 후 실행)
    await triggerInitialPreviews(ctx)
  } finally {
    contextLoading.value = false
  }
}

function initAreaValues(context) {
  const vals      = {}
  const multiVals = {}
  for (const area of context) {
    if (!area.teacher_editable) continue
    if (area.multi_value) {
      multiVals[area.area_id] = [...area.current_values]
    } else {
      vals[area.area_id] = area.current_values[0] ?? ''
    }
  }
  areaValues.value      = vals
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
  saving.value    = true
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
      student_id:        selectedStudent.value.id,
      track_id:          Number(form.trackId),
      round_id:          currentRound.value.id,
      department_name:   form.departmentName,
      base_data_entries: baseDataEntries,
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
