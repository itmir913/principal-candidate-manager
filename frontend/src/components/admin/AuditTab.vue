<template>
  <div class="py-8 px-4 sm:px-10">

    <!-- 페이지 헤더 -->
    <div class="flex items-end justify-between flex-wrap gap-3 mb-5">
      <div>
        <p class="text-base mb-1" style="color: #94a3b8;">관리자</p>
        <h1 class="text-2xl font-semibold" style="color: #1e293b; margin: 0;">감사 기록</h1>
      </div>
      <div class="flex flex-wrap items-center gap-2">
        <button
          class="flex items-center gap-1.5 text-base font-medium rounded-lg disabled:opacity-40"
          style="padding: 9px 16px; border: 1px solid #e2e8f0; background: white; color: #475569; cursor: pointer;"
          :disabled="downloading"
          @click="dlAll"
        >전체 목록 다운로드</button>
      </div>
    </div>

    <HelpBox class="mb-5" storage-key="audit" :title="HELP.title" :intro="HELP.intro" :items="HELP.items" />

    <!-- 필터 -->
    <div class="flex flex-wrap items-center gap-2 mb-4">
      <select
        v-model.number="filterRound"
        class="text-base rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-400 border border-slate-200 py-2 pl-3 pr-8 text-slate-800 bg-white"
      >
        <option :value="null">전체 라운드</option>
        <option v-for="r in rounds" :key="r.id" :value="r.id">{{ r.id }}차 라운드</option>
      </select>

      <select
        v-model="filterAction"
        class="text-base rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-400 border border-slate-200 py-2 pl-3 pr-8 text-slate-800 bg-white"
      >
        <option value="">전체 작업 유형</option>
        <option v-for="(label, key) in AUDIT_ACTION_LABELS" :key="key" :value="key">{{ label }}</option>
      </select>

      <select
        v-model="filterClass"
        class="text-base rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-400 border border-slate-200 py-2 pl-3 pr-8 text-slate-800 bg-white"
      >
        <option value="">전체 학급</option>
        <option v-for="c in classes" :key="`${c.grade}-${c.class_no}`" :value="`${c.grade}-${c.class_no}`">
          {{ classLabel(c) }}
        </option>
      </select>

      <button
        class="text-base font-medium rounded-lg px-[18px] py-2 bg-[#2563eb] text-white hover:bg-blue-700 transition-colors"
        @click="load()"
      >조회</button>

      <span class="ml-auto text-base font-medium" style="color: #64748b;">총 {{ auditPage.total }}건</span>
    </div>

    <!-- 에러 -->
    <div v-if="error" class="mb-4 rounded-lg text-base"
      style="padding: 12px 16px; background: #fef2f2; border: 1px solid #fecaca; color: #dc2626;">
      {{ error }}
    </div>

    <!-- 테이블 -->
    <div class="rounded-xl overflow-hidden mb-4"
      style="background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04);">
      <div class="overflow-x-auto">
        <table class="min-w-max w-full" style="border-collapse: collapse;">
          <thead>
            <tr style="background: #f8fafc; border-bottom: 1px solid #e2e8f0;">
              <th class="text-base font-semibold text-left" style="padding: 14px 20px; color: #475569; width: 160px;">시각</th>
              <th class="text-base font-semibold text-left" style="padding: 14px 20px; color: #475569; width: 160px;">행위자</th>
              <th class="text-base font-semibold text-left" style="padding: 14px 20px; color: #475569; width: 160px;">행위</th>
              <th class="text-base font-semibold text-left" style="padding: 14px 20px; color: #475569; width: 200px;">대상</th>
              <th class="text-base font-semibold text-left" style="padding: 14px 20px; color: #475569; width: 200px;">상세</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="row in auditPage.rows" :key="row.id"
              class="hover:bg-slate-50"
              style="border-bottom: 1px solid #f1f5f9; transition: background 0.1s;">
              <td class="text-base font-mono" style="padding: 13px 20px; color: #475569; white-space: nowrap;">{{ fmtDt(row.at) }}</td>
              <td class="text-base" style="padding: 13px 20px; color: #1e293b; white-space: nowrap;">{{ fmtActor(row) }}</td>
              <td style="padding: 13px 20px; white-space: nowrap;">
                <span class="text-base font-medium" style="color: #1e293b;">
                  {{ AUDIT_ACTION_LABELS[row.action] || row.action }}
                </span>
              </td>
              <td class="text-base" style="padding: 13px 20px; color: #475569;">{{ fmtTarget(row.detail) }}</td>
              <td class="text-base" style="padding: 13px 20px; color: #64748b;">{{ fmtDetail(row.detail) }}</td>
            </tr>
            <tr v-if="auditPage.rows.length === 0">
              <td colspan="5" class="text-base text-center" style="padding: 48px 20px; color: #94a3b8;">
                감사 기록이 없습니다.
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <!-- 페이지 내비게이션 -->
    <div v-if="auditPage.total > 0" class="flex items-center justify-center gap-4">
      <button
        class="text-base rounded-lg disabled:opacity-40 disabled:cursor-not-allowed"
        style="padding: 8px 18px; border: 1px solid #e2e8f0; background: white; color: #475569; cursor: pointer;"
        :disabled="auditPage.page <= 1"
        @click="load(auditPage.page - 1)"
      >&lt; 이전</button>
      <span class="text-base" style="color: #64748b;">
        {{ auditPage.page }} / {{ Math.ceil(auditPage.total / auditPage.per_page) }} 페이지
        (총 {{ auditPage.total }}건)
      </span>
      <button
        class="text-base rounded-lg disabled:opacity-40 disabled:cursor-not-allowed"
        style="padding: 8px 18px; border: 1px solid #e2e8f0; background: white; color: #475569; cursor: pointer;"
        :disabled="auditPage.page >= Math.ceil(auditPage.total / auditPage.per_page)"
        @click="load(auditPage.page + 1)"
      >다음 &gt;</button>
    </div>

  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import { getAuditLogs, exportAuditLogs, getRounds, getClasses, blobErrMsg } from '../../api/admin.js'
