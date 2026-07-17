<template>
  <div class="py-8 px-4 sm:px-10">

    <!-- 페이지 헤더 -->
    <div class="flex items-end justify-between flex-wrap gap-3 mb-5">
      <div>
        <p class="text-base mb-1" style="color: #94a3b8;">관리자</p>
        <h1 class="text-2xl font-semibold" style="color: #1e293b; margin: 0;">매뉴얼</h1>
      </div>
      <a
        href="/manual.html"
        target="_blank"
        rel="noopener noreferrer"
        class="inline-flex items-center gap-1.5 text-base font-medium rounded-lg"
        style="padding: 9px 16px; border: 1px solid #e2e8f0; background: white; color: #475569; text-decoration: none;"
      >
        <component :is="ExternalLink" :size="16" />
        더 자세한 매뉴얼 확인하기
      </a>
    </div>

    <!-- 서브탭 네비게이션 -->
    <div class="flex mb-6 overflow-x-auto" style="border-bottom: 1px solid #e2e8f0;">
      <button
        v-for="tab in tabs"
        :key="tab.key"
        class="text-base font-medium transition-colors flex items-center gap-2"
        style="padding: 10px 20px; border: none; background: none; cursor: pointer; border-bottom: 2px solid transparent; margin-bottom: -1px; white-space: nowrap;"
        :style="{
          borderBottomColor: activeTab === tab.key ? '#2563eb' : 'transparent',
          color: activeTab === tab.key ? '#2563eb' : '#64748b',
          fontWeight: activeTab === tab.key ? '600' : '400',
        }"
        @click="activeTab = tab.key"
      >
        <component :is="tab.icon" :size="16" />
        {{ tab.label }}
      </button>
    </div>

    <!-- ── 전체 흐름 ─────────────────────────────────────────────── -->
    <div v-if="activeTab === 'overview'">
      <p class="text-base mb-6" style="color: #475569; line-height: 1.7;">
        학교장추천 선발은 크게 <strong>사전 설정 → 라운드 운영 → 선발 완료</strong> 순서로 진행됩니다.
        처음 시스템을 사용할 때는 사전 설정을 먼저 완료해 주세요. 이후 매 선발마다 라운드 운영만 반복하면 됩니다.
        관리자·담임교사의 주요 화면 상단에는 파란색 <strong>도움말</strong> 박스가 표시되어, 현재 화면에서 해야 할 일과 주의사항을
        라운드 상태(없음·진행중·종료·마감)에 맞추어 안내합니다. 제목 줄을 클릭하면 접고 펼 수 있으며, 접은 상태는 브라우저에 기억됩니다.
      </p>

      <div class="rounded-xl mb-5" style="background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04); overflow: hidden;">
        <div class="px-6 pt-5 pb-5">
          <div class="flex items-center gap-2 mb-3">
            <span class="text-base font-bold" style="color: #1d4ed8; background: #dbeafe; padding: 3px 12px; border-radius: 6px;">1단계 · 사전 설정</span>
            <span class="text-base" style="color: #94a3b8;">학교장추천전형 시작 전 최초 1회</span>
          </div>
          <div class="flex flex-wrap gap-3">
            <div
              v-for="step in setupSteps"
              :key="step.num"
              class="flex items-start gap-3 rounded-xl"
              style="padding: 14px 16px; background: #f8fafc; border: 1px solid #e2e8f0; flex: 1 1 190px;"
            >
              <div
                class="flex items-center justify-center rounded-full flex-shrink-0 font-bold text-base"
                style="width: 28px; height: 28px; background: #dbeafe; color: #1d4ed8;"
              >{{ step.num }}</div>
              <div>
                <p class="text-base font-semibold" style="color: #1e293b; margin: 0;">{{ step.title }}</p>
                <p class="text-base" style="color: #64748b; margin: 4px 0 0; line-height: 1.5;">{{ step.desc }}</p>
              </div>
            </div>
          </div>
        </div>

        <div style="border-bottom: 1px solid #f1f5f9;"></div>

        <div class="px-6 pt-5 pb-5">
          <div class="flex items-center gap-2 mb-3">
            <span class="text-base font-bold" style="color: #7c3aed; background: #f3e8ff; padding: 3px 12px; border-radius: 6px;">2단계 · 라운드 운영</span>
            <span class="text-base" style="color: #94a3b8;">매 선발마다 반복</span>
          </div>
          <div class="flex flex-wrap gap-3">
            <div
              v-for="step in roundSteps"
              :key="step.num"
              class="flex items-start gap-3 rounded-xl"
              style="padding: 14px 16px; background: #faf5ff; border: 1px solid #e9d5ff; flex: 1 1 170px;"
            >
              <div
                class="flex items-center justify-center rounded-full flex-shrink-0 font-bold text-base"
                style="width: 28px; height: 28px; background: #f3e8ff; color: #7c3aed;"
              >{{ step.num }}</div>
              <div>
                <p class="text-base font-semibold" style="color: #1e293b; margin: 0;">{{ step.title }}</p>
                <p class="text-base" style="color: #64748b; margin: 4px 0 0; line-height: 1.5;">{{ step.desc }}</p>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div class="rounded-xl" style="padding: 18px 22px; background: #fffbeb; border: 1px solid #fcd34d;">
        <h3 class="text-base font-semibold" style="color: #92400e;">시작 전 꼭 확인하세요</h3>
        <ul class="text-base space-y-2 mt-3" style="color: #78350f; padding-left: 0; list-style: none;">
          <li class="flex items-start gap-2">
            <span>•</span>
            <span><strong>대학·모집단위는 지원 기록이 생기면 삭제할 수 없고, 전형요소는 종료·마감된 라운드가 하나라도 있으면 수정·삭제할 수 없습니다.</strong> 전형요소 설정과 대학 설정을 완전히 마친 뒤 라운드를 여세요.</span>
          </li>
          <li class="flex items-start gap-2">
            <span>•</span>
            <span>라운드 종료 시 모든 지원자의 전형요소 점수가 자동으로 계산되어 순위가 산출됩니다. <strong>기초 데이터가 누락된 지원자가 있으면 점수 계산에 실패하여 라운드를 종료할 수 없습니다.</strong> 종료 전 모든 지원자의 데이터가 입력되었는지 확인하세요.</span>
          </li>
        </ul>
      </div>
    </div>

    <!-- ── 사전 설정 ─────────────────────────────────────────────── -->
    <div v-else-if="activeTab === 'setup'">
      <p class="text-base mb-6" style="color: #475569; line-height: 1.7;">
        시스템을 처음 사용하거나 새 학년도가 시작되면 아래 순서대로 기초 정보를 설정해 주세요.
        전형요소 설정과 대학 설정은 이후 "전형요소 설정" 탭에서 자세한 방법을 확인할 수 있습니다.
      </p>

      <div class="space-y-4">
        <div
          v-for="item in setupGuides"
          :key="item.step"
          class="rounded-xl"
          style="background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04); overflow: hidden;"
        >
          <div class="flex items-center gap-4 px-6 py-4" style="border-bottom: 1px solid #f1f5f9;">
            <div
              class="flex items-center justify-center rounded-full font-bold text-base flex-shrink-0"
              style="width: 32px; height: 32px; background: #dbeafe; color: #1d4ed8;"
            >{{ item.step }}</div>
            <div class="flex-1">
              <p class="text-base font-semibold" style="color: #1e293b; margin: 0;">{{ item.title }}</p>
              <p class="text-base" style="color: #94a3b8; margin: 2px 0 0;">{{ item.where }}</p>
            </div>
          </div>
          <div class="px-6 py-5 text-base" style="color: #475569; line-height: 1.7;">
            {{ item.desc }}
          </div>
          <div v-if="item.note" class="px-6 pb-5">
            <div class="rounded-lg text-base" style="padding: 10px 14px; background: #f0fdf4; border: 1px solid #bbf7d0; color: #166534;">
              {{ item.note }}
            </div>
          </div>
          <div v-if="item.warning" class="px-6 pb-5">
            <div class="rounded-lg text-base" style="padding: 10px 14px; background: #fffbeb; border: 1px solid #fcd34d; color: #92400e;">
              {{ item.warning }}
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- ── 전형요소 설정 ─────────────────────────────────────────── -->
    <div v-else-if="activeTab === 'areas'">
      <p class="text-base mb-6" style="color: #475569; line-height: 1.7;">
        전형요소는 각 고등학교가 학교장추천 전형에서 반영하는 평가 항목입니다 (예: 교과 성적, 수상 경력, 봉사 시간 등).
        전형요소마다 계산 유형을 지정하고, 유형에 따라 점수 기준표와 기초 데이터를 업로드하면 점수가 자동으로 계산됩니다.
      </p>

      <!-- 계산 유형 -->
      <div class="rounded-xl mb-5" style="background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04); overflow: hidden;">
        <div class="px-6 py-4" style="border-bottom: 1px solid #f1f5f9;">
          <h2 class="text-base font-semibold" style="color: #1e293b; margin: 0;">계산 유형</h2>
          <p class="text-base" style="color: #64748b; margin: 4px 0 0;">전형요소를 등록할 때 아래 네 가지 계산 방식 중 하나를 선택합니다.</p>
        </div>
        <div class="px-6 py-5">
          <div class="grid gap-3" style="grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));">
            <div
              v-for="ct in calcTypes"
              :key="ct.key"
              class="rounded-xl text-base"
              style="padding: 14px 16px; background: #f8fafc; border: 1px solid #e2e8f0;"
            >
              <p class="font-semibold mb-1" style="color: #1e293b;">{{ ct.label }}</p>
              <p style="color: #64748b; margin: 0; line-height: 1.6;">{{ ct.desc }}</p>
              <p class="mt-2 text-base" style="color: #94a3b8;">예) {{ ct.example }}</p>
            </div>
          </div>
        </div>
      </div>

      <!-- 점수 기준표 업로드 -->
      <div class="rounded-xl mb-5" style="background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04); overflow: hidden;">
        <div class="px-6 py-4" style="border-bottom: 1px solid #f1f5f9;">
          <h2 class="text-base font-semibold" style="color: #1e293b; margin: 0;">점수 기준표 업로드</h2>
          <p class="text-base" style="color: #64748b; margin: 4px 0 0;">어떤 값에 몇 점을 줄지 정하는 기준표입니다. 수치 범위·범주 전형요소에만 사용하며, 직접 입력(MANUAL) 전형요소에는 필요 없습니다.</p>
        </div>
        <div class="px-6 py-5 text-base" style="color: #475569; line-height: 1.7;">
          <ul class="space-y-2 mb-4" style="padding-left: 0; list-style: none;">
            <li class="flex items-start gap-2">
              <span class="font-bold flex-shrink-0" style="color: #2563eb;">①</span>
              <span><strong>전형요소 설정 탭</strong>에서 해당 전형요소를 클릭합니다.</span>
            </li>
            <li class="flex items-start gap-2">
              <span class="font-bold flex-shrink-0" style="color: #2563eb;">②</span>
              <span>우측 상단의 <strong>점수 기준</strong> 탭을 클릭하면 양식 예시와 업로드 버튼이 나타납니다.</span>
            </li>
            <li class="flex items-start gap-2">
              <span class="font-bold flex-shrink-0" style="color: #2563eb;">③</span>
              <span>양식 예시를 참고하여 엑셀 파일을 작성한 뒤 업로드합니다.</span>
            </li>
          </ul>
          <div class="rounded-lg" style="padding: 10px 14px; background: #fffbeb; border: 1px solid #fcd34d; color: #92400e;">
            ⚠ 업로드하면 기존 점수 기준표가 전부 교체됩니다.
          </div>
        </div>
      </div>

      <!-- 기초 데이터 업로드 -->
      <div class="rounded-xl mb-5" style="background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04); overflow: hidden;">
        <div class="px-6 py-4" style="border-bottom: 1px solid #f1f5f9;">
          <h2 class="text-base font-semibold" style="color: #1e293b; margin: 0;">기초 데이터 업로드</h2>
          <p class="text-base" style="color: #64748b; margin: 4px 0 0;">학생별 실제 데이터(내신 등급, 수상 실적 등)입니다.</p>
        </div>
        <div class="px-6 py-5 text-base" style="color: #475569; line-height: 1.7;">
          <ul class="space-y-2 mb-4" style="padding-left: 0; list-style: none;">
            <li class="flex items-start gap-2">
              <span class="font-bold flex-shrink-0" style="color: #2563eb;">①</span>
              <span><strong>전형요소 설정 탭</strong>에서 해당 전형요소를 클릭합니다.</span>
            </li>
            <li class="flex items-start gap-2">
              <span class="font-bold flex-shrink-0" style="color: #2563eb;">②</span>
              <span>우측 상단의 <strong>기초 데이터</strong> 탭을 클릭하면 양식 예시와 업로드 버튼이 나타납니다.</span>
            </li>
            <li class="flex items-start gap-2">
              <span class="font-bold flex-shrink-0" style="color: #2563eb;">③</span>
              <span>학생 식별 정보(재학생: 학년·반·번호 / 졸업생: 학생코드)와 해당 항목 값이 포함된 엑셀 파일을 업로드합니다.</span>
            </li>
          </ul>
          <div class="rounded-lg" style="padding: 10px 14px; background: #f0fdf4; border: 1px solid #bbf7d0; color: #166534;">
            ✓ 담임교사 직접 입력 전형요소(담임 입력 허용 체크)는 기초 데이터 업로드가 필요 없습니다. 담임교사가 지원서 등록 시 직접 입력합니다. 대학별 기준 전형요소의 기초 데이터(석차연명부 등)를 업로드하면 대학과 모집단위가 자동으로 생성됩니다.
          </div>
          <div class="rounded-lg mt-3" style="padding: 10px 14px; background: #f0fdf4; border: 1px solid #bbf7d0; color: #166534;">
            ✓ 기초 데이터 탭에서 현재 저장된 데이터를 재학생·졸업생 구분에 따라 내보낼 수 있습니다. 재학생은 학년·반·번호·이름·값, 졸업생은 학생코드·이름·값 형식으로 내려받으며, 내려받은 파일을 수정한 뒤 같은 구분으로 그대로 재업로드할 수 있습니다.
          </div>
        </div>
      </div>

      <!-- 대학별 다른 기준 -->
      <div class="rounded-xl" style="background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04); overflow: hidden;">
        <div class="px-6 py-4" style="border-bottom: 1px solid #f1f5f9;">
          <h2 class="text-base font-semibold" style="color: #1e293b; margin: 0;">대학마다 다른 기준 적용하기</h2>
          <p class="text-base" style="color: #64748b; margin: 4px 0 0;">같은 전형요소라도 대학별로 점수 기준이 다를 때 사용합니다.</p>
        </div>
        <div class="px-6 py-5 text-base" style="color: #475569; line-height: 1.7;">
          <p class="mb-3">전형요소를 등록할 때 점수 기준을 모든 대학에 동일하게 적용할지, 대학·모집단위별로 다르게 적용할지 선택할 수 있습니다(<strong>데이터 조회 기준</strong>).</p>
          <div class="grid gap-3" style="grid-template-columns: 1fr 1fr;">
            <div class="rounded-xl" style="padding: 14px 16px; background: #f8fafc; border: 1px solid #e2e8f0;">
              <p class="text-base font-semibold mb-1" style="color: #1e293b;">공통 기준</p>
              <p class="text-base" style="color: #64748b; margin: 0; line-height: 1.6;">모든 대학에 동일한 점수 기준표를 적용합니다.</p>
            </div>
            <div class="rounded-xl" style="padding: 14px 16px; background: #f8fafc; border: 1px solid #e2e8f0;">
              <p class="text-base font-semibold mb-1" style="color: #1e293b;">대학별 기준</p>
              <p class="text-base" style="color: #64748b; margin: 0; line-height: 1.6;">대학·모집단위마다 다른 점수 기준을 적용합니다. 점수 기준표에 대학명과 모집단위명을 함께 입력합니다.</p>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- ── 라운드 운영 ─────────────────────────────────────────── -->
    <div v-else-if="activeTab === 'rounds'">
      <p class="text-base mb-6" style="color: #475569; line-height: 1.7;">
        라운드는 1회 학교장추천 선발의 단위입니다. 아래 순서대로 진행하면 됩니다.
      </p>

      <div class="space-y-4">
        <div
          v-for="step in roundGuides"
          :key="step.id"
          class="rounded-xl"
          style="background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04); overflow: hidden;"
        >
          <div class="flex items-center gap-3 px-6 py-4" style="border-bottom: 1px solid #f1f5f9;">
            <span
              class="text-base font-bold flex-shrink-0"
              style="padding: 3px 12px; border-radius: 999px;"
              :style="step.badgeStyle"
            >{{ step.badgeLabel }}</span>
            <h2 class="text-base font-semibold" style="color: #1e293b; margin: 0;">{{ step.title }}</h2>
          </div>
          <div class="px-6 py-5 text-base" style="color: #475569; line-height: 1.7;">
            <p class="mb-3">{{ step.desc }}</p>
            <ul v-if="step.items" class="space-y-2" style="padding-left: 0; list-style: none; margin: 0;">
              <li v-for="(item, i) in step.items" :key="i" class="flex items-start gap-2">
                <span class="flex-shrink-0" style="color: #7c3aed;">•</span>
                <span>{{ item }}</span>
              </li>
            </ul>
          </div>
          <div v-if="step.note" class="px-6 pb-5">
            <div class="rounded-lg text-base" style="padding: 10px 14px; background: #f0fdf4; border: 1px solid #bbf7d0; color: #166534;">
              {{ step.note }}
            </div>
          </div>
          <div v-if="step.warning" class="px-6 pb-5">
            <div class="rounded-lg text-base" style="padding: 10px 14px; background: #fffbeb; border: 1px solid #fcd34d; color: #92400e;">
              {{ step.warning }}
            </div>
          </div>
        </div>
      </div>
    </div>

  </div>
