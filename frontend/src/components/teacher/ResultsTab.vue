<template>
  <div class="py-8 px-4 sm:px-10">

    <!-- 페이지 헤더 -->
    <div class="flex items-start justify-between flex-wrap gap-3 mb-5">
      <div>
        <p class="text-base mb-1" style="color: #94a3b8;">담임 교사</p>
        <h1 class="text-2xl font-semibold" style="color: #1e293b; margin: 0;">라운드 결과</h1>
      </div>
    </div>

    <ProductInfoCard class="mb-5" />

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
      <!-- 순위 보기 토글 -->
      <div v-if="results.length > 0" class="flex gap-2">
        <button
          class="text-base font-medium rounded-lg"
          :style="{
            padding: '6px 14px', cursor: 'pointer',
            border: '1px solid',
            borderColor: rankView === 'track' ? '#2563eb' : '#e2e8f0',
            background: rankView === 'track' ? '#2563eb' : 'white',
            color: rankView === 'track' ? 'white' : '#475569',
          }"
          @click="rankView = 'track'"
        >모집단위별 순위</button>
        <button
          class="text-base font-medium rounded-lg"
          :style="{
            padding: '6px 14px', cursor: 'pointer',
            border: '1px solid',
            borderColor: rankView === 'univ' ? '#2563eb' : '#e2e8f0',
            background: rankView === 'univ' ? '#2563eb' : 'white',
            color: rankView === 'univ' ? 'white' : '#475569',
          }"
          @click="rankView = 'univ'"
        >대학 전체 순위</button>

        <!-- 문자 일괄발송용 CSV (이슈 #24). 마감된 라운드가 하나도 없으면 받을 것이 없다. -->
        <button
          v-if="hasFinalized"
          class="text-base font-medium rounded-lg disabled:opacity-40 ml-auto"
          style="padding: 6px 14px; border: none; background: #16a34a; color: white; cursor: pointer;"
          :disabled="downloading"
          @click="downloadAllCsv"
        >{{ downloading ? '내려받는 중…' : '전체 결과 CSV' }}</button>
      </div>


      <div
        v-for="round in rounds"
        :key="round.id"
        class="rounded-xl @container round-card"
        style="background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04);"
      >
        <!-- 카드 헤더 -->
        <!-- 카드 위 두 줄(라운드 제목 / 표 머리글)을 고정한다. 기준 스크롤 영역은
             TeacherView 의 <main class="overflow-y-auto"> 다.
             높이를 64px 로 고정하는 이유: 아래 thead 가 top: 64px 로 이 줄 바로 밑에 붙는다.
             값이 어긋나면 표 머리글이 제목을 가리거나 사이가 뜬다. -->
        <div class="flex items-center gap-3 px-6 round-card-head"
          style="height: 64px; border-bottom: 1px solid #f1f5f9; border-top-left-radius: 12px; border-top-right-radius: 12px;">
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
          >{{ roundStatusLabel(round.status) }}</span>

          <button
            v-if="round.status === 'FINALIZED'"
            class="text-base font-medium rounded-lg disabled:opacity-40 ml-auto"
            style="padding: 6px 14px; border: none; background: #16a34a; color: white; cursor: pointer;"
            :disabled="downloading"
            @click="downloadRoundCsv(round.id)"
          >{{ downloading ? '내려받는 중…' : '이 라운드 CSV' }}</button>
        </div>

        <!-- 진행중/종료 -->
        <div v-if="round.status === 'OPEN'" class="flex items-center justify-center" style="height: 120px;">
          <p class="text-base" style="color: #94a3b8;">현재 진행중인 라운드입니다.</p>
        </div>
        <div v-else-if="round.status === 'CLOSED'" class="flex items-center justify-center" style="height: 120px;">
          <p class="text-base" style="color: #94a3b8;">접수가 종료되어 관리자가 결과를 확정하는 중입니다.</p>
        </div>

        <!-- FINALIZED 결과 -->
        <template v-else>
          <!-- 라운드마다 표 하나. 예전에는 학생마다 표를 따로 만들어 "대학명·모집단위…" 헤더가
               학생 수만큼 반복됐다 (이슈 #29).

               표 안에서 세로로 스크롤하지 않는다 — 지원자가 많아도 페이지에서 쭉 내려가며
               본다. 한때 sticky 헤더를 걸려고 max-height 를 줬는데, 그러면 카드 안에 스크롤
               영역이 생겨 명단이 잘린다. 명단을 다 보는 쪽이 헤더 고정보다 중요하다. -->
          <!-- rounds 는 전체 라운드를, results 는 우리 반 것만 담아 온다(teacher_get_results).
               지원자가 한 명도 없는 마감 라운드가 있을 수 있는데, 그때 표를 그리면 헤더만
               덩그러니 남아 불러오기에 실패한 것처럼 보인다. -->
          <div
            v-if="(studentsByRound[round.id] ?? []).length === 0"
            class="flex items-center justify-center"
            style="height: 120px;"
          >
            <p class="text-base" style="color: #94a3b8;">
              <template v-if="auth.grade === 0">이 라운드에 지원한 졸업생이 없습니다.</template>
              <template v-else>이 라운드에 지원한 우리 반 학생이 없습니다.</template>
            </p>
          </div>

          <!-- 가로만 스크롤한다(표가 940px 보다 좁은 화면). overflow-auto 가 아니라
               overflow-x-auto 를 쓰는 이유는 style.css 의 스크롤바 숨김 규칙이 이 클래스만
               가리켜서다 — overflow-auto 로 두면 이 표에만 네이티브 스크롤바가 뜬다. -->
          <!-- overflow-x-auto 는 스크롤 컨테이너를 만들어 sticky 를 무력화한다.
               카드가 표(min-width 940px)를 담을 만큼 넓으면 가로 스크롤이 필요 없으므로 끈다.
               좁을 때는 가로 스크롤을 살리고, 그 대신 고정이 걸리지 않는다. -->
          <div v-else class="overflow-x-auto @min-[980px]:overflow-x-visible">
            <table style="border-collapse: collapse; table-layout: fixed; width: 100%; min-width: 940px;">
              <colgroup>
                <col style="width: 160px;">
                <col style="width: 190px;">
                <col style="width: 160px;">
                <col style="width: 100px;">
                <col style="width: 100px;">
                <col style="width: 110px;">
                <col style="width: 120px;">
              </colgroup>
              <thead>
                <!-- th 마다 sticky 를 건다 — tr 에 걸면 브라우저가 무시한다.
                     top: 64px 은 위 카드 헤더 높이다. -->
                <tr>
                  <th
                    v-for="h in headers"
                    :key="h.label"
                    class="text-base font-semibold round-card-th"
                    :class="h.align"
                    scope="col"
                    :style="{
                      padding: h.pad,
                      color: '#334155', background: '#e2e8f0',
                      boxShadow: 'inset 0 -1px 0 #cbd5e1',
                    }"
                  >{{ h.label }}</th>
                </tr>
              </thead>
              <tbody>
                <template
                  v-for="student in studentsByRound[round.id] ?? []"
                  :key="student.student_id"
                >
                  <!-- 학생 구분 행 — 표 안에서 학생이 바뀌는 지점이라 결과 행보다 진하게 둔다 -->
                  <tr>
                    <td
                      :colspan="headers.length"
                      style="padding: 11px 20px; background: #f1f5f9; border-top: 1px solid #cbd5e1; border-bottom: 1px solid #e2e8f0;"
                    >
                      <span class="text-base font-semibold" style="color: #0f172a;">{{ student.name }}</span>
                      <span class="text-base" style="color: #475569; margin-left: 10px;">{{ student.student_code }}</span>
                      <span v-if="auth.grade !== 0" class="text-base" style="color: #64748b; margin-left: 10px;">{{ student.seq_no }}번</span>
                    </td>
                  </tr>

                  <tr
                    v-for="r in student.results"
                    :key="r.track_id"
                    class="result-row"
                    :style="{
                      borderBottom: '1px solid #f1f5f9',
                      // 담임은 마감된 라운드만 본다. 색은 선발 결과만 나타낸다 —
                      // 추천 확정은 초록, 미선발과 포기는 빨강.
                      // 동점(노란색)은 관리자가 추천을 고르는 동안(CLOSED)에만 쓰는 '유의' 표시라
                      // 결과가 확정된 화면에는 뜻이 없다. 관리자 RoundsTab 의 FINALIZED 분기와 같다.
                      background: r.recommended && !r.abandoned ? '#f0fdf4' : '#fef2f2',
                    }"
                  >
                    <td class="text-base" style="padding: 12px 20px; color: #1e293b;">{{ r.univ_name }}</td>
                    <td class="text-base" style="padding: 12px 16px; color: #1e293b;">{{ r.track_name }}</td>
                    <!-- 학과명은 점수에 영향이 없어 마감 후에도 고칠 수 있다 (이슈 #32) -->
                    <td class="text-base" style="padding: 12px 16px; color: #475569;">
                      <div v-if="isEditing(r)" class="flex flex-col gap-1.5" style="min-width: 0;">
                        <input
                          v-model="editingName"
                          class="text-base"
                          style="padding: 4px 10px; border: 1px solid #93c5fd; border-radius: 6px; width: 100%; min-width: 0; box-sizing: border-box;"
                          placeholder="학과명"
                          @keyup.enter="saveDepartment(r)"
                          @keyup.esc="cancelEdit"
                        />
                        <div class="flex gap-1.5 flex-wrap">
                          <button
                            class="text-base whitespace-nowrap"
                            style="flex: 0 0 auto; padding: 4px 10px; border: 1px solid #2563eb; border-radius: 6px; background: #2563eb; color: white; cursor: pointer;"
                            :disabled="savingDepartment || !editingName.trim()"
                            @click="saveDepartment(r)"
                          >저장</button>
                          <button
                            class="text-base whitespace-nowrap"
                            style="flex: 0 0 auto; padding: 4px 10px; border: 1px solid #cbd5e1; border-radius: 6px; background: white; color: #64748b; cursor: pointer;"
                            :disabled="savingDepartment"
                            @click="cancelEdit"
                          >취소</button>
                        </div>
                      </div>
                      <button
                        v-else
                        class="text-base text-left"
                        style="border: none; background: none; color: #475569; cursor: pointer; padding: 0; text-decoration: underline dotted #cbd5e1; text-underline-offset: 3px;"
                        title="학과명 수정 (점수에 영향 없음)"
                        @click="startEdit(r)"
                      >{{ r.department_name || '—' }}</button>
                    </td>
                    <td class="text-base text-center" style="padding: 12px 16px; color: #64748b;">{{ rankView === 'track' ? (r.track_rank ?? '-') : (r.ranking ?? '-') }}</td>
                    <td class="text-base text-left font-semibold" style="padding: 12px 20px; color: #1e293b;">
                      {{ formatScore(r.total_score) }}
                    </td>
                    <td class="text-center" style="padding: 12px 16px;">
                      <span v-if="r.abandoned" class="text-base font-semibold" style="color: #ef4444;">포기됨</span>
                      <span v-else-if="r.recommended" class="text-base font-semibold" style="color: #16a34a;">추천 확정</span>
                      <span v-else class="text-base font-semibold" style="color: #ef4444;">미선발</span>
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
                </template>
              </tbody>
            </table>
          </div>
        </template>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted } from 'vue'
