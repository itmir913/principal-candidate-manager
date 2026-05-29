<template>
  <div class="min-h-screen bg-gray-100 flex items-center justify-center p-4">
    <div class="bg-white rounded-xl shadow-md w-full max-w-sm p-8">
      <h1 class="text-2xl font-bold text-center text-gray-800 mb-1">학교장추천전형</h1>
      <p class="text-sm text-center text-gray-400 mb-6">선발 관리 시스템</p>

      <!-- Role toggle -->
      <div class="flex rounded-lg overflow-hidden border border-gray-200 mb-6">
        <button
          type="button"
          @click="switchMode('admin')"
          class="flex-1 py-2 text-sm font-medium transition-colors"
          :class="mode === 'admin' ? 'bg-indigo-600 text-white' : 'bg-white text-gray-600 hover:bg-gray-50'"
        >
          관리자
        </button>
        <button
          type="button"
          @click="switchMode('teacher')"
          class="flex-1 py-2 text-sm font-medium transition-colors"
          :class="mode === 'teacher' ? 'bg-indigo-600 text-white' : 'bg-white text-gray-600 hover:bg-gray-50'"
        >
          담임
        </button>
      </div>

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
        >
          {{ loading ? '처리 중…' : '로그인' }}
        </button>
      </form>

      <!-- Teacher form -->
      <form v-if="mode === 'teacher'" @submit.prevent="handleTeacherLogin" class="space-y-4">
        <div class="grid grid-cols-2 gap-3">
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-1">학년</label>
            <select
              v-model="teacherGrade"
              required
              class="block w-full rounded-lg border border-gray-300 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500"
            >
              <option value="">선택</option>
              <option v-for="g in [1, 2, 3]" :key="g" :value="g">{{ g }}학년</option>
            </select>
          </div>
          <div>
            <label class="block text-sm font-medium text-gray-700 mb-1">반</label>
            <select
              v-model="teacherClassNo"
              required
              class="block w-full rounded-lg border border-gray-300 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500"
            >
              <option value="">선택</option>
              <option v-for="c in 15" :key="c" :value="c">{{ c }}반</option>
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
          :disabled="loading"
          class="w-full bg-indigo-600 text-white rounded-lg py-2 text-sm font-medium hover:bg-indigo-700 disabled:opacity-50 transition-colors"
        >
          {{ loading ? '처리 중…' : '로그인' }}
        </button>
      </form>

      <p v-if="error" class="mt-3 text-sm text-red-600 text-center">{{ error }}</p>
    </div>
  </div>
</template>

<script setup>
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { useAuthStore } from '../stores/auth'

const router = useRouter()
const auth = useAuthStore()

const mode = ref('admin')
const loading = ref(false)
const error = ref(null)

const adminPassword = ref('')
const teacherGrade = ref('')
const teacherClassNo = ref('')
const teacherPassword = ref('')

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
    router.push('/teacher')
  } catch (e) {
    error.value = e.response?.data || '로그인에 실패했습니다.'
  } finally {
    loading.value = false
  }
}
</script>
