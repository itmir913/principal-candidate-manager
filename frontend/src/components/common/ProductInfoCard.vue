<template>
  <!-- 관리자·담임 화면 맨 위의 머리 카드.
       왼쪽은 "지금 어느 프로그램인가"(학교가 붙인 이름 + 설명), 오른쪽은 "이 소프트웨어가
       무엇인가"(제작자·제품명·버전)다.

       원래 두 장이었는데 합쳤다. 제목을 지정하지 않은 학교에서는 두 카드가 같은 말을 두 번
       하고 있었다 — 왼쪽 기본 제목이 오른쪽 제품명과 사실상 같은 문구였기 때문이다.
       그래서 지정 전에는 왼쪽에 제품명 한 줄만 두고 설명 줄은 그리지 않는다. -->
  <div
    class="rounded-xl"
    style="padding: 20px 24px; background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04);"
  >
    <!-- 두 열의 줄 수가 다르다(왼쪽 최대 3줄, 오른쪽 2줄). 위로 붙이면 오른쪽 아래가 크게
         비어 카드 절반의 여백이 어긋나 보인다 — 넓은 화면에서는 세로 가운데로 맞춘다.
         좁은 화면에서는 세로로 쌓이므로 그냥 왼쪽 정렬이다. -->
    <div class="grid grid-cols-1 lg:grid-cols-2 gap-3 lg:items-center">
      <div class="min-w-0">
        <!-- 줄 높이를 명시한다. text-base 의 기본 줄 높이는 24px 이라 16px 라벨에 느슨하고,
             그만큼이 카드 위쪽 여백에 얹혀 아래쪽보다 넓어 보인다. -->
        <p
          class="text-base font-semibold"
          style="color: #94a3b8; text-transform: uppercase; letter-spacing: 0.07em; margin: 0; line-height: 1.25;"
        >Teacher Utility Kit</p>
        <p class="text-2xl font-bold" style="color: #1e293b; margin: 6px 0 0; line-height: 1.3;">{{ appInfo.cardTitle }}</p>
        <!-- 설명에는 "인원 제한 있는 대학" 같은 구분이 들어온다. 장식이 아니라 담임이 읽고
             판단하는 문구라 흐린 회색을 쓰지 않는다. -->
        <p
          v-if="appInfo.cardDesc"
          class="text-lg"
          style="color: #475569; margin: 6px 0 0; line-height: 1.4;"
        >{{ appInfo.cardDesc }}</p>
      </div>
      <div class="lg:text-right">
        <p class="text-base font-semibold" style="color: #475569; margin: 0; line-height: 1.4;">© luminousky</p>
        <p class="text-base" style="color: #94a3b8; margin: 4px 0 0; line-height: 1.4;">
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