</template>

<script setup>
import { ref } from 'vue'
import { ExternalLink, LayoutList, Settings2, SlidersHorizontal, Trophy } from 'lucide-vue-next'

const activeTab = ref('overview')

const tabs = [
  { key: 'overview', label: '전체 흐름',     icon: LayoutList        },
  { key: 'setup',    label: '사전 설정',     icon: Settings2         },
  { key: 'areas',    label: '전형요소 설정', icon: SlidersHorizontal },
  { key: 'rounds',   label: '라운드 운영',   icon: Trophy            },
]

const setupSteps = [
  { num: 1, title: '학급 데이터 불러오기',  desc: '학급·담임 정보를 업로드합니다. 담임교사 계정이 자동으로 생성됩니다.' },
  { num: 2, title: '학생 데이터 불러오기',  desc: '재학생·졸업생 학적 명렬표를 각각 업로드합니다.' },
  { num: 3, title: '전형요소 설정',         desc: '평가 항목(전형요소)을 등록하고 점수 기준표와 기초 데이터를 업로드합니다.' },
  { num: 4, title: '대학 설정',             desc: '지원 가능한 대학·모집단위와 모집 정원을 입력합니다.' },
]

const roundSteps = [
  { num: 1, title: '라운드 열기 (관리자)',   desc: '담임교사가 지원자를 등록할 수 있도록 새로운 라운드를 열어 접수를 시작합니다.' },
  { num: 2, title: '지원 접수 (담임교사)',     desc: '담임교사가 지원 희망 학생의 지원서와 전형요소 데이터를 입력합니다.' },
  { num: 3, title: '라운드 종료 (관리자)',   desc: '접수를 마감합니다. 종료와 동시에 모든 지원자의 점수와 순위가 자동으로 계산됩니다.' },
  { num: 4, title: '추천 확정 (관리자)',     desc: '모집단위별 자동 계산된 순위를 검토하고 대학별 추천자를 확정합니다.' },
  { num: 5, title: '라운드 마감 (관리자)',         desc: '추천 확정이 완료되면 라운드를 최종 마감합니다. 마감 후에는 변경할 수 없습니다.' },
]

