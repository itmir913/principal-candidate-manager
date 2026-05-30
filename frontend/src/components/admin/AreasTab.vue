<template>
  <div class="flex gap-4 min-h-0">
    <!-- ── 좌측: 전형요소 목록 ── -->
    <div class="w-72 shrink-0">
      <div class="flex items-center justify-between mb-3">
        <h2 class="text-lg font-semibold text-gray-700">전형요소 목록</h2>
        <button class="px-2 py-1 bg-blue-600 text-white text-sm rounded hover:bg-blue-700"
          @click="openAddForm">+ 추가</button>
      </div>

      <p v-if="error" class="text-red-500 text-sm mb-2">{{ error }}</p>

      <ul class="space-y-1">
        <li v-for="area in areas" :key="area.id"
          class="flex items-center justify-between px-3 py-2 rounded border cursor-pointer"
          :class="selected?.id === area.id
            ? 'border-blue-500 bg-blue-50'
            : 'border-gray-200 hover:bg-gray-50'"
          @click="selectArea(area)">
          <div class="min-w-0">
            <span class="text-sm font-medium truncate block">{{ area.name }}</span>
            <span class="text-xs text-gray-400">{{ calcTypeLabel(area.calc_type) }} · {{ lookupScopeLabel(area.lookup_scope) }}</span>
          </div>
          <button class="text-red-400 text-xs hover:text-red-600 ml-2 shrink-0"
            @click.stop="removeArea(area.id)">삭제</button>
        </li>
        <li v-if="areas.length === 0" class="text-gray-400 text-sm px-2">등록된 전형요소 없음</li>
      </ul>

      <div v-if="areas.length > 0"
        class="mt-2 px-3 py-2 border-t border-gray-200 flex items-center justify-between text-sm font-semibold text-gray-700">
        <span>총점</span>
        <span>{{ displayScore(totalMaxScore) }}점</span>
      </div>

      <!-- 전형요소 추가 폼 -->
      <div v-if="showAddForm" class="mt-3 p-3 border border-blue-200 rounded bg-blue-50 space-y-2 text-sm">
        <div>
          <label class="text-xs text-gray-500">전형요소 이름</label>
          <input v-model="newArea.name" type="text" class="w-full border rounded px-2 py-1 mt-0.5" />
        </div>
        <div>
          <label class="text-xs text-gray-500">만점(반영 비율)</label>
          <input v-model="newArea.max_score_display" type="number" step="0.00001"
            class="w-full border rounded px-2 py-1 mt-0.5" />
        </div>
        <div class="flex gap-2">
          <div class="flex-1">
            <label class="text-xs text-gray-500">점수 산출 방식</label>
            <select v-model="newArea.calc_type" class="w-full border rounded px-2 py-1 mt-0.5">
              <option value="NUMERIC">구간 조회</option>
              <option value="CATEGORY">범주 선택</option>
              <option value="MANUAL">수기 입력</option>
            </select>
          </div>
          <div class="flex-1">
            <label class="text-xs text-gray-500">데이터 조회 기준</label>
            <select v-model="newArea.lookup_scope" class="w-full border rounded px-2 py-1 mt-0.5">
              <option value="SIMPLE">기본 조회</option>
              <option value="COMPOSITE">대학별 환산점수 조회</option>
            </select>
          </div>
        </div>
        <div v-if="newArea.calc_type === 'NUMERIC'">
          <label class="text-xs text-gray-500">구간 탐색 방향 <span class="text-red-500">*</span></label>
          <select v-model="newArea.match_mode" class="w-full border rounded px-2 py-1 mt-0.5">
            <option value="">선택하세요</option>
            <option value="UPPER">▲ 기준값 이상(클수록 만점)</option>
            <option value="LOWER">▼ 기준값 이하(작을수록 만점)</option>
            <option value="EXACT">〓 정확히 일치</option>
          </select>
        </div>
        <div v-if="newArea.calc_type === 'CATEGORY'">
          <label class="text-xs text-gray-500">복수 활동 처리 방식 <span class="text-red-500">*</span></label>
          <select v-model="newArea.category_agg" class="w-full border rounded px-2 py-1 mt-0.5">
            <option value="">선택하세요</option>
            <option value="SUM">중복 선택 가능 (점수 합산)</option>
            <option value="MAX">최대 1개만 인정 (최고점 반영)</option>
          </select>
        </div>
        <div class="flex items-center gap-2">
          <input v-model="newArea.teacher_editable" type="checkbox" id="te" />
          <label for="te" class="text-xs text-gray-500">담임교사 입력 허용</label>
        </div>
        <p class="text-xs text-amber-700 bg-amber-50 border border-amber-200 rounded px-2 py-1 mt-2">
          전형요소 등록 후에는 이름과 담임교사 입력 허용 여부만 변경할 수 있습니다.
        </p>
        <div class="flex gap-1">
          <button class="px-2 py-1 bg-blue-600 text-white text-xs rounded" @click="addArea">저장</button>
          <button class="px-2 py-1 bg-gray-200 text-xs rounded" @click="showAddForm = false">취소</button>
        </div>
      </div>
    </div>

    <!-- ── 우측: 전형요소 상세 (서브탭) ── -->
    <div class="flex-1 min-w-0" v-if="selected">
      <div class="flex items-center justify-between mb-3">
        <div class="flex items-center gap-2 min-w-0">
          <h3 class="text-base font-semibold text-gray-800 truncate">{{ selected.name }}</h3>
          <span class="shrink-0 px-2 py-0.5 text-xs rounded bg-gray-100 text-gray-500">{{ calcTypeLabel(selected.calc_type) }}</span>
          <span class="shrink-0 px-2 py-0.5 text-xs rounded bg-gray-100 text-gray-500">{{ lookupScopeLabel(selected.lookup_scope) }}</span>
        </div>
        <button v-if="!showEditForm"
          class="shrink-0 ml-2 px-2 py-0.5 text-xs border border-gray-300 rounded text-gray-600 hover:bg-gray-50"
          @click="openEditForm">수정</button>
      </div>

      <!-- 기본 정보 카드 -->
      <div class="mb-4 p-3 bg-gray-50 border border-gray-200 rounded text-sm">
        <template v-if="!showEditForm">
          <div class="flex flex-wrap gap-x-6 gap-y-1 text-gray-700">
            <span><span class="text-gray-400 mr-1">만점</span>{{ displayScore(selected.max_score) }}점</span>
            <span><span class="text-gray-400 mr-1">조회 기준</span>{{ lookupScopeLabel(selected.lookup_scope) }}</span>
            <span><span class="text-gray-400 mr-1">계산 유형</span>{{ calcTypeLabel(selected.calc_type) }}</span>
            <span v-if="selected.calc_type === 'NUMERIC'"><span class="text-gray-400 mr-1">탐색 방향</span>{{ matchModeLabel(selected.match_mode) }}</span>
            <span v-if="selected.calc_type === 'CATEGORY'"><span class="text-gray-400 mr-1">범주 집계</span>{{ categoryAggLabel(selected.category_agg) }}</span>
            <span><span class="text-gray-400 mr-1">담임교사 입력</span>{{ selected.teacher_editable ? '허용' : '불가' }}</span>
          </div>
          <p class="text-xs text-gray-400 mt-2">전형요소 등록 후에는 이름과 담임교사 입력 허용 여부만 변경할 수 있습니다.</p>
        </template>
        <template v-else>
          <div class="space-y-2">
            <div>
              <label class="text-xs text-gray-500">전형요소 이름</label>
              <input v-model="editArea.name" type="text"
                class="w-full border rounded px-2 py-1 mt-0.5" />
            </div>
            <div class="flex items-center gap-2">
              <input v-model="editArea.teacher_editable" type="checkbox" id="edit-te" />
              <label for="edit-te" class="text-xs text-gray-500">담임교사 입력 허용</label>
            </div>
            <p v-if="editError" class="text-xs text-red-500">{{ editError }}</p>
            <div class="flex gap-1">
              <button class="px-2 py-1 bg-blue-600 text-white text-xs rounded hover:bg-blue-700"
                @click="saveEdit">저장</button>
              <button class="px-2 py-1 bg-gray-200 text-xs rounded hover:bg-gray-300"
                @click="cancelEdit">취소</button>
            </div>
          </div>
        </template>
      </div>

      <!-- 서브탭 -->
      <div class="flex border-b mb-4">
        <button v-if="selected.calc_type !== 'MANUAL'"
          class="px-4 py-2 text-sm font-medium border-b-2 transition-colors"
          :class="activeTab === 'score'
            ? 'border-blue-600 text-blue-600'
            : 'border-transparent text-gray-500 hover:text-gray-700'"
          @click="activeTab = 'score'">
          점수 기준
        </button>
        <button
          class="px-4 py-2 text-sm font-medium border-b-2 transition-colors"
          :class="activeTab === 'base'
            ? 'border-blue-600 text-blue-600'
            : 'border-transparent text-gray-500 hover:text-gray-700'"
          @click="activeTab = 'base'">
          기초 데이터
        </button>
      </div>

      <!-- 점수 기준 탭 -->
      <div v-if="activeTab === 'score'">
        <div class="mb-4 p-3 bg-blue-50 border border-blue-100 rounded text-xs">
          <p class="font-medium text-blue-700 mb-2">양식 예시 — {{ scoreEx.desc }}</p>
          <table class="border-collapse text-gray-700">
            <thead>
              <tr>
                <th v-for="h in scoreEx.headers" :key="h"
                    class="border border-blue-200 bg-blue-100 px-3 py-1 text-left font-medium whitespace-nowrap">{{ h }}</th>
                <th class="bg-transparent px-3 py-1"></th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="(row, i) in scoreEx.rows" :key="i">
                <td v-for="(cell, j) in row" :key="j"
                    class="border border-blue-100 px-3 py-1 whitespace-nowrap">{{ cell }}</td>
                <td class="pl-4 py-1 text-gray-400 whitespace-nowrap">{{ scoreEx.rowDescs[i] }}</td>
              </tr>
            </tbody>
          </table>
        </div>
        <p class="text-xs text-gray-500" :class="selected.lookup_scope === 'COMPOSITE' ? 'mb-1' : 'mb-3'">
          기준값·점수는 실제 값으로 작성 (예: 1.25, 30.5 / 소수점 최대 5자리)
        </p>
        <p v-if="selected.lookup_scope === 'COMPOSITE'" class="text-xs text-blue-600 mb-3">
          대학명·모집단위명을 비워두면 모든 대학에 공통 적용됩니다.
        </p>
        <ExcelPanel
          :area-id="selected.id"
          :calc-type="selected.calc_type"
          :area-name="selected.name"
          panel="score"
          @result="onScoreResult" />
        <ImportResultBox v-if="scoreResult" :result="scoreResult" class="mt-3" />

        <div class="mt-4 max-h-80 overflow-y-auto overflow-x-auto border border-gray-200 rounded">
          <p v-if="scorePage.rows.length === 0" class="text-gray-400 text-sm px-3 py-4 text-center">
            등록된 점수 기준 없음
          </p>
          <table v-else class="w-full min-w-max text-sm border-collapse">
            <thead class="sticky top-0 bg-gray-50">
              <tr>
                <th class="border-b border-gray-200 px-3 py-2 text-left text-gray-600 font-medium w-32">
                  {{ selected.calc_type === 'NUMERIC' ? '기준값' : '범주' }}
                </th>
                <th class="border-b border-gray-200 px-3 py-2 text-left text-gray-600 font-medium w-24">점수</th>
                <template v-if="selected.lookup_scope === 'COMPOSITE'">
                  <th class="border-b border-gray-200 px-3 py-2 text-left text-gray-600 font-medium w-40">대학명</th>
                  <th class="border-b border-gray-200 px-3 py-2 text-left text-gray-600 font-medium w-40">모집단위명</th>
                </template>
              </tr>
            </thead>
            <tbody>
              <tr v-for="(row, i) in scorePage.rows" :key="i"
                  :class="i % 2 === 1 ? 'bg-gray-50' : ''">
                <td class="border-b border-gray-100 px-3 py-1.5 text-gray-700">
                  {{ selected.calc_type === 'NUMERIC' ? row.threshold : row.category }}
                </td>
                <td class="border-b border-gray-100 px-3 py-1.5 text-gray-700">{{ row.score }}</td>
                <template v-if="selected.lookup_scope === 'COMPOSITE'">
                  <td class="border-b border-gray-100 px-3 py-1.5 text-gray-700">{{ row.univ_name }}</td>
                  <td class="border-b border-gray-100 px-3 py-1.5 text-gray-700">{{ row.track_name }}</td>
                </template>
              </tr>
            </tbody>
          </table>
        </div>
        <div v-if="scorePage.total > 0"
             class="mt-2 flex items-center justify-center gap-3 text-sm text-gray-600">
          <button
            class="px-2 py-1 border border-gray-300 rounded hover:bg-gray-100 disabled:opacity-40 disabled:cursor-not-allowed"
            :disabled="scorePage.page <= 1"
            @click="loadScoreRows(scorePage.page - 1)">
            &lt; 이전
          </button>
          <span>
            {{ scorePage.page }} / {{ Math.ceil(scorePage.total / scorePage.per_page) }} 페이지
            (총 {{ scorePage.total }}행)
          </span>
          <button
            class="px-2 py-1 border border-gray-300 rounded hover:bg-gray-100 disabled:opacity-40 disabled:cursor-not-allowed"
            :disabled="scorePage.page >= Math.ceil(scorePage.total / scorePage.per_page)"
            @click="loadScoreRows(scorePage.page + 1)">
            다음 &gt;
          </button>
        </div>
      </div>

      <!-- 기초 데이터 탭 -->
      <div v-if="activeTab === 'base'">
        <div class="mb-4 p-3 bg-blue-50 border border-blue-100 rounded text-xs">
          <p class="font-medium text-blue-700 mb-2">양식 예시 — {{ baseEx.desc }}</p>
          <table class="border-collapse text-gray-700">
            <thead>
              <tr>
                <th v-for="h in baseEx.headers" :key="h"
                    class="border border-blue-200 bg-blue-100 px-3 py-1 text-left font-medium whitespace-nowrap">{{ h }}</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="(row, i) in baseEx.rows" :key="i">
                <td v-for="(cell, j) in row" :key="j"
                    class="border border-blue-100 px-3 py-1 whitespace-nowrap">{{ cell }}</td>
              </tr>
            </tbody>
          </table>
        </div>
        <p class="text-xs text-gray-500 mb-3">
          <template v-if="baseStudentType === 'enrolled'">학년·반·번호로 재학생을 찾아 값을 등록합니다.</template>
          <template v-else>학생코드로 졸업생을 찾아 값을 등록합니다.</template>
          수치형·수기 입력은 소수점 최대 5자리.
        </p>
        <ExcelPanel
          :area-id="selected.id"
          :calc-type="selected.calc_type"
          :area-name="selected.name"
          panel="base"
          v-model:studentType="baseStudentType"
          @result="onBaseResult" />

        <!-- 외부 프로그램 가져오기 (COMPOSITE 전용, 재학생일 때만 표시) -->
        <div v-if="selected.lookup_scope === 'COMPOSITE'"
             v-show="baseStudentType === 'enrolled'"
             class="mt-2 flex flex-wrap gap-2">
          <label class="px-3 py-1.5 text-sm border border-gray-300 rounded cursor-pointer hover:bg-gray-50 text-gray-700">
            대교협 석차연명부
            <input type="file" accept=".xlsx" class="hidden" @change="onExternalFile('daegyo', $event)" />
          </label>
          <label class="px-3 py-1.5 text-sm border border-gray-300 rounded cursor-pointer hover:bg-gray-50 text-gray-700">
            유니브 석차연명부
            <input type="file" accept=".xls" class="hidden" @change="onExternalFile('univ', $event)" />
          </label>
        </div>

        <ImportResultBox v-if="baseResult" :result="baseResult" class="mt-3" />

        <div class="mt-4 max-h-80 overflow-y-auto overflow-x-auto border border-gray-200 rounded">
          <p v-if="basePage.rows.length === 0" class="text-gray-400 text-sm px-3 py-4 text-center">
            등록된 기초 데이터 없음
          </p>
          <table v-else class="w-full min-w-max text-sm border-collapse">
            <thead class="sticky top-0 bg-gray-50">
              <tr>
                <th class="border-b border-gray-200 px-3 py-2 text-left text-gray-600 font-medium w-36">학생코드</th>
                <th class="border-b border-gray-200 px-3 py-2 text-left text-gray-600 font-medium w-24">이름</th>
                <th class="border-b border-gray-200 px-3 py-2 text-left text-gray-600 font-medium w-24">값</th>
                <template v-if="selected.lookup_scope === 'COMPOSITE'">
                  <th class="border-b border-gray-200 px-3 py-2 text-left text-gray-600 font-medium w-40">대학명</th>
                  <th class="border-b border-gray-200 px-3 py-2 text-left text-gray-600 font-medium w-40">모집단위명</th>
                </template>
              </tr>
            </thead>
            <tbody>
              <tr v-for="(row, i) in basePage.rows" :key="i"
                  :class="i % 2 === 1 ? 'bg-gray-50' : ''">
                <td class="border-b border-gray-100 px-3 py-1.5 text-gray-700">{{ row.student_code }}</td>
                <td class="border-b border-gray-100 px-3 py-1.5 text-gray-700">{{ row.name }}</td>
                <td class="border-b border-gray-100 px-3 py-1.5 text-gray-700">{{ row.value }}</td>
                <template v-if="selected.lookup_scope === 'COMPOSITE'">
                  <td class="border-b border-gray-100 px-3 py-1.5 text-gray-700">{{ row.univ_name }}</td>
                  <td class="border-b border-gray-100 px-3 py-1.5 text-gray-700">{{ row.track_name }}</td>
                </template>
              </tr>
            </tbody>
          </table>
        </div>
        <div v-if="basePage.total > 0"
             class="mt-2 flex items-center justify-center gap-3 text-sm text-gray-600">
          <button
            class="px-2 py-1 border border-gray-300 rounded hover:bg-gray-100 disabled:opacity-40 disabled:cursor-not-allowed"
            :disabled="basePage.page <= 1"
            @click="loadBaseRows(basePage.page - 1)">
            &lt; 이전
          </button>
          <span>
            {{ basePage.page }} / {{ Math.ceil(basePage.total / basePage.per_page) }} 페이지
            (총 {{ basePage.total }}행)
          </span>
          <button
            class="px-2 py-1 border border-gray-300 rounded hover:bg-gray-100 disabled:opacity-40 disabled:cursor-not-allowed"
            :disabled="basePage.page >= Math.ceil(basePage.total / basePage.per_page)"
            @click="loadBaseRows(basePage.page + 1)">
            다음 &gt;
          </button>
        </div>
      </div>
    </div>

    <p v-else class="text-gray-400 text-sm mt-2">왼쪽에서 전형요소를 선택하세요.</p>
  </div>

  <!-- 외부 가져오기 모달 -->
  <Teleport to="body">
    <div v-if="extModal.open"
         class="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
      <div class="bg-white rounded-lg shadow-xl w-[520px] max-h-[90vh] overflow-y-auto p-6">
        <h3 class="text-base font-semibold text-gray-800 mb-1">{{ extModal.title }}</h3>
        <p class="text-xs text-gray-400 mb-4 truncate">{{ extModal.file?.name }}</p>

        <div class="space-y-3 mb-4">
          <div>
            <label class="text-xs text-gray-500">대학명 <span class="text-red-500">*</span></label>
            <input v-model="extModal.univName" type="text"
                   class="w-full border rounded px-2 py-1.5 mt-0.5 text-sm"
                   placeholder="예: 서울대학교" />
          </div>
          <div>
            <label class="text-xs text-gray-500">모집단위명 <span class="text-red-500">*</span></label>
            <input v-model="extModal.trackName" type="text"
                   class="w-full border rounded px-2 py-1.5 mt-0.5 text-sm"
                   placeholder="예: 자연계열" />
          </div>
        </div>

        <div class="mb-4">
          <p class="text-xs text-gray-500 mb-1">
            미리보기 (상위 {{ extModal.preview.length }}행 / 총 {{ extModal.total }}행)
          </p>
          <div class="overflow-x-auto border border-gray-200 rounded">
            <table class="text-xs w-full border-collapse">
              <thead>
                <tr class="bg-gray-50">
                  <th v-for="h in ['학년','반','번호','이름', extModal.valueHeader]" :key="h"
                      class="border-b border-gray-200 px-2 py-1.5 text-left font-medium text-gray-600 whitespace-nowrap">
                    {{ h }}
                  </th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="(row, i) in extModal.preview" :key="i"
                    :class="i % 2 === 1 ? 'bg-gray-50' : ''">
                  <td v-for="(cell, j) in row" :key="j"
                      class="border-b border-gray-100 px-2 py-1.5 text-gray-700 whitespace-nowrap">
                    {{ cell }}
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>

        <p v-if="extModal.error" class="text-red-500 text-sm mb-3">{{ extModal.error }}</p>

        <div class="flex justify-end gap-2">
          <button class="px-3 py-1.5 text-sm border border-gray-300 rounded hover:bg-gray-50"
                  @click="closeExtModal">취소</button>
          <button class="px-3 py-1.5 text-sm bg-blue-600 text-white rounded hover:bg-blue-700 disabled:opacity-50"
                  :disabled="extModal.importing || !extModal.univName.trim() || !extModal.trackName.trim()"
                  @click="doExtImport">
            {{ extModal.importing ? '가져오는 중…' : '가져오기' }}
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<script setup>
import { ref, watch, onMounted, defineComponent, h, computed } from 'vue'
import {
  getAreas, createArea, updateArea, deleteArea,
  downloadNumericTableTemplate, exportNumericTable, importNumericTable,
  downloadCategoryMapTemplate, exportCategoryMap, importCategoryMap,
  downloadBaseDataTemplate, exportBaseData, importBaseData,
  getNumericTableList, getCategoryMapList, getBaseDataList,
  previewDaegyoImport, importDaegyo, previewUnivImport, importUniv,
} from '../../api/admin.js'
import { getScoreExample, getBaseExample } from '../../data/areaSamples.js'

