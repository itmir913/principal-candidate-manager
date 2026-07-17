<template>
  <div class="py-8 px-4 sm:px-10">

    <!-- 페이지 헤더 -->
    <div class="flex items-start justify-between flex-wrap gap-3 mb-5">
      <div>
        <p class="text-base mb-1" style="color: #94a3b8;">담임 교사</p>
        <h1 class="text-2xl font-semibold" style="color: #1e293b; margin: 0;">라운드 결과</h1>
      </div>
    </div>

    <HelpBox
      v-if="!loading && !loadError"
      :key="helpBox.key"
      class="mb-5"
      :storage-key="helpBox.key"
      :title="helpBox.title"
      :intro="helpBox.intro"
      :items="helpBox.items"
    />

    <!-- 로딩 -->
    <div
      v-if="loading"
      class="rounded-xl flex items-center justify-center"
      style="background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04); height: 240px;"
    >
      <p class="text-base" style="color: #94a3b8;">불러오는 중...</p>
    </div>

    <!-- 로드 오류 — 서버 오류를 "라운드 없음" 빈 상태로 위장하지 않는다 -->
    <div
      v-else-if="loadError"
      class="rounded-xl flex items-center justify-center"
      style="background: #fef2f2; box-shadow: 0 0 0 1px #fca5a5; height: 240px;"
    >
      <p class="text-base" style="color: #991b1b;">결과를 불러오지 못했습니다: {{ loadError }}</p>
    </div>

    <!-- 빈 상태 -->
    <div
      v-else-if="rounds.length === 0"
      class="rounded-xl flex items-center justify-center"
      style="background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04); height: 240px;"
    >
      <p class="text-base" style="color: #94a3b8;">아직 개설된 라운드가 없습니다.</p>
    </div>

    <!-- 라운드별 결과 카드 -->
    <div v-else class="flex flex-col gap-6">
      <div
        v-for="round in rounds"
        :key="round.id"
        class="rounded-xl overflow-hidden"
        style="background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04);"
      >
        <!-- 카드 헤더 -->
        <div class="flex items-center gap-3 px-6 py-4" style="border-bottom: 1px solid #f1f5f9;">
          <h2 class="text-base font-semibold" style="color: #1e293b; margin: 0;">
            <template v-if="auth.grade === 0">졸업생 — {{ round.id }}라운드 결과</template>
            <template v-else>{{ auth.grade }}학년 {{ auth.classNo }}반 — {{ round.id }}라운드 결과</template>
          </h2>
          <span
            class="text-base font-semibold"
            style="padding: 3px 12px; border-radius: 999px;"
            :style="round.status === 'FINALIZED'
              ? { background: '#f3e8ff', color: '#7c3aed' }
              : round.status === 'CLOSED'
                ? { background: '#dbeafe', color: '#1d4ed8' }
                : { background: '#dcfce7', color: '#15803d' }"
          >{{ round.status === 'FINALIZED' ? '마감완료' : round.status === 'CLOSED' ? '집계중' : '진행중' }}</span>
        </div>

        <!-- 진행중/집계중 -->
        <div v-if="round.status === 'OPEN'" class="flex items-center justify-center" style="height: 120px;">
          <p class="text-base" style="color: #94a3b8;">현재 진행중인 라운드입니다.</p>
        </div>
        <div v-else-if="round.status === 'CLOSED'" class="flex items-center justify-center" style="height: 120px;">
          <p class="text-base" style="color: #94a3b8;">현재 집계중인 라운드입니다.</p>
        </div>

        <!-- FINALIZED 결과 -->
        <template v-else>
          <div
            v-for="student in studentsByRound[round.id] ?? []"
            :key="student.student_id"
            style="border-bottom: 1px solid #f1f5f9;"
          >
            <!-- 학생 행 헤더 -->
            <div class="flex items-center gap-3 px-6 py-3" style="background: #f8fafc; border-bottom: 1px solid #e2e8f0;">
              <span class="text-base font-semibold" style="color: #1e293b;">{{ student.name }}</span>
              <span class="text-base" style="color: #64748b;">{{ student.student_code }}</span>
              <span v-if="auth.grade !== 0" class="text-base" style="color: #94a3b8;">{{ student.seq_no }}번</span>
            </div>

            <!-- 결과 테이블 -->
            <div class="overflow-x-auto">
              <table class="w-full min-w-max" style="border-collapse: collapse;">
                <thead>
                  <tr style="background: #f8fafc; border-bottom: 1px solid #e2e8f0;">
                    <th class="text-base font-semibold text-left" style="padding: 12px 20px; color: #475569;">대학명</th>
                    <th class="text-base font-semibold text-left" style="padding: 12px 16px; color: #475569;">모집단위</th>
                    <th class="text-base font-semibold text-left" style="padding: 12px 16px; color: #475569;">지원 학과</th>
                    <th class="text-base font-semibold text-center" style="padding: 12px 16px; color: #475569; width: 80px;">순위</th>
                    <th class="text-base font-semibold text-right" style="padding: 12px 20px; color: #475569; width: 100px;">총점</th>
                    <th class="text-base font-semibold text-center" style="padding: 12px 16px; color: #475569; width: 120px;">상태</th>
                    <th class="text-base font-semibold text-center" style="padding: 12px 16px; color: #475569; width: 120px;">비고</th>
                  </tr>
                </thead>
                <tbody>
                  <tr
                    v-for="r in student.results"
                    :key="r.track_id"
                    :style="{
                      borderBottom: '1px solid #f1f5f9',
                      background: r.recommended && !r.abandoned ? '#f0fdf4' : '#fff1f2',
                    }"
                  >
                    <td class="text-base" style="padding: 12px 20px; color: #1e293b;">{{ r.univ_name }}</td>
                    <td class="text-base" style="padding: 12px 16px; color: #1e293b;">{{ r.track_name }}</td>
                    <td class="text-base" style="padding: 12px 16px; color: #475569;">{{ r.department_name }}</td>
                    <td class="text-base text-center" style="padding: 12px 16px; color: #64748b;">{{ r.ranking ?? '-' }}</td>
                    <td class="text-base text-right font-semibold" style="padding: 12px 20px; color: #1e293b;">
                      {{ r.total_score.toFixed(2) }}
                    </td>
                    <td class="text-center" style="padding: 12px 16px;">
                      <span v-if="r.abandoned" class="text-base font-semibold" style="color: #ef4444;">포기됨</span>
                      <span v-else-if="r.recommended" class="text-base font-semibold" style="color: #16a34a;">추천 확정</span>
                      <span v-else class="text-base font-semibold" style="color: #ef4444;">추천 제외</span>
                    </td>
                    <td class="text-center" style="padding: 12px 16px;">
                      <button
                        v-if="r.recommended && !r.abandoned"
                        class="text-base whitespace-nowrap"
                        style="padding: 6px 12px; border: 1px solid #fca5a5; border-radius: 6px; background: white; color: #ef4444; cursor: pointer;"
                        @click="handleAbandon(r)"
                      >추천 포기</button>
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>
          </div>
        </template>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted } from 'vue'
