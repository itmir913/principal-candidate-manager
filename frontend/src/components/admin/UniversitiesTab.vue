<template>
  <div class="flex gap-6" style="min-height: 480px">

    <!-- ── 좌측: 대학 목록 ───────────────────────────────────── -->
    <div class="w-72 flex-shrink-0 flex flex-col">
      <div class="flex items-center justify-between mb-3">
        <h2 class="text-sm font-semibold text-gray-700">대학 목록</h2>
        <button
          class="px-2.5 py-1 bg-blue-600 text-white text-xs rounded hover:bg-blue-700 disabled:opacity-40"
          :disabled="saving"
          @click="startAddUniv"
        >+ 대학 추가</button>
      </div>

      <p v-if="error" class="text-red-500 text-xs mb-2">{{ error }}</p>

      <!-- 추가 폼 -->
      <div v-if="addingUniv" class="mb-2 border border-blue-200 rounded-lg p-3 bg-blue-50">
        <div class="space-y-2">
          <div>
            <label class="text-xs text-gray-500 block mb-0.5">대학명</label>
            <input v-model="univForm.univ_name" type="text"
              class="w-full border rounded px-2 py-1 text-sm" placeholder="예) 한국대학교" />
          </div>
          <div>
            <label class="text-xs text-gray-500 block mb-0.5">전체 정원</label>
            <QuotaInput v-model:unlimited="univForm.unlimited" v-model:quota="univForm.total_quota" />
          </div>
          <div class="flex items-center gap-2">
            <input v-model="univForm.prioritize_enrolled" type="checkbox" id="add-univ-pe" />
            <label for="add-univ-pe" class="text-xs text-gray-600">재학생 우선</label>
          </div>
        </div>
        <div class="flex gap-2 mt-3">
          <button
            class="px-2.5 py-1 bg-blue-600 text-white text-xs rounded disabled:opacity-40"
            :disabled="saving || !univForm.univ_name.trim()"
            @click="saveAddUniv"
          >{{ saving ? '저장 중…' : '저장' }}</button>
          <button
            class="px-2.5 py-1 bg-gray-200 text-xs rounded"
            :disabled="saving"
            @click="addingUniv = false"
          >취소</button>
        </div>
      </div>

      <!-- 대학 카드 목록 -->
      <div class="overflow-y-auto flex-1 space-y-2 pr-1">
        <div
          v-for="u in univs"
          :key="u.id"
          class="border rounded-lg transition-colors"
          :class="selectedUnivId === u.id
            ? 'border-blue-400 bg-blue-50'
            : 'bg-white border-gray-200 hover:border-gray-300'"
        >
          <!-- 보기 모드 -->
          <template v-if="editingUnivId !== u.id">
            <div
              class="px-3 py-2.5 cursor-pointer"
              @click="selectUniv(u.id)"
            >
              <div class="text-sm font-medium text-gray-800">{{ u.univ_name }}</div>
              <div class="text-xs text-gray-500 mt-0.5">
                전체 정원: <span class="font-medium">{{ u.total_quota != null ? u.total_quota + '명' : '무제한' }}</span>
                &nbsp;·&nbsp;재학생 우선: {{ u.prioritize_enrolled ? '○' : '-' }}
              </div>
            </div>
            <div class="px-3 pb-2 flex gap-3">
              <button
                class="text-blue-500 text-xs hover:underline disabled:opacity-40"
                :disabled="saving"
                @click.stop="startEditUniv(u)"
              >편집</button>
              <button
                class="text-red-400 text-xs hover:underline disabled:opacity-40"
                :disabled="saving"
                @click.stop="removeUniv(u.id)"
              >삭제</button>
            </div>
          </template>

          <!-- 편집 모드 -->
          <template v-else>
            <div class="px-3 py-2.5 bg-yellow-50 rounded-lg">
              <div class="space-y-2">
                <div>
                  <label class="text-xs text-gray-500 block mb-0.5">대학명</label>
                  <input v-model="univForm.univ_name" type="text"
                    class="w-full border rounded px-2 py-1 text-sm" />
                </div>
                <div>
                  <label class="text-xs text-gray-500 block mb-0.5">전체 정원</label>
                  <QuotaInput v-model:unlimited="univForm.unlimited" v-model:quota="univForm.total_quota" />
                </div>
                <div class="flex items-center gap-2">
                  <input v-model="univForm.prioritize_enrolled" type="checkbox" :id="`edit-univ-pe-${u.id}`" />
                  <label :for="`edit-univ-pe-${u.id}`" class="text-xs text-gray-600">재학생 우선</label>
                </div>
              </div>
              <div class="flex gap-2 mt-3">
                <button
                  class="px-2.5 py-1 bg-blue-600 text-white text-xs rounded disabled:opacity-40"
                  :disabled="saving || !univForm.univ_name.trim()"
                  @click="saveEditUniv(u.id)"
                >{{ saving ? '저장 중…' : '저장' }}</button>
                <button
                  class="px-2.5 py-1 bg-gray-200 text-xs rounded"
                  :disabled="saving"
                  @click="editingUnivId = null"
                >취소</button>
              </div>
            </div>
          </template>
        </div>

        <div v-if="univs.length === 0 && !addingUniv"
          class="text-center text-gray-400 text-sm py-8">
          등록된 대학이 없습니다.
        </div>
      </div>
    </div>

    <!-- ── 우측: 모집단위 목록 ───────────────────────────────── -->
    <div class="flex-1 min-w-0">
      <div v-if="!selectedUniv"
        class="flex items-center justify-center h-full text-gray-400 text-sm">
        왼쪽에서 대학을 선택하면 모집단위를 관리할 수 있습니다.
      </div>

      <template v-else>
        <div class="flex items-center justify-between mb-3">
          <div>
            <h2 class="text-sm font-semibold text-gray-700">
              {{ selectedUniv.univ_name }} — 모집단위
            </h2>
            <p class="text-xs text-gray-400 mt-0.5">
              대학 전체 정원:
              <span class="font-medium text-gray-600">
                {{ selectedUniv.total_quota != null ? selectedUniv.total_quota + '명' : '무제한' }}
              </span>
            </p>
          </div>
          <button
            class="px-2.5 py-1 bg-blue-600 text-white text-xs rounded hover:bg-blue-700 disabled:opacity-40"
            :disabled="saving"
            @click="startAddTrack"
          >+ 모집단위 추가</button>
        </div>

        <table class="w-full text-sm border-collapse">
          <thead>
            <tr class="bg-gray-100 text-gray-600 text-left">
              <th class="px-3 py-2 border-b">모집단위명</th>
              <th class="px-3 py-2 border-b">제한 인원</th>
              <th class="px-3 py-2 border-b text-center">재학생 우선</th>
              <th class="px-3 py-2 border-b w-28"></th>
            </tr>
          </thead>
          <tbody>
            <!-- 추가 행 -->
            <tr v-if="addingTrack" class="bg-blue-50">
              <td class="px-2 py-1.5 border-b">
                <input v-model="trackForm.track_name" type="text"
                  class="w-full border rounded px-2 py-0.5 text-sm" placeholder="예) 자연계" />
              </td>
              <td class="px-2 py-1.5 border-b">
                <QuotaInput v-model:unlimited="trackForm.unlimited" v-model:quota="trackForm.unit_quota" />
              </td>
              <td class="px-2 py-1.5 border-b text-center">
                <input v-model="trackForm.prioritize_enrolled" type="checkbox" />
              </td>
              <td class="px-2 py-1.5 border-b">
                <button
                  class="px-2 py-0.5 bg-blue-600 text-white text-xs rounded mr-1 disabled:opacity-40"
                  :disabled="saving || !trackForm.track_name.trim()"
                  @click="saveAddTrack"
                >{{ saving ? '저장 중…' : '저장' }}</button>
                <button
                  class="px-2 py-0.5 bg-gray-200 text-xs rounded"
                  :disabled="saving"
                  @click="addingTrack = false"
                >취소</button>
              </td>
            </tr>

            <template v-for="t in tracks" :key="t.id">
              <!-- 보기 행 -->
              <tr v-if="editingTrackId !== t.id" class="hover:bg-gray-50">
                <td class="px-3 py-2 border-b">{{ t.track_name }}</td>
                <td class="px-3 py-2 border-b">
                  {{ t.unit_quota != null ? t.unit_quota + '명' : '무제한' }}
                </td>
                <td class="px-3 py-2 border-b text-center">
                  {{ t.prioritize_enrolled ? '○' : '-' }}
                </td>
                <td class="px-3 py-2 border-b">
                  <button
                    class="text-blue-500 text-xs mr-2 hover:underline disabled:opacity-40"
                    :disabled="saving"
                    @click="startEditTrack(t)"
                  >편집</button>
                  <button
                    class="text-red-400 text-xs hover:underline disabled:opacity-40"
                    :disabled="saving"
                    @click="removeTrack(t.id)"
                  >삭제</button>
                </td>
              </tr>
              <!-- 편집 행 -->
              <tr v-else class="bg-yellow-50">
                <td class="px-2 py-1.5 border-b">
                  <input v-model="trackForm.track_name" type="text"
                    class="w-full border rounded px-2 py-0.5 text-sm" />
                </td>
                <td class="px-2 py-1.5 border-b">
                  <QuotaInput v-model:unlimited="trackForm.unlimited" v-model:quota="trackForm.unit_quota" />
                </td>
                <td class="px-2 py-1.5 border-b text-center">
                  <input v-model="trackForm.prioritize_enrolled" type="checkbox" />
                </td>
                <td class="px-2 py-1.5 border-b">
                  <button
                    class="px-2 py-0.5 bg-blue-600 text-white text-xs rounded mr-1 disabled:opacity-40"
                    :disabled="saving || !trackForm.track_name.trim()"
                    @click="saveEditTrack(t.id)"
                  >{{ saving ? '저장 중…' : '저장' }}</button>
                  <button
                    class="px-2 py-0.5 bg-gray-200 text-xs rounded"
                    :disabled="saving"
                    @click="editingTrackId = null"
                  >취소</button>
                </td>
              </tr>
            </template>

            <tr v-if="tracks.length === 0 && !addingTrack">
              <td colspan="4" class="px-3 py-6 text-center text-gray-400">
                등록된 모집단위가 없습니다.
              </td>
            </tr>
          </tbody>
        </table>
      </template>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, defineComponent, h } from 'vue'
