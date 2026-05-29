<template>
  <div class="flex gap-6">
    <!-- 좌측: 라운드 목록 -->
    <div class="w-56 shrink-0">
      <div class="flex items-center justify-between mb-3">
        <span class="text-sm font-semibold text-gray-700">라운드</span>
        <button
          class="px-2 py-1 text-xs bg-blue-600 text-white rounded hover:bg-blue-700 disabled:opacity-40"
          :disabled="hasOpenRound || loading"
          @click="handleOpenRound"
        >라운드 열기</button>
      </div>

      <div v-if="rounds.length === 0" class="text-xs text-gray-400 py-4 text-center">
        라운드 없음
      </div>

      <ul class="space-y-1">
        <li
          v-for="r in rounds"
          :key="r.id"
          class="flex items-center justify-between px-3 py-2 rounded cursor-pointer text-sm"
          :class="selected?.id === r.id ? 'bg-blue-50 border border-blue-200' : 'hover:bg-gray-50 border border-transparent'"
          @click="selectRound(r)"
        >
          <span class="font-medium text-gray-800">{{ r.id }}차</span>
          <span
            class="text-xs px-1.5 py-0.5 rounded-full"
            :class="r.status === 'OPEN' ? 'bg-green-100 text-green-700' : 'bg-gray-100 text-gray-500'"
          >{{ r.status }}</span>
        </li>
      </ul>
    </div>

    <!-- 우측: 라운드 상세 -->
    <div class="flex-1 min-w-0">
      <div v-if="!selected" class="flex items-center justify-center h-48 text-gray-400 text-sm">
        라운드를 선택하거나 새 라운드를 열어주세요
      </div>

      <template v-else>
        <!-- 상단 요약 -->
        <div class="flex items-center gap-3 mb-4 flex-wrap">
          <span class="text-lg font-bold text-gray-800">{{ selected.id }}차 라운드</span>
          <span
            class="text-xs px-2 py-1 rounded-full font-medium"
            :class="selected.status === 'OPEN' ? 'bg-green-100 text-green-700' : 'bg-gray-100 text-gray-500'"
          >{{ selected.status }}</span>
          <button
            v-if="selected.status === 'OPEN'"
            class="text-xs px-2.5 py-1 border border-red-300 text-red-600 rounded hover:bg-red-50"
            @click="handleCloseRound(selected.id)"
          >마감</button>
          <span class="text-xs text-gray-400">{{ fmtDt(selected.opened_at) }} 개시</span>
          <span v-if="selected.closed_at" class="text-xs text-gray-400">→ {{ fmtDt(selected.closed_at) }} 마감</span>
        </div>

        <!-- 서브탭 -->
        <div class="flex border-b mb-4">
          <button
            v-for="t in subTabs"
            :key="t.key"
            class="px-4 py-2 text-sm font-medium border-b-2 transition-colors"
            :class="view === t.key
              ? 'border-blue-600 text-blue-600'
              : 'border-transparent text-gray-500 hover:text-gray-700'"
            @click="view = t.key"
          >{{ t.label }}</button>
        </div>

        <!-- 지원 현황 탭 -->
        <div v-if="view === 'apps'">
          <div class="flex items-center justify-between mb-3">
            <span class="text-sm text-gray-600">총 {{ apps.length }}건</span>
            <button
              class="px-3 py-1.5 text-sm bg-indigo-600 text-white rounded hover:bg-indigo-700 disabled:opacity-40"
              :disabled="calcLoading || apps.length === 0"
              @click="handleCalculate"
            >{{ calcLoading ? '계산 중…' : '점수 계산' }}</button>
          </div>
          <div v-if="calcMsg" class="mb-3 text-sm" :class="calcMsg.ok ? 'text-green-600' : 'text-red-500'">
            {{ calcMsg.text }}
          </div>

          <div v-if="apps.length === 0" class="text-sm text-gray-400 py-6 text-center">
            지원자가 없습니다
          </div>

          <div v-for="(group, key) in appsByUniv" :key="key" class="mb-4">
            <h4 class="text-sm font-semibold text-gray-700 mb-1">{{ key }}</h4>
            <table class="w-full text-sm border rounded overflow-hidden">
              <thead class="bg-gray-50">
                <tr>
                  <th class="text-left px-3 py-2 text-xs text-gray-500 font-medium">학생명</th>
                  <th class="text-left px-3 py-2 text-xs text-gray-500 font-medium">학년/반</th>
                  <th class="text-left px-3 py-2 text-xs text-gray-500 font-medium">재학</th>
                  <th class="px-3 py-2 text-xs text-gray-500 font-medium">포기처리</th>
                  <th class="px-3 py-2 text-xs text-gray-500 font-medium">미리보기</th>
                </tr>
              </thead>
              <tbody>
                <template v-for="app in group" :key="app.student_id">
                  <tr class="border-t">
                    <td class="px-3 py-2">{{ app.name }}</td>
                    <td class="px-3 py-2 text-gray-500">
                      <span v-if="app.grade">{{ app.grade }}학년 {{ app.class_no }}반</span>
                      <span v-else class="text-gray-300">졸업</span>
                    </td>
                    <td class="px-3 py-2">
                      <span :class="app.is_enrolled ? 'text-green-600' : 'text-gray-400'">
                        {{ app.is_enrolled ? '재학' : '졸업' }}
                      </span>
                    </td>
                    <td class="px-3 py-2 text-center">
                      <span v-if="app.abandoned" class="text-xs text-red-500">포기</span>
                      <button
                        v-else-if="selected.status === 'CLOSED'"
                        class="text-xs px-2 py-0.5 border border-red-300 text-red-500 rounded hover:bg-red-50"
                        @click="handleAbandon(app)"
                      >포기</button>
                    </td>
                    <td class="px-3 py-2 text-center">
                      <button
                        class="text-xs px-2 py-0.5 border rounded text-gray-500 hover:bg-gray-50"
                        :disabled="previews[previewKey(app)]?.loading"
                        @click="togglePreview(app)"
                      >{{ previews[previewKey(app)]?.open ? '접기' : '미리보기' }}</button>
                    </td>
                  </tr>
                  <tr v-if="previews[previewKey(app)]?.open" class="border-t bg-indigo-50">
                    <td colspan="5" class="px-5 py-3">
                      <div v-if="previews[previewKey(app)]?.loading" class="text-xs text-gray-400">계산 중...</div>
                      <div v-else-if="previews[previewKey(app)]?.data">
                        <div class="flex flex-wrap gap-x-6 gap-y-1 text-xs">
                          <div v-for="item in previews[previewKey(app)].data.detail" :key="item.area_id"
                               class="flex items-center gap-1.5">
                            <span class="text-gray-500">{{ item.area_name }}</span>
                            <span class="font-semibold text-gray-800">{{ (item.score / 10000).toFixed(2) }}</span>
                          </div>
                        </div>
                        <div class="mt-1.5 text-xs font-bold text-indigo-700">
                          예상 총점: {{ (previews[previewKey(app)].data.total / 10000).toFixed(2) }}
                        </div>
                      </div>
                      <div v-else class="text-xs text-red-400">불러오기 실패</div>
                    </td>
                  </tr>
                </template>
              </tbody>
            </table>
          </div>
        </div>

        <!-- 결과 탭 -->
        <div v-if="view === 'results'">
          <div class="flex items-center gap-3 mb-4 flex-wrap">
            <select
              v-model="selectedTrackId"
              class="border rounded px-2 py-1 text-sm"
              @change="loadResults"
            >
              <option value="">전체 대학</option>
              <option v-for="t in tracksInRound" :key="t.id" :value="t.id">
                {{ t.univ_name }} {{ t.track_name }}
              </option>
            </select>
            <button
              class="px-3 py-1.5 text-sm border rounded text-gray-600 hover:bg-gray-50"
              @click="loadResults"
            >새로고침</button>
            <button
              class="px-3 py-1.5 text-sm bg-emerald-600 text-white rounded hover:bg-emerald-700 disabled:opacity-40"
              :disabled="results.length === 0"
              @click="downloadExcel"
            >목록 내보내기</button>
          </div>

          <div v-if="results.length === 0" class="text-sm text-gray-400 py-6 text-center">
            결과가 없습니다. 점수 계산을 먼저 실행하세요.
          </div>

          <div v-for="(group, key) in resultsByUniv" :key="key" class="mb-6">
            <div class="flex items-center gap-2 mb-2">
              <h4 class="text-sm font-semibold text-gray-700">{{ key }}</h4>
              <span class="text-xs text-gray-400">
                <template v-if="group.unitQuota != null">
                  모집단위 정원 {{ group.unitQuota }}명 / 잔여 {{ group.remaining }}석
                </template>
                <template v-else>
                  모집단위 정원 무제한
                </template>
              </span>
            </div>
            <table class="w-full text-sm border rounded overflow-hidden">
              <thead class="bg-gray-50">
                <tr>
                  <th class="px-3 py-2 text-xs text-gray-500 font-medium w-12">순위</th>
                  <th class="text-left px-3 py-2 text-xs text-gray-500 font-medium">학생명</th>
                  <th class="text-left px-3 py-2 text-xs text-gray-500 font-medium">학생코드</th>
                  <th class="text-left px-3 py-2 text-xs text-gray-500 font-medium">학년/반</th>
                  <th class="text-left px-3 py-2 text-xs text-gray-500 font-medium">재학</th>
                  <th
                    v-for="area in areas"
                    :key="area.id"
                    class="text-right px-3 py-2 text-xs text-gray-500 font-medium"
                  >{{ area.name }}</th>
                  <th class="text-right px-3 py-2 text-xs text-gray-500 font-medium">총점</th>
                  <th class="px-3 py-2 text-xs text-gray-500 font-medium w-20">추천</th>
                </tr>
              </thead>
              <tbody>
                <tr
                  v-for="r in group.results"
                  :key="r.student_id"
                  class="border-t"
                  :class="r.recommended ? 'bg-green-50' : r.abandoned ? 'bg-red-50 opacity-60' : ''"
                >
                  <td class="px-3 py-2 text-center text-gray-500">{{ r.ranking ?? '-' }}</td>
                  <td class="px-3 py-2 font-medium">{{ r.name }}</td>
                  <td class="px-3 py-2 text-gray-500">{{ r.student_code }}</td>
                  <td class="px-3 py-2 text-gray-500">
                    <span v-if="r.grade">{{ r.grade }}-{{ r.class_no }}</span>
                    <span v-else class="text-gray-300">졸업</span>
                  </td>
                  <td class="px-3 py-2">
                    <span :class="r.is_enrolled ? 'text-green-600' : 'text-gray-400'">
                      {{ r.is_enrolled ? '재' : '졸' }}
                    </span>
                  </td>
                  <td v-for="area in areas" :key="area.id" class="px-3 py-2 text-right text-gray-600">
                    {{ getAreaScore(r, area.id) }}
                  </td>
                  <td class="px-3 py-2 text-right font-semibold">
                    {{ (r.total_score / 10000).toFixed(2) }}
                  </td>
                  <td class="px-3 py-2 text-center">
                    <span v-if="r.abandoned" class="text-xs text-red-400">포기</span>
                    <span v-else-if="r.recommended" class="text-xs text-green-600 font-semibold">추천 확정</span>
                    <button
                      v-else-if="selected.status === 'CLOSED'"
                      class="text-xs px-2 py-0.5 bg-green-600 text-white rounded hover:bg-green-700"
                      @click="handleRecommend(r)"
                    >추천 확정</button>
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>
      </template>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted } from 'vue'