// ── 상태 ──────────────────────────────────────────────────────
const areas    = ref([])
const selected = ref(null)
const error    = ref('')
const activeTab   = ref('score')
const scoreResult = ref(null)
const baseResult  = ref(null)
const scorePage   = ref({ rows: [], total: 0, page: 1, per_page: 50 })
const basePage    = ref({ rows: [], total: 0, page: 1, per_page: 50 })

const showAddForm = ref(false)
const newArea = ref(defaultNewArea())

const showEditForm = ref(false)
const editArea = ref({ name: '', teacher_editable: false })
const editError = ref('')

function defaultNewArea() {
  return { name: '', max_score_display: '', calc_type: 'NUMERIC',
           lookup_scope: 'SIMPLE', teacher_editable: true,
           match_mode: '', category_agg: '' }
}


const CALC_TYPE_LABELS    = { NUMERIC: '구간 조회', CATEGORY: '범주 선택', MANUAL: '수기 입력' }
const LOOKUP_SCOPE_LABELS = { SIMPLE: '기본 조회', COMPOSITE: '대학별 환산점수 조회' }
const MATCH_MODE_LABELS   = { UPPER: '▲ 이상(클수록 만점)', LOWER: '▼ 이하(작을수록 만점)', EXACT: '정확히 일치' }
const CATEGORY_AGG_LABELS = { SUM: '중복 선택 가능(합산)', MAX: '최대 1개 선택(최고점)' }
function calcTypeLabel(v)    { return CALC_TYPE_LABELS[v]    ?? v }
function lookupScopeLabel(v) { return LOOKUP_SCOPE_LABELS[v] ?? v }
function matchModeLabel(v)   { return v ? (MATCH_MODE_LABELS[v]   ?? v) : '—' }
function categoryAggLabel(v) { return v ? (CATEGORY_AGG_LABELS[v] ?? v) : '—' }
function displayScore(v) {
  return v % 1 === 0 ? String(v) : v.toFixed(5).replace(/\.?0+$/, '')
}