const setupGuides = [
  {
    step: 1,
    title: '학급 데이터 가져오기',
    where: '학급 관리 탭',
    desc: '학급 관리 탭에서 학급 정보를 업로드합니다. 업로드하면 담임교사 계정이 자동으로 생성되며, 담임교사는 해당 계정으로 시스템에 접속할 수 있습니다.',
    note: '✓ 학년도 초에 한 번만 불러오면 됩니다. 학급 구성이 바뀌면 다시 업로드해 주세요.',
    warning: null,
  },
  {
    step: 2,
    title: '학생 데이터 가져오기',
    where: '학생 관리 탭',
    desc: '학생 관리 탭에서 양식을 다운로드합니다. 재학생과 졸업생 양식이 구분되어 있으며, 각각 데이터를 입력하여 업로드합니다.',
    note: '✓ 가져오기는 추가·업데이트 방식으로 동작합니다. 파일에 없는 기존 학생은 삭제되지 않으며, 같은 식별자(재학생: 학년·반·번호 / 졸업생: 학생코드)가 이미 있으면 이름 등 정보가 업데이트됩니다.',
    warning: null,
  },
  {
    step: 3,
    title: '전형요소 설정',
    where: '전형요소 설정 탭',
    desc: '각 고등학교의 학교장추천전형 선발기준 평가 항목(내신, 수상, 봉사 등)을 등록합니다. 전형요소마다 점수 계산 방식을 지정하고, 유형에 따라 점수 기준표와 기초 데이터를 업로드합니다. 자세한 방법은 "전형요소 설정" 탭을 참고하세요.',
    note: '✓ 대학별 기준 전형요소(예: 환산내신등급)의 기초 데이터로 대학·모집단위별 석차연명부를 업로드하면 대학명과 모집단위명이 자동으로 생성됩니다. 이후 대학 설정 탭에서 정원과 배점만 입력하면 됩니다.',
    warning: '⚠ 종료되거나 마감된 라운드가 하나라도 있으면 전형요소 이름·점수 기준표를 수정할 수 없습니다. 라운드를 열기 전에 반드시 전형요소 설정을 완료해 주세요.',
  },
  {
    step: 4,
    title: '대학 설정',
    where: '대학 설정 탭',
    desc: '전형요소 기초 데이터 업로드로 자동 생성된 대학·모집단위를 확인합니다. 대학별 전체 정원과 재학생 우선 여부를 설정하고, 각 모집단위의 정원 및 전형요소별 배점을 입력합니다.',
    note: '✓ 모집단위를 직접 추가할 수도 있지만, 석차연명부 업로드 시 자동 생성되므로 권장하지 않습니다.',
    warning: '⚠ 대학·모집단위별 정원을 정확하게 입력해야 합니다. 추천 확정 시 정원을 기준으로 추천 가능 인원을 결정하며, 추천 포기자가 발생했을 때 추가 추천 여부도 이 값에 따릅니다.',
  },
]