import {
  getUniversities, createUniversity, updateUniversity, deleteUniversity,
  getUnivTracks, createTrack, updateTrack, deleteTrack,
} from '../../api/admin.js'

// ── 정원 입력 서브컴포넌트 (인라인 정의) ──────────────────────
const QuotaInput = defineComponent({
  props: {
    unlimited: Boolean,
    quota: { type: Number, default: 1 },
  },
  emits: ['update:unlimited', 'update:quota'],
  setup(props, { emit }) {
    return () => h('div', { class: 'flex items-center gap-2' }, [
      h('input', {
        type: 'checkbox',
        checked: props.unlimited,
        onChange: (e) => emit('update:unlimited', e.target.checked),
      }),
      h('span', { class: 'text-xs text-gray-600 select-none' }, '무제한'),
      !props.unlimited
        ? h('div', { class: 'flex items-center gap-1' }, [
            h('input', {
              type: 'number',
              value: props.quota,
              min: 1,
              class: 'w-16 border rounded px-2 py-0.5 text-sm',
              onInput: (e) => emit('update:quota', parseInt(e.target.value) || 1),
            }),
            h('span', { class: 'text-xs text-gray-500' }, '명'),
          ])
        : null,
    ])
  },
})

// ── 상태 ──────────────────────────────────────────────────────
const univs        = ref([])
const tracks       = ref([])
const error        = ref('')
const saving       = ref(false)

