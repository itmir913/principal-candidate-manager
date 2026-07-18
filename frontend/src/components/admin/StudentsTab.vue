<template>
  <div class="py-8 px-4 sm:px-10">

    <!-- 페이지 헤더 -->
    <div class="flex items-end justify-between flex-wrap gap-3 mb-5">
      <div>
        <p class="text-base mb-1" style="color: #94a3b8;">관리자</p>
        <h1 class="text-2xl font-semibold" style="color: #1e293b; margin: 0;">학생 명단 관리</h1>
      </div>
      <div class="flex flex-wrap items-center gap-2">
        <!-- 재학생/졸업생 라디오 -->
        <label class="flex items-center gap-1.5 text-base cursor-pointer" style="color: #475569;">
          <input type="radio" v-model="studentType" value="enrolled" class="accent-blue-600" />
          재학생
        </label>
        <label class="flex items-center gap-1.5 text-base cursor-pointer" style="color: #475569;">
          <input type="radio" v-model="studentType" value="graduated" class="accent-blue-600" />
          졸업생
        </label>
        <span style="color: #cbd5e1; user-select: none;">|</span>
        <button
            class="flex items-center gap-1.5 text-base font-medium rounded-lg disabled:opacity-40"
            style="padding: 9px 16px; border: 1px solid #e2e8f0; background: white; color: #475569; cursor: pointer;"
            :disabled="downloading"
            @click="dlTemplate"
        >
          {{ studentType === 'enrolled' ? '재학생 양식 다운로드' : '졸업생 양식 다운로드' }}
        </button>
        <label
            class="flex items-center gap-1.5 text-base font-medium rounded-lg cursor-pointer"
            :class="uploading ? 'opacity-60' : ''"
            style="padding: 9px 16px; background: #2563eb; color: white;"
        >
          {{ uploading
            ? '가져오는 중…'
            : (studentType === 'enrolled'
                ? '재학생 가져오기'
                : '졸업생 가져오기') }}
          <input type="file" accept=".xlsx,.csv" class="hidden" :disabled="uploading" @change="onImport" />
        </label>
        <span style="color: #cbd5e1; user-select: none;">|</span>
        <button
            class="flex items-center gap-1.5 text-base font-medium rounded-lg disabled:opacity-40"
            style="padding: 9px 16px; background: #16a34a; color: white; cursor: pointer;"
            :disabled="showAddForm"
            @click="openAddForm"
        >+ 추가</button>
        <span style="color: #cbd5e1; user-select: none;">|</span>
        <button
            class="flex items-center gap-1.5 text-base font-medium rounded-lg disabled:opacity-40"
            style="padding: 9px 16px; border: 1px solid #e2e8f0; background: white; color: #475569; cursor: pointer;"
            :disabled="downloading"
            @click="dlAll"
        >전체 목록 다운로드</button>
      </div>
    </div>

    <HelpBox class="mb-4" storage-key="students" :title="HELP.title" :intro="HELP.intro" :items="HELP.items" />

    <!-- 학생 추가 폼 -->
    <div v-if="showAddForm" class="mb-5 rounded-xl"
      style="padding: 20px 22px; background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04);">
      <h3 class="text-lg font-semibold mb-4" style="color: #1e293b;">새 학생 추가</h3>

      <!-- 재학생/졸업생 선택 -->
      <div class="flex gap-4 mb-4">
        <label class="flex items-center gap-1.5 text-base cursor-pointer" style="color: #475569;">
          <input type="radio" v-model="addType" value="enrolled" class="accent-blue-600" />
          재학생
        </label>
        <label class="flex items-center gap-1.5 text-base cursor-pointer" style="color: #475569;">
          <input type="radio" v-model="addType" value="graduated" class="accent-blue-600" />
          졸업생
        </label>
      </div>

      <!-- 재학생 필드 -->
      <div v-if="addType === 'enrolled'" class="flex gap-4 items-end flex-wrap">
        <div>
          <label class="block text-base font-medium mb-1.5" style="color: #64748b;">학년</label>
          <input v-model.number="addForm.grade" type="number" min="1" max="3"
            class="text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
            style="width: 72px; border: 1px solid #e2e8f0; border-radius: 8px; padding: 9px 12px;"
            placeholder="3" />
        </div>
        <div>
          <label class="block text-base font-medium mb-1.5" style="color: #64748b;">반</label>
          <input v-model.number="addForm.class_no" type="number" min="1"
            class="text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
            style="width: 72px; border: 1px solid #e2e8f0; border-radius: 8px; padding: 9px 12px;"
            placeholder="2" />
        </div>
        <div>
          <label class="block text-base font-medium mb-1.5" style="color: #64748b;">번호</label>
          <input v-model.number="addForm.seq_no" type="number" min="1"
            class="text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
            style="width: 72px; border: 1px solid #e2e8f0; border-radius: 8px; padding: 9px 12px;"
            placeholder="15" />
        </div>
        <div>
          <label class="block text-base font-medium mb-1.5" style="color: #64748b;">이름</label>
          <input v-model="addForm.name" type="text"
            class="text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
            style="width: 140px; border: 1px solid #e2e8f0; border-radius: 8px; padding: 9px 12px;"
            placeholder="홍길동" @keydown.enter="submitAdd" />
        </div>
        <div class="flex gap-2">
          <button
            class="text-base font-semibold rounded-lg disabled:opacity-40"
            style="padding: 9px 20px; border: none; background: #2563eb; color: white; cursor: pointer;"
            :disabled="addSaving" @click="submitAdd"
          >{{ addSaving ? '저장 중…' : '저장' }}</button>
          <button
            class="text-base rounded-lg"
            style="padding: 9px 20px; border: 1px solid #e2e8f0; background: white; color: #64748b; cursor: pointer;"
            @click="cancelAdd"
          >취소</button>
        </div>
      </div>

      <!-- 졸업생 필드 -->
      <div v-else class="flex gap-4 items-end flex-wrap">
        <div>
          <label class="block text-base font-medium mb-1.5" style="color: #64748b;">학생코드</label>
          <input v-model="addForm.student_code" type="text"
            class="text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
            style="width: 140px; border: 1px solid #e2e8f0; border-radius: 8px; padding: 9px 12px;"
            placeholder="20240001" />
        </div>
        <div>
          <label class="block text-base font-medium mb-1.5" style="color: #64748b;">졸업연도</label>
          <input v-model.number="addForm.grad_year" type="number"
            class="text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
            style="width: 100px; border: 1px solid #e2e8f0; border-radius: 8px; padding: 9px 12px;"
            placeholder="2024" />
        </div>
        <div>
          <label class="block text-base font-medium mb-1.5" style="color: #64748b;">이름</label>
          <input v-model="addForm.name" type="text"
            class="text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
            style="width: 140px; border: 1px solid #e2e8f0; border-radius: 8px; padding: 9px 12px;"
            placeholder="김철수" @keydown.enter="submitAdd" />
        </div>
        <div class="flex gap-2">
          <button
            class="text-base font-semibold rounded-lg disabled:opacity-40"
            style="padding: 9px 20px; border: none; background: #2563eb; color: white; cursor: pointer;"
            :disabled="addSaving" @click="submitAdd"
          >{{ addSaving ? '저장 중…' : '저장' }}</button>
          <button
            class="text-base rounded-lg"
            style="padding: 9px 20px; border: 1px solid #e2e8f0; background: white; color: #64748b; cursor: pointer;"
            @click="cancelAdd"
          >취소</button>
        </div>
      </div>

      <p v-if="addError" class="text-base mt-3" style="color: #ef4444;">{{ addError }}</p>
    </div>

    <!-- 업로드 결과 -->
    <div v-if="result" class="mb-5 rounded-xl text-base"
      :style="{
        padding: '14px 18px',
        border: result.errors.length ? '1px solid #fca5a5' : '1px solid #86efac',
        background: result.errors.length ? '#fef2f2' : '#f0fdf4',
        color: result.errors.length ? '#991b1b' : '#15803d',
      }">
      <p class="font-semibold mb-1">
        [{{ result.label }}]
        {{ result.errors.length
          ? '오류 발생 — 가져오기 실패'
          : `완료 — 신규 ${result.inserted}명, 수정 ${result.updated}명` }}
      </p>
      <ul v-if="result.errors.length" class="list-disc list-inside space-y-0.5">
        <li v-for="(e, i) in result.errors" :key="i">{{ e }}</li>
      </ul>
    </div>

    <p v-if="error" class="text-base mb-3" style="color: #ef4444;">{{ error }}</p>

    <!-- 필터 -->
    <div class="rounded-xl mb-4 flex flex-wrap gap-3 items-center p-[14px_18px] bg-white shadow-[0_1px_4px_rgba(0,0,0,0.07),0_0_0_1px_rgba(0,0,0,0.04)]">

      <select v-model="filterEnrolled"
              class="text-base rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-400 border border-slate-200 py-2 pl-3 pr-8 text-slate-800 bg-white">
        <option :value="null">전체 유형</option>
        <option :value="1">재학생</option>
        <option :value="0">졸업생</option>
      </select>

      <select v-model.number="filterGrade"
              class="text-base rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-400 border border-slate-200 py-2 pl-3 pr-8 text-slate-800 bg-white disabled:opacity-50 disabled:cursor-not-allowed"
              :disabled="filterEnrolled === 0"
              @change="filterClass = null">
        <option :value="null">전체 학년</option>
        <option v-for="g in gradeOptions.grades" :key="g" :value="g">{{ g }}학년</option>
      </select>

      <select v-model.number="filterClass"
              class="text-base rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-400 border border-slate-200 py-2 pl-3 pr-8 text-slate-800 bg-white disabled:opacity-50 disabled:cursor-not-allowed"
              :disabled="filterEnrolled === 0">
        <option :value="null">전체 반</option>
        <option v-for="c in availableClasses" :key="c" :value="c">{{ c }}반</option>
      </select>

      <button
          class="text-base font-medium rounded-lg px-[18px] py-2 bg-[#2563eb] text-white hover:bg-blue-700 transition-colors"
          @click="loadStudents()">조회</button>

      <span class="ml-auto text-base font-medium text-slate-500">총 {{ studentPage.total }}명</span>
    </div>

    <!-- 학생 목록 테이블 -->
    <div class="rounded-xl overflow-hidden"
      style="background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04);">
      <div class="overflow-x-auto">
        <table class="w-full min-w-max" style="border-collapse: collapse;">
          <thead>
            <tr style="background: #f8fafc; border-bottom: 1px solid #e2e8f0;">
              <th class="text-base font-semibold text-left" style="padding: 14px 20px; color: #475569; width: 130px;">학생코드</th>
              <th class="text-base font-semibold text-left" style="padding: 14px 20px; color: #475569; width: 130px;">이름</th>
              <th class="text-base font-semibold text-left" style="padding: 14px 20px; color: #475569; width: 80px;">구분</th>
              <th class="text-base font-semibold text-left" style="padding: 14px 20px; color: #475569; width: 70px;">학년</th>
              <th class="text-base font-semibold text-left" style="padding: 14px 20px; color: #475569; width: 60px;">반</th>
              <th class="text-base font-semibold text-left" style="padding: 14px 20px; color: #475569; width: 60px;">번호</th>
              <th class="text-base font-semibold text-left" style="padding: 14px 20px; color: #475569; width: 100px;">졸업연도</th>
              <th class="text-base font-semibold text-center" style="padding: 14px 20px; color: #475569; width: 80px;">삭제</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="s in studentPage.rows" :key="s.id"
              class="hover:bg-slate-50"
              style="border-bottom: 1px solid #f1f5f9; transition: background 0.1s;">
              <td class="text-base font-mono" style="padding: 13px 20px; color: #475569;">{{ s.student_code }}</td>
              <td class="text-base" style="padding: 13px 20px; color: #1e293b;">{{ s.name }}</td>
              <td style="padding: 13px 20px;">
                <span class="text-base font-medium"
                  :style="{ color: s.is_enrolled ? '#2563eb' : '#94a3b8' }">
                  {{ s.is_enrolled ? '재학' : '졸업' }}
                </span>
              </td>
              <td class="text-base" style="padding: 13px 20px; color: #1e293b;">{{ s.grade ?? '-' }}</td>
              <td class="text-base" style="padding: 13px 20px; color: #1e293b;">{{ s.class_no ?? '-' }}</td>
              <td class="text-base" style="padding: 13px 20px; color: #1e293b;">{{ s.seq_no ?? '-' }}</td>
              <td class="text-base" style="padding: 13px 20px; color: #1e293b;">{{ s.grad_year ?? '-' }}</td>
              <td class="text-center" style="padding: 13px 20px;">
                <button
                  class="text-base font-medium rounded-lg whitespace-nowrap"
                  style="padding: 5px 12px; border: 1px solid #fca5a5; background: white; color: #ef4444; cursor: pointer;"
                  @click="remove(s)"
                >삭제</button>
              </td>
            </tr>
            <tr v-if="studentPage.rows.length === 0">
              <td colspan="8" class="text-base text-center" style="padding: 48px 20px; color: #94a3b8;">
                학생 데이터가 없습니다. 양식을 다운로드하여 작성 후 가져오기 하세요.
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <!-- 페이지 내비게이션 -->
    <div v-if="studentPage.total > 0" class="mt-4 flex items-center justify-center gap-4">
      <button
        class="text-base rounded-lg disabled:opacity-40 disabled:cursor-not-allowed"
        style="padding: 8px 18px; border: 1px solid #e2e8f0; background: white; color: #475569; cursor: pointer;"
        :disabled="studentPage.page <= 1"
        @click="loadStudents(studentPage.page - 1)"
      >&lt; 이전</button>
      <span class="text-base" style="color: #64748b;">
        {{ studentPage.page }} / {{ Math.ceil(studentPage.total / studentPage.per_page) }} 페이지
        (총 {{ studentPage.total }}명)
      </span>
      <button
        class="text-base rounded-lg disabled:opacity-40 disabled:cursor-not-allowed"
        style="padding: 8px 18px; border: 1px solid #e2e8f0; background: white; color: #475569; cursor: pointer;"
        :disabled="studentPage.page >= Math.ceil(studentPage.total / studentPage.per_page)"
        @click="loadStudents(studentPage.page + 1)"
      >다음 &gt;</button>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, watch, onMounted } from 'vue'
