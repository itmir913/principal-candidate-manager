<template>
  <div>
    <div class="flex items-center justify-between mb-4">
      <h2 class="text-lg font-semibold text-gray-700">학급 목록</h2>
      <button
        class="px-3 py-1.5 bg-blue-600 text-white text-sm rounded hover:bg-blue-700"
        @click="showAddForm = true"
      >
        + 학급 추가
      </button>
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
        <label class="block text-xs text-gray-500 mb-1">비밀번호</label>
        <input v-model="newPassword" type="password"
          class="w-32 border rounded px-2 py-1 text-sm" />
      </div>
      <button class="px-3 py-1.5 bg-blue-600 text-white text-sm rounded hover:bg-blue-700" @click="addRow">저장</button>
      <button class="px-3 py-1.5 bg-gray-200 text-gray-700 text-sm rounded hover:bg-gray-300" @click="cancelAdd">취소</button>
    </div>

    <p v-if="error" class="text-red-500 text-sm mb-2">{{ error }}</p>

    <table class="w-full text-sm border-collapse">
      <thead>
        <tr class="bg-gray-100 text-gray-600 text-left">
          <th class="px-3 py-2 border-b">학년</th>
          <th class="px-3 py-2 border-b">반</th>
          <th class="px-3 py-2 border-b">담임명</th>
          <th class="px-3 py-2 border-b w-48">편집</th>
        </tr>
      </thead>
      <tbody>
        <template v-for="row in classes" :key="`${row.grade}-${row.class_no}`">
          <!-- 일반 행 -->
          <tr v-if="editing?.grade !== row.grade || editing?.class_no !== row.class_no"
            class="hover:bg-gray-50 cursor-pointer"
            @click="startEdit(row)">
            <td class="px-3 py-2 border-b">{{ row.grade }}</td>
            <td class="px-3 py-2 border-b">{{ row.class_no }}</td>
            <td class="px-3 py-2 border-b">{{ row.teacher_name ?? '-' }}</td>
            <td class="px-3 py-2 border-b text-blue-500 text-xs">클릭하여 편집</td>
          </tr>
          <!-- 인라인 편집 행 -->
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
                <button class="px-2 py-0.5 bg-blue-600 text-white text-xs rounded" @click="saveEdit(row)">저장</button>
                <button class="px-2 py-0.5 bg-gray-200 text-xs rounded" @click="editing = null">취소</button>
              </div>
            </td>
          </tr>
        </template>
        <tr v-if="classes.length === 0">
          <td colspan="4" class="px-3 py-4 text-center text-gray-400">등록된 학급이 없습니다.</td>
        </tr>
      </tbody>
    </table>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import { getClasses, upsertClass } from '../../api/admin.js'

const classes = ref([])
const error = ref('')
const editing = ref(null)
const editTeacherName = ref('')
const editPassword = ref('')

const showAddForm = ref(false)
const newGrade = ref(1)
const newClassNo = ref(1)
const newTeacherName = ref('')
const newPassword = ref('')

async function load() {
  try {
    classes.value = await getClasses()
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
  try {
    await upsertClass(row.grade, row.class_no, body)
    editing.value = null
    await load()
  } catch (e) {
    error.value = e.response?.data ?? e.message
  }
}

async function addRow() {
  if (!newGrade.value || !newClassNo.value) { error.value = '학년과 반을 입력하세요.'; return }
  const body = {}
  if (newTeacherName.value) body.teacher_name = newTeacherName.value
  if (newPassword.value) body.password = newPassword.value
  try {
    await upsertClass(newGrade.value, newClassNo.value, body)
    cancelAdd()
    await load()
  } catch (e) {
    error.value = e.response?.data ?? e.message
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

onMounted(load)
</script>
