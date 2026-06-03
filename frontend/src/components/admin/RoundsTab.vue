<template>
  <div class="py-8 px-4 sm:px-10">

    <!-- 페이지 헤더 -->
    <div class="mb-5">
      <p class="text-base mb-1" style="color: #94a3b8;">관리자</p>
      <h1 class="text-2xl font-semibold" style="color: #1e293b; margin: 0;">라운드 관리</h1>
    </div>

    <div class="flex flex-col lg:flex-row lg:items-start gap-6">

      <!-- ── 좌측: 라운드 목록 ────────────────────────────────── -->
      <div class="flex-shrink-0 flex flex-col w-full lg:w-[300px]">
        <div class="flex items-center justify-between mb-4">
          <h2 class="text-lg font-semibold" style="color: #1e293b;">라운드 목록</h2>
          <button
            class="text-base font-medium rounded-lg disabled:opacity-40"
            style="padding: 7px 14px; border: none; background: #2563eb; color: white; cursor: pointer;"
            :disabled="hasOpenRound || loading"
            @click="handleOpenRound"
          >+ 라운드 열기</button>
        </div>

        <div class="flex flex-col gap-2">
          <div
            v-for="r in rounds"
            :key="r.id"
            class="rounded-xl transition-all"
            :style="{
              background: 'white',
              border: selected?.id === r.id ? '1px solid #93c5fd' : '1px solid #e2e8f0',
              boxShadow: '0 1px 4px rgba(0,0,0,0.07)',
            }"
          >
            <!-- 클릭 영역 -->
            <div class="cursor-pointer" style="padding: 14px 16px;" @click="selectRound(r)">
              <p class="text-lg font-semibold" style="color: #1e293b; margin: 0;">{{ r.id }}차 라운드</p>

              <div class="mt-1.5 flex items-center justify-start gap-2">
                <span
                  class="text-base font-medium"
                  style="padding: 2px 10px; border-radius: 999px; white-space: nowrap;"
                  :style="{
                    background: r.status === 'OPEN' ? '#dcfce7' : r.status === 'CLOSED' ? '#dbeafe' : '#f3e8ff',
                    color:      r.status === 'OPEN' ? '#15803d' : r.status === 'CLOSED' ? '#1d4ed8' : '#7c3aed',
                  }"
                >
                  {{ { OPEN: '진행중', CLOSED: '종료', FINALIZED: '마감' }[r.status] || r.status }}
                </span>

                <span class="text-base" style="color: #94a3b8;">
                  <template v-if="r.status === 'OPEN'">{{ fmtDt(r.opened_at) }}</template>
                  <template v-else-if="r.status === 'CLOSED'">{{ fmtDt(r.closed_at) }}</template>
                  <template v-else-if="r.status === 'FINALIZED'">{{ fmtDt(r.finalized_at) }}</template>
                </span>

              </div>
            </div>
          </div>

          <div v-if="rounds.length === 0" class="text-base text-center" style="padding: 32px 0; color: #94a3b8;">
            라운드 없음
          </div>
        </div>
      </div>

      <!-- ── 우측: 라운드 상세 ──────────────────────────────────── -->
      <div class="flex-1 min-w-0">
        <div v-if="!selected" class="flex items-center justify-center" style="height: 240px;">
          <p class="text-base" style="color: #94a3b8;">라운드를 선택하거나 새 라운드를 열어주세요</p>
        </div>

        <template v-else>
          <div class="rounded-xl mb-5"
            style="padding: 18px 22px; background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04);">
            <div class="flex items-center gap-3 flex-wrap">
              <span class="text-xl font-bold" style="color: #1e293b;">{{ selected.id }}차 라운드</span>
              <span
                  class="text-base font-semibold"
                  style="padding: 4px 14px; border: 1px solid; border-radius: 999px;"
                  :style="{
                    background:  selected.status === 'OPEN' ? '#dcfce7' : selected.status === 'CLOSED' ? '#dbeafe' : '#f3e8ff',
                    color:       selected.status === 'OPEN' ? '#15803d' : selected.status === 'CLOSED' ? '#1d4ed8' : '#7c3aed',
                    borderColor: selected.status === 'OPEN' ? '#bbf7d0' : selected.status === 'CLOSED' ? '#bfdbfe' : '#e9d5ff'
                  }"
              >
                {{ { OPEN: '진행중', CLOSED: '종료', FINALIZED: '마감 완료' }[selected.status] || selected.status }}</span>

              <!-- 상태 액션 버튼 -->
              <template v-if="selected.status === 'OPEN'">
                <button
                  class="text-base font-medium rounded-lg disabled:opacity-40"
                  style="padding: 4px 14px; border: 1px solid #fca5a5; background: white; color: #ef4444; cursor: pointer;"
                  :disabled="roundActing"
                  @click="handleCloseRound(selected.id)"
                >종료하기</button>
              </template>
              <template v-else-if="selected.status === 'CLOSED'">
                <button
                  class="text-base font-medium rounded-lg disabled:opacity-40"
                  style="padding: 4px 14px; border: 1px solid #e2e8f0; background: white; color: #64748b; cursor: pointer;"
                  :disabled="roundActing"
                  @click="handleReopenRound(selected.id)"
                >다시 열기</button>
                <button
                  class="text-base font-medium rounded-lg disabled:opacity-40"
                  style="padding: 4px 14px; border: 1px solid #d8b4fe; background: white; color: #7c3aed; cursor: pointer;"
                  :disabled="roundActing"
                  @click="handleFinalizeRound(selected.id)"
                >마감하기</button>
              </template>

              <!-- 날짜 -->
              <div class="w-full flex gap-1 items-center flex-wrap">
              <span class="text-base" style="color: #94a3b8;">{{ fmtDt(selected.opened_at) }} 개시</span>
              <span v-if="selected.closed_at" class="text-base" style="color: #94a3b8;">→ {{ fmtDt(selected.closed_at) }} 입력 종료</span>
              <span v-if="selected.finalized_at" class="text-base" style="color: #94a3b8;">→ {{ fmtDt(selected.finalized_at) }} 최종 마감</span>
              </div>
            </div>
          </div>

          <!-- 서브탭 -->
          <div class="flex mb-5" style="border-bottom: 1px solid #e2e8f0;">
            <button
              v-for="t in subTabs"
              :key="t.key"
              class="text-base font-medium transition-colors"
              style="padding: 10px 20px; border: none; background: none; cursor: pointer; border-bottom: 2px solid transparent; margin-bottom: -1px;"
              :style="{
                borderBottomColor: view === t.key ? '#2563eb' : 'transparent',
                color: view === t.key ? '#2563eb' : '#64748b',
                fontWeight: view === t.key ? '600' : '400',
              }"
              @click="view = t.key"
            >{{ t.label }}</button>
          </div>

          <!-- ── 지원 현황 탭 ──────────────────────────────── -->
          <div v-if="view === 'apps'">
            <div class="flex items-center justify-between mb-4 flex-wrap gap-2">
              <span class="text-base" style="color: #64748b;">총 {{ apps.length }}건</span>
              <div v-if="selected.status === 'CLOSED'" class="flex items-center gap-3">
                <span v-if="calcMsg" class="text-base font-medium"
                  :style="{ color: calcMsg.ok ? '#16a34a' : '#ef4444' }">{{ calcMsg.text }}</span>
                <button
                  class="text-base font-semibold rounded-lg disabled:opacity-40"
                  style="padding: 9px 18px; border: none; background: #4f46e5; color: white; cursor: pointer;"
                  :disabled="calcLoading || apps.length === 0"
                  @click="handleCalculate"
                >{{ calcLoading ? '계산 중…' : '점수 전체 재계산' }}</button>
              </div>
            </div>

            <div v-if="apps.length === 0" class="text-base text-center" style="padding: 48px 0; color: #94a3b8;">
              지원자가 없습니다
            </div>

            <div v-for="(group, key) in appsByUniv" :key="key" class="mb-6">
              <h4 class="text-base font-semibold mb-3" style="color: #1e293b;">{{ key }}</h4>
              <div class="rounded-xl overflow-hidden"
                style="background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04);">
                <div class="overflow-x-auto">
                  <table class="w-full min-w-max" style="border-collapse: collapse;">
                    <thead>
                      <tr style="background: #f8fafc; border-bottom: 1px solid #e2e8f0;">
                        <th class="text-base font-semibold text-left" style="padding: 13px 18px; color: #475569; width: 160px;">학번/학생코드</th>
                        <th class="text-base font-semibold text-left" style="padding: 13px 18px; color: #475569; width: 100px;">학생 이름</th>
                        <th class="text-base font-semibold text-left" style="padding: 13px 18px; color: #475569; width: 90px;">구분</th>
                        <th class="text-base font-semibold text-left" style="padding: 13px 18px; color: #475569; width: 110px;">모집단위</th>
                        <th class="text-base font-semibold text-left" style="padding: 13px 18px; color: #475569; width: 140px;">지원 학과</th>
                        <th class="text-base font-semibold text-center" style="padding: 13px 18px; color: #475569; width: 110px;">추천</th>
                        <th class="text-base font-semibold text-center" style="padding: 13px 18px; color: #475569; width: 110px;">포기처리</th>
                        <th class="text-base font-semibold text-right" style="padding: 13px 18px; color: #475569; width: 90px;">총점</th>
                      </tr>
                    </thead>
                    <tbody>
                      <tr v-for="app in group" :key="app.student_id"
                        class="hover:bg-slate-50"
                        style="border-bottom: 1px solid #f1f5f9; transition: background 0.1s;">
                        <td class="text-base" style="padding: 12px 18px; color: #475569;">
                          <span v-if="app.is_enrolled">{{ app.grade }}학년 {{ app.class_no }}반 {{ app.seq_no }}번</span>
                          <span v-else class="font-mono">{{ app.student_code }}</span>
                        </td>
                        <td class="text-base font-medium" style="padding: 12px 18px; color: #1e293b;">{{ app.name }}</td>
                        <td style="padding: 12px 18px;">
                          <span class="text-base font-medium"
                            :style="{ color: app.is_enrolled ? '#16a34a' : '#94a3b8' }">
                            {{ app.is_enrolled ? '재학생' : '졸업생' }}
                          </span>
                        </td>
                        <td class="text-base" style="padding: 12px 18px; color: #1e293b;">{{ app.track_name }}</td>
                        <td class="text-base" style="padding: 12px 18px; color: #475569;">{{ app.department_name }}</td>
                        <td class="text-base text-center" style="padding: 12px 18px;">
                          <span v-if="app.abandoned" style="color: #cbd5e1;">-</span>
                          <span v-else-if="selected.status === 'FINALIZED' && app.recommended"
                            class="text-base font-semibold" style="color: #16a34a;">추천 확정</span>
                          <span v-else-if="selected.status === 'FINALIZED' && !app.recommended"
                            class="text-base font-semibold" style="color: #ef4444;">추천 제외</span>
                          <span v-else style="color: #cbd5e1;">-</span>
                        </td>
                        <td class="text-center" style="padding: 12px 18px;">
                          <span v-if="app.abandoned" class="text-base font-semibold" style="color: #ef4444;">포기됨</span>
                          <button
                            v-else-if="selected.status === 'FINALIZED' && app.recommended"
                            class="text-base rounded-lg whitespace-nowrap"
                            style="padding: 5px 12px; border: 1px solid #fca5a5; background: white; color: #ef4444; cursor: pointer;"
                            @click="handleAbandon(app)"
                          >포기하기</button>
                          <span v-else style="color: #cbd5e1;">-</span>
                        </td>
                        <td class="text-base text-right font-semibold" style="padding: 12px 18px; color: #1e293b;">
                          {{ appTotalScore(app) }}
                        </td>
                      </tr>
                    </tbody>
                  </table>
                </div>
              </div>
            </div>
          </div>

          <!-- ── 결과 탭 ──────────────────────────────────── -->
          <div v-if="view === 'results'">
            <div class="flex items-center gap-3 mb-5 flex-wrap">
              <select
                v-model="selectedTrackId"
                class="text-base rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-400"
                style="border: 1px solid #e2e8f0; padding: 9px 12px; color: #1e293b;"
                @change="loadResults"
              >
                <option value="">전체 대학</option>
                <option v-for="t in tracksInRound" :key="t.id" :value="t.id">
                  {{ t.univ_name }} {{ t.track_name }}
                </option>
              </select>
              <button
                class="text-base font-medium rounded-lg"
                style="padding: 9px 16px; border: 1px solid #e2e8f0; background: white; color: #475569; cursor: pointer;"
                @click="loadResults"
              >새로고침</button>
              <span style="color: #cbd5e1; user-select: none;">|</span>
              <button
                class="text-base font-medium rounded-lg disabled:opacity-40"
                style="padding: 9px 16px; border: none; background: #059669; color: white; cursor: pointer;"
                :disabled="results.length === 0 || downloading"
                @click="downloadExcel"
              >전체 지원자 목록 다운로드</button>
              <button
                class="text-base font-medium rounded-lg disabled:opacity-40"
                style="padding: 9px 16px; border: none; background: #2563eb; color: white; cursor: pointer;"
                :disabled="selected.status !== 'FINALIZED' || downloadingSummary"
                @click="downloadSummary"
              >라운드 결과 다운로드</button>
            </div>

            <div v-if="results.length === 0" class="text-base text-center" style="padding: 48px 0; color: #94a3b8;">
              결과가 없습니다. 점수 계산을 먼저 실행하세요.
            </div>

            <div v-for="(group, key) in resultsByUniv" :key="key" class="mb-6">
              <div class="flex items-center gap-3 mb-3 flex-wrap">
                <h4 class="text-base font-semibold" style="color: #1e293b; margin: 0;">{{ key }}</h4>
                <span class="text-base" style="color: #94a3b8;">
                  <template v-if="group.totalQuota != null">
                    대학 정원 {{ group.totalQuota }}명 / 잔여 {{ group.univRemaining }}석
                  </template>
                  <template v-else>대학 정원 무제한</template>
                  <span style="margin: 0 6px; color: #e2e8f0;">|</span>
                  <template v-if="group.unitQuota != null">
                    모집단위 정원 {{ group.unitQuota }}명 / 잔여 {{ group.remaining }}석
                  </template>
                  <template v-else>모집단위 정원 무제한</template>
                </span>
              </div>
              <div class="rounded-xl overflow-hidden"
                style="background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04);">
                <div class="overflow-x-auto">
                  <table class="w-full min-w-max" style="border-collapse: collapse;">
                    <thead>
                      <tr style="background: #f8fafc; border-bottom: 1px solid #e2e8f0;">
                        <th style="width: 36px; padding: 13px 8px;"></th>
                        <th class="text-base font-semibold text-center" style="padding: 13px 16px; color: #475569; width: 70px;">순위</th>
                        <th class="text-base font-semibold text-left" style="padding: 13px 18px; color: #475569; width: 160px;">학번/학생코드</th>
                        <th class="text-base font-semibold text-left" style="padding: 13px 18px; color: #475569; width: 100px;">학생 이름</th>
                        <th class="text-base font-semibold text-left" style="padding: 13px 18px; color: #475569; width: 90px;">구분</th>
                        <th class="text-base font-semibold text-left" style="padding: 13px 18px; color: #475569; width: 140px;">지원 학과</th>
                        <th class="text-base font-semibold text-right" style="padding: 13px 18px; color: #475569; width: 90px;">총점</th>
                        <th class="text-base font-semibold text-center" style="padding: 13px 18px; color: #475569; width: 120px;">추천</th>
                        <th class="text-base font-semibold text-center" style="padding: 13px 18px; color: #475569; width: 110px;">포기처리</th>
                      </tr>
                    </thead>
                    <tbody>
                      <template v-for="r in group.results" :key="r.student_id">
                        <tr
                          class="cursor-pointer transition-colors"
                          :style="{
                            borderBottom: '1px solid #f1f5f9',
                            background:
                              selected.status === 'FINALIZED' && (r.abandoned || !r.recommended) ? '#fef2f2' :
                              selected.status === 'FINALIZED' && r.recommended && !r.abandoned ? '#f0fdf4' :
                              undefined,
                          }"
                          @click="toggleRow(`${r.student_id}-${r.track_id}`)"
                        >
                          <td class="text-center" style="padding: 12px 8px; color: #94a3b8; font-size: 12px; user-select: none;">
                            {{ expandedRows[`${r.student_id}-${r.track_id}`] ? '▼' : '▶' }}
                          </td>
                          <td class="text-base text-center" style="padding: 12px 16px; color: #475569;">{{ r.ranking ?? '-' }}</td>
                          <td class="text-base" style="padding: 12px 18px; color: #475569; max-width: 160px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">
                            <span v-if="r.is_enrolled">{{ r.grade }}학년 {{ r.class_no }}반 {{ r.seq_no }}번</span>
                            <span v-else class="font-mono">{{ r.student_code }}</span>
                          </td>
                          <td class="text-base font-medium" style="padding: 12px 18px; color: #1e293b; max-width: 100px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">{{ r.name }}</td>
                          <td style="padding: 12px 18px;">
                            <span class="text-base font-medium"
                              :style="{ color: r.is_enrolled ? '#16a34a' : '#94a3b8' }">
                              {{ r.is_enrolled ? '재학생' : '졸업생' }}
                            </span>
                          </td>
                          <td class="text-base" style="padding: 12px 18px; color: #475569; max-width: 140px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">{{ r.department_name }}</td>
                          <td class="text-base text-right font-semibold" style="padding: 12px 18px; color: #1e293b;">
                            {{ r.total_score.toFixed(2) }}
                          </td>
                          <td class="text-center" style="padding: 12px 18px;" @click.stop>
                            <span v-if="r.abandoned" class="text-base font-semibold" style="color: #ef4444;">포기됨</span>
                            <template v-else-if="r.recommended">
                              <span class="text-base font-semibold" style="color: #16a34a;">추천 확정됨</span>
                              <button
                                v-if="selected.status === 'CLOSED'"
                                class="text-base rounded-lg ml-2"
                                style="padding: 3px 10px; border: 1px solid #fca5a5; background: white; color: #ef4444; cursor: pointer;"
                                @click="handleUnrecommend(r)"
                              >추천 취소</button>
                            </template>
                            <button
                              v-else-if="selected.status === 'CLOSED'"
                              class="text-base font-semibold rounded-lg"
                              style="padding: 5px 12px; border: none; background: #16a34a; color: white; cursor: pointer;"
                              @click="handleRecommend(r)"
                            >추천 확정</button>
                            <span v-else-if="selected.status === 'FINALIZED'" class="text-base font-semibold" style="color: #ef4444;">추천 제외</span>
                            <span v-else class="text-base font-semibold" style="color: #94a3b8;">-</span>
                          </td>
                          <td class="text-center" style="padding: 12px 18px;" @click.stop>
                            <button
                              v-if="r.recommended && !r.abandoned && selected.status === 'FINALIZED'"
                              class="text-base rounded-lg whitespace-nowrap"
                              style="padding: 5px 12px; border: 1px solid #fca5a5; background: white; color: #ef4444; cursor: pointer;"
                              @click="handleAbandon(r)"
                            >포기하기</button>
                            <span v-else style="color: #cbd5e1;">-</span>
                          </td>
                        </tr>
                        <!-- 전형요소 점수 상세 -->
                        <tr v-if="expandedRows[`${r.student_id}-${r.track_id}`]"
                          style="border-bottom: 1px solid #f1f5f9; background: #f8fafc;">
                          <td colspan="9" style="padding: 14px 36px;">
                            <div class="flex flex-wrap gap-x-6 gap-y-2">
                              <div v-for="area in areas" :key="area.id" class="flex items-center gap-2">
                                <span class="text-base" style="color: #64748b;">{{ area.name }}</span>
                                <span class="text-base font-semibold" style="color: #1e293b;">{{ getAreaScore(r, area.id) }}</span>
                              </div>
                            </div>
                          </td>
                        </tr>
                      </template>
                    </tbody>
                  </table>
                </div>
              </div>
            </div>
          </div>

        </template>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, inject } from 'vue'