const selectedUnivId  = ref(null)
const addingUniv      = ref(false)
const editingUnivId   = ref(null)
const addingTrack     = ref(false)
const editingTrackId  = ref(null)

const univForm  = ref(emptyUnivForm())
const trackForm = ref(emptyTrackForm())

const selectedUniv = computed(() => univs.value.find(u => u.id === selectedUnivId.value) ?? null)

// ── 폼 초기값 ─────────────────────────────────────────────────
function emptyUnivForm() {
  return { univ_name: '', unlimited: true, total_quota: 1, prioritize_enrolled: true }
}

function emptyTrackForm() {
  return { track_name: '', unlimited: true, unit_quota: 1, prioritize_enrolled: false }
}

function univToForm(u) {
  return {
    univ_name: u.univ_name,
    unlimited: u.total_quota == null,
    total_quota: u.total_quota ?? 1,
    prioritize_enrolled: u.prioritize_enrolled === 1,
  }
}

function trackToForm(t) {
  return {
    track_name: t.track_name,
    unlimited: t.unit_quota == null,
    unit_quota: t.unit_quota ?? 1,
    prioritize_enrolled: t.prioritize_enrolled === 1,
  }
}

function univFormToBody(f) {
  return {
    univ_name: f.univ_name.trim(),
    total_quota: f.unlimited ? null : f.total_quota,
    prioritize_enrolled: f.prioritize_enrolled,
  }
}

