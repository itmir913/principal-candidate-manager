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

      <!-- 역할 토글 -->
      <div
        class="flex rounded-xl overflow-hidden mb-6"
        style="border: 1px solid #e2e8f0;"
      >
        <button
          type="button"
          @click="switchMode('teacher')"
          class="flex-1 text-base font-semibold py-2.5 transition-colors"
          :style="mode === 'teacher'
            ? { background: '#2563eb', color: 'white', border: 'none', cursor: 'pointer' }
            : { background: 'white', color: '#64748b', border: 'none', cursor: 'pointer' }"
        >담임</button>
        <button
          type="button"
          @click="switchMode('admin')"
          class="flex-1 text-base font-semibold py-2.5 transition-colors"
          :style="mode === 'admin'
            ? { background: '#2563eb', color: 'white', border: 'none', cursor: 'pointer' }
            : { background: 'white', color: '#64748b', border: 'none', cursor: 'pointer' }"
        >관리자</button>
      </div>

      <!-- 담임 로그인 폼 -->
      <form v-if="mode === 'teacher'" @submit.prevent="handleTeacherLogin" class="flex flex-col gap-4">
        <div :class="isGraduated ? '' : 'grid grid-cols-2 gap-3'">
          <div>
            <label class="block text-base font-medium mb-1.5" style="color: #64748b;">학년</label>
            <select
              v-model.number="teacherGrade"
              required
              :disabled="classesLoading"
              class="w-full text-base focus:outline-none focus:ring-2 focus:ring-blue-400 disabled:opacity-50"
              style="border: 1px solid #e2e8f0; border-radius: 8px; padding: 10px 14px; box-sizing: border-box; background: white;"
              @change="onGradeChange"
            >
              <option :value="''">{{ classesLoading ? '로딩 중…' : '선택' }}</option>
              <option v-for="g in availableGrades" :key="g" :value="g">{{ g === 0 ? '졸업생' : g + '학년' }}</option>
            </select>
          </div>
          <div v-if="!isGraduated">
            <label class="block text-base font-medium mb-1.5" style="color: #64748b;">반</label>
            <select
              v-model.number="teacherClassNo"
              required
              :disabled="classesLoading || !teacherGrade"
              class="w-full text-base focus:outline-none focus:ring-2 focus:ring-blue-400 disabled:opacity-50"
              style="border: 1px solid #e2e8f0; border-radius: 8px; padding: 10px 14px; box-sizing: border-box; background: white;"
            >
              <option :value="''">선택</option>
              <option v-for="c in availableClassNos" :key="c" :value="c">{{ c }}반</option>
            </select>
          </div>
        </div>

        <div>
          <label class="block text-base font-medium mb-1.5" style="color: #64748b;">
            비밀번호
            <span v-if="isGraduated" class="text-base font-normal" style="color: #94a3b8;">(관리자 비밀번호)</span>
          </label>
          <input
            v-model="teacherPassword"
            type="password"
            autocomplete="current-password"
            required
            class="w-full text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
            style="border: 1px solid #e2e8f0; border-radius: 8px; padding: 10px 14px; box-sizing: border-box;"
          />
        </div>

        <button
          type="submit"
          :disabled="loading || teacherGrade === '' || (!isGraduated && !teacherClassNo)"
          class="w-full text-base font-semibold disabled:opacity-40 transition-colors"
          style="padding: 12px; border: none; border-radius: 10px; background: #2563eb; color: white; cursor: pointer; margin-top: 4px;"
        >{{ loading ? '로그인 중…' : '로그인' }}</button>
      </form>

      <!-- 관리자 로그인 폼 -->
      <form v-if="mode === 'admin'" @submit.prevent="handleAdminLogin" class="flex flex-col gap-4">
        <div>
          <label class="block text-base font-medium mb-1.5" style="color: #64748b;">비밀번호</label>
          <input
            v-model="adminPassword"
            type="password"
            autocomplete="current-password"
            required
            class="w-full text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
            style="border: 1px solid #e2e8f0; border-radius: 8px; padding: 10px 14px; box-sizing: border-box;"
          />
        </div>

        <button
          type="submit"
          :disabled="loading"
          class="w-full text-base font-semibold disabled:opacity-40 transition-colors"
          style="padding: 12px; border: none; border-radius: 10px; background: #2563eb; color: white; cursor: pointer; margin-top: 4px;"
        >{{ loading ? '로그인 중…' : '로그인' }}</button>
      </form>

      <!-- 에러 메시지 -->
      <p v-if="error" class="text-base text-center mt-4" style="color: #ef4444;">{{ error }}</p>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted } from 'vue'
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

const isGraduated = computed(() => teacherGrade.value === 0)

const availableClassNos = computed(() => {
  if (!teacherGrade.value || isGraduated.value) return []
  return classes.value
    .filter(c => c.grade === teacherGrade.value)
    .map(c => c.class_no)
    .sort((a, b) => a - b)
})

function onGradeChange() {
  if (isGraduated.value) {
    teacherClassNo.value = 0
  } else if (!availableClassNos.value.includes(teacherClassNo.value)) {
    teacherClassNo.value = ''
  }
}

async function fetchClasses() {
  classesLoading.value = true
  try {
    const res = await axios.get('/api/classes')
    classes.value = res.data
    // 저장된 학년·반이 실제 목록에 없으면 초기화
    if (teacherGrade.value !== '' && !availableGrades.value.includes(Number(teacherGrade.value))) {
      teacherGrade.value = ''
      teacherClassNo.value = ''
    } else if (teacherClassNo.value !== '' && !availableClassNos.value.includes(Number(teacherClassNo.value))) {
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
    const classNoVal = isGraduated.value ? 0 : Number(teacherClassNo.value)
    await auth.loginTeacher(Number(teacherGrade.value), classNoVal, teacherPassword.value)
    localStorage.setItem(LS_GRADE, teacherGrade.value)
    localStorage.setItem(LS_CLASS, classNoVal)
    router.push('/teacher')
  } catch (e) {
    error.value = e.response?.data || '로그인에 실패했습니다.'
  } finally {
    loading.value = false
  }
}

onMounted(fetchClasses)
</script>