const totalMaxScore = computed(() => areas.value.reduce((sum, a) => sum + a.max_score, 0))

const baseStudentType = ref('enrolled')
watch(baseStudentType, () => loadBaseRows(1))

const scoreEx = computed(() => selected.value ? getScoreExample(selected.value) : null)
const baseEx  = computed(() => selected.value ? getBaseExample(selected.value, baseStudentType.value) : null)

// ── 전형요소 목록 ─────────────────────────────────────────────────
async function load() {
  try { areas.value = await getAreas() }
  catch (e) { error.value = e.response?.data ?? e.message }
}

function selectArea(area) {
  selected.value = area
  activeTab.value = area.calc_type === 'MANUAL' ? 'base' : 'score'
  showEditForm.value = false

  scoreResult.value = null
  baseResult.value  = null
  loadScoreRows(1)
  loadBaseRows(1)
}

async function loadScoreRows(page = 1) {
  const area = selected.value
  const empty = { rows: [], total: 0, page: 1, per_page: 50 }
  if (!area || area.calc_type === 'MANUAL') { scorePage.value = empty; return }
  try {
    const data = area.calc_type === 'CATEGORY'
      ? await getCategoryMapList(area.id, page, scorePage.value.per_page)
      : await getNumericTableList(area.id, page, scorePage.value.per_page)
    scorePage.value = data
  } catch { scorePage.value = empty }
}

