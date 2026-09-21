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

    <ProductInfoCard class="mb-5" />

    <HelpBox
      :key="helpBox.key"
      class="mb-5"
      :storage-key="helpBox.key"
      :title="helpBox.title"
      :intro="helpBox.intro"
      :items="helpBox.items"
    />

    <!-- 로드 오류 — 서버 오류를 "학생 없음" 빈 상태로 위장하지 않는다 -->
    <div
      v-if="loadError"
      class="rounded-xl flex items-center justify-center"
      style="background: #fef2f2; box-shadow: 0 0 0 1px #fca5a5; height: 240px;"
    >
      <p class="text-base" style="color: #991b1b;">학급 현황을 불러오지 못했습니다: {{ loadError }}</p>
    </div>

    <!-- 빈 상태 -->
    <div
      v-else-if="students.length === 0"
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
        <!-- min-width 는 **고정 열 합계보다 커야 한다.** 같으면 가변 열(지원 대학)이
             0px 이 되고, 그때 그 열은 스크롤 대신 글자 단위로 쪼개진다.
             단, 그 0px 은 **컨테이너가 min-width 이하로 좁아졌을 때만** 나타난다 —
             넓은 화면에서는 남는 폭을 가변 열이 가져가므로 보이지 않는다.
             지원 대학은 "동국대학교(서울) — 자연계열 — 의료인공지능공학과" 처럼 길어
             최소 320px 을 둔다(고정 360 + 320 = 680).
             1024×768 학교 PC 에서 가로 스크롤이 나지 않는 것을 기준으로 잡았다 —
             그 화면의 표 컨테이너는 약 704px 다. 스크롤바는 style.css 가 숨기므로
             가로 스크롤이 나면 사용자가 손잡이를 볼 수 없다.
             졸업생 담당(grade=0)은 번호 열이 없어 80px 이 더 남는다. -->
        <table class="w-full" style="border-collapse: collapse; table-layout: fixed; min-width: 680px;">
          <colgroup>
            <col v-if="auth.grade !== 0" style="width: 80px;">
            <col style="width: 140px;">   <!-- 학생코드: "202630601" 9자 + padding 40px -->
            <col style="width: 140px;">   <!-- 이름 -->
            <col>   <!-- 지원 대학: 남는 폭을 전부 가진다(최소 320px) -->
          </colgroup>
          <thead>
            <tr style="background: #f8fafc; border-bottom: 1px solid #e2e8f0;">
              <th v-if="auth.grade !== 0" class="text-base font-semibold text-left" style="padding: 14px 20px; color: #475569;">번호</th>
              <th class="text-base font-semibold text-left" style="padding: 14px 20px; color: #475569;">학생코드</th>
              <th class="text-base font-semibold text-left" style="padding: 14px 20px; color: #475569;">이름</th>
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
                <!-- 이 목록은 `teacherGetApplications()` 를 인자 없이 불러 **전 라운드**를
                     담는다. applications 의 PK 는 (student_id, track_id, round_id) 라
                     같은 모집단위에 1차·2차로 지원하면 track_id 만으로는 키가 중복된다.
                     주석은 반드시 **여는 태그 밖**에 둔다 — 속성 사이에 넣으면 잘못된
                     마크업이라 글자가 화면에 그대로 새어 나온다(실제로 그랬다). -->
                <div
                  v-for="app in getStudentApps(s.id)"
                  :key="`${app.round_id}-${app.track_id}`"
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
                  <span v-else-if="!app.recommended && app.round_status === 'FINALIZED'" class="text-base font-semibold" style="color: #ef4444;">미선발</span>
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
import { ref, computed, onMounted } from 'vue'
import { useAuthStore } from '../../stores/auth.js'
import ProductInfoCard from '../common/ProductInfoCard.vue'
import { dialog } from '../common/dialog.js'
import HelpBox from '../common/HelpBox.vue'
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
const loadError    = ref('')

// 이 화면은 조회 전용이라 무엇을 할 수 없는지(학생 추가·지원 등록)를 먼저
// 알려야 담임이 다른 탭을 찾아 헤매지 않는다.
const helpBox = computed(() => {
  if (students.value.length === 0) {
    return {
      key: 'class-empty',
      title: '도움말 — 학생 명단이 비어 있습니다',
      intro: '학생 명단은 관리자가 일괄 등록합니다. 이 화면에서는 추가할 수 없습니다.',
      items: [
        '담당 학급의 학생이 보이지 않으면 관리자(학교장추천 담당 교사)에게 명단 등록을 요청하세요.',
        '명단이 등록되면 이 화면에 학생과 지원 현황이 표시됩니다.',
      ],
    }
  }
  return {
    key: 'class-main',
    title: '도움말 — 우리 반 지원 현황 보기',
    intro: '담당 학급 학생과 각자의 지원 대학·모집단위를 한눈에 확인하는 화면입니다.',
    items: [
      '지원자를 새로 등록하려면 왼쪽 "지원자 등록" 탭으로 이동하세요.',
      '"취소" 버튼은 현재 진행 중인 라운드의 지원에만 나타납니다. 이전 라운드의 지원은 취소할 수 없습니다.',
      '가로줄이 그어진 지원은 학생이 포기했거나 마감 결과 미선발된 것입니다. 오른쪽 라벨에서 "(포기됨)"·"추천 확정"·"미선발"을 구분할 수 있습니다.',
      '학생 추가·삭제와 재학/졸업 구분 변경은 관리자만 할 수 있습니다.',
    ],
  }
})

function getStudentApps(studentId) {
  return applications.value.filter(a => a.student_id === studentId)
}

async function loadAll() {
  loadError.value = ''
  try {
    const [round, sts, apps] = await Promise.all([
      getCurrentRound(),
      teacherGetStudents(),
      teacherGetApplications(),
    ])
    currentRound.value = round
    students.value = sts
    applications.value = apps
  } catch (e) {
    currentRound.value = null
    students.value = []
    applications.value = []
    loadError.value = e.response?.data ?? e.message ?? '오류가 발생했습니다'
  }
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