import {
  getRounds, openRound, closeRound, reopenRound, finalizeRound,
  calculateScores, getResults, recommendResult, unrecommendResult,
  getApplications, abandonApplication,
  getAreas,
  exportResultsExcel,
  exportRoundSummary,
  getQuotaStats,
  blobErrMsg,
} from '../../api/admin.js'

function fmtDt(s) {
  if (!s) return ''
  const d = new Date(s)
  const pad = n => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${pad(d.getMonth()+1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`
}

const refreshSidebarRound = inject('refreshRound', () => {})

const rounds  = ref([])
const selected = ref(null)
const view    = ref('apps')
const loading = ref(false)

const apps    = ref([])
const results = ref([])
const areas   = ref([])

const roundActing        = ref(false)
const calcLoading        = ref(false)
const calcMsg            = ref(null)
const downloading        = ref(false)
const downloadingSummary = ref(false)
const expandedRows       = ref({})
const quotaStats         = ref(null)

const selectedTrackId = ref('')

const subTabs = [
  { key: 'apps',    label: '지원 현황' },
  { key: 'results', label: '결과' },
]

const hasOpenRound = computed(() => rounds.value.some(r => r.status === 'OPEN' || r.status === 'CLOSED'))

const appsByUniv = computed(() => {
  const map = {}
  for (const app of apps.value) {
    const key = app.univ_name
    if (!map[key]) map[key] = []
    map[key].push(app)
  }
  for (const key of Object.keys(map)) {
    map[key].sort((a, b) => {
      const t = a.track_name.localeCompare(b.track_name, 'ko')
      if (t !== 0) return t
      return (a.student_code ?? '').localeCompare(b.student_code ?? '')
    })
  }
  return map
})

function appTotalScore(app) {
  const r = results.value.find(r => r.student_id === app.student_id && r.track_id === app.track_id)
  return r ? r.total_score.toFixed(2) : '-'
}

const tracksInRound = computed(() => {
  const seen = new Set()
  return results.value
    .filter(r => { if (seen.has(r.track_id)) return false; seen.add(r.track_id); return true })
    .map(r => ({ id: r.track_id, univ_name: r.univ_name, track_name: r.track_name }))
})

const trackQuotaMap = computed(() => {
  const map = {}
  if (!quotaStats.value) return map
  for (const u of quotaStats.value.univs) {
    for (const t of u.tracks) {
      map[t.track_id] = {
        unitQuota: t.unit_quota,
        unitUsed: t.unit_used,
        totalQuota: u.total_quota,
        totalUsed: u.total_used,
      }
    }
  }
  return map
})

const resultsByUniv = computed(() => {
  const map = {}
  for (const r of results.value) {
    const key = `${r.univ_name} ${r.track_name}`
    if (!map[key]) {
      const q = trackQuotaMap.value[r.track_id]
      const unitQuota = q?.unitQuota ?? null
      const totalQuota = q?.totalQuota ?? null
      map[key] = {
        unitQuota,
        totalQuota,
        remaining: unitQuota != null ? Math.max(0, unitQuota - (q?.unitUsed ?? 0)) : null,
        univRemaining: totalQuota != null ? Math.max(0, totalQuota - (q?.totalUsed ?? 0)) : null,
        results: [],
      }
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
    return v !== undefined ? Number(v).toFixed(2) : '-'
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
  await Promise.all([loadApps(), loadResults(), loadAreas()])
}

async function loadApps() {
  if (!selected.value) return
  apps.value = await getApplications(selected.value.id)
}

async function loadResults() {
  if (!selected.value) return
  ;[results.value, quotaStats.value] = await Promise.all([
    getResults(selected.value.id, selectedTrackId.value || null),
    getQuotaStats(),
  ])
  expandedRows.value = {}
}

function toggleRow(key) {
  const next = { ...expandedRows.value }
  if (next[key]) delete next[key]
  else next[key] = true
  expandedRows.value = next
}

async function loadAreas() {
  areas.value = await getAreas()
}

async function handleOpenRound() {
  loading.value = true
  try {
    await openRound()
    await loadRounds()
    const open = rounds.value.find(r => r.status === 'OPEN')
    if (open) await selectRound(open)
    await refreshSidebarRound()
  } catch (e) {
    alert(e.response?.data || e.message)
  } finally {
    loading.value = false
  }
}

async function handleCloseRound(id) {
  if (roundActing.value) return
  if (!confirm('라운드를 종료하시겠습니까? (담임 입력이 차단됩니다)')) return
  roundActing.value = true
  try {
    await closeRound(id)
    await loadRounds()
    if (selected.value?.id === id) {
      const updated = rounds.value.find(r => r.id === id)
      if (updated) selected.value = updated
      await loadResults()
    }
    await refreshSidebarRound()
  } catch (e) {
    alert(e.response?.data || e.message)
  } finally {
    roundActing.value = false
  }
}

async function handleReopenRound(id) {
  if (roundActing.value) return
  if (!confirm('라운드를 다시 열시겠습니까? (추천 플래그가 초기화됩니다)')) return
  roundActing.value = true
  try {
    await reopenRound(id)
    await loadRounds()
    if (selected.value?.id === id) {
      const updated = rounds.value.find(r => r.id === id)
      if (updated) selected.value = updated
    }
    await refreshSidebarRound()
  } catch (e) {
    alert(e.response?.data || e.message)
  } finally {
    roundActing.value = false
  }
}

async function handleFinalizeRound(id) {
  if (roundActing.value) return
  if (!confirm('라운드를 마감하시겠습니까? (추천 확정이 박제되고 결과가 공개됩니다)')) return
  roundActing.value = true
  try {
    await finalizeRound(id)
    await loadRounds()
    if (selected.value?.id === id) {
      const updated = rounds.value.find(r => r.id === id)
      if (updated) selected.value = updated
    }
    await refreshSidebarRound()
  } catch (e) {
    alert(e.response?.data || e.message)
  } finally {
    roundActing.value = false
  }
}

async function handleCalculate() {
  if (!selected.value) return
  const roundId = selected.value.id
  calcLoading.value = true
  calcMsg.value = null
  try {
    const res = await calculateScores(roundId)
    if (selected.value?.id !== roundId) return
    calcMsg.value = { ok: true, text: `점수 재계산 완료: ${res.calculated}건` }
    await loadResults()
  } catch (e) {
    calcMsg.value = { ok: false, text: e.response?.data || e.message }
  } finally {
    calcLoading.value = false
  }
}

async function handleAbandon(app) {
  if (!confirm(`${app.name} 학생의 지원을 포기 처리하시겠습니까? 한 번 포기하면 다시 되돌릴 수 없으며, 재추천 희망할 경우 다음 라운드에서 재지원해야 합니다.`)) return
  try {
    await abandonApplication(app.student_id, app.track_id, app.round_id)
    await Promise.all([loadApps(), loadResults()])
  } catch (e) {
    alert(e.response?.data || e.message)
  }
}

async function downloadExcel() {
  if (!selected.value) return
  downloading.value = true
  try {
    const res = await exportResultsExcel(selected.value.id)
    const url = URL.createObjectURL(res.data)
    const a = document.createElement('a')
    a.href = url
    a.download = `results_round_${selected.value.id}.xlsx`
    a.click()
    URL.revokeObjectURL(url)
  } catch (e) {
    alert(await blobErrMsg(e))
  } finally {
    downloading.value = false
  }
}

async function downloadSummary() {
  if (!selected.value) return
  downloadingSummary.value = true
  try {
    const res = await exportRoundSummary(selected.value.id)
    const url = URL.createObjectURL(res.data)
    const a = document.createElement('a')
    a.href = url
    a.download = `round_${selected.value.id}_summary.xlsx`
    a.click()
    URL.revokeObjectURL(url)
  } catch (e) {
    alert(await blobErrMsg(e))
  } finally {
    downloadingSummary.value = false
  }
}

async function handleRecommend(r) {
  try {
    await recommendResult(r.student_id, r.track_id, r.round_id)
    await loadResults()
  } catch (e) {
    alert(e.response?.data || e.message)
  }
}

async function handleUnrecommend(r) {
  if (!confirm(`${r.name} 학생의 추천을 취소하시겠습니까?`)) return
  try {
    await unrecommendResult(r.student_id, r.track_id, r.round_id)
    await loadResults()
  } catch (e) {
    alert(e.response?.data || e.message)
  }
}

onMounted(loadRounds)
</script>