async function loadBaseRows(page = 1) {
  if (!selected.value) { basePage.value = { rows: [], total: 0, page: 1, per_page: 50 }; return }
  try {
    const data = await getBaseDataList(selected.value.id, page, basePage.value.per_page, baseStudentType.value)
    basePage.value = data
  } catch { basePage.value = { rows: [], total: 0, page: 1, per_page: 50 } }
}

function onScoreResult(evt) { scoreResult.value = evt; loadScoreRows(1) }
function onBaseResult(evt)  { baseResult.value = evt;  loadBaseRows(1)  }

async function addArea() {
  const maxScore = parseFloat(String(newArea.value.max_score_display).trim())
  if (isNaN(maxScore) || maxScore < 0) {
    error.value = '만점: 0 이상의 숫자를 입력하세요'
    return
  }
  const body = {
    name: newArea.value.name,
    max_score: maxScore,
    calc_type: newArea.value.calc_type,
    lookup_scope: newArea.value.lookup_scope,
    teacher_editable: newArea.value.teacher_editable,
    match_mode: newArea.value.match_mode || null,
    category_agg: newArea.value.category_agg || null,
  }
  try {
    await createArea(body)
    showAddForm.value = false
    await load()
  } catch (e) { error.value = e.response?.data ?? e.message }
}