import {
  getRounds, openRound, closeRound,
  calculateScores, getResults, recommendResult,
  getApplications, abandonApplication,
  getAllTracks, getAreas,
  exportResultsExcel,
  scorePreview,
} from '../../api/admin.js'

function fmtDt(s) {
  if (!s) return ''
  return s.slice(0, 19).replace('T', ' ')
}

const rounds  = ref([])
const selected = ref(null)
const view    = ref('apps')
const loading = ref(false)

const apps    = ref([])
const results = ref([])
const areas   = ref([])
const tracks  = ref([])   // 전체 모집단위 (unit_quota 포함)

const calcLoading = ref(false)
const calcMsg     = ref(null)

const selectedTrackId = ref('')

const previews = ref({})

function previewKey(app) {
  return `${app.student_id}-${app.track_id}`
}

async function togglePreview(app) {
  const key = previewKey(app)
  const existing = previews.value[key]

  if (existing?.open) {
    previews.value = { ...previews.value, [key]: { ...existing, open: false } }
    return
  }

  previews.value = { ...previews.value, [key]: { open: true, loading: true, data: null } }
  try {
    const data = await scorePreview(app.student_id, app.track_id)
    previews.value = { ...previews.value, [key]: { open: true, loading: false, data } }
  } catch {
    previews.value = { ...previews.value, [key]: { open: true, loading: false, data: null } }
  }
}

