<template>
  <div style="padding: 2rem 2.5rem;">

    <!-- 페이지 헤더 -->
    <div class="flex items-end justify-between flex-wrap gap-3 mb-5">
      <div>
        <p class="text-base mb-1" style="color: #94a3b8;">관리자</p>
        <h1 class="text-2xl font-semibold" style="color: #1e293b; margin: 0;">학급 관리</h1>
      </div>
      <div class="flex flex-wrap items-center gap-2">
        <button
            class="flex items-center gap-1.5 text-base font-medium rounded-lg disabled:opacity-40"
            style="padding: 9px 16px; border: none; background: #16a34a; color: white; cursor: pointer;"
            :disabled="showAddForm"
            @click="showAddForm = true"
        >+ 추가</button>
        <span style="color: #cbd5e1; user-select: none;">|</span>
        <button
          class="flex items-center gap-1.5 text-base font-medium rounded-lg disabled:opacity-40"
          style="padding: 9px 16px; border: 1px solid #e2e8f0; background: white; color: #475569; cursor: pointer;"
          :disabled="uploading || downloading"
          @click="dlTemplate"
        >양식 다운로드</button>
        <label
          class="flex items-center gap-1.5 text-base font-medium rounded-lg cursor-pointer"
          :class="uploading ? 'opacity-60' : ''"
          style="padding: 9px 16px; background: #2563eb; color: white;"
        >
          {{ uploading ? '가져오는 중…' : '가져오기' }}
          <input type="file" accept=".xlsx,.csv" class="hidden" :disabled="uploading" @change="onFileChange" />
        </label>
        <span style="color: #cbd5e1; user-select: none;">|</span>
        <button
          class="flex items-center gap-1.5 text-base font-medium rounded-lg disabled:opacity-40"
          style="padding: 9px 16px; border: 1px solid #e2e8f0; background: white; color: #475569; cursor: pointer;"
          :disabled="uploading || downloading"
          @click="dlExport"
        >전체 목록 다운로드</button>
      </div>
    </div>

    <!-- 업로드 결과 -->
    <div v-if="importResult" class="mb-5 rounded-xl text-base"
      :style="{
        padding: '14px 18px',
        border: importResult.errors.length ? '1px solid #fca5a5' : '1px solid #86efac',
        background: importResult.errors.length ? '#fef2f2' : '#f0fdf4',
        color: importResult.errors.length ? '#991b1b' : '#15803d',
      }">
      <p class="font-semibold mb-1">
        {{ importResult.errors.length
          ? '오류 발생 — 가져오기 실패'
          : `완료 — 신규 ${importResult.inserted}건, 수정 ${importResult.updated}건` }}
      </p>
      <ul v-if="importResult.errors.length" class="list-disc list-inside space-y-0.5">
        <li v-for="(e, i) in importResult.errors" :key="i">{{ e }}</li>
      </ul>
    </div>

    <!-- 학급 추가 폼 -->
    <div v-if="showAddForm" class="mb-5 rounded-xl"
      style="padding: 20px 22px; background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04);">
      <h3 class="text-lg font-semibold mb-4" style="color: #1e293b;">새 학급 추가</h3>
      <div class="flex gap-4 items-end flex-wrap">
        <div>
          <label class="block text-base font-medium mb-1.5" style="color: #64748b;">학년</label>
          <input v-model.number="newGrade" type="number" min="1" max="3"
            class="text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
            style="width: 72px; border: 1px solid #e2e8f0; border-radius: 8px; padding: 9px 12px;"
            placeholder="1" />
        </div>
        <div>
          <label class="block text-base font-medium mb-1.5" style="color: #64748b;">반</label>
          <input v-model.number="newClassNo" type="number" min="1"
            class="text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
            style="width: 72px; border: 1px solid #e2e8f0; border-radius: 8px; padding: 9px 12px;"
            placeholder="1" />
        </div>
        <div>
          <label class="block text-base font-medium mb-1.5" style="color: #64748b;">담임명</label>
          <input v-model="newTeacherName" type="text"
            class="text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
            style="width: 140px; border: 1px solid #e2e8f0; border-radius: 8px; padding: 9px 12px;"
            placeholder="홍길동" />
        </div>
        <div>
          <label class="block text-base font-medium mb-1.5" style="color: #64748b;">
            비밀번호 <span style="color: #94a3b8;">(4자 이상)</span>
          </label>
          <input v-model="newPassword" type="password"
            class="text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
            :style="{
              width: '140px', border: '1px solid', borderRadius: '8px', padding: '9px 12px',
              borderColor: newPassword && newPassword.length < 4 ? '#f87171' : '#e2e8f0',
            }" />
          <p v-if="newPassword && newPassword.length < 4" class="text-base mt-1" style="color: #ef4444;">
            4자 이상 입력하세요
          </p>
        </div>
        <div class="flex gap-2">
          <button
            class="text-base font-semibold rounded-lg disabled:opacity-40"
            style="padding: 9px 20px; border: none; background: #2563eb; color: white; cursor: pointer;"
            :disabled="saving"
            @click="addRow"
          >{{ saving ? '저장 중…' : '저장' }}</button>
          <button
            class="text-base rounded-lg"
            style="padding: 9px 20px; border: 1px solid #e2e8f0; background: white; color: #64748b; cursor: pointer;"
            @click="cancelAdd"
          >취소</button>
        </div>
      </div>
    </div>

    <p v-if="error" class="text-base mb-3" style="color: #ef4444;">{{ error }}</p>

    <!-- 학급 테이블 -->
    <div class="rounded-xl overflow-hidden"
      style="background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04);">
      <div class="overflow-x-auto">
        <table class="w-full min-w-max" style="border-collapse: collapse;">
          <thead>
            <tr style="background: #f8fafc; border-bottom: 1px solid #e2e8f0;">
              <th class="text-base font-semibold text-left" style="padding: 14px 20px; color: #475569; width: 80px;">학년</th>
              <th class="text-base font-semibold text-left" style="padding: 14px 20px; color: #475569; width: 60px;">반</th>
              <th class="text-base font-semibold text-left" style="padding: 14px 20px; color: #475569; width: 140px;">담임명</th>
              <th class="text-base font-semibold text-left" style="padding: 14px 20px; color: #475569;">편집</th>
            </tr>
          </thead>
          <tbody>
            <template v-for="row in classes" :key="`${row.grade}-${row.class_no}`">
              <!-- 일반 행 -->
              <tr v-if="editing?.grade !== row.grade || editing?.class_no !== row.class_no"
                style="border-bottom: 1px solid #f1f5f9; transition: background 0.1s;"
                class="hover:bg-slate-50">
                <td class="text-base" style="padding: 14px 20px; color: #1e293b;">{{ row.grade }}</td>
                <td class="text-base" style="padding: 14px 20px; color: #1e293b;">{{ row.class_no }}</td>
                <td class="text-base" style="padding: 14px 20px; color: #1e293b;">{{ row.teacher_name ?? '-' }}</td>
                <td style="padding: 14px 20px;">
                  <div class="flex gap-3">
                    <button class="text-base font-medium disabled:opacity-40"
                      style="color: #2563eb; background: none; border: none; cursor: pointer; padding: 0;"
                      :disabled="saving" @click="startEdit(row)">편집</button>
                    <button class="text-base font-medium disabled:opacity-40"
                      style="color: #ef4444; background: none; border: none; cursor: pointer; padding: 0;"
                      :disabled="saving" @click="remove(row)">삭제</button>
                  </div>
                </td>
              </tr>
              <!-- 편집 행 -->
              <tr v-else style="background: #eff6ff; border-bottom: 1px solid #bfdbfe;">
                <td class="text-base" style="padding: 14px 20px; color: #1e293b;">{{ row.grade }}</td>
                <td class="text-base" style="padding: 14px 20px; color: #1e293b;">{{ row.class_no }}</td>
                <td style="padding: 10px 20px;">
                  <input v-model="editTeacherName" type="text"
                    class="text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
                    style="width: 120px; border: 1px solid #93c5fd; border-radius: 6px; padding: 7px 10px;" />
                </td>
                <td style="padding: 10px 20px;">
                  <div class="flex gap-2 items-center flex-wrap">
                    <input v-model="editPassword" type="password" placeholder="새 비밀번호"
                      class="text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
                      style="width: 140px; border: 1px solid #93c5fd; border-radius: 6px; padding: 7px 10px;" />
                    <button
                      class="text-base font-semibold rounded-lg disabled:opacity-40"
                      style="padding: 7px 16px; border: none; background: #2563eb; color: white; cursor: pointer;"
                      :disabled="saving"
                      @click="saveEdit(row)"
                    >{{ saving ? '저장 중…' : '저장' }}</button>
                    <button
                      class="text-base rounded-lg"
                      style="padding: 7px 16px; border: 1px solid #e2e8f0; background: white; color: #64748b; cursor: pointer;"
                      :disabled="saving" @click="editing = null"
                    >취소</button>
                  </div>
                </td>
              </tr>
            </template>
            <tr v-if="classes.length === 0">
              <td colspan="4" class="text-base text-center" style="padding: 48px 20px; color: #94a3b8;">
                등록된 학급이 없습니다. 양식을 다운로드하여 업로드하세요.
              </td>
            </tr>
          </tbody>
        </table>
      </div>
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
