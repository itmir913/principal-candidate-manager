<template>
  <div style="padding: 2rem 2.5rem;">

    <!-- 페이지 헤더 -->
    <div class="mb-6">
      <p class="text-base mb-1" style="color: #94a3b8;">관리자</p>
      <h1 class="text-2xl font-semibold" style="color: #1e293b; margin: 0;">개요</h1>
    </div>

    <!-- 로딩 -->
    <div v-if="loading" class="text-base text-center" style="padding: 60px 0; color: #94a3b8;">
      불러오는 중…
    </div>

    <!-- 오류 -->
    <div v-else-if="error" class="rounded-xl text-base"
      style="padding: 16px 20px; background: #fef2f2; color: #ef4444;">
      {{ error }}
    </div>

    <!-- 본문 -->
    <div v-else-if="data" class="flex flex-col gap-4">

      <!-- ① 앱 정보 -->
      <div class="rounded-xl" style="padding: 20px 24px; background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04);">
        <div class="grid grid-cols-1 lg:grid-cols-2 gap-3">
          <div>
            <p class="text-base font-semibold" style="color: #94a3b8; text-transform: uppercase; letter-spacing: 0.07em;">
              Teacher Utility Kit
            </p>
            <p class="text-xl font-bold mt-0.5" style="color: #1e293b;">학교장 추천자 선발 관리 시스템</p>
          </div>
          <div class="lg:text-right">
            <p class="text-base font-semibold" style="color: #475569;">© luminousky</p>
            <p class="text-base mt-0.5" style="color: #94a3b8;">
              Principal Candidate Manager · v{{ data.version }}
            </p>
          </div>
        </div>
      </div>

      <!-- ② 서버 접속 정보 -->
      <div class="rounded-xl" style="padding: 20px 24px; background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04);">
        <SectionLabel title="서버 접속 정보" />
        <div class="grid grid-cols-1 lg:grid-cols-[1fr_auto] gap-2">
          <div class="rounded-lg font-mono text-base select-all"
            style="padding: 10px 16px; background: #f8fafc; border: 1px solid #e2e8f0; color: #1e293b; min-width: 0; overflow-x: auto; white-space: nowrap;">
            http://{{ data.server_addr }}
          </div>
          <button
            @click="handleCopy"
            class="flex items-center justify-center gap-1.5 text-base font-semibold rounded-lg transition-colors"
            style="padding: 10px 20px; border: none; cursor: pointer; white-space: nowrap;"
            :style="copied
              ? { background: '#16a34a', color: 'white' }
              : { background: '#2563eb', color: 'white' }"
          >
            <Check v-if="copied" :size="16" />
            <Copy v-else :size="16" />
            {{ copied ? '복사됨' : '복사' }}
          </button>
        </div>
        <p class="text-base mt-2" style="color: #94a3b8;">
          교사들이 이 주소로 접속합니다. 같은 네트워크에 연결되어 있어야 합니다.
        </p>
      </div>

      <!-- 라운드 시작 전 준비 체크리스트 -->
      <div v-if="!data.round && checklist" class="rounded-xl"
           style="padding: 20px 24px; background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04);">
        <SectionLabel title="첫 번째 라운드 시작 전 준비 체크리스트" />

        <div class="flex flex-col gap-2">
          <div
              v-for="item in checklist"
              :key="item.key"
              class="flex items-center gap-3 rounded-lg flex-wrap"
              style="padding: 12px 16px;"
              :style="{ background: item.count > 0 ? '#f0fdf4' : '#fef2f2' }"
          >
            <CheckCircle2 v-if="item.count > 0" :size="20" style="color: #16a34a;" class="flex-shrink-0" />
            <XCircle v-else :size="20" style="color: #ef4444;" class="flex-shrink-0" />
            <div class="min-w-0">
              <p class="text-base font-semibold" style="margin: 0;"
                 :style="{ color: item.count > 0 ? '#15803d' : '#b91c1c' }">
                {{ item.label }}
                <span class="font-normal">— {{ item.count > 0 ? `${item.count}${item.unit} 등록됨` : '아직 등록되지 않음' }}</span>
              </p>
              <p class="text-base" style="margin: 2px 0 0; color: #94a3b8;">{{ item.desc }}</p>
            </div>
            <button
                v-if="item.count === 0"
                class="flex items-center gap-1 text-base font-medium rounded-lg ml-auto flex-shrink-0"
                style="padding: 7px 14px; border: none; background: #2563eb; color: white; cursor: pointer;"
                @click="setActiveTab(item.tab)"
            >설정하러 가기 <ArrowRight :size="15" /></button>
            <button
                v-else
                class="flex items-center gap-1 text-base rounded-lg ml-auto flex-shrink-0"
                style="padding: 7px 14px; border: 1px solid #e2e8f0; background: white; color: #64748b; cursor: pointer;"
                @click="setActiveTab(item.tab)"
            >보기 <ArrowRight :size="15" /></button>
          </div>
        </div>

        <!-- 전부 완료 시 -->
        <div v-if="allReady" class="flex items-center gap-3 rounded-lg mt-3 flex-wrap"
             style="padding: 12px 16px; background: #eff6ff; border: 1px solid #bfdbfe;">
          <p class="text-base font-semibold" style="margin: 0; color: #1d4ed8;">
            모든 준비가 끝났습니다. 이제 첫 번째 라운드를 열어 담임교사의 입력을 시작할 수 있습니다.
          </p>
          <button
              class="flex items-center gap-1 text-base font-semibold rounded-lg ml-auto flex-shrink-0"
              style="padding: 7px 14px; border: none; background: #2563eb; color: white; cursor: pointer;"
              @click="setActiveTab('rounds')"
          >라운드 관리로 이동 <ArrowRight :size="15" /></button>
        </div>
      </div>

      <HelpBox
          v-if="helpBox"
          :key="helpBox.key"
          :storage-key="helpBox.key"
          :title="helpBox.title"
          :intro="helpBox.intro"
          :items="helpBox.items"
      />

      <!-- ③ 현재 라운드 -->
      <div class="rounded-xl" style="padding: 20px 24px; background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04);">
        <SectionLabel title="현재 라운드" />
        <div v-if="data.round" class="flex items-center gap-3 flex-wrap">
          <span class="text-3xl font-bold" style="color: #1e293b;">{{ data.round.id }}차 라운드</span>
          <span
            class="text-base font-semibold"
            style="padding: 4px 14px; border-radius: 999px;"
            :style="data.round.status === 'OPEN'
              ? { background: '#dcfce7', color: '#15803d' }
              : { background: '#dbeafe', color: '#1d4ed8' }"
          >
            {{ data.round.status === 'OPEN' ? '진행중' : '종료' }}
          </span>
          <span class="text-base ml-auto" style="color: #94a3b8;">개시일 {{ data.round.opened_at.slice(0, 10) }}</span>
        </div>
        <div v-else class="flex items-center justify-between">
          <p class="text-base" style="color: #94a3b8;">현재 진행 중인 라운드가 없습니다.</p>
        </div>
      </div>

      <!-- ④ 학급별 지원자 현황 (라운드 있을 때만) -->
      <template v-if="data.round">
        <div class="rounded-xl overflow-hidden flex flex-col" style="min-height: 200px; background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04);">
          <div style="padding: 20px 24px 0;">
            <SectionLabel title="이번 라운드 · 학급별 지원자 현황" />
          </div>

          <div v-if="data.classes.length === 0" class="flex-1 flex items-center justify-center text-base" style="color: #94a3b8;">
            등록된 학급이 없습니다.
          </div>

          <div v-else class="flex flex-col lg:flex-row gap-6" style="padding: 0 24px 20px;">
            <!-- 요약 카드 (lg 미만: 표 위에 가로 배치 / lg 이상: 표 오른쪽에 세로 배치) -->
            <div class="flex flex-col md:flex-row lg:flex-col gap-3 lg:justify-center lg:order-last" style="min-width: 140px;">
              <div class="flex-1 lg:flex-none rounded-xl text-center" style="padding: 16px; background: #f8fafc;">
                <p class="text-2xl font-bold" style="color: #1e293b;">{{ totalApplicants }}명</p>
                <p class="text-base mt-0.5" style="color: #94a3b8;">총 지원자</p>
              </div>
              <div class="flex-1 lg:flex-none rounded-xl text-center"
                :style="zeroClassCount > 0
                  ? { padding: '16px', background: '#fef2f2' }
                  : { padding: '16px', background: '#f0fdf4' }"
              >
                <p class="text-2xl font-bold" :style="zeroClassCount > 0 ? { color: '#ef4444' } : { color: '#16a34a' }">
                  {{ zeroClassCount }}개
                </p>
                <p class="text-base mt-0.5" :style="zeroClassCount > 0 ? { color: '#f87171' } : { color: '#4ade80' }">
                  미입력 학급
                </p>
              </div>
              <div class="flex-1 lg:flex-none rounded-xl text-center"
                :style="unconfirmedCount > 0
                  ? { padding: '16px', background: '#fffbeb' }
                  : { padding: '16px', background: '#f0fdf4' }"
              >
                <p class="text-2xl font-bold" :style="unconfirmedCount > 0 ? { color: '#d97706' } : { color: '#16a34a' }">
                  {{ unconfirmedCount > 0 ? `${unconfirmedCount}개` : '모두 확정' }}
                </p>
                <p class="text-base mt-0.5" :style="unconfirmedCount > 0 ? { color: '#d97706' } : { color: '#4ade80' }">
                  미확정 학급
                </p>
              </div>
            </div>

            <!-- 테이블 -->
            <div class="flex-1 rounded-xl overflow-hidden" style="border: 1px solid #e2e8f0;">
              <div class="overflow-x-auto">
              <table class="w-full" style="border-collapse: collapse;">
                <thead>
                  <tr style="background: #f8fafc; border-bottom: 1px solid #e2e8f0;">
                    <th class="text-base font-semibold text-left" style="padding: 14px 20px; color: #475569;">학급</th>
                    <th class="text-base font-semibold text-left" style="padding: 14px 20px; color: #475569;">담임</th>
                    <th class="text-base font-semibold text-right" style="padding: 14px 20px; color: #475569;">지원자 수</th>
                    <th class="text-base font-semibold text-center" style="padding: 14px 20px; color: #475569;">입력 확정</th>
                  </tr>
                </thead>
                <tbody>
                  <tr
                    v-for="c in data.classes"
                    :key="`${c.grade}-${c.class_no}`"
                    style="border-bottom: 1px solid #f1f5f9; transition: background 0.1s;"
                    :style="c.submitted === 0 ? { background: '#fef2f2' } : {}"
                    class="hover:bg-slate-50"
                  >
                    <td class="text-base font-semibold" style="padding: 14px 20px; color: #1e293b;">
                      {{ c.grade }}학년 {{ c.class_no }}반
                    </td>
                    <td class="text-base" style="padding: 14px 20px; color: #475569;">
                      {{ c.teacher_name ?? '—' }}
                    </td>
                    <td class="text-base text-right" style="padding: 14px 20px;">
                      <span v-if="c.submitted === 0" class="flex items-center justify-end gap-1 font-bold" style="color: #ef4444;">
                        <AlertTriangle :size="16" />
                        0명
                      </span>
                      <span v-else class="font-semibold" style="color: #1e293b;">{{ c.submitted }}명</span>
                    </td>
                    <td class="text-base text-center" style="padding: 14px 20px;">
                      <span v-if="c.confirmed"
                        class="font-semibold"
                        style="padding: 3px 10px; border-radius: 999px; background: #f0fdf4; color: #16a34a;">
                        ✓ 확정
                      </span>
                      <span v-else
                        style="padding: 3px 10px; border-radius: 999px; background: #fffbeb; color: #d97706;">
                        미확정
                      </span>
                    </td>
                  </tr>
                  <!-- 졸업생 행 -->
                  <tr
                    v-if="data.graduated"
                    style="border-bottom: 1px solid #f1f5f9; transition: background 0.1s;"
                    :style="data.graduated.submitted === 0 ? { background: '#fef2f2' } : {}"
                    class="hover:bg-slate-50"
                  >
                    <td class="text-base font-semibold" style="padding: 14px 20px; color: #1e293b;">졸업생 담당</td>
                    <td class="text-base" style="padding: 14px 20px; color: #475569;">{{ data.graduated.teacher_name ?? '관리자' }}</td>
                    <td class="text-base text-right" style="padding: 14px 20px;">
                      <span v-if="data.graduated.submitted === 0" class="flex items-center justify-end gap-1 font-bold" style="color: #ef4444;">
                        <AlertTriangle :size="16" />
                        0명
                      </span>
                      <span v-else class="font-semibold" style="color: #1e293b;">{{ data.graduated.submitted }}명</span>
                    </td>
                    <td class="text-base text-center" style="padding: 14px 20px;">
                      <span v-if="data.graduated.confirmed"
                        class="font-semibold"
                        style="padding: 3px 10px; border-radius: 999px; background: #f0fdf4; color: #16a34a;">
                        ✓ 확정
                      </span>
                      <span v-else
                        style="padding: 3px 10px; border-radius: 999px; background: #fffbeb; color: #d97706;">
                        미확정
                      </span>
                    </td>
                  </tr>
                </tbody>
              </table>
              </div>
            </div>
          </div>
        </div>

        <!-- ⑤ 모집단위별 지원 현황 -->
        <div class="rounded-xl overflow-hidden flex flex-col" style="min-height: 200px; background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04);">
          <div style="padding: 20px 24px 0;">
            <SectionLabel title="이번 라운드 · 모집단위별 지원 현황" />
          </div>

          <div v-if="data.universities.length === 0" class="flex-1 flex items-center justify-center text-base" style="color: #94a3b8;">
            등록된 대학·모집단위가 없습니다.
          </div>

          <div v-else class="rounded-xl overflow-hidden" style="margin: 0 24px 20px; border: 1px solid #e2e8f0;">
            <div class="overflow-x-auto">
            <table class="w-full" style="border-collapse: collapse;">
              <thead>
                <tr style="background: #f8fafc; border-bottom: 1px solid #e2e8f0;">
                  <th style="width: 48px; padding: 14px 20px;"></th>
                  <th class="text-base font-semibold text-left" style="padding: 14px 20px; color: #475569;">모집단위</th>
                  <th class="text-base font-semibold text-left" style="padding: 14px 20px; color: #475569;">지원자 / 정원</th>
                  <th class="text-base font-semibold text-right" style="padding: 14px 20px; color: #475569;">현황</th>
                </tr>
              </thead>
              <tbody>
                <template v-for="univ in data.universities" :key="univ.univ_id">
                  <!-- 대학 헤더 행 -->
                  <tr style="background: #f8fafc; border-bottom: 1px solid #e2e8f0;">
                    <td colspan="4" style="padding: 12px 20px;">
                      <span class="text-base font-bold" style="color: #1e293b;">{{ univ.univ_name }}</span>
                      <span v-if="univ.total_quota !== null" class="text-base ml-3" style="color: #64748b;">
                        총 정원 {{ univ.total_quota }}명
                      </span>
                      <span v-if="univ.tracks.length === 0" class="text-base ml-3" style="color: #94a3b8;">
                        (모집단위 없음)
                      </span>
                    </td>
                  </tr>
                  <!-- 모집단위 행 -->
                  <tr
                    v-for="track in univ.tracks"
                    :key="track.track_id"
                    class="hover:bg-slate-50"
                    style="border-bottom: 1px solid #f1f5f9; transition: background 0.1s;"
                  >
                    <!-- 파이 차트 or 무제한 표시 -->
                    <td style="padding: 14px 20px 14px 28px;">
                      <MiniPie
                        v-if="track.unit_quota !== null"
                        :filled="track.applicants"
                        :total="track.unit_quota"
                        :size="40"
                      />
                      <span v-else class="text-base font-semibold" style="color: #94a3b8;">∞</span>
                    </td>
                    <!-- 모집단위명 -->
                    <td class="text-base" style="padding: 14px 20px; color: #1e293b;">{{ track.track_name }}</td>
                    <!-- 지원자/정원 -->
                    <td class="text-base tabular-nums" style="padding: 14px 20px;">
                      <span v-if="track.unit_quota !== null">
                        <span class="font-semibold"
                          :style="track.applicants >= track.unit_quota ? { color: '#ef4444' } : { color: '#1e293b' }">
                          {{ track.applicants }}
                        </span>
                        <span style="color: #cbd5e1;"> / </span>
                        <span style="color: #475569;">{{ track.unit_quota }}명</span>
                      </span>
                      <span v-else class="font-semibold" style="color: #1e293b;">{{ track.applicants }}명</span>
                    </td>
                    <!-- 현황 배지 -->
                    <td class="text-right" style="padding: 14px 20px;">
                      <template v-if="track.unit_quota !== null">
                        <span v-if="track.applicants >= track.unit_quota"
                          class="text-base font-semibold"
                          style="padding: 3px 12px; border-radius: 999px; background: #fef2f2; color: #ef4444;">
                          마감
                        </span>
                        <span v-else-if="track.applicants === 0"
                          class="text-base font-semibold"
                          style="padding: 3px 12px; border-radius: 999px; background: #f1f5f9; color: #64748b;">
                          미지원
                        </span>
                        <span v-else class="text-base" style="color: #64748b;">
                          {{ track.unit_quota - track.applicants }}자리 남음
                        </span>
                      </template>
                      <span v-else
                        class="text-base font-semibold"
                        style="padding: 3px 12px; border-radius: 999px; background: #f1f5f9; color: #64748b;">
                        무제한
                      </span>
                    </td>
                  </tr>
                </template>
              </tbody>
            </table>
            </div>
          </div>
        </div>
      </template>

      <!-- ⑥ 전체 누적 통계 (항상 표시) -->
      <div class="rounded-xl" style="padding: 20px 24px; background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04);">
        <SectionLabel title="전체 누적 통계" />
        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-3">
          <div v-for="stat in allTimeStats" :key="stat.label"
            class="rounded-xl text-center"
            style="padding: 18px 12px; background: #f8fafc;">
            <p class="text-2xl font-bold" style="color: #1e293b;">{{ stat.value }}</p>
            <p class="text-base mt-0.5" style="color: #94a3b8;">{{ stat.label }}</p>
          </div>
        </div>
      </div>

    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, inject, h } from 'vue'
