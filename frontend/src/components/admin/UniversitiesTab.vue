<template>
  <div>
    <div class="flex items-center justify-between mb-4">
      <h2 class="text-lg font-semibold text-gray-700">대학 목록</h2>
      <button
        class="px-3 py-1.5 bg-blue-600 text-white text-sm rounded hover:bg-blue-700"
        @click="startAdd"
      >
        + 추가
      </button>
    </div>

    <p v-if="error" class="text-red-500 text-sm mb-2">{{ error }}</p>

    <table class="w-full text-sm border-collapse">
      <thead>
        <tr class="bg-gray-100 text-gray-600 text-left">
          <th class="px-3 py-2 border-b">대학명</th>
          <th class="px-3 py-2 border-b">모집단위명</th>
          <th class="px-3 py-2 border-b">정원</th>
          <th class="px-3 py-2 border-b">재학생우선</th>
          <th class="px-3 py-2 border-b w-32"></th>
        </tr>
      </thead>
      <tbody>
        <!-- 추가 행 -->
        <tr v-if="adding" class="bg-blue-50">
          <td class="px-2 py-1 border-b"><input v-model="form.univ_name" type="text" class="w-full border rounded px-2 py-0.5 text-sm" placeholder="대학교" /></td>
          <td class="px-2 py-1 border-b"><input v-model="form.track_name" type="text" class="w-full border rounded px-2 py-0.5 text-sm" placeholder="모집단위명" /></td>
          <td class="px-2 py-1 border-b"><input v-model.number="form.capacity" type="number" min="1" class="w-16 border rounded px-2 py-0.5 text-sm" /></td>
          <td class="px-2 py-1 border-b text-center"><input v-model="form.prioritize_enrolled" type="checkbox" /></td>
          <td class="px-2 py-1 border-b">
            <button class="px-2 py-0.5 bg-blue-600 text-white text-xs rounded mr-1 disabled:opacity-40" :disabled="saving" @click="saveAdd">{{ saving ? '저장 중…' : '저장' }}</button>
            <button class="px-2 py-0.5 bg-gray-200 text-xs rounded" :disabled="saving" @click="adding = false">취소</button>
          </td>
        </tr>

        <template v-for="row in universities" :key="row.id">
          <!-- 일반 행 -->
          <tr v-if="editingId !== row.id" class="hover:bg-gray-50">
            <td class="px-3 py-2 border-b">{{ row.univ_name }}</td>
            <td class="px-3 py-2 border-b">{{ row.track_name }}</td>
            <td class="px-3 py-2 border-b">{{ row.capacity }}</td>
            <td class="px-3 py-2 border-b text-center">{{ row.prioritize_enrolled ? '○' : '-' }}</td>
            <td class="px-3 py-2 border-b">
              <button class="text-blue-500 text-xs mr-2 hover:underline disabled:opacity-40" :disabled="saving" @click="startEdit(row)">편집</button>
              <button class="text-red-400 text-xs hover:underline disabled:opacity-40" :disabled="saving" @click="remove(row.id)">삭제</button>
            </td>
          </tr>
          <!-- 인라인 편집 행 -->
          <tr v-else class="bg-yellow-50">
            <td class="px-2 py-1 border-b"><input v-model="form.univ_name" type="text" class="w-full border rounded px-2 py-0.5 text-sm" /></td>
            <td class="px-2 py-1 border-b"><input v-model="form.track_name" type="text" class="w-full border rounded px-2 py-0.5 text-sm" /></td>
            <td class="px-2 py-1 border-b"><input v-model.number="form.capacity" type="number" min="1" class="w-16 border rounded px-2 py-0.5 text-sm" /></td>
            <td class="px-2 py-1 border-b text-center"><input v-model="form.prioritize_enrolled" type="checkbox" /></td>
            <td class="px-2 py-1 border-b">
              <button class="px-2 py-0.5 bg-blue-600 text-white text-xs rounded mr-1 disabled:opacity-40" :disabled="saving" @click="saveEdit(row.id)">{{ saving ? '저장 중…' : '저장' }}</button>
              <button class="px-2 py-0.5 bg-gray-200 text-xs rounded" :disabled="saving" @click="editingId = null">취소</button>
            </td>
          </tr>
        </template>

        <tr v-if="universities.length === 0 && !adding">
          <td colspan="5" class="px-3 py-4 text-center text-gray-400">등록된 대학이 없습니다.</td>
        </tr>
      </tbody>
    </table>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import { getUniversities, createUniversity, updateUniversity, deleteUniversity } from '../../api/admin.js'

const universities = ref([])
const error = ref('')
const adding = ref(false)
const editingId = ref(null)
const form = ref(emptyForm())
const saving = ref(false)

function emptyForm() {
  return { univ_name: '', track_name: '', capacity: 1, prioritize_enrolled: false }
}

async function load() {
  try { universities.value = await getUniversities() } catch (e) { error.value = e.response?.data ?? e.message }
}

function startAdd() {
  form.value = emptyForm()
  editingId.value = null
  adding.value = true
}

async function saveAdd() {
  saving.value = true
  error.value = ''
  try {
    await createUniversity({ ...form.value })
    adding.value = false
    await load()
  } catch (e) { error.value = e.response?.data ?? e.message }
  finally { saving.value = false }
}

function startEdit(row) {
  adding.value = false
  editingId.value = row.id
  form.value = {
    univ_name: row.univ_name,
    track_name: row.track_name,
    capacity: row.capacity,
    prioritize_enrolled: row.prioritize_enrolled === 1,
  }
}

async function saveEdit(id) {
  saving.value = true
  error.value = ''
  try {
    await updateUniversity(id, { ...form.value })
    editingId.value = null
    await load()
  } catch (e) { error.value = e.response?.data ?? e.message }
  finally { saving.value = false }
}

async function remove(id) {
  if (!confirm('이 대학을 삭제하시겠습니까?')) return
  saving.value = true
  error.value = ''
  try {
    await deleteUniversity(id)
    await load()
  } catch (e) { error.value = e.response?.data ?? e.message }
  finally { saving.value = false }
}

onMounted(load)
</script>
