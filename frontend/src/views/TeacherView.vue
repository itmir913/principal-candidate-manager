<template>
  <div class="min-h-screen bg-gray-50">
    <!-- 헤더 -->
    <header class="bg-white border-b px-6 py-3 flex items-center justify-between">
      <div>
        <span class="text-lg font-bold text-gray-800">학교장추천전형 지원 관리</span>
        <span class="ml-3 text-sm text-gray-500">{{ auth.grade }}학년 {{ auth.classNo }}반 담임</span>
      </div>
      <div class="flex items-center gap-2">
        <button
          class="px-3 py-1.5 text-sm text-gray-600 border rounded hover:bg-gray-100"
          @click="showPwModal = true"
        >비밀번호 변경</button>
        <button
          class="px-3 py-1.5 text-sm text-gray-600 border rounded hover:bg-gray-100"
          @click="logout"
        >로그아웃</button>
      </div>
    </header>

    <main class="p-6 max-w-4xl mx-auto">
      <!-- 라운드 상태 배너 -->
      <div
        class="mb-6 px-4 py-3 rounded-lg border text-sm"
        :class="currentRound ? 'bg-green-50 border-green-200 text-green-800' : 'bg-gray-50 border-gray-200 text-gray-500'"
      >
        <template v-if="currentRound">
          <span class="font-semibold">{{ currentRound.id }}차 라운드 진행 중</span>
          <span class="ml-2 text-green-600">— 지원 접수 기간입니다</span>
          <span class="ml-2 text-green-500 text-xs">(개시일: {{ currentRound.opened_at?.slice(0, 10) }})</span>
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
            <select
              v-model="newApp.studentId"
              class="w-full border rounded px-2 py-1.5 text-sm"
            >
              <option value="">학생을 선택하세요</option>
              <option v-for="s in students" :key="s.id" :value="s.id">
                {{ s.seq_no }}번 {{ s.name }}
              </option>
            </select>
          </div>
          <div class="flex-1">
            <label class="block text-xs text-gray-500 mb-1">지원 대학/모집단위</label>
            <select
              v-model="newApp.univId"
              class="w-full border rounded px-2 py-1.5 text-sm"
            >
              <option value="">대학을 선택하세요</option>
              <option v-for="u in univs" :key="u.id" :value="u.id">
                {{ u.univ_name }} — {{ u.track_name }}
              </option>
            </select>
          </div>
          <button
            class="px-4 py-1.5 text-sm bg-blue-600 text-white rounded hover:bg-blue-700 disabled:opacity-40"
            :disabled="!newApp.studentId || !newApp.univId || submitting"
            @click="addApplication"
          >{{ submitting ? '등록 중...' : '등록' }}</button>
        </div>
        <p v-if="addError" class="mt-2 text-xs text-red-500">{{ addError }}</p>
      </div>

      <!-- 학생별 지원 현황 -->
      <div class="bg-white border rounded-lg overflow-hidden">
        <div class="px-4 py-3 border-b bg-gray-50 flex items-center justify-between">
          <h2 class="text-sm font-semibold text-gray-700">
            {{ auth.grade }}학년 {{ auth.classNo }}반 지원 현황
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
              <th class="text-left px-4 py-2 text-xs text-gray-500 font-medium w-28">학번</th>
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
                <div v-for="app in getStudentApps(s.id)" :key="app.univ_id"
                     class="flex items-center gap-2 mb-1">
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
    </main>

    <!-- 비밀번호 변경 모달 -->
    <div v-if="showPwModal" class="fixed inset-0 bg-black/40 flex items-center justify-center z-50">
      <div class="bg-white rounded-lg shadow-xl p-6 w-80">
        <h2 class="text-base font-semibold text-gray-800 mb-4">비밀번호 변경</h2>
        <input
          v-model="newPw"
          type="password"
          placeholder="새 비밀번호"
          class="w-full border rounded px-3 py-2 text-sm mb-3 focus:outline-none focus:ring-2 focus:ring-blue-400"
          @keyup.enter="changePw"
        />
        <p v-if="pwError" class="text-xs text-red-500 mb-2">{{ pwError }}</p>
        <div class="flex gap-2 justify-end">
          <button
            class="px-3 py-1.5 text-sm text-gray-600 border rounded hover:bg-gray-100"
            @click="closePwModal"
          >취소</button>
          <button
            class="px-3 py-1.5 text-sm bg-blue-600 text-white rounded hover:bg-blue-700 disabled:opacity-40"
            :disabled="!newPw || pwLoading"
            @click="changePw"
          >{{ pwLoading ? '변경 중...' : '변경' }}</button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { useAuthStore } from '../stores/auth.js'
import {
  getCurrentRound,
  teacherGetStudents,
  teacherGetUniversities,
  teacherGetApplications,
  teacherCreateApplication,
  teacherDeleteApplication,
  teacherChangePassword,
} from '../api/teacher.js'

const router = useRouter()
const auth   = useAuthStore()

const currentRound = ref(null)
const students     = ref([])
const univs        = ref([])
const applications = ref([])

const newApp = ref({ studentId: '', univId: '' })
const submitting = ref(false)
const addError   = ref('')

const showPwModal = ref(false)
const newPw = ref('')
const pwError = ref('')
const pwLoading = ref(false)

function getStudentApps(studentId) {
  return applications.value.filter(a => a.student_id === studentId)
}

async function loadAll() {
  const [round, sts, us] = await Promise.all([
    getCurrentRound(),
    teacherGetStudents(),
    teacherGetUniversities(),
  ])
  currentRound.value = round
  students.value = sts
  univs.value = us

  if (round) {
    applications.value = await teacherGetApplications(round.id)
  }
}

async function addApplication() {
  if (!newApp.value.studentId || !newApp.value.univId) return
  submitting.value = true
  addError.value = ''
  try {
    await teacherCreateApplication({
      student_id: newApp.value.studentId,
      univ_id:    newApp.value.univId,
      round_id:   currentRound.value.id,
    })
    newApp.value.studentId = ''
    newApp.value.univId    = ''
    applications.value = await teacherGetApplications(currentRound.value.id)
  } catch (e) {
    addError.value = e.response?.data || e.message
  } finally {
    submitting.value = false
  }
}

async function removeApplication(app) {
  if (!confirm(`${app.name} 학생의 ${app.univ_name} 지원을 취소하시겠습니까?`)) return
  try {
    await teacherDeleteApplication(app.student_id, app.univ_id, app.round_id)
    applications.value = await teacherGetApplications(currentRound.value.id)
  } catch (e) {
    alert(e.response?.data || e.message)
  }
}

function closePwModal() {
  showPwModal.value = false
  newPw.value = ''
  pwError.value = ''
}

async function changePw() {
  if (!newPw.value) return
  pwLoading.value = true
  pwError.value = ''
  try {
    await teacherChangePassword(newPw.value)
    closePwModal()
    alert('비밀번호가 변경되었습니다.')
  } catch (e) {
    pwError.value = e.response?.data || e.message
  } finally {
    pwLoading.value = false
  }
}

function logout() {
  auth.logout()
  router.push('/login')
}

onMounted(loadAll)
</script>