function trackFormToBody(f) {
  return {
    track_name: f.track_name.trim(),
    unit_quota: f.unlimited ? null : f.unit_quota,
    prioritize_enrolled: f.prioritize_enrolled,
  }
}

// ── 로드 ──────────────────────────────────────────────────────
async function loadUnivs() {
  try {
    univs.value = await getUniversities()
  } catch (e) {
    error.value = e.response?.data ?? e.message
  }
}

async function loadTracks(univId) {
  try {
    tracks.value = await getUnivTracks(univId)
  } catch (e) {
    error.value = e.response?.data ?? e.message
  }
}

// ── 대학 선택 ─────────────────────────────────────────────────
function selectUniv(id) {
  if (selectedUnivId.value === id) return
  selectedUnivId.value = id
  editingTrackId.value = null
  addingTrack.value    = false
  tracks.value         = []
  loadTracks(id)
}

// ── 대학 CRUD ─────────────────────────────────────────────────
function startAddUniv() {
  univForm.value     = emptyUnivForm()
  editingUnivId.value = null
  addingUniv.value   = true
}

async function saveAddUniv() {
  saving.value = true
  error.value  = ''
  try {
    await createUniversity(univFormToBody(univForm.value))
    addingUniv.value = false
    await loadUnivs()
  } catch (e) {
    error.value = e.response?.data ?? e.message
  } finally {
    saving.value = false
  }
}

function startEditUniv(u) {
  addingUniv.value   = false
  editingUnivId.value = u.id
  univForm.value     = univToForm(u)
}

async function saveEditUniv(id) {
  saving.value = true
  error.value  = ''
  try {
    await updateUniversity(id, univFormToBody(univForm.value))
    editingUnivId.value = null
    await loadUnivs()
  } catch (e) {
    error.value = e.response?.data ?? e.message
  } finally {
    saving.value = false
  }
}

async function removeUniv(id) {
  if (!confirm('이 대학과 모든 모집단위를 삭제하시겠습니까?')) return
  saving.value = true
  error.value  = ''
  try {
    await deleteUniversity(id)
    if (selectedUnivId.value === id) {
      selectedUnivId.value = null
      tracks.value = []
    }
    await loadUnivs()
  } catch (e) {
    error.value = e.response?.data ?? e.message
  } finally {
    saving.value = false
  }
}

// ── 모집단위 CRUD ─────────────────────────────────────────────
function startAddTrack() {
  trackForm.value     = emptyTrackForm()
  editingTrackId.value = null
  addingTrack.value   = true
}

async function saveAddTrack() {
  if (!selectedUnivId.value) return
  saving.value = true
  error.value  = ''
  try {
    await createTrack(selectedUnivId.value, trackFormToBody(trackForm.value))
    addingTrack.value = false
    await loadTracks(selectedUnivId.value)
  } catch (e) {
    error.value = e.response?.data ?? e.message
  } finally {
    saving.value = false
  }
}

function startEditTrack(t) {
  addingTrack.value    = false
  editingTrackId.value = t.id
  trackForm.value      = trackToForm(t)
}

async function saveEditTrack(id) {
  saving.value = true
  error.value  = ''
  try {
    await updateTrack(id, trackFormToBody(trackForm.value))
    editingTrackId.value = null
    await loadTracks(selectedUnivId.value)
  } catch (e) {
    error.value = e.response?.data ?? e.message
  } finally {
    saving.value = false
  }
}

async function removeTrack(id) {
  if (!confirm('이 모집단위를 삭제하시겠습니까?')) return
  saving.value = true
  error.value  = ''
  try {
    await deleteTrack(id)
    await loadTracks(selectedUnivId.value)
  } catch (e) {
    error.value = e.response?.data ?? e.message
  } finally {
    saving.value = false
  }
}

onMounted(loadUnivs)
</script>