import { Copy, Check, AlertTriangle, CheckCircle2, XCircle, ArrowRight } from 'lucide-vue-next'
import { getOverview, getClasses, getStudents, getAreas, getUniversities } from '../../api/admin.js'
import MiniPie from './MiniPie.vue'
import HelpBox from '../common/HelpBox.vue'

// ── 섹션 레이블 헬퍼 컴포넌트 (인라인) ─────────────────────────
const SectionLabel = {
  props: ['title'],
  setup(props) {
    return () => h('div', { class: 'flex items-center gap-3 mb-4' }, [
      h('span', { class: 'text-base font-semibold', style: 'color: #94a3b8; text-transform: uppercase; letter-spacing: 0.07em;' }, props.title),
      h('div', { class: 'flex-1', style: 'height: 1px; background: #f1f5f9;' }),
    ])
  },
}

const setActiveTab = inject('setActiveTab', () => {})

// ── 상태 ──────────────────────────────────────────────────────
const data    = ref(null)
const loading = ref(true)
const error   = ref('')
const copied  = ref(false)

// null = 아직 로드 전 또는 로드 실패(카드 숨김)
const readiness = ref(null)

async function loadReadiness() {
  try {
    const [classes, studentPage, areas, univs] = await Promise.all([
      getClasses(),
      getStudents({ page: 1, per_page: 1 }),
      getAreas(),
      getUniversities(),
    ])
    readiness.value = {
      classes:  classes.filter(c => !(c.grade === 0 && c.class_no === 0)).length,
      students: studentPage.total,
      areas:    areas.length,
      univs:    univs.length,
    }
  } catch {
    readiness.value = null
  }
}

