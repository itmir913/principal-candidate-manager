<template>
  <!-- 담임 화면 상단의 프로그램 식별 카드.
       인원제한 O/X 두 프로그램을 함께 돌릴 때 담임이 지금 어느 쪽에 접속했는지 알아야 한다.
       설명(desc)에는 "인원 제한 있는 학교" 같은 구분이 들어오므로 장식이 아니라 읽는
       문구다 — 제목보다 작게 두되 회색으로 흐리지 않는다.

       컴포넌트로 뽑은 이유: 담임 세 탭과 관리자 개요에 같은 카드가 들어가는데,
       각자 마크업을 갖고 있으면 한쪽만 고쳐져 서서히 갈라진다.

       관리자가 제목을 지정하지 않았으면 아예 그리지 않는다. 이 카드는 인스턴스가 둘 이상일 때
       어느 쪽인지 가리는 장치인데, 설정하지 않았다는 건 대개 인스턴스가 하나라는 뜻이다.
       게다가 기본 문구는 고정 제품명을 쪼갠 것이라 바로 위 제품 정보 카드와 같은 말이 두 번 뜬다.
       로그인·시작 화면과 브라우저 탭은 무엇이든 띄워야 하므로 거기서는 기본 문구를 그대로 쓴다. -->
  <div
    v-if="appInfo.configured"
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