import { useAuthStore } from '../../stores/auth.js'
import {
  teacherGetResults,
  teacherAbandonApplication,
  teacherRoundResultsCsv,
  teacherAllResultsCsv,
  teacherUpdateApplicationDepartment,
} from '../../api/teacher.js'
import { roundStatusLabel } from '../../data/roundStatus.js'
import { dialog } from '../common/dialog.js'
import { blobErrMsg } from '../../utils/blobError.js'
import HelpBox from '../common/HelpBox.vue'
import ProductInfoCard from '../common/ProductInfoCard.vue'
import { formatScore } from '../../utils/scorePreviewShared.js'

const auth = useAuthStore()

const rounds    = ref([])
const results   = ref([])
const loading   = ref(false)
const loadError = ref('')
// 기본값은 대학 전체 순위 — 관리자 [결과] 탭과 같은 기준이다. 역할마다 기본값이 다르면
// 같은 학생의 순위가 담임과 관리자에게 다른 수로 보여 통화가 꼬인다.
const rankView  = ref('univ')
// 다운로드 중에는 버튼을 잠근다 — 같은 파일을 두 번 받는 조작을 막는다(저장소 공통 패턴)
const downloading = ref(false)

// 학과명 인라인 수정 (이슈 #32). 담임의 학과명 수정은 이 화면 하나뿐이다.
// 이 화면이 FINALIZED 만 보여주므로 담임 엔드포인트도 FINALIZED 전용이다.
const editingKey       = ref(null)
const editingName      = ref('')
const savingDepartment = ref(false)

