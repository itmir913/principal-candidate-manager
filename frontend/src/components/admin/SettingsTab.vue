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
    <div class="rounded-xl" style="border: 1px solid #e2e8f0; background: white; padding: 24px;">
      <h2 class="text-lg font-semibold" style="color: #1e293b; margin: 0 0 4px;">프로그램 제목</h2>
      <p class="text-base" style="color: #64748b; margin: 0 0 20px; line-height: 1.6;">
        로그인 화면과 사이드바에 표시됩니다. 학교 이름을 넣거나, 두 개의 프로그램을 함께
        운영할 때 서로 구분하는 데 씁니다.
      </p>

      <!-- 입력 | 미리보기 — 좁은 화면에서는 세로로 쌓인다.
           lg 기준을 쓰는 이유: 사이드바가 폭을 먹어서 md 에서는 두 열이 다 좁아진다. -->
      <div class="grid grid-cols-1 lg:grid-cols-2 gap-6 mb-5">
        <!-- 왼쪽: 입력 -->
        <div>
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
          <p class="text-base" style="color: #94a3b8; margin: 4px 0 0;">
            비워 두면 제목만 표시됩니다. {{ desc.length }} / {{ MAX_DESC }}자
          </p>
        </div>

        <!-- 오른쪽: 미리보기. 라벨은 상자 밖에 두어 왼쪽의 "제목"·"설명" 라벨과 같은 층위로 읽힌다 -->
        <div class="flex flex-col">
          <p class="text-base font-medium mb-1.5" style="color: #475569;">로그인 화면 미리보기</p>
          <!-- flex-1 로 남은 높이를 채운다 — h-full 은 라벨 높이만큼 넘친다 -->
          <div
            class="rounded-lg flex flex-col items-center justify-center flex-1"
            style="background: #f8fafc; border: 1px solid #e2e8f0; padding: 24px 20px; text-align: center; min-height: 140px;"
          >
            <p class="text-2xl font-bold" style="color: #1e293b; margin: 0 0 6px;">{{ title.trim() || '제목을 입력하세요' }}</p>
            <p v-if="desc.trim()" class="text-base" style="color: #94a3b8; margin: 0;">{{ desc.trim() }}</p>
          </div>
        </div>
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

    <!-- About — 업데이트 & 백업 탭에서 옮겨 왔다. 프로그램 자체에 대한 정보라
         버전·백업 안내보다 설정 화면에 있는 편이 찾기 쉽다. -->
    <div
      class="rounded-xl mt-6"
      style="border: 1px solid #e2e8f0; background: white; overflow: hidden;"
    >
      <div class="px-6 py-4" style="border-bottom: 1px solid #f1f5f9;">
        <h2 class="text-base font-semibold" style="color: #1e293b;">About</h2>
      </div>
      <div class="px-6 py-5 flex flex-col gap-1.5">
        <!-- 제품명은 고정이다 — 제작자·라이선스와 함께 "이 소프트웨어가 무엇인가"를 밝히는
             자리라, 학교가 설정한 이름(바로 위 카드)을 여기 다시 쓰면 같은 문구가 두 번 뜬다 -->
        <p class="text-xl font-semibold" style="color: #1e293b;">학교장 추천자 선발 관리 시스템</p>
        <div class="flex flex-col gap-2">
          <p class="text-base" style="color:#64748b; line-height:1.6;">
            <strong>luminousky</strong> · © 2026<br>
            <a
                href="https://luminousky.com/teacher-utility-kit/principal-candidate-manager/"
                target="_blank"
                rel="noopener noreferrer"
                style="color:#3b82f6; text-decoration:none;"
            >
              luminousky.com
            </a>
            ·
            <a
                href="mailto:hello@luminousky.com"
                style="color:#3b82f6; text-decoration:none;"
            >
              hello@luminousky.com
            </a>
          </p>

          <a
              href="https://github.com/itmir913/principal-candidate-manager"
              target="_blank"
              rel="noopener noreferrer"
              class="inline-flex items-center gap-1.5 text-base w-fit"
              style="color:#64748b; text-decoration:none;"
          >
            <svg width="15" height="15" viewBox="0 0 98 96" fill="currentColor" xmlns="http://www.w3.org/2000/svg">
              <path fill-rule="evenodd" clip-rule="evenodd" d="M48.854 0C21.839 0 0 22 0 49.217c0 21.756 13.993 40.172 33.405 46.69 2.427.49 3.316-1.059 3.316-2.362 0-1.141-.08-5.052-.08-9.127-13.59 2.934-16.42-5.867-16.42-5.867-2.184-5.704-5.42-7.17-5.42-7.17-4.448-3.015.324-3.015.324-3.015 4.934.326 7.523 5.052 7.523 5.052 4.367 7.496 11.404 5.378 14.235 4.074.404-3.178 1.699-5.378 3.074-6.6-10.839-1.141-22.243-5.378-22.243-24.283 0-5.378 1.94-9.778 5.014-13.2-.485-1.222-2.184-6.275.486-13.038 0 0 4.125-1.304 13.426 5.052a46.97 46.97 0 0 1 12.214-1.63c4.125 0 8.33.571 12.213 1.63 9.302-6.356 13.427-5.052 13.427-5.052 2.67 6.763.97 11.816.485 13.038 3.155 3.422 5.015 7.822 5.015 13.2 0 18.905-11.404 23.06-22.324 24.283 1.78 1.548 3.316 4.481 3.316 9.126 0 6.6-.08 11.897-.08 13.526 0 1.304.89 2.853 3.316 2.364 19.412-6.52 33.405-24.935 33.405-46.691C97.707 22 75.788 0 48.854 0z"/>
            </svg>
            GitHub Repository
          </a>
        </div>
        <div class="mt-3 pt-3" style="border-top: 1px solid #f1f5f9;">
          <p class="text-base" style="color: #64748b;">
            본 프로그램은
            <a
              href="https://polyformproject.org/licenses/noncommercial/1.0.0"
              target="_blank"
              rel="noopener noreferrer"
              style="color:#3b82f6; text-decoration:none;"
            >PolyForm Noncommercial 1.0.0</a>
            라이선스에 따라 학교·교육청 등 <strong style="color:#374151;">비상업적 목적에 한해</strong> 무료로 사용할 수 있습니다.<br class="hidden xl:block" />
            학원·유료 입시 컨설팅 등 영리 목적의 사교육 기관에서의 사용은 엄격히 금지됩니다.
          </p>
        </div>
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
