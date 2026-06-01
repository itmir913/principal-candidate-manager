<template>
  <div class="py-8 px-4 sm:px-10">

    <!-- 페이지 헤더 -->
    <div class="mb-5">
      <p class="text-base mb-1" style="color: #94a3b8;">관리자</p>
      <h1 class="text-2xl font-semibold" style="color: #1e293b; margin: 0;">매뉴얼</h1>
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
                <p class="text-base mt-1" style="color: #64748b; margin: 0; line-height: 1.5;">{{ step.desc }}</p>
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
                <p class="text-base mt-1" style="color: #64748b; margin: 0; line-height: 1.5;">{{ step.desc }}</p>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div class="rounded-xl" style="padding: 18px 22px; background: #fffbeb; border: 1px solid #fcd34d;">
        <h3 class="text-base font-semibold mb-3" style="color: #92400e; margin: 0;">시작 전 꼭 확인하세요</h3>
        <ul class="text-base space-y-2" style="color: #78350f; padding-left: 0; list-style: none; margin: 0;">
          <li class="flex items-start gap-2"><span>•</span><span>전형요소 설정과 대학 설정은 라운드를 열기 전에 완료해 주세요.</span></li>
          <li class="flex items-start gap-2"><span>•</span><span>학급·학생 데이터는 매 학년도 초에 새로 불러오기를 권장합니다.</span></li>
          <li class="flex items-start gap-2"><span>•</span><span>라운드 종료 시 모든 점수가 자동 계산됩니다. 종료 전 모든 지원자의 데이터가 입력되었는지 확인하세요.</span></li>
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
        전형요소를 먼저 등록한 뒤 점수 기준표와 기초 데이터를 업로드해야 점수가 자동으로 계산됩니다.
      </p>

      <!-- 계산 유형 -->
      <div class="rounded-xl mb-5" style="background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04); overflow: hidden;">
        <div class="px-6 py-4" style="border-bottom: 1px solid #f1f5f9;">
          <h2 class="text-base font-semibold" style="color: #1e293b; margin: 0;">계산 유형</h2>
          <p class="text-base mt-1" style="color: #64748b; margin: 0;">전형요소를 등록할 때 아래 네 가지 계산 방식 중 하나를 선택합니다.</p>
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
          <p class="text-base mt-1" style="color: #64748b; margin: 0;">어떤 값에 몇 점을 줄지 정하는 기준표입니다.</p>
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
            ⚠ 업로드하면 기존 점수 기준표가 전부 교체됩니다. 재학생과 졸업생 데이터는 따로 관리합니다.
          </div>
        </div>
      </div>

      <!-- 기초 데이터 업로드 -->
      <div class="rounded-xl mb-5" style="background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04); overflow: hidden;">
        <div class="px-6 py-4" style="border-bottom: 1px solid #f1f5f9;">
          <h2 class="text-base font-semibold" style="color: #1e293b; margin: 0;">기초 데이터 업로드</h2>
          <p class="text-base mt-1" style="color: #64748b; margin: 0;">학생별 실제 데이터(내신 등급, 수상 실적 등)입니다.</p>
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
              <span>학번·이름·해당 항목 값이 포함된 엑셀 파일을 양식에 맞춰 업로드합니다.</span>
            </li>
          </ul>
          <div class="rounded-lg" style="padding: 10px 14px; background: #f0fdf4; border: 1px solid #bbf7d0; color: #166534;">
            ✓ 담임교사 직접 입력 전형요소(담임 입력 허용 체크)는 기초 데이터 업로드가 필요 없습니다. 담임교사가 지원서 등록 시 직접 입력합니다.
          </div>
        </div>
      </div>

      <!-- 대학별 다른 기준 -->
      <div class="rounded-xl" style="background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04); overflow: hidden;">
        <div class="px-6 py-4" style="border-bottom: 1px solid #f1f5f9;">
          <h2 class="text-base font-semibold" style="color: #1e293b; margin: 0;">대학마다 다른 기준 적용하기</h2>
          <p class="text-base mt-1" style="color: #64748b; margin: 0;">같은 전형요소라도 대학별로 점수 기준이 다를 때 사용합니다.</p>
        </div>
        <div class="px-6 py-5 text-base" style="color: #475569; line-height: 1.7;">
          <p class="mb-3">전형요소를 등록할 때 <strong>데이터 조회 기준</strong>을 설정할 수 있습니다.</p>
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
import { LayoutList, Settings2, SlidersHorizontal, Trophy } from 'lucide-vue-next'

