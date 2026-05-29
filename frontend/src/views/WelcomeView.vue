<template>
  <div class="min-h-screen bg-indigo-50 flex items-center justify-center p-4">
    <div class="bg-white rounded-2xl shadow-lg w-full max-w-md p-10">

      <!-- 헤더 -->
      <div class="text-center mb-8">
        <div class="inline-flex items-center justify-center w-16 h-16 rounded-full bg-indigo-100 mb-4">
          <svg class="w-8 h-8 text-indigo-600" fill="none" stroke="currentColor" stroke-width="1.5" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round"
              d="M4.26 10.147a60.438 60.438 0 0 0-.491 6.347A48.62 48.62 0 0 1 12 20.904a48.62 48.62 0 0 1 8.232-4.41 60.46 60.46 0 0 0-.491-6.347m-15.482 0a50.636 50.636 0 0 0-2.658-.813A59.906 59.906 0 0 1 12 3.493a59.903 59.903 0 0 1 10.399 5.84c-.896.248-1.783.52-2.658.814m-15.482 0A50.717 50.717 0 0 1 12 13.489a50.702 50.702 0 0 1 3.741-3.342M6.75 15a.75.75 0 1 0 0-1.5.75.75 0 0 0 0 1.5Zm0 0v-3.675A55.378 55.378 0 0 1 12 8.443m-7.007 11.55A5.981 5.981 0 0 0 6.75 15.75v-1.5" />
          </svg>
        </div>
        <h1 class="text-2xl font-bold text-gray-900">학교장추천전형</h1>
        <p class="text-sm text-gray-500 mt-1">선발 관리 시스템</p>
      </div>

      <!-- 안내 -->
      <div class="bg-indigo-50 rounded-xl p-4 mb-6 text-sm text-indigo-800 leading-relaxed">
        처음 실행되었습니다.<br>
        관리자 비밀번호를 설정하면 시스템이 시작됩니다.
      </div>

      <!-- 설정 폼 -->
      <form @submit.prevent="handleSetup" class="space-y-4">
        <div>
          <label class="block text-sm font-medium text-gray-700 mb-1">새 비밀번호</label>
          <input
            v-model="password"
            type="password"
            autocomplete="new-password"
            required
            minlength="8"
            placeholder="8자 이상"
            class="block w-full rounded-lg border border-gray-300 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500 focus:border-transparent"
          />
        </div>
        <div>
          <label class="block text-sm font-medium text-gray-700 mb-1">비밀번호 확인</label>
          <input
            v-model="confirm"
            type="password"
            autocomplete="new-password"
            required
            placeholder="동일한 비밀번호 입력"
            class="block w-full rounded-lg border border-gray-300 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-indigo-500 focus:border-transparent"
            :class="{ 'border-red-400 focus:ring-red-400': confirm && password !== confirm }"
          />
          <p v-if="confirm && password !== confirm" class="mt-1 text-xs text-red-500">
            비밀번호가 일치하지 않습니다.
          </p>
        </div>

        <button
          type="submit"
          :disabled="loading || !canSubmit"
          class="w-full bg-indigo-600 text-white rounded-lg py-2.5 text-sm font-semibold hover:bg-indigo-700 disabled:opacity-50 transition-colors mt-2"
        >
          {{ loading ? '설정 중…' : '시작하기' }}
        </button>
      </form>

      <p v-if="error" class="mt-3 text-sm text-red-600 text-center">{{ error }}</p>
    </div>
  </div>
</template>

<script setup>
import { ref, computed } from 'vue'
import { useRouter } from 'vue-router'
import { useAuthStore } from '../stores/auth'

const router = useRouter()
const auth = useAuthStore()

const password = ref('')
const confirm = ref('')
const loading = ref(false)
const error = ref(null)

const canSubmit = computed(
  () => password.value.length >= 8 && password.value === confirm.value
)

async function handleSetup() {
  if (!canSubmit.value) return
  error.value = null
  loading.value = true
  try {
    await auth.loginAdmin(password.value)  // 미초기화 상태 → 비밀번호 등록 + 토큰 발급
    router.push('/admin')
  } catch (e) {
    error.value = e.response?.data || '설정에 실패했습니다. 다시 시도해주세요.'
  } finally {
    loading.value = false
  }
}
</script>