import {
  getStudents,
  getStudentGradeOptions,
  exportStudents,
  downloadEnrolledTemplate,
  importEnrolled,
  downloadGraduatedTemplate,
  importGraduated,
  addEnrolledStudent,
  addGraduatedStudent,
  deleteStudent,
  blobErrMsg,
} from '../../api/admin.js'
import HelpBox from '../common/HelpBox.vue'
import { dialog } from '../common/dialog.js'

const HELP = {
  title: '도움말 — 학생 명단 관리',
  intro: '추천 대상이 될 학생 명단을 등록하는 곳입니다. 재학생과 졸업생은 각각 별도 파일로 관리합니다.',
  items: [
    '먼저 위에서 재학생/졸업생을 선택한 뒤, "양식 다운로드"로 받은 엑셀 파일에 학생 명단을 채워 "가져오기"로 업로드하세요.',
    '재학생은 학년·반·번호·이름, 졸업생은 학생코드·졸업연도·이름이 필요합니다.',
    '한두 명만 추가할 때는 "+ 추가" 버튼으로 직접 입력할 수 있습니다.',
    '학생 가져오기는 기존 명단을 지우지 않습니다. 같은 학생이 있으면 이름 등 정보만 업데이트되고, 없으면 새로 추가됩니다.',
    '라운드가 마감된 동안에는 이미 결과가 있는 학생의 재학/졸업 구분을 바꿀 수 없습니다. 저장된 순위가 마감 시점의 구분을 기준으로 계산된 값이기 때문입니다. 바꾸려면 라운드를 다시 열고 명단을 반영한 뒤 다시 마감하세요(이름 수정과 신규 추가는 마감 중에도 가능합니다).',
  ],
}