async function removeArea(id) {
  if (!confirm('전형요소를 삭제하면 구간표·범주표·기초데이터도 함께 삭제됩니다. 계속할까요?')) return
  try {
    await deleteArea(id)
    if (selected.value?.id === id) selected.value = null
    await load()
  } catch (e) { error.value = e.response?.data ?? e.message }
}

function openEditForm() {
  editArea.value = {
    name: selected.value.name,
    teacher_editable: selected.value.teacher_editable,
  }
  editError.value = ''
  showEditForm.value = true
}

function cancelEdit() {
  showEditForm.value = false
}

async function saveEdit() {
  editError.value = ''
  const body = {
    name: editArea.value.name,
    teacher_editable: editArea.value.teacher_editable,
  }
  try {
    await updateArea(selected.value.id, body)
    const prevId = selected.value.id
    await load()
    selected.value = areas.value.find(a => a.id === prevId) ?? null
    showEditForm.value = false
  } catch (e) {
    editError.value = e.response?.data ?? e.message
  }
}

function openAddForm() {
  newArea.value = defaultNewArea()
  showAddForm.value = true
}

// ── 외부 가져오기 모달 ────────────────────────────────────────────
const extModal = ref({
  open: false, format: '', title: '', file: null,
  univName: '', trackName: '', valueHeader: '',
  preview: [], total: 0, importing: false, error: '',
})

