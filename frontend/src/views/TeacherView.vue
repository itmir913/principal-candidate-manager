<template>
  <div class="flex h-screen overflow-hidden" style="background: #eeecea;">

    <!-- 사이드바 -->
    <aside
      class="flex flex-col flex-shrink-0 bg-white overflow-hidden"
      :style="{
        width: collapsed ? '64px' : '240px',
        borderRight: '1px solid #d4d0cc',
        transition: 'width 0.2s ease',
      }"
    >
      <!-- 로고 + 접기 버튼 -->
      <div
        class="flex items-center flex-shrink-0"
        :style="{
          height: '60px',
          borderBottom: '1px solid #f1f5f9',
          justifyContent: collapsed ? 'center' : 'space-between',
          padding: collapsed ? '0' : '0 14px 0 16px',
        }"
      >
        <div v-if="!collapsed" class="flex items-center gap-2 whitespace-nowrap">
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="#2563eb" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M22 10v6M2 10l10-5 10 5-10 5z"/>
            <path d="M6 12v5c3 3 9 3 12 0v-5"/>
          </svg>
          <span class="text-base font-bold" style="color: #1e293b;">학교장추천 선발 시스템</span>
        </div>
        <button
          @click="collapsed = !collapsed"
          class="flex items-center justify-center p-1.5 rounded-md"
          style="background: none; border: none; cursor: pointer; color: #94a3b8;"
        >
          <ChevronRight v-if="collapsed" :size="18" />
          <Menu v-else :size="18" />
        </button>
      </div>

      <!-- 메뉴 내비게이션 -->
      <nav class="flex-1 overflow-y-auto" style="padding: 10px 8px; display: flex; flex-direction: column; gap: 2px;">
        <button
          v-for="item in sidebarMenus"
          :key="item.key"
          @click="active = item.key"
          :title="item.label"
          class="w-full rounded-lg text-base transition-all duration-150"
          :style="{
            display: 'flex',
            alignItems: 'center',
            gap: collapsed ? '0' : '12px',
            justifyContent: collapsed ? 'center' : 'flex-start',
            padding: collapsed ? '10px 0' : '10px 14px',
            border: 'none',
            cursor: 'pointer',
            fontWeight: active === item.key ? '600' : '400',
            color: active === item.key ? '#1d4ed8' : '#64748b',
            background: active === item.key ? '#eff6ff' : 'transparent',
          }"
        >
          <span class="relative flex-shrink-0 flex">
            <component :is="item.icon" :size="20" />
          </span>
          <span v-if="!collapsed" class="whitespace-nowrap">{{ item.label }}</span>
        </button>
      </nav>

      <!-- 하단 사용자 카드 -->
      <div :style="{ padding: collapsed ? '10px 8px' : '10px', flexShrink: 0, borderTop: '1px solid #e8e5e2' }">
        <!-- 접힘: 아바타 -->
        <div v-if="collapsed" class="flex justify-center items-center" style="padding: 6px 0;">
          <div
            class="flex items-center justify-center rounded-full font-bold"
            style="width: 36px; height: 36px; background: #dbeafe; color: #1d4ed8; font-size: 16px;"
          >{{ auth.grade === 0 ? '졸' : '담' }}</div>
        </div>
        <!-- 펼침: 정보 카드 -->
        <div
          v-else
          style="background: #f5f3f0; border-radius: 10px; padding: 12px 14px; display: flex; flex-direction: column; gap: 10px;"
        >
          <!-- 라운드 상태 -->
          <div class="flex items-center gap-2 pb-2" style="border-bottom: 1px solid #e8e5e2;">
            <div
              class="rounded-full flex-shrink-0"
              :style="{ width: '8px', height: '8px', background: currentRound ? '#22c55e' : '#94a3b8' }"
            />
            <span
              class="text-base font-medium whitespace-nowrap"
              :style="{ color: currentRound ? '#15803d' : '#64748b' }"
            >
              {{ currentRound ? `${currentRound.id}차 라운드 진행 중` : '진행 중인 라운드 없음' }}
            </span>
          </div>
          <!-- 사용자 정보 -->
          <div>
            <p class="text-base font-semibold whitespace-nowrap" style="margin: 0; color: #1e293b;">
              {{ auth.teacherName ? `${auth.teacherName} 선생님` : '선생님' }}
            </p>
            <p class="text-base whitespace-nowrap" style="margin: 2px 0 0; color: #94a3b8;">{{ roleLabel }}</p>
          </div>
          <!-- 액션 버튼 -->
          <div class="flex gap-3">
            <button
              v-if="auth.grade !== 0"
              @click="showPwModal = true"
              class="flex items-center gap-1 text-base"
              style="background: none; border: none; cursor: pointer; color: #94a3b8; padding: 0;"
            >
              <KeyRound :size="14" /> 비번변경
            </button>
            <button
              @click="logout"
              class="flex items-center gap-1 text-base"
              style="background: none; border: none; cursor: pointer; color: #94a3b8; padding: 0;"
            >
              <LogOut :size="14" /> 로그아웃
            </button>
          </div>
        </div>
      </div>
    </aside>

    <!-- 메인 콘텐츠 -->
    <main class="flex-1 overflow-y-auto" style="scrollbar-gutter: stable;">
      <Suspense v-if="currentTab">
        <component :is="currentTab" />
      </Suspense>
      <div v-else class="flex items-center justify-center" style="height: 320px;">
        <p class="text-base" style="color: #94a3b8;">{{ currentMenuItem?.label ?? '' }} 탭 준비 중</p>
      </div>
    </main>

    <!-- 비밀번호 변경 모달 -->
    <div v-if="showPwModal" class="fixed inset-0 flex items-center justify-center z-50" style="background: rgba(0,0,0,0.35);">
      <div class="bg-white" style="border-radius: 14px; box-shadow: 0 8px 32px rgba(0,0,0,0.15); padding: 1.75rem; width: 340px;">
        <h2 class="text-lg font-semibold mb-5" style="color: #1e293b;">비밀번호 변경</h2>
        <div class="space-y-4 mb-5">
          <div>
            <label class="block text-base font-medium mb-1.5" style="color: #64748b;">현재 비밀번호</label>
            <input
              v-model="currentPw"
              type="password"
              autocomplete="current-password"
              class="w-full text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
              style="border: 1px solid #e2e8f0; border-radius: 8px; padding: 10px 14px; box-sizing: border-box;"
            />
          </div>
          <div>
            <label class="block text-base font-medium mb-1.5" style="color: #64748b;">새 비밀번호</label>
            <input
              v-model="newPw"
              type="password"
              autocomplete="new-password"
              class="w-full text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
              style="border: 1px solid #e2e8f0; border-radius: 8px; padding: 10px 14px; box-sizing: border-box;"
            />
          </div>
          <div>
            <label class="block text-base font-medium mb-1.5" style="color: #64748b;">새 비밀번호 재입력</label>
            <input
              v-model="confirmPw"
              type="password"
              autocomplete="new-password"
              class="w-full text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
              style="border: 1px solid #e2e8f0; border-radius: 8px; padding: 10px 14px; box-sizing: border-box;"
              @keyup.enter="changePw"
            />
          </div>
        </div>
        <p v-if="pwError" class="text-base text-red-500 mb-3">{{ pwError }}</p>
        <div class="flex gap-2 justify-end">
          <button
            @click="closePwModal"
            class="text-base"
            style="padding: 10px 20px; border-radius: 8px; border: 1px solid #e2e8f0; background: white; cursor: pointer; color: #64748b;"
          >취소</button>
          <button
            :disabled="!currentPw || !newPw || !confirmPw || pwLoading"
            @click="changePw"
            class="text-base font-semibold disabled:opacity-40"
            style="padding: 10px 20px; border-radius: 8px; border: none; background: #2563eb; cursor: pointer; color: white;"
          >{{ pwLoading ? '변경 중...' : '변경' }}</button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, defineAsyncComponent, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { useAuthStore } from '../stores/auth.js'