const studentPage = ref({ rows: [], total: 0, page: 1, per_page: 100 })
const error = ref('')
const result = ref(null)
const filterEnrolled = ref(null)   // null=전체, 1=재학생, 0=졸업생
const filterGrade = ref(null)
const filterClass = ref(null)
const gradeOptions = ref({ grades: [], by_grade: {} })
const downloading = ref(false)
const uploading = ref(false)
const studentType = ref('enrolled')

// ── 개별 추가 폼 ────────────────────────────────────────────────
const showAddForm = ref(false)
const addType = ref('enrolled')
const addForm = ref({ name: '', grade: null, class_no: null, seq_no: null, student_code: '', grad_year: null })
const addError = ref('')
const addSaving = ref(false)

function openAddForm() {
  addType.value = studentType.value
  addForm.value = { name: '', grade: null, class_no: null, seq_no: null, student_code: '', grad_year: null }
  addError.value = ''
  showAddForm.value = true
}

function cancelAdd() {
  showAddForm.value = false
  addForm.value = { name: '', grade: null, class_no: null, seq_no: null, student_code: '', grad_year: null }
  addError.value = ''
}

async function submitAdd() {
  addError.value = ''
  addSaving.value = true
  try {
    if (addType.value === 'enrolled') {
      const { grade, class_no, seq_no, name } = addForm.value
      if (!grade || !class_no || !seq_no || !name?.trim()) {
        addError.value = '학년, 반, 번호, 이름을 모두 입력하세요.'
        return
      }
      await addEnrolledStudent({ grade, class_no, seq_no, name: name.trim() })
    } else {
      const { student_code, name, grad_year } = addForm.value
      if (!student_code?.trim() || !name?.trim() || !grad_year) {
        addError.value = '학생코드, 이름, 졸업연도를 모두 입력하세요.'
        return
      }
      await addGraduatedStudent({ student_code: student_code.trim(), name: name.trim(), grad_year })
    }
    cancelAdd()
    await Promise.all([loadStudents(), loadGradeOptions()])
  } catch (e) {
    addError.value = e.response?.data ?? e.message ?? '오류가 발생했습니다'
  } finally {
    addSaving.value = false
  }
}

