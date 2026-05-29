<template>
  <div>
    <h2 class="text-lg font-semibold text-gray-700 mb-4">학생 명단 관리</h2>

    <!-- 분류별 가져오기/내보내기 -->
    <div class="mb-4 border border-gray-200 rounded divide-y divide-gray-200">
      <div v-for="cat in categories" :key="cat.key" class="flex items-center gap-2 px-3 py-2">
        <span class="w-14 text-sm font-medium text-gray-600 flex-shrink-0">{{ cat.label }}</span>
        <button
          class="px-2.5 py-1 border border-gray-300 text-gray-700 text-xs rounded hover:bg-gray-50"
          @click="cat.dlTemplate"
        >양식 다운로드</button>
        <button
          class="px-2.5 py-1 border border-gray-300 text-gray-700 text-xs rounded hover:bg-gray-50"
          @click="cat.dlExport"
        >목록 내보내기</button>
        <label class="px-2.5 py-1 bg-blue-600 text-white text-xs rounded hover:bg-blue-700 cursor-pointer">
          가져오기
          <input type="file" accept=".xlsx,.csv" class="hidden" @change="e => cat.onImport(e)" />
        </label>
      </div>
    </div>

    <!-- 업로드 결과 -->
    <div v-if="result" class="mb-4 p-3 rounded border text-sm"
      :class="result.errors.length ? 'border-yellow-400 bg-yellow-50' : 'border-green-400 bg-green-50'">
      <p class="font-medium mb-1">
        [{{ result.label }}] {{ result.errors.length ? '오류 발생 — 가져오기 실패' : `완료 — 신규 ${result.inserted}명, 수정 ${result.updated}명` }}
      </p>
      <ul v-if="result.errors.length" class="list-disc list-inside text-yellow-700 space-y-0.5">
        <li v-for="(e, i) in result.errors" :key="i">{{ e }}</li>
      </ul>
    </div>

    <p v-if="error" class="text-red-500 text-sm mb-3">{{ error }}</p>

    <!-- 필터 -->
    <div class="flex gap-2 mb-3 items-center flex-wrap">
      <select v-model="filterEnrolled" class="border rounded px-2 py-1 text-sm">
        <option :value="null">전체</option>
        <option :value="1">재학생</option>
        <option :value="0">졸업생</option>
      </select>
      <select
        v-model.number="filterGrade"
        class="border rounded px-2 py-1 text-sm"
        :disabled="filterEnrolled === 0"
        @change="filterClass = null"
      >
        <option :value="null">전체 학년</option>
        <option v-for="g in gradeOptions.grades" :key="g" :value="g">{{ g }}학년</option>
      </select>
      <select
        v-model.number="filterClass"
        class="border rounded px-2 py-1 text-sm"
        :disabled="filterEnrolled === 0"
      >
        <option :value="null">전체 반</option>
        <option v-for="c in availableClasses" :key="c" :value="c">{{ c }}반</option>
      </select>
      <button class="text-sm text-blue-600 underline" @click="loadStudents">조회</button>
      <span class="ml-auto text-sm text-gray-500">총 {{ students.length }}명</span>
    </div>

    <!-- 학생 목록 -->
    <div class="overflow-x-auto">
      <table class="w-full text-sm border-collapse">
        <thead>
          <tr class="bg-gray-100 text-gray-600 text-left">
            <th class="px-3 py-2 border-b">학생코드</th>
            <th class="px-3 py-2 border-b">이름</th>
            <th class="px-3 py-2 border-b">구분</th>
            <th class="px-3 py-2 border-b">학년</th>
            <th class="px-3 py-2 border-b">반</th>
            <th class="px-3 py-2 border-b">번호</th>
            <th class="px-3 py-2 border-b">졸업연도</th>
            <th class="px-3 py-2 border-b w-14"></th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="s in students" :key="s.id" class="hover:bg-gray-50">
            <td class="px-3 py-1.5 border-b font-mono text-xs">{{ s.student_code }}</td>
            <td class="px-3 py-1.5 border-b">{{ s.name }}</td>
            <td class="px-3 py-1.5 border-b">
              <span :class="s.is_enrolled ? 'text-blue-600' : 'text-gray-400'">
                {{ s.is_enrolled ? '재학' : '졸업' }}
              </span>
            </td>
            <td class="px-3 py-1.5 border-b">{{ s.grade ?? '-' }}</td>
            <td class="px-3 py-1.5 border-b">{{ s.class_no ?? '-' }}</td>
            <td class="px-3 py-1.5 border-b">{{ s.seq_no ?? '-' }}</td>
            <td class="px-3 py-1.5 border-b">{{ s.grad_year ?? '-' }}</td>
            <td class="px-3 py-1.5 border-b">
              <button
                class="px-2 py-0.5 text-xs text-red-600 border border-red-300 rounded hover:bg-red-50"
                @click="remove(s)"
              >삭제</button>
            </td>
          </tr>
          <tr v-if="students.length === 0">
            <td colspan="8" class="px-3 py-4 text-center text-gray-400">
              학생 데이터가 없습니다. 양식을 다운로드하여 작성 후 가져오기 하세요.
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, watch, onMounted } from 'vue'
import {
  getStudents,
  getStudentGradeOptions,
  downloadStudentTemplate,
  exportStudents,
  importStudents,
  downloadEnrolledTemplate,
  exportEnrolled,
  importEnrolled,
  downloadGraduatedTemplate,
  exportGraduated,
  importGraduated,
  deleteStudent,
} from '../../api/admin.js'