async function onExternalFile(format, evt) {
  const file = evt.target.files?.[0]
  evt.target.value = ''
  if (!file) return
  try {
    const data = format === 'daegyo'
      ? await previewDaegyoImport(selected.value.id, file)
      : await previewUnivImport(selected.value.id, file)
    extModal.value = {
      open: true,
      format,
      title: format === 'daegyo' ? '대교협 석차연명부 가져오기' : '유니브 석차연명부 가져오기',
      file,
      univName: data.univ_name,
      trackName: '',
      valueHeader: data.value_header,
      preview: data.preview,
      total: data.total,
      importing: false,
      error: '',
    }
  } catch (e) {
    alert(e.response?.data ?? e.message ?? '파일 파싱 오류')
  }
}

function closeExtModal() {
  extModal.value.open = false
}

async function doExtImport() {
  const m = extModal.value
  if (!m.trackName.trim()) return
  m.importing = true
  m.error = ''
  try {
    const res = m.format === 'daegyo'
      ? await importDaegyo(selected.value.id, m.file, m.univName, m.trackName)
      : await importUniv(selected.value.id, m.file, m.univName, m.trackName)
    closeExtModal()
    onBaseResult(res.data)
  } catch (e) {
    const d = e.response?.data
    if (d != null && typeof d === 'object' && Array.isArray(d.errors)) {
      closeExtModal()
      onBaseResult(d)
    } else {
      m.error = typeof d === 'string' ? d : (e.message ?? '오류가 발생했습니다')
    }
  } finally {
    m.importing = false
  }
}

