<template>
  <div class="min-h-screen bg-gray-100 flex items-center justify-center p-4">
    <div class="bg-white rounded-xl shadow-md w-full max-w-sm p-8">
      <h1 class="text-2xl font-bold text-center text-gray-800 mb-1">학교장추천전형</h1>
      <p class="text-sm text-center text-gray-400 mb-6">선발 관리 시스템</p>

      <!-- Role toggle -->
      <div class="flex rounded-lg overflow-hidden border border-gray-200 mb-6">
        <button
          type="button"
          @click="switchMode('teacher')"
          class="flex-1 py-2 text-sm font-medium transition-colors"
          :class="mode === 'teacher' ? 'bg-indigo-600 text-white' : 'bg-white text-gray-600 hover:bg-gray-50'"
        >담임</button>
        <button
          type="button"
          @click="switchMode('admin')"
          class="flex-1 py-2 text-sm font-medium transition-colors"
          :class="mode === 'admin' ? 'bg-indigo-600 text-white' : 'bg-white text-gray-600 hover:bg-gray-50'"
        >관리자</button>
      </div>

      <!-- Teacher form -->
      <form v-if="mode === 'teacher'" @submit.prevent="handleTeacherLogin" class="space-y-4">
        <div class="grid grid-cols-2 gap-3">
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-1">학년</label>
            <select
              v-model.number="teacherGrade"
              required
              :disabled="classesLoading"
              class="block w-full rounded-lg border border-gray-300 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500 disabled:bg-gray-50"
              @change="onGradeChange"
            >
              <option :value="''">{{ classesLoading ? '로딩 중…' : '선택' }}</option>
              <option v-for="g in availableGrades" :key="g" :value="g">{{ g }}학년</option>
            </select>
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-1">반</label>
            <select
              v-model.number="teacherClassNo"
              required
              :disabled="classesLoading || !teacherGrade"
              class="block w-full rounded-lg border border-gray-300 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500 disabled:bg-gray-50"
            >
              <option :value="''">선택</option>
              <option v-for="c in availableClassNos" :key="c" :value="c">{{ c }}반</option>
            </select>
          </div>
        </div>
        <div>
          <label class="block text-sm font-medium text-gray-700 mb-1">비밀번호</label>
          <input
            v-model="teacherPassword"
            type="password"
            autocomplete="current-password"
            required
            class="block w-full rounded-lg border border-gray-300 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500 focus:border-transparent"
          />
        </div>
        <button
          type="submit"
          :disabled="loading || !teacherGrade || !teacherClassNo"
          class="w-full bg-indigo-600 text-white rounded-lg py-2 text-sm font-medium hover:bg-indigo-700 disabled:opacity-50 transition-colors"
        >{{ loading ? '처리 중…' : '로그인' }}</button>
      </form>

      <!-- Admin form -->
      <form v-if="mode === 'admin'" @submit.prevent="handleAdminLogin" class="space-y-4">
        <div>
          <label class="block text-sm font-medium text-gray-700 mb-1">비밀번호</label>
          <input
            v-model="adminPassword"
            type="password"
            autocomplete="current-password"
            required
            class="block w-full rounded-lg border border-gray-300 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500 focus:border-transparent"
          />
        </div>
        <button
          type="submit"
          :disabled="loading"
          class="w-full bg-indigo-600 text-white rounded-lg py-2 text-sm font-medium hover:bg-indigo-700 disabled:opacity-50 transition-colors"
        >{{ loading ? '처리 중…' : '로그인' }}</button>
      </form>

      <p v-if="error" class="mt-3 text-sm text-red-600 text-center">{{ error }}</p>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, watch, onMounted } from 'vue'
import axios from 'axios'
import { useRouter } from 'vue-router'
import { useAuthStore } from '../stores/auth'

const router = useRouter()
const auth = useAuthStore()

const LS_GRADE = 'login_teacher_grade'
const LS_CLASS = 'login_teacher_class'

const mode = ref('teacher')
const loading = ref(false)
const error = ref(null)

const adminPassword = ref('')
const teacherGrade = ref(Number(localStorage.getItem(LS_GRADE)) || '')
const teacherClassNo = ref(Number(localStorage.getItem(LS_CLASS)) || '')
const teacherPassword = ref('')

const classes = ref([])
const classesLoading = ref(false)

const availableGrades = computed(() =>
  [...new Set(classes.value.map(c => c.grade))].sort((a, b) => a - b)
)

const availableClassNos = computed(() => {
  if (!teacherGrade.value) return []
  return classes.value
    .filter(c => c.grade === teacherGrade.value)
    .map(c => c.class_no)
    .sort((a, b) => a - b)
})

function onGradeChange() {
  if (!availableClassNos.value.includes(teacherClassNo.value)) {
    teacherClassNo.value = ''
  }
  if (teacherGrade.value) localStorage.setItem(LS_GRADE, teacherGrade.value)
  else localStorage.removeItem(LS_GRADE)
}

watch(teacherClassNo, (v) => {
  if (v) localStorage.setItem(LS_CLASS, v)
  else localStorage.removeItem(LS_CLASS)
})

async function fetchClasses() {
  classesLoading.value = true
  try {
    const res = await axios.get('/api/classes')
    classes.value = res.data
    // 저장된 학년·반이 실제 목록에 없으면 초기화
    if (teacherGrade.value && !availableGrades.value.includes(Number(teacherGrade.value))) {
      teacherGrade.value = ''
      teacherClassNo.value = ''
    } else if (teacherClassNo.value && !availableClassNos.value.includes(Number(teacherClassNo.value))) {
      teacherClassNo.value = ''
    }
  } catch {
    // 반 목록 조회 실패 시 빈 상태로 진행
  } finally {
    classesLoading.value = false
  }
}

function switchMode(m) {
  mode.value = m
  error.value = null
}

async function handleAdminLogin() {
  error.value = null
  loading.value = true
  try {
    await auth.loginAdmin(adminPassword.value)
    router.push('/admin')
  } catch (e) {
    error.value = e.response?.data || '로그인에 실패했습니다.'
  } finally {
    loading.value = false
  }
}

async function handleTeacherLogin() {
  error.value = null
  loading.value = true
  try {
    await auth.loginTeacher(Number(teacherGrade.value), Number(teacherClassNo.value), teacherPassword.value)
    localStorage.setItem(LS_GRADE, teacherGrade.value)
    localStorage.setItem(LS_CLASS, teacherClassNo.value)
    router.push('/teacher')
  } catch (e) {
    error.value = e.response?.data || '로그인에 실패했습니다.'
  } finally {
    loading.value = false
  }
}

onMounted(fetchClasses)
</script>
