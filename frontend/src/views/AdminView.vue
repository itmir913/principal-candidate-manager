<template>
  <div class="min-h-screen bg-gray-50">
    <!-- 헤더 -->
    <header class="bg-white border-b px-6 py-3 flex items-center justify-between">
      <h1 class="text-xl font-bold text-gray-800">학교장추천전형 관리</h1>
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
        <h2 class="text-base font-semibold text-gray-800 mb-4">관리자 비밀번호 변경</h2>
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
import { ref, computed, defineAsyncComponent } from 'vue'
import { useRouter } from 'vue-router'
import { useAuthStore } from '../stores/auth.js'
import { changeAdminPassword } from '../api/admin.js'

const router = useRouter()
const auth = useAuthStore()

const tabs = [
  { key: 'rounds',   label: '선발 관리' },
  { key: 'classes',  label: '학급 관리' },
  { key: 'students', label: '학생 관리' },
  { key: 'areas',    label: '영역 설정' },
  { key: 'univs',    label: '대학 설정' },
]
const active = ref('rounds')

const RoundsTab   = defineAsyncComponent(() => import('../components/admin/RoundsTab.vue'))
const ClassesTab  = defineAsyncComponent(() => import('../components/admin/ClassesTab.vue'))
const StudentsTab = defineAsyncComponent(() => import('../components/admin/StudentsTab.vue'))
const AreasTab    = defineAsyncComponent(() => import('../components/admin/AreasTab.vue'))
const UnivTab     = defineAsyncComponent(() => import('../components/admin/UniversitiesTab.vue'))

const currentTab = computed(() => {
  if (active.value === 'rounds')   return RoundsTab
  if (active.value === 'classes')  return ClassesTab
  if (active.value === 'students') return StudentsTab
  if (active.value === 'areas')    return AreasTab
  return UnivTab
})

const showPwModal = ref(false)
const newPw = ref('')
const pwError = ref('')
const pwLoading = ref(false)

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
    await changeAdminPassword(newPw.value)
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