const students = ref([])
const error = ref('')
const result = ref(null)
const filterEnrolled = ref(null)   // null=전체, 1=재학생, 0=졸업생
const filterGrade = ref(null)
const filterClass = ref(null)
const gradeOptions = ref({ grades: [], by_grade: {} })

// 선택 학년에 따라 드롭다운에 표시할 반 목록
const availableClasses = computed(() => {
  if (!filterGrade.value) {
    const all = new Set()
    Object.values(gradeOptions.value.by_grade).forEach(arr => arr.forEach(c => all.add(c)))
    return [...all].sort((a, b) => a - b)
  }
  return gradeOptions.value.by_grade[String(filterGrade.value)] ?? []
})

// 졸업생 선택 시 학년·반 필터 초기화
watch(filterEnrolled, (val) => {
  if (val === 0) {
    filterGrade.value = null
    filterClass.value = null
  }
})

async function loadStudents() {
  error.value = ''
  try {
    const params = {}
    if (filterGrade.value)              params.grade = filterGrade.value
    if (filterClass.value)              params.class_no = filterClass.value
    if (filterEnrolled.value !== null)  params.is_enrolled = filterEnrolled.value
    students.value = await getStudents(params)
  } catch (e) {
    error.value = e.response?.data ?? e.message
  }
}

async function loadGradeOptions() {
  try {
    gradeOptions.value = await getStudentGradeOptions()
  } catch { /* 실패해도 빈 목록으로 동작 */ }
}

function saveBlob(response, fallback) {
  const disposition = response.headers?.['content-disposition'] ?? ''
  const match = disposition.match(/filename="?([^";]+)"?/i)
  const filename = match ? match[1] : fallback
  const url = URL.createObjectURL(new Blob([response.data]))
  const a = document.createElement('a')
  a.href = url; a.download = filename; a.click()
  URL.revokeObjectURL(url)
}

async function runImport(apiFn, label, evt) {
  const file = evt.target.files?.[0]
  if (!file) return
  error.value = ''
  result.value = null
  try {
    const data = await apiFn(file)
    result.value = { label, ...data }
    await Promise.all([loadStudents(), loadGradeOptions()])
  } catch (e) {
    const d = e.response?.data
    if (d != null && typeof d === 'object' && Array.isArray(d.errors)) {
      result.value = { label, ...d }
    } else {
      error.value = typeof d === 'string' ? d : (e.message ?? '오류가 발생했습니다')
    }
  }
  evt.target.value = ''
}

async function remove(s) {
  const label = `${s.name}(${s.student_code})`
  if (!window.confirm(`${label} 학생을 삭제하시겠습니까?`)) return
  error.value = ''
  try {
    await deleteStudent(s.id)
    students.value = students.value.filter(x => x.id !== s.id)
  } catch (e) {
    error.value = e.response?.data ?? e.message
  }
}

const categories = [
  {
    key: 'enrolled',
    label: '재학생',
    dlTemplate: async () => {
      try { saveBlob(await downloadEnrolledTemplate(), 'students_enrolled_template.xlsx') }
      catch (e) { error.value = e.response?.data ?? e.message }
    },
    dlExport: async () => {
      try { saveBlob(await exportEnrolled(), 'students_enrolled.xlsx') }
      catch (e) { error.value = e.response?.data ?? e.message }
    },
    onImport: (e) => runImport(importEnrolled, '재학생', e),
  },
  {
    key: 'graduated',
    label: '졸업생',
    dlTemplate: async () => {
      try { saveBlob(await downloadGraduatedTemplate(), 'students_graduated_template.xlsx') }
      catch (e) { error.value = e.response?.data ?? e.message }
    },
    dlExport: async () => {
      try { saveBlob(await exportGraduated(), 'students_graduated.xlsx') }
      catch (e) { error.value = e.response?.data ?? e.message }
    },
    onImport: (e) => runImport(importGraduated, '졸업생', e),
  },
  {
    key: 'all',
    label: '전체',
    dlTemplate: async () => {
      try { saveBlob(await downloadStudentTemplate(), 'students_all_template.xlsx') }
      catch (e) { error.value = e.response?.data ?? e.message }
    },
    dlExport: async () => {
      try { saveBlob(await exportStudents(), 'students_all.xlsx') }
      catch (e) { error.value = e.response?.data ?? e.message }
    },
    onImport: (e) => runImport(importStudents, '전체', e),
  },
]

onMounted(() => {
  loadGradeOptions()
  loadStudents()
})
</script>
