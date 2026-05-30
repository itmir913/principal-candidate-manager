<template>
  <div class="bg-white border rounded-lg overflow-hidden">
    <div class="px-4 py-3 border-b bg-gray-50">
      <h2 class="text-sm font-semibold text-gray-700">
        {{ auth.grade }}학년 {{ auth.classNo }}반 결과 조회
      </h2>
    </div>

    <div v-if="loading" class="px-4 py-8 text-center text-sm text-gray-400">
      불러오는 중...
    </div>

    <div v-else-if="results.length === 0" class="px-4 py-8 text-center text-sm text-gray-400">
      라운드가 아직 마감되지 않았습니다. 관리자가 라운드를 마감할 때까지 기다려주세요.
    </div>

    <template v-else>
      <div v-for="student in resultsByStudent" :key="student.student_id" class="border-b last:border-b-0">
        <div class="px-4 py-2 bg-gray-50 flex items-center gap-2">
          <span class="text-xs text-gray-400 w-6">{{ student.seq_no }}</span>
          <span class="text-sm font-semibold text-gray-800">{{ student.name }}</span>
          <span class="text-xs text-gray-400">{{ student.student_code }}</span>
        </div>
        <table class="w-full text-sm">
          <thead>
            <tr class="border-b">
              <th class="text-left px-6 py-1.5 text-xs text-gray-400 font-medium">지원 대학</th>
              <th class="text-center px-3 py-1.5 text-xs text-gray-400 font-medium w-16">순위</th>
              <th class="text-right px-4 py-1.5 text-xs text-gray-400 font-medium w-20">총점</th>
              <th class="text-center px-3 py-1.5 text-xs text-gray-400 font-medium w-20">상태</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="r in student.results"
              :key="r.track_id"
              class="border-b last:border-b-0"
              :class="r.recommended ? 'bg-green-50' : r.abandoned ? 'bg-red-50 opacity-60' : ''"
            >
              <td class="px-6 py-2 text-gray-700">{{ r.univ_name }} — {{ r.track_name }}</td>
              <td class="px-3 py-2 text-center text-gray-500">{{ r.ranking ?? '-' }}</td>
              <td class="px-4 py-2 text-right font-semibold text-gray-800">
                {{ r.total_score.toFixed(2) }}
              </td>
              <td class="px-3 py-2 text-center">
                <span v-if="r.recommended" class="text-xs text-green-600 font-semibold">추천 확정</span>
                <span v-else-if="r.abandoned" class="text-xs text-red-400">포기</span>
                <span v-else class="text-xs text-gray-300">-</span>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </template>
  </div>
</template>

<script setup>
import { ref, computed, onMounted } from 'vue'
import { useAuthStore } from '../../stores/auth.js'
import { teacherGetResults } from '../../api/teacher.js'

const auth = useAuthStore()

const results = ref([])
const loading = ref(false)

const resultsByStudent = computed(() => {
  const map = new Map()
  for (const r of results.value) {
    if (!map.has(r.student_id)) {
      map.set(r.student_id, {
        student_id: r.student_id,
        name: r.name,
        student_code: r.student_code,
        seq_no: r.seq_no,
        results: [],
      })
    }
    map.get(r.student_id).results.push(r)
  }
  return [...map.values()].sort((a, b) => (a.seq_no ?? 999) - (b.seq_no ?? 999))
})

onMounted(async () => {
  loading.value = true
  try {
    results.value = await teacherGetResults()
  } catch {
    results.value = []
  } finally {
    loading.value = false
  }
})
</script>
