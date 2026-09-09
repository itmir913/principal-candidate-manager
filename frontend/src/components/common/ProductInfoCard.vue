<template>
  <!-- 제품 정보 카드 — 무엇을 쓰고 있는지(제품명·제작자·버전)를 알린다.
       관리자 개요 탭에만 있던 것을 담임 화면과 공유하려고 컴포넌트로 뽑았다.

       AppInfoCard 와 역할이 다르다: 이쪽은 "이 소프트웨어가 무엇인가",
       AppInfoCard 는 "이 설치본을 학교가 뭐라고 이름 붙였는가"다. -->
  <div
    class="rounded-xl"
    style="padding: 20px 24px; background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04);"
  >
    <div class="grid grid-cols-1 lg:grid-cols-2 gap-3">
      <div>
        <p class="text-base font-semibold" style="color: #94a3b8; text-transform: uppercase; letter-spacing: 0.07em;">
          Teacher Utility Kit
        </p>
        <p class="text-xl font-bold mt-0.5" style="color: #1e293b;">{{ appInfo.fullTitle }}</p>
      </div>
      <div class="lg:text-right">
        <p class="text-base font-semibold" style="color: #475569;">© luminousky</p>
        <p class="text-base mt-0.5" style="color: #94a3b8;">
          Principal Candidate Manager<template v-if="shownVersion"> · v{{ shownVersion }}</template>
        </p>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted } from 'vue'
import axios from 'axios'
import { useAppInfoStore } from '../../stores/appInfo.js'

// 관리자 개요는 이미 overview 응답에 버전이 실려 오므로 그대로 넘긴다.
// 담임 화면에는 그 API 가 없어서, 값이 없으면 공개 엔드포인트로 직접 받는다.
const props = defineProps({
  version: { type: String, default: '' },
})

const appInfo = useAppInfoStore()
const fetched = ref('')

const shownVersion = computed(() => props.version || fetched.value)

onMounted(async () => {
  if (props.version) return
  try {
    const res = await axios.get('/api/version')
    fetched.value = res.data.version
  } catch {
    // 버전을 못 받아도 카드 자체는 그린다 — 제품명·제작자만으로도 쓸모가 있다
  }
})
</script>
