<template>
  <div class="min-h-screen flex items-center justify-center p-6" style="background: #eeecea;">
    <div
      class="w-full bg-white"
      style="max-width: 420px; border-radius: 20px; box-shadow: 0 8px 40px rgba(0,0,0,0.12), 0 0 0 1px rgba(0,0,0,0.05); padding: 2.5rem;"
    >
      <!-- 헤더 -->
      <div class="text-center mb-8">
        <div
          class="inline-flex items-center justify-center rounded-2xl mb-4"
          style="width: 56px; height: 56px; background: #eff6ff;"
        >
          <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="#2563eb" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M22 10v6M2 10l10-5 10 5-10 5z"/>
            <path d="M6 12v5c3 3 9 3 12 0v-5"/>
          </svg>
        </div>
        <h1 class="text-2xl font-bold" style="color: #1e293b; margin: 0 0 6px;">학교장추천전형</h1>
        <p class="text-base" style="color: #94a3b8; margin: 0;">선발 관리 시스템</p>
      </div>

      <!-- 안내 -->
      <div
        class="rounded-xl text-base leading-relaxed mb-6"
        style="padding: 14px 16px; background: #eff6ff; border: 1px solid #bfdbfe; color: #1d4ed8;"
      >
        처음 실행되었습니다.<br>
        관리자 비밀번호를 설정하면 시스템이 시작됩니다.
      </div>

      <!-- 설정 폼 -->
      <form @submit.prevent="handleSetup" class="flex flex-col gap-4">
        <div>
          <label class="block text-base font-medium mb-1.5" style="color: #64748b;">새 비밀번호</label>
          <input
            v-model="password"
            type="password"
            autocomplete="new-password"
            required
            minlength="8"
            placeholder="8자 이상"
            class="w-full text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
            style="border: 1px solid #e2e8f0; border-radius: 8px; padding: 10px 14px; box-sizing: border-box;"
          />
        </div>
        <div>
          <label class="block text-base font-medium mb-1.5" style="color: #64748b;">비밀번호 확인</label>
          <input
            v-model="confirm"
            type="password"
            autocomplete="new-password"
            required
            placeholder="동일한 비밀번호 입력"
            class="w-full text-base focus:outline-none focus:ring-2"
            :class="confirm && password !== confirm ? 'focus:ring-red-400' : 'focus:ring-blue-400'"
            :style="{
              border: confirm && password !== confirm ? '1px solid #fca5a5' : '1px solid #e2e8f0',
              borderRadius: '8px', padding: '10px 14px', boxSizing: 'border-box',
            }"
          />
          <p v-if="confirm && password !== confirm" class="text-base mt-1.5" style="color: #ef4444;">
            비밀번호가 일치하지 않습니다.
          </p>
        </div>

        <button
          type="submit"
          :disabled="loading || !canSubmit"
          class="w-full text-base font-semibold disabled:opacity-40 transition-colors"
          style="padding: 12px; border: none; border-radius: 10px; background: #2563eb; color: white; cursor: pointer; margin-top: 4px;"
        >{{ loading ? '설정 중…' : '시작하기' }}</button>
      </form>

      <p v-if="error" class="text-base text-center mt-4" style="color: #ef4444;">{{ error }}</p>
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
