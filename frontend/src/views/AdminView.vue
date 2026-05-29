<template>
  <div class="min-h-screen bg-gray-50">
    <!-- 헤더 -->
    <header class="bg-white border-b px-6 py-3 flex items-center justify-between">
      <h1 class="text-xl font-bold text-gray-800">학교장추천전형 관리</h1>
      <button
        class="px-3 py-1.5 text-sm text-gray-600 border rounded hover:bg-gray-100"
        @click="logout"
      >
        로그아웃
      </button>
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
  </div>
</template>

<script setup>
import { ref, computed, defineAsyncComponent } from 'vue'
import { useRouter } from 'vue-router'
import { useAuthStore } from '../stores/auth.js'

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

function logout() {
  auth.logout()
  router.push('/login')
}
</script>