import { useAuthStore } from '../../stores/auth.js'
import { teacherGetResults, teacherAbandonApplication } from '../../api/teacher.js'
import { dialog } from '../common/dialog.js'
import HelpBox from '../common/HelpBox.vue'

const auth = useAuthStore()

const rounds    = ref([])
const results   = ref([])
const loading   = ref(false)
const loadError = ref('')

const hasFinalized = computed(() => rounds.value.some(r => r.status === 'FINALIZED'))

const helpBox = computed(() => {
  if (hasFinalized.value) {
    return {
      key: 'results-final',
      title: '도움말 — 결과 보는 방법',
      intro: '마감된 라운드의 우리 반 학생 결과입니다.',
      items: [
        '초록색 배경의 "추천 확정"은 학교장추천 대상으로 확정된 것이고, 붉은색 배경의 "추천 제외"는 이번 라운드에서 추천되지 않은 것입니다.',
        '추천이 확정된 학생이 추천을 포기하려면 "추천 포기"를 누르세요.',
        { text: '포기는 되돌릴 수 없습니다. 반드시 학생·학부모와 확인한 뒤 처리하세요. 다시 추천받으려면 다음 라운드에서 재지원해야 합니다.', warn: true },
        '"추천 제외"된 학생은 다음 라운드가 열리면 다시 지원할 수 있습니다.',
      ],
    }
  }
  if (rounds.value.length === 0) {
    return {
      key: 'results-none',
      title: '도움말 — 결과는 마감 후 공개됩니다',
      intro: '아직 라운드가 열리지 않았습니다.',
      items: [
        '관리자가 라운드를 열면 지원자 등록이 시작되고, 라운드가 마감되면 이 화면에 우리 반 학생들의 순위·총점·추천 여부가 표시됩니다.',
      ],
    }
  }
  return {
    key: 'results-waiting',
    title: '도움말 — 결과는 마감 후 공개됩니다',
    intro: '라운드 결과는 관리자가 라운드를 "마감"한 뒤에만 표시됩니다.',
    items: [
      '"진행중" 또는 "집계중"으로 표시된 라운드는 아직 결과가 공개되지 않은 것입니다.',
      '마감되면 이 화면에 우리 반 학생들의 순위·총점·추천 여부가 표시됩니다.',
    ],
  }
})

// round_id → { student_id → { ...student, results[] } } 구조
const studentsByRound = computed(() => {
  const map = {}
  for (const r of results.value) {
    if (!map[r.round_id]) map[r.round_id] = new Map()
    const studentMap = map[r.round_id]
    if (!studentMap.has(r.student_id)) {
      studentMap.set(r.student_id, {
        student_id:   r.student_id,
        name:         r.name,
        student_code: r.student_code,
        seq_no:       r.seq_no,
        results:      [],
      })
    }
    studentMap.get(r.student_id).results.push(r)
  }
  // Map → 정렬된 배열로 변환
  const out = {}
  for (const [roundId, studentMap] of Object.entries(map)) {
    out[roundId] = [...studentMap.values()].sort((a, b) =>
      auth.grade === 0
        ? a.student_code.localeCompare(b.student_code)
        : (a.seq_no ?? 999) - (b.seq_no ?? 999)
    )
  }
  return out
})

async function load() {
  loading.value = true
  loadError.value = ''
  try {
    const data = await teacherGetResults()
    rounds.value  = data.rounds
    results.value = data.results
  } catch (e) {
    rounds.value  = []
    results.value = []
    loadError.value = e.response?.data ?? e.message ?? '오류가 발생했습니다'
  } finally {
    loading.value = false
  }
}

async function handleAbandon(r) {
  if (!(await dialog.confirm({
    title: '추천 포기',
    message: `${r.name} 학생의 ${r.univ_name} ${r.track_name} 지원을 포기 처리하시겠습니까?`,
    confirmText: '포기 처리',
    level: 'danger',
    dangerNotice: '한 번 포기하면 다시 되돌릴 수 없습니다. 재추천을 희망하면 다음 라운드에서 재지원해야 합니다.',
    finalConfirmText: '포기 확정',
  }))) return
  try {
    await teacherAbandonApplication(r.student_id, r.track_id, r.round_id)
    await load()
  } catch (e) {
    await dialog.alert({ title: '오류', message: e.response?.data || e.message, level: 'error' })
  }
}

onMounted(load)
</script>