import { teacherChangePassword, getCurrentRound } from '../api/teacher.js'
import { dialog } from '../components/common/dialog.js'
import { LayoutGrid, UserPlus, Trophy, ChevronRight, LogOut, KeyRound, Menu } from 'lucide-vue-next'

const router = useRouter()
const auth   = useAuthStore()

// ── 탭 컴포넌트 ──────────────────────────────────────────────
const ClassTab       = defineAsyncComponent(() => import('../components/teacher/ClassTab.vue'))
const ApplicationTab = defineAsyncComponent(() => import('../components/teacher/ApplicationTab.vue'))
const ResultsTab     = defineAsyncComponent(() => import('../components/teacher/ResultsTab.vue'))

// ── 메뉴 정의 ────────────────────────────────────────────────
const sidebarMenus = [
  { key: 'class',       label: '학급 관리',   icon: LayoutGrid },
  { key: 'application', label: '지원자 등록', icon: UserPlus },
  { key: 'results',     label: '라운드 결과', icon: Trophy },
]

// ── 활성 탭 ──────────────────────────────────────────────────
const active = ref('class')

const currentTab = computed(() => {
  if (active.value === 'class')       return ClassTab
  if (active.value === 'application') return ApplicationTab
  return ResultsTab
})

const currentMenuItem = computed(() => sidebarMenus.find(m => m.key === active.value))

// ── 사이드바 접기 ─────────────────────────────────────────────
const collapsed = ref(false)

// ── 역할 레이블 ───────────────────────────────────────────────
const roleLabel = computed(() => {
  if (auth.grade === 0) return '졸업생 담당'
  return `${auth.grade}학년 ${auth.classNo}반 담임`
})

// ── 현재 라운드 ───────────────────────────────────────────────
const currentRound = ref(null)
onMounted(async () => {
  currentRound.value = await getCurrentRound()
})

// ── 비밀번호 변경 ─────────────────────────────────────────────
const showPwModal = ref(false)
const currentPw   = ref('')
const newPw       = ref('')
const confirmPw   = ref('')
const pwError     = ref('')
const pwLoading   = ref(false)

function closePwModal() {
  showPwModal.value = false
  currentPw.value   = ''
  newPw.value       = ''
  confirmPw.value   = ''
  pwError.value     = ''
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
  pwError.value   = ''
  try {
    await teacherChangePassword(currentPw.value, newPw.value)
    closePwModal()
    await dialog.alert({ title: '완료', message: '비밀번호가 변경되었습니다.' })
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
