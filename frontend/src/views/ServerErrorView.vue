<template>
  <div class="min-h-screen flex items-center justify-center p-6" style="background: #eeecea;">
    <div
      class="w-full bg-white"
      style="max-width: 840px; border-radius: 20px; box-shadow: 0 8px 40px rgba(0,0,0,0.12), 0 0 0 1px rgba(0,0,0,0.05); padding: 2.5rem;"
    >
      <!-- 헤더 -->
      <div class="text-center mb-8">
        <div
          class="inline-flex items-center justify-center rounded-2xl mb-4"
          style="width: 56px; height: 56px; background: #fef2f2;"
        >
          <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="#dc2626" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="10"/>
            <line x1="12" y1="8" x2="12" y2="12"/>
            <line x1="12" y1="16" x2="12.01" y2="16"/>
          </svg>
        </div>
        <h1 class="text-2xl font-bold" style="color: #1e293b; margin: 0 0 6px;">서버 시작 오류</h1>
        <p class="text-base" style="color: #94a3b8; margin: 0;">학교장 추천자 선발 관리 시스템</p>
      </div>

      <!-- 스키마 버전 불일치 -->
      <template v-if="code === 'SCHEMA_TOO_NEW'">
        <div
          class="rounded-xl text-base leading-relaxed mb-6"
          style="padding: 20px 22px; background: #fff7ed; border: 1px solid #fed7aa; color: #7c2d12;"
        >
          <p class="font-semibold mb-3" style="color: #9a3412;">버전 불일치 오류</p>
          <p class="mb-4">
            현재 데이터베이스는 <strong>더 높은 버전의 앱</strong>에서 생성된 것입니다.<br>
            이 앱으로는 해당 데이터베이스를 열 수 없습니다.
          </p>
          <div class="flex gap-6 text-base" style="color: #92400e;">
            <div
              class="flex-1 rounded-lg text-center"
              style="padding: 12px; background: #fef3c7; border: 1px solid #fde68a;"
            >
              <div class="font-semibold mb-1">현재 데이터베이스</div>
              <div class="text-2xl font-bold" style="color: #b45309;">v{{ dbVer }}</div>
            </div>
            <div class="flex items-center" style="color: #d97706; font-size: 1.5rem;">→</div>
            <div
              class="flex-1 rounded-lg text-center"
              style="padding: 12px; background: #fef9c3; border: 1px solid #fef08a;"
            >
              <div class="font-semibold mb-1">이 앱 지원 버전</div>
              <div class="text-2xl font-bold" style="color: #b45309;">v{{ appVer }}</div>
            </div>
          </div>
        </div>

        <div
          class="rounded-xl text-base leading-relaxed mb-6"
          style="padding: 16px 20px; background: #eff6ff; border: 1px solid #bfdbfe; color: #1d4ed8;"
        >
          <p class="font-semibold mb-1.5" style="color: #1e40af;">해결 방법</p>
          <p>최신 버전의 앱을 다운로드하여 설치한 후 다시 접속해 주세요.</p>
        </div>
      </template>

      <!-- 일반 서버 오류 -->
      <template v-else>
        <div
          class="rounded-xl text-base leading-relaxed mb-6"
          style="padding: 20px 22px; background: #fef2f2; border: 1px solid #fecaca; color: #7f1d1d;"
        >
          <p class="font-semibold mb-3" style="color: #991b1b;">서버 초기화 실패</p>
          <p class="mb-0" style="white-space: pre-wrap; word-break: break-all;">{{ message }}</p>
        </div>
      </template>

      <!-- 공통 안내 -->
      <div
        class="rounded-xl text-base"
        style="padding: 14px 18px; background: #f8fafc; border: 1px solid #e2e8f0; color: #64748b;"
      >
        서버를 실행 중인 컴퓨터의 담당자에게 문의하세요.
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed } from 'vue'
import { useRoute } from 'vue-router'

const route = useRoute()

const code    = computed(() => route.query.code ?? 'SERVER_ERROR')
const message = computed(() => route.query.message ?? '알 수 없는 오류가 발생했습니다.')
const dbVer   = computed(() => route.query.db_ver)
const appVer  = computed(() => route.query.app_ver)
</script>
