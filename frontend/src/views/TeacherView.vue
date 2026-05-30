<template>
  <div class="min-h-screen bg-gray-50">
    <!-- 헤더 -->
    <header class="bg-white border-b px-6 py-3 flex items-center justify-between">
      <div>
        <span class="text-lg font-bold text-gray-800">학교장 추천자 선발 관리 시스템</span>
        <span class="ml-3 text-sm text-gray-500">{{ auth.grade }}학년 {{ auth.classNo }}반 담임<template v-if="auth.teacherName"> · {{ auth.teacherName }}</template></span>
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

    <!-- 탭 -->
    <nav class="bg-white border-b px-6 flex gap-0">
      <button
        v-for="tab in tabs"
        :key="tab.key"
        class="px-5 py-3 text-sm font-medium border-b-2 transition-colors"
        :class="active === tab.key
          ? 'border-blue-600 text-blue-600'
          : 'border-transparent text-gray-500 hover:text-gray-700'"
        @click="active = tab.key"
      >
        {{ tab.label }}
      </button>
    </nav>

    <!-- 탭 컨텐츠 -->
    <main class="p-6">
      <Suspense>
        <component :is="currentTab" />
      </Suspense>
    </main>

    <!-- 비밀번호 변경 모달 -->
    <div v-if="showPwModal" class="fixed inset-0 bg-black/40 flex items-center justify-center z-50">
      <div class="bg-white rounded-lg shadow-xl p-6 w-80">
        <h2 class="text-base font-semibold text-gray-800 mb-4">비밀번호 변경</h2>
        <div class="space-y-3 mb-4">
          <div>
            <label class="block text-xs font-medium text-gray-600 mb-1">현재 비밀번호</label>
            <input
              v-model="currentPw"
              type="password"
              autocomplete="current-password"
              class="w-full border rounded px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-400"
            />
          </div>
          <div>
            <label class="block text-xs font-medium text-gray-600 mb-1">새 비밀번호</label>
            <input
              v-model="newPw"
              type="password"
              autocomplete="new-password"
              class="w-full border rounded px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-400"
            />
          </div>
          <div>
            <label class="block text-xs font-medium text-gray-600 mb-1">새 비밀번호 재입력</label>
            <input
              v-model="confirmPw"
              type="password"
              autocomplete="new-password"
              class="w-full border rounded px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-400"
              @keyup.enter="changePw"
            />
          </div>
        </div>
        <p v-if="pwError" class="text-xs text-red-500 mb-3">{{ pwError }}</p>
        <div class="flex gap-2 justify-end">
          <button
            class="px-3 py-1.5 text-sm text-gray-600 border rounded hover:bg-gray-100"
            @click="closePwModal"
          >취소</button>
          <button
            class="px-3 py-1.5 text-sm bg-blue-600 text-white rounded hover:bg-blue-700 disabled:opacity-40"
            :disabled="!currentPw || !newPw || !confirmPw || pwLoading"
            @click="changePw"
          >{{ pwLoading ? '변경 중...' : '변경' }}</button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, defineAsyncComponent } from 'vue'
import { useRouter } from 'vue-router'
import { useAuthStore } from '../stores/auth.js'
import { teacherChangePassword } from '../api/teacher.js'

const router = useRouter()
const auth   = useAuthStore()

const tabs = [
  { key: 'class',   label: '학급 관리' },
  { key: 'data',    label: '성적 입력' },
  { key: 'results', label: '결과 조회' },
]
const active = ref('class')

const ClassTab   = defineAsyncComponent(() => import('../components/teacher/ClassTab.vue'))
const DataTab    = defineAsyncComponent(() => import('../components/teacher/DataTab.vue'))
const ResultsTab = defineAsyncComponent(() => import('../components/teacher/ResultsTab.vue'))

const currentTab = computed(() => {
  if (active.value === 'class')   return ClassTab
  if (active.value === 'data')    return DataTab
  return ResultsTab
})

const showPwModal = ref(false)
const currentPw = ref('')
const newPw = ref('')
const confirmPw = ref('')
const pwError = ref('')
const pwLoading = ref(false)

function closePwModal() {
  showPwModal.value = false
  currentPw.value = ''
  newPw.value = ''
  confirmPw.value = ''
  pwError.value = ''
}

async function changePw() {
  if (!currentPw.value || !newPw.value || !confirmPw.value) return
  if (newPw.value.length < 4) {
    pwError.value = '새 비밀번호는 4자 이상이어야 합니다.'
    return
  }
  if (newPw.value !== confirmPw.value) {
    pwError.value = '새 비밀번호가 일치하지 않습니다.'
    return
  }
  pwLoading.value = true
  pwError.value = ''
  try {
    await teacherChangePassword(currentPw.value, newPw.value)
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
</script>