const subTabs = [
  { key: 'apps',    label: '지원 현황' },
  { key: 'results', label: '결과' },
]

const hasOpenRound = computed(() => rounds.value.some(r => r.status === 'OPEN'))

const appsByUniv = computed(() => {
  const map = {}
  for (const app of apps.value) {
    const key = `${app.univ_name} ${app.track_name}`
    if (!map[key]) map[key] = []
    map[key].push(app)
  }
  return map
})

const tracksInRound = computed(() => {
  const seen = new Set()
  return results.value
    .filter(r => { if (seen.has(r.track_id)) return false; seen.add(r.track_id); return true })
    .map(r => ({ id: r.track_id, univ_name: r.univ_name, track_name: r.track_name }))
})

const resultsByUniv = computed(() => {
  const map = {}
  for (const r of results.value) {
    const key = `${r.univ_name} ${r.track_name}`
    if (!map[key]) {
      const t = tracks.value.find(t => t.id === r.track_id)
      const unitQuota = t?.unit_quota ?? null
      const recommended = results.value.filter(x => x.track_id === r.track_id && x.recommended).length
      map[key] = { unitQuota, remaining: unitQuota != null ? unitQuota - recommended : null, results: [] }
    }
    map[key].results.push(r)
  }
  return map
})

function getAreaScore(r, areaId) {
  try {
    const detail = typeof r.score_detail === 'string'
      ? JSON.parse(r.score_detail)
      : r.score_detail
    const v = detail[String(areaId)]
    return v !== undefined ? (v / 10000).toFixed(2) : '-'
  } catch {
    return '-'
  }
}

