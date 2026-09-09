<template>
  <div class="py-8 px-4 sm:px-10">

    <!-- 페이지 헤더 -->
    <div class="flex items-end justify-between flex-wrap gap-3 mb-5">
      <div>
        <p class="text-base mb-1" style="color: #94a3b8;">관리자</p>
        <h1 class="text-2xl font-semibold" style="color: #1e293b; margin: 0;">설정</h1>
      </div>
    </div>

    <HelpBox v-bind="HELP" storage-key="help_settings" class="mb-5" />

    <!-- 제목 설정 카드 -->
    <div class="rounded-xl" style="border: 1px solid #e2e8f0; background: white; padding: 24px; max-width: 640px;">
      <h2 class="text-lg font-semibold" style="color: #1e293b; margin: 0 0 4px;">프로그램 제목</h2>
      <p class="text-base" style="color: #64748b; margin: 0 0 20px; line-height: 1.6;">
        로그인 화면과 사이드바에 표시됩니다. 학교 이름을 넣거나, 두 개의 프로그램을 함께
        운영할 때 서로 구분하는 데 씁니다.
      </p>

      <label class="block text-base font-medium mb-1.5" style="color: #475569;">제목</label>
      <input
        v-model="title"
        type="text"
        :maxlength="MAX_TITLE"
        placeholder="학교장 추천자"
        class="w-full rounded-lg text-base"
        style="padding: 10px 12px; border: 1px solid #cbd5e1;"
      />
      <p class="text-base" style="color: #94a3b8; margin: 4px 0 16px;">{{ title.length }} / {{ MAX_TITLE }}자</p>

      <label class="block text-base font-medium mb-1.5" style="color: #475569;">설명</label>
      <input
        v-model="desc"
        type="text"
        :maxlength="MAX_DESC"
        placeholder="선발 관리 시스템"
        class="w-full rounded-lg text-base"
        style="padding: 10px 12px; border: 1px solid #cbd5e1;"
      />
      <p class="text-base" style="color: #94a3b8; margin: 4px 0 20px;">
        비워 두면 제목만 표시됩니다. {{ desc.length }} / {{ MAX_DESC }}자
      </p>

      <!-- 미리보기 — 로그인 화면과 같은 배치 -->
      <div class="rounded-lg mb-5" style="background: #f8fafc; border: 1px solid #e2e8f0; padding: 20px; text-align: center;">
        <p class="text-base" style="color: #94a3b8; margin: 0 0 10px;">로그인 화면 미리보기</p>
        <p class="text-2xl font-bold" style="color: #1e293b; margin: 0 0 6px;">{{ title.trim() || '제목을 입력하세요' }}</p>
        <p v-if="desc.trim()" class="text-base" style="color: #94a3b8; margin: 0;">{{ desc.trim() }}</p>
      </div>

      <!-- 트레이 아이콘은 시작할 때 한 번만 문구를 읽는다 -->
      <div class="rounded-lg mb-5" style="background: #fffbeb; border: 1px solid #fde68a; padding: 12px 14px;">
        <p class="text-base" style="color: #92400e; margin: 0; line-height: 1.6;">
          작업 표시줄 아이콘에 표시되는 이름은 <strong>프로그램을 다시 시작한 뒤</strong> 바뀝니다.
          화면의 제목은 저장 즉시 반영됩니다.
        </p>
      </div>

      <p v-if="error" class="text-base mb-3" style="color: #dc2626; margin-top: 0;">{{ error }}</p>

      <div class="flex items-center gap-2">
        <button
          class="text-base font-medium rounded-lg disabled:opacity-40"
          style="padding: 10px 20px; border: none; background: #2563eb; color: white; cursor: pointer;"
          :disabled="!canSave"
          @click="save"
        >{{ saving ? '저장 중…' : '저장' }}</button>
        <button
          class="text-base font-medium rounded-lg disabled:opacity-40"
          style="padding: 10px 20px; border: 1px solid #e2e8f0; background: white; color: #475569; cursor: pointer;"
          :disabled="saving || !dirty"
          @click="reset"
        >되돌리기</button>
        <span v-if="saved" class="text-base" style="color: #16a34a;">저장되었습니다</span>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, watch } from 'vue'
import { updateAppInfo } from '../../api/admin.js'
import { useAppInfoStore } from '../../stores/appInfo.js'
import HelpBox from '../common/HelpBox.vue'

// 백엔드 app_info::MAX_TITLE_LEN / MAX_DESC_LEN 과 같은 값.
// 여기서 막는 것은 편의일 뿐이고, 실제 관문은 서버다.
const MAX_TITLE = 40
const MAX_DESC = 60

const appInfo = useAppInfoStore()

const title = ref(appInfo.title)
const desc = ref(appInfo.desc)
const saving = ref(false)
const saved = ref(false)
const error = ref('')

// 앱 시작 직후 저장소가 아직 서버 응답을 못 받았으면 기본값이 들어 있다.
// 도착하면 사용자가 손대기 전에 한해 입력칸을 채운다.
watch(
  () => [appInfo.title, appInfo.desc],
  ([t, d]) => {
    if (!dirty.value) {
      title.value = t
      desc.value = d
    }
  }
)

const dirty = computed(() => title.value !== appInfo.title || desc.value !== appInfo.desc)
const canSave = computed(() => !saving.value && dirty.value && title.value.trim().length > 0)

function reset() {
  title.value = appInfo.title
  desc.value = appInfo.desc
  error.value = ''
  saved.value = false
}

async function save() {
  saving.value = true
  error.value = ''
  saved.value = false
  try {
    const data = await updateAppInfo({ title: title.value, desc: desc.value })
    // 저장 응답이 곧 서버가 확정한 값(trim 적용본)이다 — 그대로 반영한다
    appInfo.set(data)
    title.value = data.title
    desc.value = data.desc
    saved.value = true
  } catch (e) {
    error.value = e.response?.data ?? e.message ?? '저장에 실패했습니다'
  } finally {
    saving.value = false
  }
}

const HELP = {
  title: '도움말 — 설정',
  intro: '프로그램 제목은 로그인 화면·사이드바·브라우저 탭에 표시됩니다. 학교 이름을 넣거나, 인원제한이 있는 전형과 없는 전형을 각각 다른 프로그램으로 운영할 때 서로 구분하는 용도로 씁니다.',
  items: [
    '제목은 필수이고, 설명은 비워 둘 수 있습니다.',
    '저장하면 화면의 제목은 즉시 바뀝니다.',
    { text: '작업 표시줄 아이콘의 이름은 프로그램을 다시 시작해야 바뀝니다.', warn: true },
  ],
}
</script>