const calcTypes = [
  {
    key: 'upper',
    label: '수치 범위 — 이상/이하',
    desc: '숫자 값이 특정 기준 이상(또는 이하)일 때 해당 점수를 부여합니다.',
    example: '내신 1등급 이상 → 100점, 2등급 이상 → 90점',
  },
  {
    key: 'exact',
    label: '수치 범위 — 정확히 일치',
    desc: '입력값과 정확히 일치하는 기준값에 해당하는 점수를 부여합니다.',
    example: '수상 횟수 3회 → 30점, 2회 → 20점',
  },
  {
    key: 'category',
    label: '범주 (문자)',
    desc: '문자로 된 항목 값에 따라 점수를 부여합니다. 수상 등급, 자격증 종류 등에 적합합니다.',
    example: '금상 → 100점, 은상 → 80점, 동상 → 60점',
  },
  {
    key: 'manual',
    label: '직접 입력',
    desc: '담임교사 또는 관리자가 점수를 직접 입력합니다. 점수 기준표가 필요 없습니다.',
    example: '면접 점수, 교사 추천 점수 등',
  },
]

const roundGuides = [
  {
    id: 'open',
    badgeLabel: '진행중',
    badgeStyle: { background: '#dcfce7', color: '#15803d' },
    title: '라운드 열기 — 관리자',
    desc: '라운드를 열면 담임교사가 지원자를 등록하고 데이터를 입력할 수 있는 상태가 됩니다.',
    items: [
      '관리자는 라운드 관리 탭에서 "라운드 열기" 버튼을 클릭합니다. 확인 대화상자가 표시되며, 확인을 누르면 라운드가 생성됩니다.',
      '동시에 진행할 수 있는 라운드는 하나입니다.',
      '라운드를 연 뒤 담임교사에게 시스템 서버 주소와 학급 관리에서 생성한 계정·비밀번호를 안내하세요.',
    ],
    note: null,
    warning: null,
  },
  {
    id: 'application',
    badgeLabel: '지원 접수 중',
    badgeStyle: { background: '#e0f2fe', color: '#0369a1' },
    title: '담임교사 지원 접수 — 담임교사',
    desc: '담임교사는 시스템에 로그인하여 지원 희망 학생의 지원서를 등록합니다.',
    items: [
      '담임교사는 관리자에게 전달받은 계정으로 로그인합니다.',
      '지원자 정보와 전형요소 데이터를 입력하면 점수 미리보기를 확인할 수 있습니다.',
      '담임교사가 직접 입력하는 전형요소는 지원서 등록 화면에서 함께 입력합니다.',
      '관리자는 언제든지 지원 현황을 라운드 관리 탭에서 확인할 수 있습니다.',
    ],
    note: null,
    warning: null,
  },
  {
    id: 'close',
    badgeLabel: '종료',
    badgeStyle: { background: '#dbeafe', color: '#1d4ed8' },
    title: '라운드 종료 — 관리자',
    desc: '지원 접수가 끝나면 라운드를 종료합니다. 종료와 동시에 모든 지원자의 점수와 순위가 자동으로 계산됩니다.',
    items: [
      '관리자는 라운드 관리 탭에서 "종료하기" 버튼을 클릭합니다.',
      '기초 데이터가 누락된 지원자가 있으면 종료가 거부됩니다. 해당 데이터를 입력한 뒤 다시 종료해 주세요.',
    ],
    note: '✓ 점수 계산 중 오류가 발생하면 라운드가 자동으로 "진행중" 상태로 되돌아갑니다. 오류 내용을 확인하고 데이터를 수정한 뒤 다시 종료하면 됩니다. 종료 후에도 담임교사의 추가 입력이 필요하면 "재개방" 버튼으로 다시 진행중 상태로 전환할 수 있습니다.',
    warning: null,
  },
  {
    id: 'recommend',
    badgeLabel: '추천 확정 중',
    badgeStyle: { background: '#f3e8ff', color: '#7c3aed' },
    title: '결과 확인 및 추천 확정 — 관리자',
    desc: '자동 계산된 순위를 바탕으로 추천자를 확정합니다.',
    items: [
      '라운드 상세에서 대학·모집단위별 점수 순위를 확인합니다.',
      '결과 탭의 "자동 추천 확정" 버튼을 누르면 모집단위별 순위순으로 잔여 정원까지 추천을 일괄 확정합니다. 동점이 커트라인에 걸리거나 대학 전체 정원이 초과되는 모집단위는 자동 확정 없이 "수동 확인 필요 모집단위" 목록으로 안내됩니다. 정원이 무제한인 모집단위는 후보 전원이 자동 확정됩니다. 버튼을 다시 눌러도 이미 확정된 추천은 유지됩니다.',
      '자동 확정이 어려운 모집단위는 모집단위별 "추천 확정" 버튼으로 직접 선택합니다.',
      '동점자가 정원을 초과하는 경우 관리자가 직접 선택해야 합니다.',
      '재학생 우선 설정이 된 대학에서는 점수와 무관하게 재학생이 졸업생보다 먼저 순위가 매겨집니다. 전체 정원 내에서 재학생이 먼저 채워지고, 남은 자리에 졸업생이 배정됩니다.',
    ],
    note: null,
    warning: null,
  },
  {
    id: 'finalized',
    badgeLabel: '마감 완료',
    badgeStyle: { background: '#f1f5f9', color: '#475569' },
    title: '라운드 마감 — 관리자',
    desc: '추천 확정이 모두 끝나면 라운드를 최종 마감합니다. 마감 후에는 내용을 변경할 수 없습니다.',
    items: [
      '"마감하기" 버튼을 클릭하여 라운드를 완료 상태로 전환합니다.',
      '결과 내보내기를 통해 전체 결과를 엑셀 파일로 저장할 수 있습니다.',
    ],
    note: '✓ 마감된 라운드의 데이터는 삭제되지 않으며 언제든지 다시 조회할 수 있습니다.',
    warning: null,
  },
]
</script>