import HelpBox from '../common/HelpBox.vue'
import { AUDIT_ACTION_LABELS } from '../../data/auditLabels.js'

const HELP = {
  title: '도움말 — 감사 기록',
  intro: '누가 언제 무엇을 했는지 자동으로 기록된 목록입니다. 이 기록은 수정하거나 삭제할 수 없습니다.',
  items: [
    '추천 확정, 라운드 마감, 명단 가져오기 등 모든 주요 작업이 자동으로 남습니다.',
    '라운드·작업 종류·학급으로 걸러서 볼 수 있고, \'전체 목록 다운로드\'로 엑셀 파일로 내려받을 수 있습니다. 학급 필터에는 졸업생 담당 계정도 포함됩니다.',
  ],
}

const auditPage = ref({ rows: [], total: 0, page: 1, per_page: 50 })
const rounds = ref([])
const classes = ref([])
const filterRound = ref(null)
const filterAction = ref('')
const filterClass = ref('')   // '' | `${grade}-${class_no}` (0-0 = 졸업생 담당)
const downloading = ref(false)
const error = ref('')

function classLabel(c) {
  // grade=0/class_no=0은 졸업생 담당 특수 계정 — 0학년 0반으로 표기하지 않는다
  if (c.grade === 0 && c.class_no === 0) return '졸업생 담당'
  const base = `${c.grade}학년 ${c.class_no}반`
  return c.teacher_name ? `${base} (${c.teacher_name})` : base
}

function filterParams() {
  const params = {}
  if (filterRound.value != null) params.round_id = filterRound.value
  if (filterAction.value) params.action = filterAction.value
  if (filterClass.value) {
    const [grade, classNo] = filterClass.value.split('-')
    params.grade = Number(grade)
    params.class_no = Number(classNo)
  }
  return params
}

async function load(page = 1) {
  error.value = ''
  try {
    const params = { page, per_page: auditPage.value.per_page, ...filterParams() }
    auditPage.value = await getAuditLogs(params)
  } catch (e) {
    error.value = e.response?.data ?? e.message ?? '오류가 발생했습니다'
  }
}

async function loadRounds() {
  try {
    rounds.value = await getRounds()
  } catch { /* 라운드 목록 실패 시 필터 없이 동작 */ }
}

async function loadClasses() {
  try {
    // 졸업생 담당(0/0) 포함 전체 학급 — ClassesTab과 달리 여기서는 숨기지 않는다
    classes.value = await getClasses()
  } catch { /* 학급 목록 실패 시 필터 없이 동작 */ }
}

function saveBlob(response, fallback) {
  const url = URL.createObjectURL(new Blob([response.data]))
  const a = document.createElement('a')
  a.href = url; a.download = fallback; a.click()
  URL.revokeObjectURL(url)
}

async function dlAll() {
  downloading.value = true
  try {
    saveBlob(await exportAuditLogs(filterParams()), 'audit_log.xlsx')
  } catch (e) {
    error.value = await blobErrMsg(e)
  } finally {
    downloading.value = false
  }
}

function fmtDt(s) {
  if (!s) return ''
  const d = new Date(s)
  const pad = n => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${pad(d.getMonth()+1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`
}

function fmtActor(row) {
  if (row.actor_type === 'ADMIN') return '관리자'
  const g = row.actor_grade
  const c = row.actor_class_no
  const n = row.actor_name
  // grade=0/class_no=0은 졸업생 담당 특수 계정 — 0은 falsy이므로 != null로 검사해야 한다
  if (g === 0 && c === 0) return '졸업생 담당'
  if (g != null && c != null && n) return `${g}학년 ${c}반 ${n}`
  if (g != null && c != null) return `${g}학년 ${c}반`
  return row.actor_type
}

function fmtTarget(detailStr) {
  try {
    const d = JSON.parse(detailStr)
    const parts = []
    if (d.student_name) parts.push(d.student_name)
    if (d.univ_name) parts.push(d.univ_name)
    if (d.track_name) parts.push(d.track_name)
    if (d.name) parts.push(d.name)
    return parts.join(' / ')
  } catch {
    return ''
  }
}

function fmtDetail(detailStr) {
  try {
    const d = JSON.parse(detailStr)
    const parts = []
    if (d.inserted != null) parts.push(`추가 ${d.inserted}건`)
    if (d.updated != null) parts.push(`수정 ${d.updated}건`)
    if (d.rows != null) parts.push(`${d.rows}행`)
    if (d.calculated != null) parts.push(`${d.calculated}건 계산`)
    if (d.confirmed_tracks != null) parts.push(`자동확정 ${d.confirmed_tracks}개 모집단위`)
    if (d.manual_tracks != null && d.manual_tracks > 0) parts.push(`수동처리 필요 ${d.manual_tracks}개`)
    if (d.source) parts.push(d.source)
    if (d.student_type) parts.push(d.student_type === 'enrolled' ? '재학생' : '졸업생')
    if (d.auto != null) parts.push(d.auto ? '자동 해제 (지원 변경)' : '수동 해제')
    return parts.join(' · ')
  } catch {
    return ''
  }
}

onMounted(() => {
  loadRounds()
  loadClasses()
  load()
})
</script>
