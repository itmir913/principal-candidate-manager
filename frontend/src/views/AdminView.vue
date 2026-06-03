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
        <!-- 주 메뉴 -->
        <button
          v-for="item in mainMenus"
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

        <div style="margin: 8px 0; border-top: 1px solid #f1f5f9;" />

        <!-- 하단 서브 메뉴 -->
        <button
          v-for="item in subMenus"
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
            <span
              v-if="item.badge"
              class="absolute rounded-full"
              style="top: -3px; right: -3px; width: 7px; height: 7px; background: #ef4444; border: 1.5px solid white;"
            />
          </span>
          <template v-if="!collapsed">
            <span class="whitespace-nowrap">{{ item.label }}</span>
            <span
              v-if="item.badge"
              class="ml-auto text-base font-bold"
              style="color: #dc2626; background: #fee2e2; padding: 2px 8px; border-radius: 999px; font-size: 11px;"
            >NEW</span>
          </template>
        </button>
      </nav>

      <!-- 하단 사용자 카드 -->
      <div :style="{ padding: collapsed ? '10px 8px' : '10px', flexShrink: 0, borderTop: '1px solid #e8e5e2' }">
        <!-- 접힘: 아바타 -->
        <div v-if="collapsed" class="flex justify-center items-center" style="padding: 6px 0;">
          <div
            class="flex items-center justify-center rounded-full font-bold"
            style="width: 36px; height: 36px; background: #dbeafe; color: #1d4ed8; font-size: 14px;"
          >관</div>
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
            <p class="text-base font-semibold whitespace-nowrap" style="margin: 0; color: #1e293b;">관리자</p>
            <p class="text-base whitespace-nowrap" style="margin: 2px 0 0; color: #94a3b8;">시스템 관리자</p>
          </div>
          <!-- 액션 버튼 -->
          <div class="flex gap-3">
            <button
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
    <main
        class="flex-1 overflow-y-auto"
        style="scrollbar-gutter: stable;"
    >
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
        <h2 class="text-lg font-semibold mb-5" style="color: #1e293b;">관리자 비밀번호 변경</h2>
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
import { ref, computed, defineAsyncComponent, onMounted, provide } from 'vue'
import axios from 'axios'
import { useRouter } from 'vue-router'
import { useAuthStore } from '../stores/auth.js'
import { changeAdminPassword, getCurrentRound } from '../api/admin.js'
import {
  Home, Trophy, LayoutGrid, Users, SlidersHorizontal,
  Building2, BookOpen, RefreshCw, ChevronRight, LogOut, KeyRound, Menu,
} from 'lucide-vue-next'

const router = useRouter()
const auth = useAuthStore()

// ── 탭 컴포넌트 ──────────────────────────────────────────────
const OverviewTab = defineAsyncComponent(() => import('../components/admin/OverviewTab.vue'))
const RoundsTab   = defineAsyncComponent(() => import('../components/admin/RoundsTab.vue'))
const ClassesTab  = defineAsyncComponent(() => import('../components/admin/ClassesTab.vue'))
const StudentsTab = defineAsyncComponent(() => import('../components/admin/StudentsTab.vue'))
const AreasTab    = defineAsyncComponent(() => import('../components/admin/AreasTab.vue'))
const UnivTab     = defineAsyncComponent(() => import('../components/admin/UniversitiesTab.vue'))
const UpdateTab   = defineAsyncComponent(() => import('../components/admin/UpdateTab.vue'))
const ManualTab   = defineAsyncComponent(() => import('../components/admin/ManualTab.vue'))

// ── 메뉴 정의 ────────────────────────────────────────────────
const mainMenus = [
  { key: 'home',     label: '개요',          icon: Home },
  { key: 'rounds',   label: '라운드 관리',   icon: Trophy },
  { key: 'classes',  label: '학급 관리',     icon: LayoutGrid },
  { key: 'students', label: '학생 관리',     icon: Users },
  { key: 'areas',    label: '전형요소 설정', icon: SlidersHorizontal },
  { key: 'univs',    label: '대학 설정',     icon: Building2 },
]

const hasUpdate = ref(false)

const subMenus = computed(() => [
  { key: 'manual',  label: '매뉴얼',   icon: BookOpen,  badge: false      },
  { key: 'update',  label: '업데이트 & 백업', icon: RefreshCw, badge: hasUpdate.value },
])

const allMenus = computed(() => [...mainMenus, ...subMenus.value])

// ── 활성 탭 ──────────────────────────────────────────────────
const active = ref('home')

const currentTab = computed(() => {
  if (active.value === 'home')     return OverviewTab
  if (active.value === 'rounds')   return RoundsTab
  if (active.value === 'classes')  return ClassesTab
  if (active.value === 'students') return StudentsTab
  if (active.value === 'areas')    return AreasTab
  if (active.value === 'univs')    return UnivTab
  if (active.value === 'update')   return UpdateTab
  if (active.value === 'manual')   return ManualTab
  return null
})

const currentMenuItem = computed(() => allMenus.value.find(m => m.key === active.value))

// ── 사이드바 접기 ─────────────────────────────────────────────
const collapsed = ref(false)

// ── 현재 라운드 ───────────────────────────────────────────────
const currentRound = ref(null)

async function refreshRound() {
  try {
    currentRound.value = await getCurrentRound()
  } catch {
    currentRound.value = null
  }
}

provide('refreshRound', refreshRound)

function stripV(v) {
  return (v ?? '').replace(/^v/i, '').trim()
}

onMounted(async () => {
  await refreshRound()

  // 업데이트 뱃지: 백그라운드에서 조용히 확인
  try {
    const [verRes, ghRes] = await Promise.all([
      axios.get('/api/version'),
      fetch('https://api.github.com/repos/itmir913/principal-candidate-manager/releases/latest', {
        headers: { Accept: 'application/vnd.github+json' },
      }),
    ])
    if (ghRes.ok) {
      const gh = await ghRes.json()
      hasUpdate.value = stripV(verRes.data.version) !== stripV(gh.tag_name)
    }
  } catch {
    // 업데이트 확인 실패 시 뱃지 표시 안 함
  }
})

// ── 비밀번호 변경 ─────────────────────────────────────────────
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
  if (newPw.value.length < 8) {
    pwError.value = '새 비밀번호는 8자 이상이어야 합니다.'
    return
  }
  if (newPw.value !== confirmPw.value) {
    pwError.value = '새 비밀번호가 일치하지 않습니다.'
    return
  }
  pwLoading.value = true
  pwError.value = ''
  try {
    await changeAdminPassword(currentPw.value, newPw.value)
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
