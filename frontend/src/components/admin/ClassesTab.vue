<template>
  <div>
    <div class="flex items-center justify-between mb-4 flex-wrap gap-2">
      <h2 class="text-lg font-semibold text-gray-700">학급 목록</h2>
      <div class="flex flex-wrap gap-2">
          <button
            class="px-3 py-1.5 border border-gray-300 text-gray-700 text-sm rounded hover:bg-gray-50 disabled:opacity-40"
            :disabled="uploading || downloading"
            @click="dlTemplate"
          >양식 다운로드</button>
          <label
            class="px-3 py-1.5 text-sm rounded cursor-pointer"
            :class="uploading ? 'bg-gray-400 text-white' : 'bg-blue-600 text-white hover:bg-blue-700'"
          >
            {{ uploading ? '가져오는 중…' : '가져오기' }}
            <input type="file" accept=".xlsx,.csv" class="hidden" :disabled="uploading" @change="onFileChange" />
          </label>
          <button
            class="px-3 py-1.5 bg-green-600 text-white text-sm rounded hover:bg-green-700 disabled:opacity-40"
            :disabled="showAddForm"
            @click="showAddForm = true"
          >+ 추가</button>
          <span class="text-gray-300 select-none">|</span>
          <button
            class="px-3 py-1.5 border border-gray-300 text-gray-700 text-sm rounded hover:bg-gray-50 disabled:opacity-40"
            :disabled="uploading || downloading"
            @click="dlExport"
          >전체 목록 다운로드</button>
      </div>
    </div>

    <!-- 업로드 결과 -->
    <div v-if="importResult" class="mb-4 p-3 rounded border text-sm"
      :class="importResult.errors.length ? 'border-yellow-400 bg-yellow-50' : 'border-green-400 bg-green-50'">
      <p class="font-medium mb-1">
        {{ importResult.errors.length ? '오류 발생 — 가져오기 실패' : `완료 — 신규 ${importResult.inserted}건, 수정 ${importResult.updated}건` }}
      </p>
      <ul v-if="importResult.errors.length" class="list-disc list-inside text-yellow-700 space-y-0.5">
        <li v-for="(e, i) in importResult.errors" :key="i">{{ e }}</li>
      </ul>
    </div>

    <!-- 학급 추가 폼 -->
    <div v-if="showAddForm" class="mb-4 p-4 border border-blue-200 rounded bg-blue-50 flex gap-2 items-end flex-wrap">
      <div>
        <label class="block text-xs text-gray-500 mb-1">학년</label>
        <input v-model.number="newGrade" type="number" min="1" max="3"
          class="w-16 border rounded px-2 py-1 text-sm" placeholder="1" />
      </div>
      <div>
        <label class="block text-xs text-gray-500 mb-1">반</label>
        <input v-model.number="newClassNo" type="number" min="1"
          class="w-16 border rounded px-2 py-1 text-sm" placeholder="1" />
      </div>
      <div>
        <label class="block text-xs text-gray-500 mb-1">담임명</label>
        <input v-model="newTeacherName" type="text"
          class="w-32 border rounded px-2 py-1 text-sm" placeholder="홍길동" />
      </div>
      <div>
        <label class="block text-xs text-gray-500 mb-1">
          비밀번호 <span class="text-gray-400">(4자 이상)</span>
        </label>
        <input v-model="newPassword" type="password"
          class="w-32 border rounded px-2 py-1 text-sm"
          :class="newPassword && newPassword.length < 4 ? 'border-red-400 focus:ring-red-400' : ''" />
        <p v-if="newPassword && newPassword.length < 4" class="text-xs text-red-500 mt-0.5">
          4자 이상 입력하세요
        </p>
      </div>
      <button
        class="px-3 py-1.5 bg-blue-600 text-white text-sm rounded hover:bg-blue-700 disabled:opacity-40"
        :disabled="saving"
        @click="addRow"
      >{{ saving ? '저장 중…' : '저장' }}</button>
      <button class="px-3 py-1.5 bg-gray-200 text-gray-700 text-sm rounded hover:bg-gray-300" @click="cancelAdd">취소</button>
    </div>

    <p v-if="error" class="text-red-500 text-sm mb-2">{{ error }}</p>

    <div class="overflow-x-auto border border-gray-200 rounded">
    <table class="w-full min-w-max text-sm border-collapse">
      <thead>
        <tr class="bg-gray-100 text-gray-600 text-left">
          <th class="px-3 py-2 border-b w-16">학년</th>
          <th class="px-3 py-2 border-b w-12">반</th>
          <th class="px-3 py-2 border-b w-32">담임명</th>
          <th class="px-3 py-2 border-b w-48">편집</th>
        </tr>
      </thead>
      <tbody>
        <template v-for="row in classes" :key="`${row.grade}-${row.class_no}`">
          <tr v-if="editing?.grade !== row.grade || editing?.class_no !== row.class_no"
            class="hover:bg-gray-50">
            <td class="px-3 py-2 border-b">{{ row.grade }}</td>
            <td class="px-3 py-2 border-b">{{ row.class_no }}</td>
            <td class="px-3 py-2 border-b">{{ row.teacher_name ?? '-' }}</td>
            <td class="px-3 py-2 border-b">
              <div class="flex gap-2">
                <button class="text-blue-500 text-xs hover:underline disabled:opacity-40"
                  :disabled="saving" @click="startEdit(row)">편집</button>
                <button class="text-red-400 text-xs hover:underline disabled:opacity-40"
                  :disabled="saving" @click="remove(row)">삭제</button>
              </div>
            </td>
          </tr>
          <tr v-else class="bg-yellow-50">
            <td class="px-3 py-2 border-b">{{ row.grade }}</td>
            <td class="px-3 py-2 border-b">{{ row.class_no }}</td>
            <td class="px-3 py-2 border-b">
              <input v-model="editTeacherName" type="text"
                class="border rounded px-2 py-0.5 text-sm w-28" />
            </td>
            <td class="px-3 py-2 border-b">
              <div class="flex gap-1 items-center flex-wrap">
                <input v-model="editPassword" type="password" placeholder="새 비밀번호"
                  class="border rounded px-2 py-0.5 text-sm w-28" />
                <button
                  class="px-2 py-0.5 bg-blue-600 text-white text-xs rounded disabled:opacity-40"
                  :disabled="saving"
                  @click="saveEdit(row)"
                >{{ saving ? '저장 중…' : '저장' }}</button>
                <button class="px-2 py-0.5 bg-gray-200 text-xs rounded" :disabled="saving" @click="editing = null">취소</button>
              </div>
            </td>
          </tr>
        </template>
        <tr v-if="classes.length === 0">
          <td colspan="4" class="px-3 py-4 text-center text-gray-400">등록된 학급이 없습니다. 샘플 양식을 다운로드하여 업로드하세요.</td>
        </tr>
      </tbody>
    </table>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import { getClasses, upsertClass, deleteClass, downloadClassTemplate, exportClasses, importClasses } from '../../api/admin.js'

