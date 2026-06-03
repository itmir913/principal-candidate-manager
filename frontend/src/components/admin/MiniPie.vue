<template>
  <svg :width="size" :height="size" :viewBox="`0 0 ${size} ${size}`">
    <circle :cx="cx" :cy="cy" :r="R" fill="none" stroke="#e2e8f0" stroke-width="6" />
    <circle
      v-if="filled > 0"
      :cx="cx"
      :cy="cy"
      :r="R"
      fill="none"
      :stroke="isFull ? '#ef4444' : '#2563eb'"
      stroke-width="6"
      :stroke-dasharray="`${arc} ${circ}`"
      :stroke-dashoffset="circ / 4"
      :stroke-linecap="isFull ? 'butt' : 'round'"
    />
  </svg>
</template>

<script setup>
import { computed } from 'vue'

const props = defineProps({
  filled: { type: Number, required: true },
  total:  { type: Number, required: true },
  size:   { type: Number, default: 40 },
})

const R    = 14
const cx   = computed(() => props.size / 2)
const cy   = computed(() => props.size / 2)
const circ = computed(() => 2 * Math.PI * R)
const arc  = computed(() => (props.total > 0 ? props.filled / props.total : 0) * circ.value)
const isFull = computed(() => props.filled >= props.total)
</script>