const activeTab = ref('overview')

const tabs = [
  { key: 'overview', label: '전체 흐름',     icon: LayoutList        },
  { key: 'setup',    label: '사전 설정',     icon: Settings2         },
  { key: 'areas',    label: '전형요소 설정', icon: SlidersHorizontal },
  { key: 'rounds',   label: '라운드 운영',   icon: Trophy            },
]

const setupSteps = [
  { num: 1, title: '학급 데이터 불러오기',  desc: '학급 관리 탭에서 NEIS 학급 데이터 업로드' },
  { num: 2, title: '학생 데이터 불러오기',  desc: '학생 관리 탭에서 NEIS 학생 데이터 업로드' },
  { num: 3, title: '전형요소 설정',         desc: '전형요소 설정 탭에서 평가 항목 정의 및 점수 기준표 업로드' },
  { num: 4, title: '대학 설정',             desc: '대학 설정 탭에서 지원 가능 대학·모집단위·정원 입력' },
]

const roundSteps = [
  { num: 5, title: '라운드 열기',   desc: '담임교사가 지원자를 등록할 수 있는 상태로 전환' },
  { num: 6, title: '지원 접수',     desc: '담임교사가 담당 학생 지원서 및 데이터 입력' },
  { num: 7, title: '라운드 종료',   desc: '지원 마감 후 라운드 종료 → 점수 자동 계산' },
  { num: 8, title: '추천 확정',     desc: '산출된 순위를 보고 추천자 확정' },
  { num: 9, title: '마감',          desc: '처리 완료 후 라운드 최종 마감' },
]

