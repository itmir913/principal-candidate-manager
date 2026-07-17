<template>
  <div
    class="fixed inset-0 flex items-center justify-center z-50"
    style="background: rgba(0,0,0,0.35);"
    @click.self="$emit('close')"
    @keydown.escape.window="$emit('close')"
  >
    <div
      class="bg-white flex flex-col"
      style="border-radius: 14px; box-shadow: 0 8px 32px rgba(0,0,0,0.15); width: 100%; max-width: 640px; margin: 0 16px; max-height: 85vh;"
    >
      <!-- 헤더 -->
      <div
        class="flex items-center justify-between flex-shrink-0"
        style="padding: 18px 22px; border-bottom: 1px solid #f1f5f9;"
      >
        <div>
          <h3 class="text-lg font-semibold" style="color: #1e293b; margin: 0;">
            {{ studentName }} — {{ app.univ_name }} {{ app.track_name }}
          </h3>
        </div>
        <button
          class="text-xl leading-none"
          style="background: none; border: none; cursor: pointer; color: #94a3b8;"
          @click="$emit('close')"
        >✕</button>
      </div>

      <!-- 본문 -->
      <div class="overflow-y-auto flex-1" style="padding: 18px 22px;">

        <!-- 기본 정보 -->
        <div class="rounded-xl mb-5" style="background: #f8fafc; padding: 14px 16px; border: 1px solid #e2e8f0;">
          <div class="grid grid-cols-1 gap-2" style="grid-template-columns: auto 1fr;">
            <span class="text-base font-medium" style="color: #64748b;">대학</span>
            <span class="text-base" style="color: #1e293b;">{{ app.univ_name }}</span>
            <span class="text-base font-medium" style="color: #64748b;">모집단위</span>
            <span class="text-base" style="color: #1e293b;">{{ app.track_name }}</span>
            <span class="text-base font-medium" style="color: #64748b;">학과명</span>
            <span class="text-base" style="color: #1e293b;">{{ app.department_name || '—' }}</span>
          </div>
        </div>

        <!-- 전형요소 표 -->
        <div v-if="loading" class="text-base text-center" style="color: #94a3b8; padding: 2rem 0;">
          로딩 중...
        </div>

        <template v-else-if="areaContext.length > 0">
          <p class="text-base font-semibold mb-3" style="color: #475569; text-transform: uppercase; letter-spacing: 0.05em;">
            전형요소
          </p>
          <div class="rounded-xl overflow-hidden" style="border: 1px solid #e2e8f0;">
            <table class="w-full" style="border-collapse: collapse;">
              <thead>
                <tr style="background: #f8fafc; border-bottom: 1px solid #e2e8f0;">
                  <th class="text-base font-semibold text-left" style="padding: 10px 14px; color: #475569;">전형요소명</th>
                  <th class="text-base font-semibold text-left" style="padding: 10px 14px; color: #475569;">유형</th>
                  <th class="text-base font-semibold text-left" style="padding: 10px 14px; color: #475569;">입력값</th>
                  <th class="text-base font-semibold text-right" style="padding: 10px 14px; color: #475569; white-space: nowrap;">예상 점수</th>
                </tr>
              </thead>
              <tbody>
                <tr
                  v-for="area in areaContext"
                  :key="area.area_id"
                  style="border-bottom: 1px solid #f1f5f9;"
                >
                  <td class="text-base" style="padding: 10px 14px; color: #1e293b;">
                    <div class="flex items-center gap-2">
                      <span>{{ area.area_name }}</span>
                      <span
                        v-if="!area.teacher_editable"
                        class="text-base flex-shrink-0"
                        style="padding: 2px 7px; border-radius: 6px; background: #fffbeb; color: #92400e;"
                      >관리자 입력 고정</span>
                    </div>
                  </td>
                  <td class="text-base" style="padding: 10px 14px;">
                    <span style="padding: 2px 8px; border-radius: 6px; background: #f1f5f9; color: #64748b;">{{ area.calc_type }}</span>
                  </td>
                  <td class="text-base" style="padding: 10px 14px;">
                    <span
                      v-if="area.current_values.length > 0"
                      style="color: #1e293b;"
                    >{{ area.current_values.join(', ') }}</span>
                    <span v-else style="color: #ef4444;">데이터 없음</span>
                  </td>
                  <td class="text-base text-right" style="padding: 10px 14px; white-space: nowrap;">
                    <span v-if="scorePreviews[area.area_id] === undefined" style="color: #94a3b8;">—</span>
                    <span v-else-if="scorePreviews[area.area_id]?.error" style="color: #ef4444; font-size: 0.875rem;">
                      {{ scorePreviews[area.area_id].error }}
                    </span>
                    <span
                      v-else-if="scorePreviews[area.area_id]?.score !== null && scorePreviews[area.area_id]?.score !== undefined"
                      style="color: #2563eb; font-weight: 500;"
                    >{{ Number(scorePreviews[area.area_id].score).toFixed(2) }}점</span>
                    <span v-else style="color: #94a3b8;">—</span>
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </template>
      </div>

      <!-- 버튼 영역 -->
      <div
        class="flex items-center justify-end gap-3 flex-shrink-0"
        style="padding: 14px 22px; border-top: 1px solid #f1f5f9;"
      >
        <button
          class="text-base"
          style="padding: 9px 20px; border: 1px solid #e2e8f0; background: white; color: #475569; border-radius: 8px; cursor: pointer;"
          @click="$emit('close')"
        >닫기</button>
        <button
          class="text-base font-medium"
          style="padding: 9px 20px; border: 1px solid #fca5a5; background: white; color: #ef4444; border-radius: 8px; cursor: pointer;"
          :disabled="deleting"
          @click="onDelete"
        >{{ deleting ? '취소 중...' : '지원 취소' }}</button>
        <button
          class="text-base font-semibold"
          style="padding: 9px 22px; border: none; background: #2563eb; color: white; border-radius: 8px; cursor: pointer;"
          @click="onEdit"
        >수정</button>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import { dialog } from '../common/dialog.js'
