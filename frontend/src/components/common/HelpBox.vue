<template>
  <div class="rounded-xl" :style="{ background: c.bg, border: `1px solid ${c.border}` }">
    <!-- 헤더(토글 버튼) — 접힌 상태에서도 항상 표시 -->
    <button
      type="button"
      class="w-full flex items-center gap-2 text-base font-semibold"
      style="padding: 12px 16px; background: none; border: none; cursor: pointer; text-align: left;"
      :style="{ color: c.title }"
      @click="toggle"
    >
      <HelpCircle :size="18" class="flex-shrink-0" />
      <span class="flex-1 min-w-0">{{ title }}</span>
      <ChevronUp v-if="open" :size="18" class="flex-shrink-0" />
      <ChevronDown v-else :size="18" class="flex-shrink-0" />
    </button>

    <!-- 본문 -->
    <div v-if="open" style="padding: 0 16px 14px 46px;">
      <p v-if="intro" class="text-base" style="margin: 0 0 8px; line-height: 1.6;" :style="{ color: c.body }">
        {{ intro }}
      </p>
      <ul
        v-if="normalizedItems.length"
        class="text-base"
        style="margin: 0; padding-left: 20px; line-height: 1.6; display: flex; flex-direction: column; gap: 4px; list-style: disc;"
      >
        <li v-for="(item, i) in normalizedItems" :key="i" :style="{ color: item.warn ? c.warn : c.body }">
          <template v-if="item.warn">⚠ </template>{{ item.text }}
        </li>
      </ul>
      <slot />
    </div>
  </div>
</template>

<script setup>
import { ref, computed, watch } from 'vue'
import { HelpCircle, ChevronDown, ChevronUp } from 'lucide-vue-next'

const props = defineProps({
  /** 접힌 상태에서도 항상 표시되는 제목 */
  title: { type: String, required: true },
  /** 접힘 상태 기억용 localStorage 키. 탭×상태별로 고유해야 함 */
  storageKey: { type: String, required: true },
  /** 'info'(파랑, 기본) | 'warning'(호박) */
  variant: { type: String, default: 'info' },
  /** 이 탭이 무엇을 하는 곳인지 설명하는 1문장 */
  intro: { type: String, default: '' },
  /** 불릿 목록: 문자열 또는 { text, warn: true } */
  items: { type: Array, default: () => [] },
})

const COLORS = {
  info:    { bg: '#eff6ff', border: '#bfdbfe', title: '#1d4ed8', body: '#1e40af', warn: '#b45309' },
  warning: { bg: '#fffbeb', border: '#fcd34d', title: '#92400e', body: '#78350f', warn: '#b91c1c' },
}
const c = computed(() => COLORS[props.variant] ?? COLORS.info)

const normalizedItems = computed(() =>
  props.items.map(it =>
    typeof it === 'string' ? { text: it, warn: false } : { text: it.text, warn: !!it.warn }
  )
)

const lsKey = computed(() => `pcm-help-collapsed:${props.storageKey}`)

function readOpen() {
  // 기본 접힘. 사용자가 펼친 적 있으면('1') 펼침 유지.
  try { return localStorage.getItem(lsKey.value) === '1' } catch { return false }
}

const open = ref(readOpen())

// 상태 전환으로 storageKey가 바뀌면 새 키 기준으로 펼침 상태 재계산
watch(() => props.storageKey, () => { open.value = readOpen() })

function toggle() {
  open.value = !open.value
  try { localStorage.setItem(lsKey.value, open.value ? '1' : '0') } catch { /* 저장 불가 환경에서는 세션 내 토글만 동작 */ }
}
</script>