// ── 체크리스트 ────────────────────────────────────────────────
const checklist = computed(() => {
  if (!readiness.value) return null
  const r = readiness.value
  return [
    { key: 'classes',  label: '학급 등록',     desc: '담임교사 계정을 생성합니다',            count: r.classes,  unit: '개 학급', tab: 'classes' },
    { key: 'students', label: '학생 명단 등록', desc: '추천 대상 재학생·졸업생 명단을 입력합니다',      count: r.students, unit: '명',     tab: 'students' },
    { key: 'areas',    label: '전형요소 설정',  desc: '학교장추천 선발을 위한 영역과 배점을 정합니다',             count: r.areas,    unit: '개 항목', tab: 'areas' },
    { key: 'univs',    label: '대학 설정',      desc: '지원할 대학·모집단위와 정원을 정합니다',  count: r.univs,    unit: '개 대학', tab: 'univs' },
  ]
})

const allReady = computed(() => checklist.value?.every(item => item.count > 0) ?? false)

// ── 파생값 ────────────────────────────────────────────────────
const totalApplicants = computed(() => {
  const classSum = data.value?.classes.reduce((s, c) => s + c.submitted, 0) ?? 0
  const gradSum  = data.value?.graduated?.submitted ?? 0
  return classSum + gradSum
})
const zeroClassCount = computed(() => {
  let count = data.value?.classes.filter(c => c.submitted === 0).length ?? 0
  if (data.value?.graduated && data.value.graduated.submitted === 0) count++
  return count
})
const unconfirmedCount = computed(() => {
  let count = data.value?.classes.filter(c => !c.confirmed).length ?? 0
  if (data.value?.graduated && !data.value.graduated.confirmed) count++
  return count
})
const helpBox = computed(() => {
  if (!data.value) return null
  const round = data.value.round
  if (!round) {
    if (data.value.all_time.total_rounds === 0) {
      return {
        key: 'overview-first',
        title: '도움말 — 처음 시작하기',
        intro: '이 화면은 시스템의 전체 현황을 한눈에 보여줍니다. 아직 진행 중인 라운드가 없습니다.',
        items: [
          '처음 사용하신다면 왼쪽 메뉴에서 [학급 관리] → [학생 관리] → [전형요소 설정] → [대학 설정] 순서로 기초 정보를 먼저 입력하세요.',
          '준비가 끝나면 [라운드 관리]에서 "+ 라운드 열기"를 눌러 담임교사의 입력을 시작할 수 있습니다.',
          '자세한 사용 방법은 왼쪽 아래 [매뉴얼] 메뉴에서 볼 수 있습니다.',
        ],
      }
    }
    return {
      key: 'overview-idle',
      title: '도움말 — 진행 중인 라운드 없음',
      intro: '이전 라운드는 모두 마감되었고, 지금은 진행 중인 라운드가 없습니다.',
      items: [
        '추가 추천이 필요하면 [라운드 관리]에서 "+ 라운드 열기"로 다음 차수를 시작하세요.',
        '이전 라운드의 결과는 [라운드 관리]에서 해당 라운드를 선택해 다시 확인하거나 내려받을 수 있습니다.',
      ],
    }
  }
  if (round.status === 'OPEN') {
    return {
      key: 'overview-open',
      title: '도움말 — 라운드 진행 중',
      intro: '지금은 담임교사들이 지원자를 등록하는 기간입니다.',
      items: [
        '아래 "학급별 지원자 현황"에서 학급별 입력 상황을 확인하세요. 빨간색으로 표시된 학급은 아직 지원자를 한 명도 등록하지 않은 학급입니다.',
        '위의 "서버 접속 정보" 주소를 담임교사들에게 알려주세요. 담임교사는 이 주소로 접속해 로그인합니다.',
        '모든 담임교사의 입력이 끝나면 [라운드 관리]에서 "종료하기"를 눌러 입력을 마감하세요.',
      ],
    }
  }
  return {
    key: 'overview-closed',
    title: '도움말 — 입력 종료, 추천 확정 단계',
    intro: '담임교사 입력이 종료되었습니다. 이제 관리자가 추천자를 확정할 차례입니다.',
    items: [
      '[라운드 관리]에서 이 라운드를 선택한 뒤 [결과] 탭에서 "자동 추천 확정"을 누르거나 학생별로 "추천 확정"을 누르세요.',
      '추천 확정이 모두 끝나면 "마감하기"를 눌러 결과를 담임교사에게 공개하세요.',
      '입력을 다시 받아야 하면 "다시 열기"를 누르면 됩니다.',
    ],
  }
})