import { teacherGetAreaContext, teacherAreaScorePreview, teacherDeleteApplication } from '../../api/teacher.js'

const props = defineProps({
  app: { type: Object, required: true },
  studentName: { type: String, required: true },
})
const emit = defineEmits(['close', 'edit', 'deleted'])

const loading   = ref(true)
const deleting  = ref(false)
const areaContext   = ref([])
const scorePreviews = ref({})

onMounted(async () => {
  try {
    const ctx = await teacherGetAreaContext(props.app.student_id, props.app.track_id)
    areaContext.value = ctx

    for (const area of ctx) {
      const vals = area.current_values.filter(v => v !== '')
      if (vals.length > 0) {
        try {
          const result = await teacherAreaScorePreview(area.area_id, props.app.track_id, vals)
          scorePreviews.value = { ...scorePreviews.value, [area.area_id]: result }
        } catch {
          scorePreviews.value = { ...scorePreviews.value, [area.area_id]: { score: null, error: '계산 실패' } }
        }
      }
    }
  } finally {
    loading.value = false
  }
})

async function onDelete() {
  const confirmed = await dialog.confirm({
    title: '지원 취소',
    message: `${props.app.univ_name} ${props.app.track_name} 지원을 취소하시겠습니까?\n라운드가 진행 중인 동안에는 다시 등록할 수 있습니다.`,
    confirmText: '지원 취소',
    level: 'warn',
  })
  if (!confirmed) return

  deleting.value = true
  try {
    await teacherDeleteApplication(props.app.student_id, props.app.track_id, props.app.round_id)
    emit('deleted')
  } catch (e) {
    await dialog.alert({ title: '오류', message: e.response?.data || e.message, level: 'error' })
  } finally {
    deleting.value = false
  }
}

function onEdit() {
  emit('edit', props.app)
  emit('close')
}
</script>
