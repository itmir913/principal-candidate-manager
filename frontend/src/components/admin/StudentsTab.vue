<template>
  <div>
    <h2 class="text-lg font-semibold text-gray-700 mb-4">학생 관리 (명렬표)</h2>

    <!-- 액션 버튼 -->
    <div class="flex flex-wrap gap-2 mb-4">
      <button class="px-3 py-1.5 border border-gray-300 text-gray-700 text-sm rounded hover:bg-gray-50" @click="dlTemplate">⬇ 샘플 양식 다운로드</button>
      <button class="px-3 py-1.5 border border-gray-300 text-gray-700 text-sm rounded hover:bg-gray-50" @click="dlExport">⬇ 현재 데이터 다운로드</button>
      <label class="px-3 py-1.5 bg-blue-600 text-white text-sm rounded hover:bg-blue-700 cursor-pointer">
        ⬆ 파일 업로드
        <input type="file" accept=".xlsx,.csv" class="hidden" @change="onFileChange" />
      </label>
    </div>

    <!-- 업로드 결과 -->
    <div v-if="result" class="mb-4 p-3 rounded border text-sm"
      :class="result.errors.length ? 'border-yellow-400 bg-yellow-50' : 'border-green-400 bg-green-50'">
      <p class="font-medium mb-1">
        업로드 완료 — 신규 {{ result.inserted }}명, 수정 {{ result.updated }}명
      </p>
      <ul v-if="result.errors.length" class="list-disc list-inside text-yellow-700 space-y-0.5">
        <li v-for="(e, i) in result.errors" :key="i">{{ e }}</li>
      </ul>
    </div>

    <p v-if="error" class="text-red-500 text-sm mb-3">{{ error }}</p>

    <!-- 필터 -->
    <div class="flex gap-2 mb-3 items-center">
      <select v-model.number="filterGrade" class="border rounded px-2 py-1 text-sm">
        <option :value="null">전체 학년</option>
        <option v-for="g in [1,2,3]" :key="g" :value="g">{{ g }}학년</option>
      </select>
      <select v-model.number="filterClass" class="border rounded px-2 py-1 text-sm">
        <option :value="null">전체 반</option>
        <option v-for="c in 20" :key="c" :value="c">{{ c }}반</option>
      </select>
      <button class="text-sm text-blue-600 underline" @click="loadStudents">조회</button>
      <span class="ml-auto text-sm text-gray-500">총 {{ students.length }}명</span>
    </div>

    <!-- 학생 목록 -->
    <div class="overflow-x-auto">
      <table class="w-full text-sm border-collapse">
        <thead>
          <tr class="bg-gray-100 text-gray-600 text-left">
            <th class="px-3 py-2 border-b">학번</th>
            <th class="px-3 py-2 border-b">이름</th>
            <th class="px-3 py-2 border-b">구분</th>
            <th class="px-3 py-2 border-b">학년</th>
            <th class="px-3 py-2 border-b">반</th>
            <th class="px-3 py-2 border-b">번호</th>
            <th class="px-3 py-2 border-b">졸업연도</th>
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
          </tr>
          <tr v-if="students.length === 0">
            <td colspan="7" class="px-3 py-4 text-center text-gray-400">
              학생 데이터가 없습니다. 샘플 양식을 다운로드하여 작성 후 업로드하세요.
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import {
  getStudents,
  downloadStudentTemplate,
  exportStudents,
  importStudents,
} from '../../api/admin.js'

const students = ref([])
const error = ref('')
const result = ref(null)
const filterGrade = ref(null)
const filterClass = ref(null)

async function loadStudents() {
  error.value = ''
  try {
    const params = {}
    if (filterGrade.value) params.grade = filterGrade.value
    if (filterClass.value) params.class_no = filterClass.value
    students.value = await getStudents(params)
  } catch (e) {
    error.value = e.response?.data ?? e.message
  }
}

function saveBlob(response, filename) {
  const url = URL.createObjectURL(new Blob([response.data]))
  const a = document.createElement('a')
  a.href = url
  a.download = filename
  a.click()
  URL.revokeObjectURL(url)
}

async function dlTemplate() {
  try {
    const res = await downloadStudentTemplate()
    saveBlob(res, 'students_template.xlsx')
  } catch (e) {
    error.value = e.response?.data ?? e.message
  }
}

async function dlExport() {
  try {
    const res = await exportStudents()
    saveBlob(res, 'students.xlsx')
  } catch (e) {
    error.value = e.response?.data ?? e.message
  }
}

async function onFileChange(evt) {
  const file = evt.target.files?.[0]
  if (!file) return
  error.value = ''
  result.value = null
  try {
    result.value = await importStudents(file)
    await loadStudents()
  } catch (e) {
    error.value = e.response?.data ?? e.message
  }
  // input 초기화 (같은 파일 재업로드 허용)
  evt.target.value = ''
}

onMounted(loadStudents)
</script>

