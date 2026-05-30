<template>
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
  teacherGetApplications,
  teacherDeleteApplication,
} from '../../api/teacher.js'

const auth = useAuthStore()

const currentRound = ref(null)
const students     = ref([])
const applications = ref([])

function getStudentApps(studentId) {
  return applications.value.filter(a => a.student_id === studentId)
}

async function loadAll() {
  const [round, sts] = await Promise.all([
    getCurrentRound(),
    teacherGetStudents(),
  ])
  currentRound.value = round
  students.value = sts

  if (round) {
    applications.value = await teacherGetApplications(round.id)
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
