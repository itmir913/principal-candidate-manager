<template>
  <div class="space-y-4">
    <div v-if="loading" class="bg-white border rounded-lg px-4 py-8 text-center text-sm text-gray-400">
      불러오는 중...
    </div>

    <div v-else-if="rounds.length === 0" class="bg-white border rounded-lg px-4 py-8 text-center text-sm text-gray-400">
      아직 개설된 라운드가 없습니다.
    </div>

    <div
      v-for="round in rounds"
      :key="round.id"
      class="bg-white border rounded-lg overflow-hidden"
    >
      <div class="px-4 py-3 border-b bg-gray-50 flex items-center gap-2">
        <h2 class="text-sm font-semibold text-gray-700">
          {{ auth.grade }}학년 {{ auth.classNo }}반 — {{ round.id }}라운드 결과
        </h2>
        <span
          class="text-xs px-1.5 py-0.5 rounded-full"
          :class="round.status === 'FINALIZED' ? 'bg-purple-100 text-purple-700' : round.status === 'CLOSED' ? 'bg-blue-100 text-blue-700' : 'bg-green-100 text-green-700'"
        >{{ round.status === 'FINALIZED' ? '마감완료' : round.status === 'CLOSED' ? '집계중' : '진행중' }}</span>
      </div>

      <!-- 진행중/집계중 라운드 -->
      <div v-if="round.status === 'OPEN'" class="px-4 py-8 text-center text-sm text-gray-400">
        현재 진행중인 라운드입니다.
      </div>

      <div v-else-if="round.status === 'CLOSED'" class="px-4 py-8 text-center text-sm text-gray-400">
        현재 집계중인 라운드입니다.
      </div>

      <!-- FINALIZED 라운드 결과 -->
      <template v-else>
        <div v-for="student in studentsByRound[round.id] ?? []" :key="student.student_id" class="border-b last:border-b-0">
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
      </template>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted } from 'vue'
import { useAuthStore } from '../../stores/auth.js'
import { teacherGetResults, teacherAbandonApplication } from '../../api/teacher.js'

const auth = useAuthStore()

const rounds = ref([])
const results = ref([])
const loading = ref(false)

// round_id → { student_id → { ...student, results[] } } 구조
const studentsByRound = computed(() => {
  const map = {}
  for (const r of results.value) {
    if (!map[r.round_id]) map[r.round_id] = new Map()
    const studentMap = map[r.round_id]
    if (!studentMap.has(r.student_id)) {
      studentMap.set(r.student_id, {
        student_id: r.student_id,
        name: r.name,
        student_code: r.student_code,
        seq_no: r.seq_no,
        results: [],
      })
    }
    studentMap.get(r.student_id).results.push(r)
  }
  // Map → 정렬된 배열로 변환
  const out = {}
  for (const [roundId, studentMap] of Object.entries(map)) {
    out[roundId] = [...studentMap.values()].sort((a, b) => (a.seq_no ?? 999) - (b.seq_no ?? 999))
  }
  return out
})

async function load() {
  loading.value = true
  try {
    const data = await teacherGetResults()
    rounds.value = data.rounds
    results.value = data.results
  } catch {
    rounds.value = []
    results.value = []
  } finally {
    loading.value = false
  }
}

async function handleAbandon(r) {
  if (!confirm(`${r.name} 학생의 ${r.univ_name} ${r.track_name} 지원을 포기 처리하시겠습니까? 한 번 포기하면 다시 되돌릴 수 없으며, 재추천 희망할 경우 다음 라운드에서 재지원해야 합니다.`)) return
  try {
    await teacherAbandonApplication(r.student_id, r.track_id, r.round_id)
    await load()
  } catch (e) {
    alert(e.response?.data || e.message)
  }
}

onMounted(load)
</script>