const classes = ref([])
const error = ref('')
const editing = ref(null)
const editTeacherName = ref('')
const editPassword = ref('')
const saving = ref(false)
const uploading = ref(false)
const downloading = ref(false)
const importResult = ref(null)

const showAddForm = ref(false)
const newGrade = ref(1)
const newClassNo = ref(1)
const newTeacherName = ref('')
const newPassword = ref('')

async function load() {
  try {
    classes.value = (await getClasses()).filter(r => !(r.grade === 0 && r.class_no === 0))
  } catch (e) {
    error.value = e.response?.data ?? e.message
  }
}

function startEdit(row) {
  editing.value = { grade: row.grade, class_no: row.class_no }
  editTeacherName.value = row.teacher_name ?? ''
  editPassword.value = ''
}

async function saveEdit(row) {
  const body = {}
  if (editTeacherName.value !== (row.teacher_name ?? '')) body.teacher_name = editTeacherName.value
  if (editPassword.value) body.password = editPassword.value
  saving.value = true
  error.value = ''
  try {
    await upsertClass(row.grade, row.class_no, body)
    editing.value = null
    await load()
  } catch (e) {
    error.value = e.response?.data ?? e.message
  } finally {
    saving.value = false
  }
}

async function addRow() {
  if (!newGrade.value || !newClassNo.value) { error.value = '학년과 반을 입력하세요.'; return }
  if (!newPassword.value) { error.value = '비밀번호를 설정해야 합니다.'; return }
  const body = {}
  if (newTeacherName.value) body.teacher_name = newTeacherName.value
  body.password = newPassword.value
  saving.value = true
  error.value = ''
  try {
    await upsertClass(newGrade.value, newClassNo.value, body)
    cancelAdd()
    await load()
  } catch (e) {
    error.value = e.response?.data ?? e.message
  } finally {
    saving.value = false
  }
}

async function remove(row) {
  if (!confirm(`${row.grade}학년 ${row.class_no}반을 삭제하시겠습니까?`)) return
  saving.value = true
  error.value = ''
  try {
    await deleteClass(row.grade, row.class_no)
    await load()
  } catch (e) {
    error.value = e.response?.data ?? e.message
  } finally {
    saving.value = false
  }
}

function cancelAdd() {
  showAddForm.value = false
  newGrade.value = 1
  newClassNo.value = 1
  newTeacherName.value = ''
  newPassword.value = ''
  error.value = ''
}

function saveBlob(response, filename) {
  const url = URL.createObjectURL(new Blob([response.data]))
  const a = document.createElement('a')
  a.href = url
  a.download = filename
  a.click()
  URL.revokeObjectURL(url)
}

async function dlTemplate() {
  downloading.value = true
  try {
    const res = await downloadClassTemplate()
    saveBlob(res, 'classes_template.xlsx')
  } catch (e) {
    error.value = e.response?.data ?? e.message
  } finally {
    downloading.value = false
  }
}

async function dlExport() {
  downloading.value = true
  try {
    const res = await exportClasses()
    saveBlob(res, 'classes.xlsx')
  } catch (e) {
    error.value = e.response?.data ?? e.message
  } finally {
    downloading.value = false
  }
}

async function onFileChange(evt) {
  const file = evt.target.files?.[0]
  if (!file) return
  error.value = ''
  importResult.value = null
  uploading.value = true
  try {
    importResult.value = await importClasses(file)
    await load()
  } catch (e) {
    const d = e.response?.data
    if (d != null && typeof d === 'object' && Array.isArray(d.errors)) {
      importResult.value = d
    } else {
      error.value = typeof d === 'string' ? d : (e.message ?? '오류가 발생했습니다')
    }
  } finally {
    uploading.value = false
    evt.target.value = ''
  }
}

onMounted(load)
</script>