async function loadRounds() {
  rounds.value = await getRounds()
}

async function selectRound(r) {
  selected.value = r
  calcMsg.value = null
  previews.value = {}
  await Promise.all([loadApps(), loadResults(), loadAreas()])
}

async function loadApps() {
  if (!selected.value) return
  apps.value = await getApplications(selected.value.id)
}

async function loadResults() {
  if (!selected.value) return
  results.value = await getResults(selected.value.id, selectedTrackId.value || null)
}

async function loadAreas() {
  ;[areas.value, tracks.value] = await Promise.all([getAreas(), getAllTracks()])
}

async function handleOpenRound() {
  loading.value = true
  try {
    await openRound()
    await loadRounds()
    const open = rounds.value.find(r => r.status === 'OPEN')
    if (open) await selectRound(open)
  } catch (e) {
    alert(e.response?.data || e.message)
  } finally {
    loading.value = false
  }
}

async function handleCloseRound(id) {
  if (!confirm('라운드를 마감하시겠습니까?')) return
  try {
    await closeRound(id)
    await loadRounds()
    if (selected.value?.id === id) {
      const updated = rounds.value.find(r => r.id === id)
      if (updated) selected.value = updated
    }
  } catch (e) {
    alert(e.response?.data || e.message)
  }
}

async function handleCalculate() {
  if (!selected.value) return
  calcLoading.value = true
  calcMsg.value = null
  try {
    const res = await calculateScores(selected.value.id)
    calcMsg.value = { ok: true, text: `점수 계산 완료: ${res.calculated}건` }
    await loadResults()
  } catch (e) {
    calcMsg.value = { ok: false, text: e.response?.data || e.message }
  } finally {
    calcLoading.value = false
  }
}

async function handleAbandon(app) {
  if (!confirm(`${app.name} 학생의 지원을 포기 처리하시겠습니까?`)) return
  try {
    await abandonApplication(app.student_id, app.track_id, app.round_id)
    await loadApps()
  } catch (e) {
    alert(e.response?.data || e.message)
  }
}

async function downloadExcel() {
  if (!selected.value) return
  try {
    const res = await exportResultsExcel(selected.value.id)
    const url = URL.createObjectURL(res.data)
    const a = document.createElement('a')
    a.href = url
    a.download = `results_round_${selected.value.id}.xlsx`
    a.click()
    URL.revokeObjectURL(url)
  } catch (e) {
    alert(e.response?.data || e.message)
  }
}

async function handleRecommend(r) {
  if (!confirm(`${r.name} 학생을 추천 확정하시겠습니까?`)) return
  try {
    await recommendResult(r.student_id, r.track_id, r.round_id)
    await loadResults()
  } catch (e) {
    alert(e.response?.data || e.message)
  }
}

onMounted(loadRounds)
</script>