const allTimeStats = computed(() => {
  if (!data.value) return []
  const t = data.value.all_time
  return [
    { label: '진행된 라운드 차수',  value: `${t.total_rounds}차` },
    { label: '누적 지원자',  value: `${t.total_applicants}명` },
    { label: '확정 추천자',  value: `${t.confirmed}명` },
    { label: '포기자',       value: `${t.abandoned}명` },
  ]
})

// ── 데이터 로드 ───────────────────────────────────────────────
onMounted(async () => {
  loadReadiness() // 개요 로드와 병렬 실행 — 실패해도 체크리스트 카드만 숨겨진다
  try {
    data.value = await getOverview()
  } catch (e) {
    error.value = e.response?.data ?? e.message ?? '데이터를 불러오지 못했습니다.'
  } finally {
    loading.value = false
  }
})

// ── 클립보드 복사 ─────────────────────────────────────────────
function handleCopy() {
  const url = `http://${data.value.server_addr}`
  const markCopied = () => {
    copied.value = true
    setTimeout(() => { copied.value = false }, 2000)
  }
  if (navigator.clipboard) {
    navigator.clipboard.writeText(url).then(markCopied)
  } else {
    const el = document.createElement('textarea')
    el.value = url
    el.style.cssText = 'position:fixed;opacity:0'
    document.body.appendChild(el)
    el.select()
    document.execCommand('copy')
    document.body.removeChild(el)
    markCopied()
  }
}
</script>
