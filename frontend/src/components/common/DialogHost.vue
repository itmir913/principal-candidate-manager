<template>
  <Teleport to="body">
    <!-- 배경 클릭으로 닫히지 않음: backdrop에 클릭 핸들러를 달지 않는다 -->
    <div
      v-if="s.open"
      class="fixed inset-0 flex items-center justify-center"
      style="background: rgba(0,0,0,0.35); z-index: 60;"
      role="dialog"
      aria-modal="true"
    >
      <div
        class="bg-white flex flex-col"
        style="border-radius: 14px; box-shadow: 0 8px 32px rgba(0,0,0,0.15); width: 100%; max-width: 460px; margin: 0 16px; padding: 1.5rem 1.75rem; max-height: 80vh; overflow-y: auto;"
      >
        <!-- 제목 -->
        <div class="flex items-center gap-2 mb-3">
          <AlertTriangle
            v-if="isErrorAlert || (s.kind === 'confirm' && s.level === 'danger')"
            :size="20" style="color: #ef4444;" class="flex-shrink-0"
          />
          <h2 class="text-lg font-semibold" style="margin: 0;" :style="{ color: isErrorAlert ? '#b91c1c' : '#1e293b' }">
            {{ s.title }}
          </h2>
        </div>

        <!-- 메시지 (\n 줄바꿈 유지) -->
        <p class="text-base" style="margin: 0; color: #475569; line-height: 1.6; white-space: pre-line;">{{ s.message }}</p>

        <!-- danger 2단계 경고 패널 -->
        <div
          v-if="s.kind === 'confirm' && s.level === 'danger' && s.step === 2"
          class="rounded-lg mt-4"
          style="padding: 12px 16px; background: #fef2f2; border: 1px solid #fca5a5;"
        >
          <div class="flex items-center gap-2" :class="s.dangerNotice ? 'mb-1' : ''">
            <AlertTriangle :size="16" style="color: #ef4444;" class="flex-shrink-0" />
            <span class="text-base font-semibold" style="color: #b91c1c;">정말로 진행하시겠습니까?</span>
          </div>
          <p v-if="s.dangerNotice" class="text-base" style="margin: 0; color: #b91c1c; line-height: 1.6; white-space: pre-line;">
            {{ s.dangerNotice }}
          </p>
        </div>

        <!-- 버튼 -->
        <div class="flex gap-2 justify-end mt-6">
          <!-- alert: 확인 하나 -->
          <button
            v-if="s.kind === 'alert'"
            ref="primaryBtn"
            class="text-base font-semibold rounded-lg"
            style="padding: 9px 20px; border: none; background: #2563eb; color: white; cursor: pointer;"
            @click="settleDialog(true)"
          >{{ s.confirmText }}</button>

          <template v-else>
            <button
              ref="cancelBtn"
              class="text-base rounded-lg"
              style="padding: 9px 20px; border: 1px solid #e2e8f0; background: white; color: #64748b; cursor: pointer;"
              @click="settleDialog(false)"
            >{{ s.cancelText }}</button>

            <!-- normal: 파란 배경 -->
            <button
              v-if="s.level === 'normal'"
              class="text-base font-semibold rounded-lg"
              style="padding: 9px 20px; border: none; background: #2563eb; color: white; cursor: pointer;"
              @click="settleDialog(true)"
            >{{ s.confirmText }}</button>

            <!-- warn: 흰 배경 + 빨간 테두리 -->
            <button
              v-else-if="s.level === 'warn'"
              class="text-base font-semibold rounded-lg"
              style="padding: 9px 20px; border: 1px solid #fca5a5; background: white; color: #ef4444; cursor: pointer;"
              @click="settleDialog(true)"
            >{{ s.confirmText }}</button>

            <!-- danger 1단계: 흰 배경 + 빨간 테두리 → 누르면 2단계 진입 -->
            <button
              v-else-if="s.step === 1"
              class="text-base font-semibold rounded-lg"
              style="padding: 9px 20px; border: 1px solid #fca5a5; background: white; color: #ef4444; cursor: pointer;"
              @click="s.step = 2"
            >{{ s.confirmText }}</button>

            <!-- danger 2단계: 빨간 배경 최종 실행 -->
            <button
              v-else
              class="text-base font-semibold rounded-lg"
              style="padding: 9px 20px; border: none; background: #ef4444; color: white; cursor: pointer;"
              @click="settleDialog(true)"
            >{{ s.finalConfirmText }}</button>
          </template>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<script setup>
import { ref, computed, watch, nextTick, onMounted, onBeforeUnmount } from 'vue'
import { AlertTriangle } from 'lucide-vue-next'
import { dialogState as s, settleDialog } from './dialog.js'

const isErrorAlert = computed(() => s.kind === 'alert' && s.level === 'error')

const cancelBtn = ref(null)
const primaryBtn = ref(null)

// 열릴 때 기본 포커스: confirm은 취소 버튼(Enter 오조작 방지), alert는 확인 버튼
watch(() => s.open, async (open) => {
  if (!open) return
  await nextTick()
  if (s.kind === 'alert') primaryBtn.value?.focus()
  else cancelBtn.value?.focus()
})

// ESC = 취소 (alert는 닫기). danger 2단계 상태에서도 즉시 취소.
function onKeydown(e) {
  if (!s.open) return
  if (e.key === 'Escape') {
    e.preventDefault()
    settleDialog(s.kind === 'alert')
  }
}

onMounted(() => window.addEventListener('keydown', onKeydown))
onBeforeUnmount(() => window.removeEventListener('keydown', onKeydown))
</script>
