<template>
  <!-- 라운드 상태 배너 -->
  <div
    class="mb-6 px-4 py-3 rounded-lg border text-sm"
    :class="currentRound ? 'bg-green-50 border-green-200 text-green-800' : 'bg-gray-50 border-gray-200 text-gray-500'"
  >
    <template v-if="currentRound">
      <span class="font-semibold">{{ currentRound.id }}차 라운드 진행 중</span>
      <span class="ml-2 text-green-600">— 지원 접수 기간입니다</span>
      <span class="ml-2 text-green-500 text-xs">(개시일: {{ fmtLocalDate(currentRound.opened_at) }})</span>
    </template>
    <template v-else>
      현재 지원 접수 기간이 아닙니다. 관리자에게 문의하세요.
    </template>
  </div>

  <!-- 지원 등록 폼 (OPEN 라운드 있을 때만) -->
  <div v-if="currentRound" class="mb-6 bg-white border rounded-lg p-4">
    <h2 class="text-sm font-semibold text-gray-700 mb-3">지원 등록</h2>
    <div class="flex gap-3 items-end">
      <div class="flex-1">
        <label class="block text-xs text-gray-500 mb-1">학생</label>
        <select v-model="newApp.studentId" class="w-full border rounded px-2 py-1.5 text-sm">
          <option value="">학생을 선택하세요</option>
          <option v-for="s in students" :key="s.id" :value="s.id">
            {{ s.seq_no }}번 {{ s.name }}
          </option>
        </select>
      </div>
      <div class="flex-1">
        <label class="block text-xs text-gray-500 mb-1">지원 대학/모집단위명</label>
        <select v-model="newApp.trackId" class="w-full border rounded px-2 py-1.5 text-sm">
          <option value="">대학을 선택하세요</option>
          <option v-for="t in univs" :key="t.id" :value="t.id">
            {{ t.univ_name }} — {{ t.track_name }}
          </option>
        </select>
      </div>
      <button
        class="px-4 py-1.5 text-sm bg-blue-600 text-white rounded hover:bg-blue-700 disabled:opacity-40"
        :disabled="!newApp.studentId || !newApp.trackId || submitting"
        @click="addApplication"
      >{{ submitting ? '등록 중...' : '등록' }}</button>
    </div>
    <p v-if="addError" class="mt-2 text-xs text-red-500">{{ addError }}</p>
  </div>

  <!-- 학생별 지원 현황 -->
  <div class="bg-white border rounded-lg overflow-hidden">
    <div class="px-4 py-3 border-b bg-gray-50 flex items-center justify-between">
      <h2 class="text-sm font-semibold text-gray-700">
        {{ auth.grade }}학년 {{ auth.classNo }}반 학급 현황
      </h2>
      <span class="text-xs text-gray-400">{{ students.length }}명</span>
    </div>

    <div v-if="students.length === 0" class="px-4 py-8 text-center text-sm text-gray-400">
      학생이 없습니다. 관리자에게 학생 데이터 등록을 요청하세요.
    </div>

    <table v-else class="w-full text-sm">
      <thead>
        <tr class="border-b bg-gray-50">
          <th class="text-left px-4 py-2 text-xs text-gray-500 font-medium w-12">번호</th>
          <th class="text-left px-4 py-2 text-xs text-gray-500 font-medium w-24">이름</th>
          <th class="text-left px-4 py-2 text-xs text-gray-500 font-medium w-28">학생코드</th>
          <th class="text-left px-4 py-2 text-xs text-gray-500 font-medium">지원 대학</th>
        </tr>
      </thead>
      <tbody>
        <tr
          v-for="s in students"
          :key="s.id"
          class="border-b last:border-b-0 hover:bg-gray-50"
        >
          <td class="px-4 py-3 text-gray-400">{{ s.seq_no }}</td>
          <td class="px-4 py-3 font-medium text-gray-800">{{ s.name }}</td>
          <td class="px-4 py-3 text-gray-500 text-xs">{{ s.student_code }}</td>
          <td class="px-4 py-3">
            <div v-if="getStudentApps(s.id).length === 0" class="text-gray-300 text-xs">-</div>
            <div
              v-for="app in getStudentApps(s.id)"
              :key="app.track_id"
              class="flex items-center gap-2 mb-1"
            >
              <span class="text-gray-700">{{ app.univ_name }} — {{ app.track_name }}</span>
              <span v-if="app.abandoned" class="text-xs text-red-400">(포기)</span>
              <button
                v-if="currentRound && !app.abandoned"
                class="text-xs px-1.5 py-0.5 border border-gray-300 text-gray-400 rounded hover:border-red-300 hover:text-red-400"
                @click="removeApplication(app)"
              >취소</button>
            </div>
          </td>
        </tr>
      </tbody>
    </table>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import { useAuthStore } from '../../stores/auth.js'
import {
  getCurrentRound,
  teacherGetStudents,
  teacherGetAllTracks,
  teacherGetApplications,
  teacherCreateApplication,
  teacherDeleteApplication,
} from '../../api/teacher.js'

const auth = useAuthStore()

const currentRound = ref(null)
const students     = ref([])
const univs        = ref([])
const applications = ref([])

const newApp     = ref({ studentId: '', trackId: '' })
const submitting = ref(false)
const addError   = ref('')

function fmtLocalDate(s) {
  if (!s) return ''
  const d = new Date(s)
  const pad = n => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${pad(d.getMonth()+1)}-${pad(d.getDate())}`
}

function getStudentApps(studentId) {
  return applications.value.filter(a => a.student_id === studentId)
}

async function loadAll() {
  const [round, sts, ts] = await Promise.all([
    getCurrentRound(),
    teacherGetStudents(),
    teacherGetAllTracks(),
  ])
  currentRound.value = round
  students.value = sts
  univs.value = ts

  if (round) {
    applications.value = await teacherGetApplications(round.id)
  }
}

async function addApplication() {
  if (!newApp.value.studentId || !newApp.value.trackId) return
  submitting.value = true
  addError.value = ''
  try {
    await teacherCreateApplication({
      student_id: newApp.value.studentId,
      track_id:   newApp.value.trackId,
      round_id:   currentRound.value.id,
    })
    newApp.value.studentId = ''
    newApp.value.trackId   = ''
    applications.value = await teacherGetApplications(currentRound.value.id)
  } catch (e) {
    addError.value = e.response?.data || e.message
  } finally {
    submitting.value = false
  }
}

async function removeApplication(app) {
  if (!confirm(`${app.name} 학생의 ${app.univ_name} ${app.track_name} 지원을 취소하시겠습니까?`)) return
  try {
    await teacherDeleteApplication(app.student_id, app.track_id, app.round_id)
    applications.value = await teacherGetApplications(currentRound.value.id)
  } catch (e) {
    alert(e.response?.data || e.message)
  }
}

onMounted(loadAll)
</script>