// 선택 학년에 따라 드롭다운에 표시할 반 목록
const availableClasses = computed(() => {
  if (!filterGrade.value) {
    const all = new Set()
    Object.values(gradeOptions.value.by_grade).forEach(arr => arr.forEach(c => all.add(c)))
    return [...all].sort((a, b) => a - b)
  }
  return gradeOptions.value.by_grade[String(filterGrade.value)] ?? []
})

// 졸업생 선택 시 학년·반 필터 초기화
watch(filterEnrolled, (val) => {
  if (val === 0) {
    filterGrade.value = null
    filterClass.value = null
  }
})

async function loadStudents(page = 1) {
  error.value = ''
  try {
    const params = { page, per_page: studentPage.value.per_page }
    if (filterGrade.value)              params.grade = filterGrade.value
    if (filterClass.value)              params.class_no = filterClass.value
    if (filterEnrolled.value !== null)  params.is_enrolled = filterEnrolled.value
    studentPage.value = await getStudents(params)
  } catch (e) {
    error.value = e.response?.data ?? e.message
  }
}

async function loadGradeOptions() {
  try {
    gradeOptions.value = await getStudentGradeOptions()
  } catch { /* 실패해도 빈 목록으로 동작 */ }
}

function saveBlob(response, fallback) {
  const filename = fallback
  const url = URL.createObjectURL(new Blob([response.data]))
  const a = document.createElement('a')
  a.href = url; a.download = filename; a.click()
  URL.revokeObjectURL(url)
}

