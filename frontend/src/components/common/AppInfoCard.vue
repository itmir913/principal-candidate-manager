<template>
  <!-- 담임 화면 상단의 프로그램 식별 카드.
       인원제한 O/X 두 프로그램을 함께 돌릴 때 담임이 지금 어느 쪽에 접속했는지 알아야 한다.
       설명(desc)에는 "인원 제한 있는 학교" 같은 구분이 들어오므로 장식이 아니라 읽는
       문구다 — 제목보다 작게 두되 회색으로 흐리지 않는다.

       컴포넌트로 뽑은 이유: 학급 관리·지원자 등록 두 탭에 같은 카드가 들어가는데,
       각자 마크업을 갖고 있으면 한쪽만 고쳐져 서서히 갈라진다. -->
  <div
    class="rounded-xl"
    style="padding: 22px 24px; background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04);"
  >
    <p class="text-2xl font-bold" style="color: #1e293b; margin: 0;">{{ appInfo.title }}</p>
    <p
      v-if="appInfo.desc"
      class="text-lg"
      style="color: #475569; margin: 8px 0 0; line-height: 1.5;"
    >{{ appInfo.desc }}</p>
  </div>
</template>

<script setup>
import { useAppInfoStore } from '../../stores/appInfo.js'

const appInfo = useAppInfoStore()
</script>