function resultKey(r) {
  return `${r.student_id}-${r.track_id}-${r.round_id}`
}

function isEditing(r) {
  return editingKey.value === resultKey(r)
}

function startEdit(r) {
  editingKey.value  = resultKey(r)
  editingName.value = r.department_name || ''
}

function cancelEdit() {
  editingKey.value  = null
  editingName.value = ''
}

async function saveDepartment(r) {
  const name = editingName.value.trim()
  if (!name || savingDepartment.value) return
  savingDepartment.value = true
  try {
    await teacherUpdateApplicationDepartment(r.student_id, r.track_id, r.round_id, name)
    await load()
    cancelEdit()
  } catch (e) {
    await dialog.alert({ title: '오류', message: e.response?.data || e.message, level: 'error' })
  } finally {
    savingDepartment.value = false
  }
}

// 표 헤더 정의. 한 곳에 모아 두면 열을 늘릴 때 colgroup 과 함께 여기만 보면 된다.
// pad 는 아래 본문 td 의 좌우 패딩과 짝을 맞춘다 — 다르면 헤더 글자와 값의 시작점이 어긋난다
const headers = computed(() => [
  { label: '대학명',   align: 'text-left',   pad: '12px 20px' },
  { label: '모집단위', align: 'text-left',   pad: '12px 16px' },
  { label: '지원 학과', align: 'text-left',   pad: '12px 16px' },
  { label: rankView.value === 'track' ? '모집단위 순위' : '대학 순위', align: 'text-center', pad: '12px 16px' },
  { label: '총점',     align: 'text-left',   pad: '12px 20px' },
  { label: '상태',     align: 'text-center', pad: '12px 16px' },
  { label: '비고',     align: 'text-center', pad: '12px 16px' },
])