onMounted(load)

// ── 다운로드 헬퍼 ─────────────────────────────────────────────
function saveBlob(response, filename) {
  const url = URL.createObjectURL(new Blob([response.data]))
  const a = document.createElement('a')
  a.href = url; a.download = filename; a.click()
  URL.revokeObjectURL(url)
}

// ── ExcelPanel (인라인 컴포넌트) ──────────────────────────────
const ExcelPanel = defineComponent({
  props: {
    areaId:      { type: Number, required: true },
    calcType:    { type: String, required: true },
    areaName:    { type: String, required: true },
    panel:       { type: String, required: true }, // 'score' | 'base'
    studentType: { type: String, default: 'enrolled' },
  },
  emits: ['result', 'update:studentType'],
  setup(props, { emit }) {
    const err = ref('')
    const uploading = ref(false)
    const downloading = ref(false)

    async function dlTemplate() {
      err.value = ''
      downloading.value = true
      try {
        if (props.panel === 'score') {
          const res = props.calcType === 'CATEGORY'
            ? await downloadCategoryMapTemplate(props.areaId)
            : await downloadNumericTableTemplate(props.areaId)
          saveBlob(res, props.calcType === 'CATEGORY'
            ? `${props.areaName}_category_map_template.xlsx`
            : `${props.areaName}_numeric_table_template.xlsx`)
        } else {
          const res = await downloadBaseDataTemplate(props.areaId, props.studentType)
          saveBlob(res, `${props.areaName}_base_data_${props.studentType}_template.xlsx`)
        }
      } catch (e) { err.value = e.response?.data ?? e.message }
      finally { downloading.value = false }
    }

    async function dlExport() {
      err.value = ''
      downloading.value = true
      try {
        if (props.panel === 'score') {
          const res = props.calcType === 'CATEGORY'
            ? await exportCategoryMap(props.areaId)
            : await exportNumericTable(props.areaId)
          saveBlob(res, props.calcType === 'CATEGORY'
            ? `${props.areaName}_category_map.xlsx`
            : `${props.areaName}_numeric_table.xlsx`)
        } else {
          const res = await exportBaseData(props.areaId)
          saveBlob(res, `${props.areaName}_base_data.xlsx`)
        }
      } catch (e) { err.value = e.response?.data ?? e.message }
      finally { downloading.value = false }
    }

    async function onFile(evt) {
      const file = evt.target.files?.[0]
      if (!file) return
      err.value = ''
      uploading.value = true
      try {
        let result
        if (props.panel === 'score') {
          result = props.calcType === 'CATEGORY'
            ? await importCategoryMap(props.areaId, file)
            : await importNumericTable(props.areaId, file)
        } else {
          result = await importBaseData(props.areaId, file, props.studentType)
        }
        emit('result', result)
      } catch (e) {
        const d = e.response?.data
        if (d != null && typeof d === 'object' && Array.isArray(d.errors)) {
          emit('result', d)
        } else {
          err.value = typeof d === 'string' ? d : (e.message ?? '오류가 발생했습니다')
        }
      }
      finally { uploading.value = false; evt.target.value = '' }
    }

    const btnBase = 'px-3 py-1.5 border border-gray-300 text-gray-700 text-sm rounded hover:bg-gray-50 disabled:opacity-40'

    return () => h('div', { class: 'space-y-1' }, [
      h('div', { class: 'flex flex-wrap gap-2 items-center' }, [

        // ── 기초 데이터: 재학생/졸업생 라디오 + 양식 다운로드 + 불러오기
        ...(props.panel === 'base' ? [
          h('label', { class: 'flex items-center gap-1 text-sm cursor-pointer' }, [
            h('input', {
              type: 'radio',
              name: `st-${props.areaId}`,
              checked: props.studentType === 'enrolled',
              onChange: () => emit('update:studentType', 'enrolled'),
            }),
            '재학생',
          ]),
          h('label', { class: 'flex items-center gap-1 text-sm cursor-pointer' }, [
            h('input', {
              type: 'radio',
              name: `st-${props.areaId}`,
              checked: props.studentType === 'graduated',
              onChange: () => emit('update:studentType', 'graduated'),
            }),
            '졸업생',
          ]),
        ] : []),

        // ── 양식 다운로드 (score 패널은 기존 그대로)
        h('button', { class: btnBase, disabled: downloading.value, onClick: dlTemplate }, '양식 다운로드'),

        // ── 불러오기
        h('label', {
          class: `px-3 py-1.5 text-sm rounded cursor-pointer ${uploading.value ? 'bg-gray-400 text-white' : 'bg-blue-600 text-white hover:bg-blue-700'}`,
        }, [
          uploading.value ? '가져오는 중…' : '불러오기',
          h('input', { type: 'file', accept: '.xlsx,.csv', class: 'hidden', onChange: onFile }),
        ]),

        // ── 구분선 + 전체 목록 다운로드
        h('span', { class: 'text-gray-300 select-none' }, '|'),
        h('button', { class: btnBase, disabled: downloading.value, onClick: dlExport }, '전체 목록 다운로드'),
      ]),
      h('p', { class: 'text-xs text-amber-600' },
        props.panel === 'base'
          ? `※ 불러오기 시 ${props.studentType === 'enrolled' ? '재학생' : '졸업생'} 기존 기초데이터가 모두 교체됩니다.`
          : '※ 불러오기 시 기존 점수 기준이 모두 교체됩니다.'),
      err.value ? h('p', { class: 'text-red-500 text-sm' }, err.value) : null,
    ])
  },
})

// ── ImportResultBox (인라인 컴포넌트) ─────────────────────────
const ImportResultBox = defineComponent({
  props: { result: Object },
  setup(props) {
    return () => {
      const r = props.result
      const hasErrors = r.errors?.length > 0
      const hasWarnings = r.warnings?.length > 0
      const borderCls = hasErrors ? 'border-yellow-400 bg-yellow-50' : 'border-green-400 bg-green-50'
      const countStr = r.rows != null ? `${r.rows}건` : r.inserted != null ? `신규 ${r.inserted}명, 수정 ${r.updated}명` : ''
      return h('div', {
        class: `p-3 rounded border text-sm ${borderCls}`,
      }, [
        h('p', { class: 'font-medium mb-1' },
          hasErrors ? '오류 발생 — 가져오기 실패' : `완료 — ${countStr} 처리됨`),
        hasWarnings
          ? h('ul', { class: 'list-disc list-inside text-green-700 text-xs mb-1' },
              r.warnings.map((w, i) => h('li', { key: i }, w)))
          : null,
        hasErrors
          ? h('ul', { class: 'list-disc list-inside text-yellow-700 text-xs' },
              r.errors.map((e, i) => h('li', { key: i }, e)))
          : null,
      ])
    }
  },
})
</script>