async function runImport(apiFn, label, evt) {
  const file = evt.target.files?.[0]
  if (!file) return
  error.value = ''
  result.value = null
  uploading.value = true
  try {
    const data = await apiFn(file)
    result.value = { label, ...data }
    await Promise.all([loadStudents(), loadGradeOptions()])
  } catch (e) {
    const d = e.response?.data
    if (d != null && typeof d === 'object' && Array.isArray(d.errors)) {
      result.value = { label, ...d }
    } else {
      error.value = typeof d === 'string' ? d : (e.message ?? '오류가 발생했습니다')
    }
  } finally {
    uploading.value = false
    evt.target.value = ''
  }
}

async function remove(s) {
  const label = `${s.name}(${s.student_code})`
  if (!(await dialog.confirm({
    title: '학생 삭제',
    message: `${label} 학생을 삭제하시겠습니까?`,
    confirmText: '삭제',
    level: 'danger',
    dangerNotice: '삭제된 학생 정보는 복구할 수 없습니다.',
    finalConfirmText: '영구 삭제',
  }))) return
  error.value = ''
  try {
    await deleteStudent(s.id)
    await loadStudents(studentPage.value.page)
  } catch (e) {
    error.value = e.response?.data ?? e.message
  }
}

async function dlTemplate() {
  downloading.value = true
  try {
    if (studentType.value === 'enrolled') {
      saveBlob(await downloadEnrolledTemplate(), 'students_enrolled_template.xlsx')
    } else {
      saveBlob(await downloadGraduatedTemplate(), 'students_graduated_template.xlsx')
    }
  } catch (e) { error.value = await blobErrMsg(e) }
  finally { downloading.value = false }
}

function onImport(evt) {
  const label = studentType.value === 'enrolled' ? '재학생' : '졸업생'
  const apiFn = studentType.value === 'enrolled' ? importEnrolled : importGraduated
  runImport(apiFn, label, evt)
}

async function dlAll() {
  downloading.value = true
  try { saveBlob(await exportStudents(), 'students_all.xlsx') }
  catch (e) { error.value = await blobErrMsg(e) }
  finally { downloading.value = false }
}

onMounted(() => {
  loadGradeOptions()
  loadStudents()
})
</script>