const hasFinalized = computed(() => rounds.value.some(r => r.status === 'FINALIZED'))

const helpBox = computed(() => {
  if (hasFinalized.value) {
    return {
      key: 'results-final',
      title: '도움말 — 결과 보는 방법',
      intro: '마감된 라운드의 우리 반 학생 결과입니다.',
      items: [
        '초록색 배경의 "추천 확정"은 학교장추천 대상으로 확정된 것이고, 붉은색 배경은 이번 라운드에서 추천되지 않았거나("미선발") 추천을 포기한("포기됨") 것입니다.',
        '추천이 확정된 학생이 추천을 포기하려면 "추천 포기"를 누르세요.',
        { text: '포기는 되돌릴 수 없습니다. 반드시 학생·학부모와 확인한 뒤 처리하세요. 다시 추천받으려면 다음 라운드에서 재지원해야 합니다.', warn: true },
        '"미선발"된 학생은 다음 라운드가 열리면 다시 지원할 수 있습니다.',
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
      '"진행중" 또는 "종료"로 표시된 라운드는 아직 결과가 공개되지 않은 것입니다.',
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

/// 응답 헤더의 파일명을 그대로 쓴다 — 서버가 라운드 번호와 시각을 붙여 준다.
async function saveCsv(request, fallbackName) {
  downloading.value = true
  try {
    const res = await request()
    const url = URL.createObjectURL(res.data)
    const a = document.createElement('a')
    a.href = url
    a.download = res.headers['content-disposition']?.match(/filename="(.+)"/)?.[1] ?? fallbackName
    a.click()
    URL.revokeObjectURL(url)
  } catch (e) {
    await dialog.alert({
      title: 'CSV 내려받기 실패',
      message: await blobErrMsg(e),
      level: 'error',
    })
  } finally {
    downloading.value = false
  }
}

function downloadRoundCsv(roundId) {
  return saveCsv(() => teacherRoundResultsCsv(roundId), `round${roundId}_results.csv`)
}

function downloadAllCsv() {
  return saveCsv(teacherAllResultsCsv, 'all_results.csv')
}

async function load() {
  cancelEdit()
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

<style scoped>
/* 배경은 항상 칠한다. 투명하면 고정됐을 때 아래 행들이 헤더 글자 뒤로 비쳐 지나간다. */
.round-card-head { background: white; }

/* 카드 제목 줄과 표 머리글을 **같은 조건에서 함께** 고정한다.
   기준 스크롤 영역은 TeacherView 의 <main class="overflow-y-auto"> 다.

   980px 은 표 감싸개가 가로 스크롤을 끄는 지점과 같다. 그보다 좁으면 감싸개가 스크롤
   컨테이너가 되어 표 머리글은 고정될 수 없는데, 그때 제목 줄만 붙어 있으면 둘 사이가
   흰 여백으로 벌어진다. 둘 다 붙거나 둘 다 풀리거나여야 한다. */
@container (min-width: 980px) {
  .round-card-head {
    position: sticky;
    top: 0;
    z-index: 3;   /* 표 머리글(2)보다 위 */
  }
  .round-card-th {
    position: sticky;
    top: 64px;    /* 위 제목 줄 높이 */
    z-index: 2;
  }
}

/* overflow-hidden 을 걷어냈으므로(그게 sticky 를 막는다) 표가 카드의 둥근 모서리를 넘는다.
   마지막 행의 아래 모서리를 직접 둥글린다. rounded-xl 과 같은 12px. */
.round-card tbody tr:last-child td:first-child { border-bottom-left-radius: 12px; }
.round-card tbody tr:last-child td:last-child  { border-bottom-right-radius: 12px; }
/* 표가 아닌 마지막 블록(진행중·종료 안내, 지원자 없음)도 같은 모서리를 갖는다 */
.round-card > :last-child {
  border-bottom-left-radius: 12px;
  border-bottom-right-radius: 12px;
}

/* 행 배경(추천 확정·동점·미선발)은 의미를 담은 색이라 호버로 덮으면 안 된다.
   td 배경은 tr 배경 위에 얹히므로, 반투명 한 겹으로 색조는 두고 어둡게만 만든다.
   tr 의 background 는 인라인 style 이라 hover:bg-* 같은 클래스로는 애초에 덮이지도 않는다. */
.result-row td {
  transition: background-color 0.12s ease;
}
.result-row:hover td {
  background-color: rgba(15, 23, 42, 0.06);
}
</style>
