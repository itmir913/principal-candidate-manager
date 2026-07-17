<template>
  <div class="py-8 px-4 sm:px-10">

    <!-- 페이지 헤더 -->
    <div class="flex items-start justify-between flex-wrap gap-3 mb-5">
      <div>
        <p class="text-base mb-1" style="color: #94a3b8;">담임 교사</p>
        <h1 class="text-2xl font-semibold" style="color: #1e293b; margin: 0;">
          <template v-if="auth.grade === 0">졸업생 현황</template>
          <template v-else>{{ auth.grade }}학년 {{ auth.classNo }}반 학급 현황</template>
        </h1>
      </div>
      <span class="text-base" style="color: #64748b;">{{ students.length }}명</span>
    </div>

    <!-- 빈 상태 -->
    <div
      v-if="students.length === 0"
      class="rounded-xl flex items-center justify-center"
      style="background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04); height: 240px;"
    >
      <p class="text-base" style="color: #94a3b8;">학생이 없습니다. 관리자에게 학생 데이터 등록을 요청하세요.</p>
    </div>

    <!-- 학생 테이블 -->
    <div
      v-else
      class="rounded-xl overflow-hidden"
      style="background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04);"
    >
      <div class="overflow-x-auto">
        <table class="w-full min-w-max" style="border-collapse: collapse;">
          <thead>
            <tr style="background: #f8fafc; border-bottom: 1px solid #e2e8f0;">
              <th v-if="auth.grade !== 0" class="text-base font-semibold text-left" style="padding: 14px 20px; color: #475569; width: 80px;">번호</th>
              <th class="text-base font-semibold text-left" style="padding: 14px 20px; color: #475569; width: 180px;">학생코드</th>
              <th class="text-base font-semibold text-left" style="padding: 14px 20px; color: #475569; width: 200px;">이름</th>
              <th class="text-base font-semibold text-left" style="padding: 14px 20px; color: #475569;">지원 대학</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="s in students"
              :key="s.id"
              class="hover:bg-slate-50"
              style="border-bottom: 1px solid #f1f5f9; transition: background 0.1s;"
            >
              <td v-if="auth.grade !== 0" class="text-base" style="padding: 13px 20px; color: #94a3b8;">{{ s.seq_no }}</td>
              <td class="text-base" style="padding: 13px 20px; color: #64748b;">{{ s.student_code }}</td>
              <td class="text-base font-medium" style="padding: 13px 20px; color: #1e293b;">{{ s.name }}</td>
              <td style="padding: 13px 20px;">
                <div v-if="getStudentApps(s.id).length === 0" class="text-base" style="color: #cbd5e1;">-</div>
                <div
                  v-for="app in getStudentApps(s.id)"
                  :key="app.track_id"
                  class="flex items-center gap-2 mb-1.5"
                >
                  <span
                    class="text-base"
                    :class="{ 'line-through': app.abandoned || (!app.recommended && app.round_status === 'FINALIZED') }"
                    :style="{ color: (app.abandoned || (!app.recommended && app.round_status === 'FINALIZED')) ? '#94a3b8' : '#1e293b' }"
                  >
                    {{ app.univ_name }} — {{ app.track_name }} — {{ app.department_name }}
                  </span>
                  <span v-if="app.abandoned" class="text-base font-semibold" style="color: #ef4444;">(포기됨)</span>
                  <span v-else-if="app.recommended && app.round_status === 'FINALIZED'" class="text-base font-semibold" style="color: #16a34a;">추천 확정</span>
                  <span v-else-if="!app.recommended && app.round_status === 'FINALIZED'" class="text-base font-semibold" style="color: #ef4444;">추천 제외</span>
                  <button
                    v-if="currentRound && app.round_id === currentRound.id && !app.abandoned"
                    class="text-base"
                    style="padding: 4px 12px; border: 1px solid #fca5a5; border-radius: 6px; background: white; color: #ef4444; cursor: pointer;"
                    @click="removeApplication(app)"
                  >취소</button>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import { useAuthStore } from '../../stores/auth.js'
import { dialog } from '../common/dialog.js'
import {
  getCurrentRound,
  teacherGetStudents,
  teacherGetApplications,
  teacherDeleteApplication,
} from '../../api/teacher.js'

const auth = useAuthStore()

const currentRound = ref(null)
const students     = ref([])
const applications = ref([])

function getStudentApps(studentId) {
  return applications.value.filter(a => a.student_id === studentId)
}

async function loadAll() {
  const [round, sts, apps] = await Promise.all([
    getCurrentRound(),
    teacherGetStudents(),
    teacherGetApplications(),
  ])
  currentRound.value = round
  students.value = sts
  applications.value = apps
}

async function removeApplication(app) {
  if (!(await dialog.confirm({
    title: '지원 취소',
    message: `${app.name} 학생의 ${app.univ_name} ${app.track_name} 지원을 취소하시겠습니까?\n라운드가 진행 중인 동안에는 다시 등록할 수 있습니다.`,
    confirmText: '지원 취소',
    level: 'warn',
  }))) return
  try {
    await teacherDeleteApplication(app.student_id, app.track_id, app.round_id)
    applications.value = await teacherGetApplications()
  } catch (e) {
    await dialog.alert({ title: '오류', message: e.response?.data || e.message, level: 'error' })
  }
}

onMounted(loadAll)
</script>
