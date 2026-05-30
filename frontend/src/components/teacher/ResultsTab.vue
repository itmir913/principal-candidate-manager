<template>
  <div class="space-y-4">
    <div v-if="loading" class="bg-white border rounded-lg px-4 py-8 text-center text-sm text-gray-400">
      불러오는 중...
    </div>

    <div v-else-if="resultsByRound.length === 0 && !currentRound" class="bg-white border rounded-lg px-4 py-8 text-center text-sm text-gray-400">
      라운드가 아직 마감되지 않았습니다. 관리자가 라운드를 마감할 때까지 기다려주세요.
    </div>

    <div
      v-for="round in resultsByRound"
      :key="round.round_id"
      class="bg-white border rounded-lg overflow-hidden"
    >
      <div class="px-4 py-3 border-b bg-gray-50">
        <h2 class="text-sm font-semibold text-gray-700">
          {{ auth.grade }}학년 {{ auth.classNo }}반 — {{ round.round_id }}라운드 결과
        </h2>
      </div>

      <div v-for="student in round.students" :key="student.student_id" class="border-b last:border-b-0">
        <div class="px-4 py-2 bg-gray-50 flex items-center gap-2">
          <span class="text-xs text-gray-400 w-6">{{ student.seq_no }}</span>
          <span class="text-sm font-semibold text-gray-800">{{ student.name }}</span>
          <span class="text-xs text-gray-400">{{ student.student_code }}</span>
        </div>
        <table class="w-full text-sm">
          <thead>
            <tr class="border-b">
              <th class="text-left px-6 py-1.5 text-xs text-gray-400 font-medium">대학명</th>
              <th class="text-left px-3 py-1.5 text-xs text-gray-400 font-medium">모집단위</th>
              <th class="text-left px-3 py-1.5 text-xs text-gray-400 font-medium">지원 학과</th>
              <th class="text-center px-3 py-1.5 text-xs text-gray-400 font-medium w-16">순위</th>
              <th class="text-right px-4 py-1.5 text-xs text-gray-400 font-medium w-20">총점</th>
              <th class="text-center px-3 py-1.5 text-xs text-gray-400 font-medium w-24">상태</th>
              <th class="text-center px-3 py-1.5 text-xs text-gray-400 font-medium w-24">비고</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="r in student.results"
              :key="r.track_id"
              class="border-b last:border-b-0"
              :class="r.recommended ? 'bg-green-50' : r.abandoned ? 'bg-red-50 opacity-60' : 'bg-red-50'"
            >
              <td class="px-6 py-2 text-gray-700">{{ r.univ_name }}</td>
              <td class="px-3 py-2 text-gray-700">{{ r.track_name }}</td>
              <td class="px-3 py-2 text-gray-600">{{ r.department_name }}</td>
              <td class="px-3 py-2 text-center text-gray-500">{{ r.ranking ?? '-' }}</td>
              <td class="px-4 py-2 text-right font-semibold text-gray-800">
                {{ r.total_score.toFixed(2) }}
              </td>
              <td class="px-3 py-2 text-center">
                <span v-if="r.abandoned" class="text-xs text-red-400 font-semibold">포기됨</span>
                <span v-else-if="r.recommended" class="text-xs text-green-600 font-semibold">추천 확정</span>
                <span v-else class="text-xs text-red-400 font-semibold">추천 제외</span>
              </td>
              <td class="px-3 py-2 text-center">
                <button
                  v-if="r.recommended && !r.abandoned"
                  class="text-xs px-1.5 py-0.5 border border-red-300 text-red-500 rounded hover:bg-red-50"
                  @click="handleAbandon(r)"
                >추천 포기</button>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <!-- 진행중인 라운드 플레이스홀더 -->
    <div v-if="!loading && currentRound" class="bg-white border rounded-lg overflow-hidden">
      <div class="px-4 py-3 border-b bg-gray-50">
        <h2 class="text-sm font-semibold text-gray-700">
          {{ auth.grade }}학년 {{ auth.classNo }}반 — {{ currentRound.id }}라운드 결과
        </h2>
      </div>
      <div class="px-4 py-8 text-center text-sm text-gray-400">
        현재 진행중인 라운드입니다.
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted } from 'vue'
import { useAuthStore } from '../../stores/auth.js'
import { getCurrentRound, teacherGetResults, teacherAbandonApplication } from '../../api/teacher.js'

const auth = useAuthStore()

const results = ref([])
const currentRound = ref(null)
const loading = ref(false)

const resultsByRound = computed(() => {
  const roundMap = new Map()
  for (const r of results.value) {
    if (!roundMap.has(r.round_id)) {
      roundMap.set(r.round_id, { round_id: r.round_id, students: new Map() })
    }
    const round = roundMap.get(r.round_id)
    if (!round.students.has(r.student_id)) {
      round.students.set(r.student_id, {
        student_id: r.student_id,
        name: r.name,
        student_code: r.student_code,
        seq_no: r.seq_no,
        results: [],
      })
    }
    round.students.get(r.student_id).results.push(r)
  }
  return [...roundMap.values()]
    .sort((a, b) => a.round_id - b.round_id)
    .map(round => ({
      ...round,
      students: [...round.students.values()].sort((a, b) => (a.seq_no ?? 999) - (b.seq_no ?? 999)),
    }))
})

async function handleAbandon(r) {
  if (!confirm(`${r.name} 학생의 ${r.univ_name} ${r.track_name} 지원을 포기 처리하시겠습니까? 한 번 포기하면 다시 되돌릴 수 없으며, 재추천 희망할 경우 다음 라운드에서 재지원해야 합니다.`)) return
  try {
    await teacherAbandonApplication(r.student_id, r.track_id, r.round_id)
    results.value = await teacherGetResults()
  } catch (e) {
    alert(e.response?.data || e.message)
  }
}

onMounted(async () => {
  loading.value = true
  try {
    [results.value, currentRound.value] = await Promise.all([
      teacherGetResults(),
      getCurrentRound(),
    ])
  } catch {
    results.value = []
  } finally {
    loading.value = false
  }
})
</script>