const setupGuides = [
  {
    step: 1,
    title: '학급 데이터 불러오기',
    where: '학급 관리 탭',
    desc: 'NEIS(나이스)에서 학급 정보를 엑셀로 내보낸 뒤, 학급 관리 탭에서 업로드합니다. 업로드하면 담임교사 계정이 자동으로 생성되며, 담임교사는 해당 계정으로 시스템에 접속할 수 있습니다.',
    note: '✓ 학년도 초에 한 번만 불러오면 됩니다. 학급 구성이 바뀌면 다시 업로드해 주세요.',
    warning: null,
  },
  {
    step: 2,
    title: '학생 데이터 불러오기',
    where: '학생 관리 탭',
    desc: 'NEIS에서 학생 명단을 엑셀로 내보낸 뒤, 학생 관리 탭에서 업로드합니다. 재학생과 졸업생을 따로 업로드하며, 동일 구분의 기존 데이터는 교체됩니다.',
    note: null,
    warning: '⚠ 업로드하면 동일 구분(재학생 또는 졸업생)의 기존 학생 데이터가 전부 교체됩니다.',
  },
  {
    step: 3,
    title: '전형요소 설정',
    where: '전형요소 설정 탭',
    desc: '각 대학이 반영하는 평가 항목(내신, 수상, 봉사 등)을 전형요소로 등록합니다. 전형요소마다 점수 계산 방식을 지정하고, 점수 기준표와 기초 데이터를 업로드합니다. 자세한 방법은 "전형요소 설정" 탭을 참고하세요.',
    note: null,
    warning: null,
  },
  {
    step: 4,
    title: '대학 설정',
    where: '대학 설정 탭',
    desc: '지원 가능한 대학과 모집단위(학과·전공)를 등록합니다. 대학별 전체 정원과 재학생 우선 여부를 설정하고, 각 모집단위의 정원 및 전형요소별 배점을 입력합니다.',
    note: '✓ 모집단위에 배정된 전형요소 배점 합계가 만점과 일치하는지 확인해 주세요.',
    warning: null,
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
    title: '라운드 열기',
    desc: '라운드를 열면 담임교사가 지원자를 등록하고 데이터를 입력할 수 있는 상태가 됩니다.',
    items: [
      '라운드 관리 탭에서 "라운드 열기" 버튼을 클릭합니다.',
      '동시에 진행할 수 있는 라운드는 하나입니다.',
      '라운드를 연 뒤 담임교사에게 시스템 접속 방법을 안내하세요.',
    ],
    note: null,
    warning: null,
  },
  {
    id: 'application',
    badgeLabel: '지원 접수 중',
    badgeStyle: { background: '#e0f2fe', color: '#0369a1' },
    title: '담임교사 지원 접수',
    desc: '담임교사는 시스템에 로그인하여 지원 희망 학생의 지원서를 등록합니다.',
    items: [
      '담임교사는 학급 관리에서 자동 생성된 계정으로 로그인합니다.',
      '지원자 정보와 전형요소 데이터를 입력하면 점수 미리보기를 확인할 수 있습니다.',
      '직접 입력 전형요소는 지원서 등록 화면에서 함께 입력합니다.',
      '관리자는 언제든지 지원 현황을 라운드 관리 탭에서 확인할 수 있습니다.',
    ],
    note: null,
    warning: null,
  },
  {
    id: 'close',
    badgeLabel: '종료',
    badgeStyle: { background: '#dbeafe', color: '#1d4ed8' },
    title: '라운드 종료 — 점수 자동 계산',
    desc: '지원 접수가 끝나면 라운드를 종료합니다. 종료와 동시에 모든 지원자의 점수와 순위가 자동으로 계산됩니다.',
    items: [
      '라운드 관리 탭에서 "종료하기" 버튼을 클릭합니다.',
      '기초 데이터가 누락된 지원자가 있으면 종료가 거부됩니다. 해당 데이터를 입력한 뒤 다시 종료해 주세요.',
    ],
    note: '✓ 점수 계산 중 오류가 발생하면 라운드가 자동으로 "진행중" 상태로 되돌아갑니다. 오류 내용을 확인하고 데이터를 수정한 뒤 다시 종료하면 됩니다.',
    warning: null,
  },
  {
    id: 'recommend',
    badgeLabel: '추천 확정 중',
    badgeStyle: { background: '#f3e8ff', color: '#7c3aed' },
    title: '결과 확인 및 추천 확정',
    desc: '자동 계산된 순위를 바탕으로 추천자를 확정합니다.',
    items: [
      '라운드 상세에서 대학·모집단위별 점수 순위를 확인합니다.',
      '정원 내 학생을 추천 확정 처리합니다.',
      '동점자가 정원을 초과하는 경우 관리자가 직접 선택해야 합니다.',
      '재학생 우선 설정이 된 대학에서는 같은 점수라면 재학생이 먼저 배정됩니다.',
    ],
    note: null,
    warning: null,
  },
  {
    id: 'finalized',
    badgeLabel: '마감 완료',
    badgeStyle: { background: '#f1f5f9', color: '#475569' },
    title: '라운드 마감',
    desc: '추천 확정이 모두 끝나면 라운드를 최종 마감합니다. 마감 후에는 내용을 변경할 수 없습니다.',
    items: [
      '"마감 처리" 버튼을 클릭하여 라운드를 완료 상태로 전환합니다.',
      '결과 내보내기를 통해 전체 결과를 엑셀 파일로 저장할 수 있습니다.',
    ],
    note: '✓ 마감된 라운드의 데이터는 삭제되지 않으며 언제든지 다시 조회할 수 있습니다.',
    warning: null,
  },
]
</script>


