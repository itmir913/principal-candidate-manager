<template>
  <div style="padding: 2rem 2.5rem;">

    <!-- 페이지 헤더 -->
    <div class="mb-5">
      <p class="text-base mb-1" style="color: #94a3b8;">업데이트</p>
      <h1 class="text-2xl font-semibold" style="color: #1e293b; margin: 0;">최신 버전 확인 및 데이터 백업 안내</h1>
    </div>

    <!-- 버전 상태 카드 -->
    <div
      class="rounded-xl mb-6"
      style="border: 1px solid #e2e8f0; background: white; overflow: hidden;"
    >
      <div class="px-6 py-4" style="border-bottom: 1px solid #f1f5f9;">
        <h2 class="text-base font-semibold" style="color: #1e293b;">버전 정보</h2>
      </div>

      <div class="px-6 py-5">
        <!-- 로딩 -->
        <div v-if="loading" class="flex items-center gap-3" style="color: #94a3b8;">
          <div class="animate-spin rounded-full" style="width:18px;height:18px;border:2px solid #e2e8f0;border-top-color:#3b82f6;"></div>
          <span class="text-base">버전 정보를 불러오는 중...</span>
        </div>

        <!-- 오류 -->
        <div v-else-if="error" class="flex items-center gap-3 p-4 rounded-lg" style="background:#fef2f2; border: 1px solid #fecaca;">
          <AlertCircle :size="18" style="color:#dc2626; flex-shrink:0; margin-top:1px;" />
          <div>
            <p class="text-base font-medium" style="color:#991b1b;">버전 확인 실패</p>
            <p class="text-base" style="color:#b91c1c; margin-top:2px;">{{ error }}</p>
          </div>
        </div>

        <!-- 최신 버전 -->
        <div v-else-if="isLatest" class="flex items-center gap-4">
          <div
            class="flex items-center justify-center rounded-full flex-shrink-0"
            style="width:40px;height:40px;background:#dcfce7;"
          >
            <CheckCircle2 :size="22" style="color:#16a34a;" />
          </div>
          <div>
            <p class="text-base font-semibold" style="color:#15803d;">최신 버전입니다</p>
            <p class="text-base mt-0.5" style="color:#64748b;">
              현재 버전: <span class="font-mono font-medium" style="color:#1e293b;">v{{ currentVersion }}</span>
            </p>
          </div>
        </div>

        <!-- 업데이트 필요 -->
        <div v-else class="flex items-center gap-4">
          <div
            class="flex items-center justify-center rounded-full flex-shrink-0"
            style="width:40px;height:40px;background:#fef3c7;"
          >
            <RefreshCw :size="20" style="color:#d97706;" />
          </div>
          <div class="flex-1">
            <p class="text-base font-semibold" style="color:#92400e;">새 버전이 있습니다</p>
            <div class="flex items-center gap-3 mt-1 flex-wrap">
              <span class="text-base" style="color:#64748b;">
                현재: <span class="font-mono font-medium" style="color:#1e293b;">v{{ currentVersion }}</span>
              </span>
              <span style="color:#cbd5e1;">→</span>
              <span class="text-base" style="color:#64748b;">
                최신: <span class="font-mono font-semibold" style="color:#1d4ed8;">v{{ latestVersion }}</span>
              </span>
            </div>
          </div>
        </div>

        <!-- 재확인 버튼 -->
        <div class="mt-4 flex gap-2">
          <button
            @click="checkUpdate"
            :disabled="loading"
            class="flex items-center gap-1.5 text-base disabled:opacity-40"
            style="background:none;border:1px solid #e2e8f0;border-radius:8px;padding:7px 14px;cursor:pointer;color:#64748b;"
          >
            <RefreshCw :size="14" /> 다시 확인
          </button>
          <a
            v-if="!isLatest && releaseUrl"
            :href="releaseUrl"
            target="_blank"
            rel="noopener noreferrer"
            class="flex items-center gap-1.5 text-base font-medium"
            style="background:#2563eb;border:none;border-radius:8px;padding:7px 16px;cursor:pointer;color:white;text-decoration:none;"
          >
            <Download :size="14" /> 최신 버전 다운로드
          </a>
        </div>
      </div>
    </div>

    <!-- 업데이트 방법 (업데이트가 있을 때) -->
    <div
      v-if="!loading && !error && !isLatest"
      class="rounded-xl mb-6"
      style="border: 1px solid #e2e8f0; background: white; overflow: hidden;"
    >
      <div class="px-6 py-4" style="border-bottom: 1px solid #f1f5f9;">
        <h2 class="text-base font-semibold" style="color: #1e293b;">업데이트 방법</h2>
      </div>
      <div class="px-6 py-5 space-y-3">
        <div class="flex gap-3">
          <span
            class="flex-shrink-0 flex items-center justify-center rounded-full text-base font-bold"
            style="width:24px;height:24px;background:#dbeafe;color:#1d4ed8;font-size:12px;"
          >1</span>
          <p class="text-base" style="color:#374151;">아래 <strong>데이터 백업 안내</strong>를 먼저 읽고 DB를 백업합니다.</p>
        </div>
        <div class="flex gap-3">
          <span
            class="flex-shrink-0 flex items-center justify-center rounded-full text-base font-bold"
            style="width:24px;height:24px;background:#dbeafe;color:#1d4ed8;font-size:12px;"
          >2</span>
          <p class="text-base" style="color:#374151;">위 <strong>최신 버전 다운로드</strong> 버튼을 눌러 GitHub에서 설치 파일을 받습니다.</p>
        </div>
        <div class="flex gap-3">
          <span
            class="flex-shrink-0 flex items-center justify-center rounded-full text-base font-bold"
            style="width:24px;height:24px;background:#dbeafe;color:#1d4ed8;font-size:12px;"
          >3</span>
          <p class="text-base" style="color:#374151;">기존 프로그램이 실행 중이면 종료하고, 새 설치 파일(`.exe`)을 실행합니다.</p>
        </div>
        <div class="flex gap-3">
          <span
            class="flex-shrink-0 flex items-center justify-center rounded-full text-base font-bold"
            style="width:24px;height:24px;background:#dbeafe;color:#1d4ed8;font-size:12px;"
          >4</span>
          <p class="text-base" style="color:#374151;">설치 완료 후 프로그램을 실행하면 업데이트가 적용됩니다.</p>
        </div>
      </div>
    </div>

    <!-- 최신 릴리스 노트 -->
    <div
      v-if="!loading && !error && releaseNotes"
      class="rounded-xl mb-6"
      style="border: 1px solid #e2e8f0; background: white; overflow: hidden;"
    >
      <div class="px-6 py-4 flex items-center justify-between" style="border-bottom: 1px solid #f1f5f9;">
        <h2 class="text-base font-semibold" style="color: #1e293b;">
          최신 버전 변경 내용
          <span class="font-mono text-base font-normal ml-2" style="color:#64748b;">v{{ latestVersion }}</span>
        </h2>
        <a
          v-if="releaseUrl"
          :href="releaseUrl"
          target="_blank"
          rel="noopener noreferrer"
          class="flex items-center gap-1 text-base"
          style="color:#3b82f6;text-decoration:none;"
        >
          <ExternalLink :size="13" /> GitHub
        </a>
      </div>
      <div class="px-6 py-5">
        <pre
          class="text-base whitespace-pre-wrap"
          style="color:#374151;font-family:inherit;margin:0;line-height:1.7;"
        >{{ releaseNotes }}</pre>
      </div>
    </div>

    <!-- DB 백업 안내 -->
    <div
      class="rounded-xl"
      style="border: 1px solid #e2e8f0; background: white; overflow: hidden;"
    >
      <div class="px-6 py-4 flex items-center gap-2" style="border-bottom: 1px solid #f1f5f9;">
        <Database :size="16" style="color:#64748b;" />
        <h2 class="text-base font-semibold" style="color: #1e293b;">데이터 백업 안내</h2>
      </div>
      <div class="px-6 py-5 space-y-5">
        <div
          class="flex items-center gap-3 p-4 rounded-lg"
          style="background:#fffbeb;border:1px solid #fde68a;"
        >
          <AlertTriangle :size="16" style="color:#d97706;flex-shrink:0;margin-top:2px;" />
          <p class="text-base" style="color:#92400e;">
            업데이트 전 반드시 데이터베이스 파일을 백업하세요.
            설치 파일 실행 시 기존 데이터가 덮어써질 수 있습니다.
          </p>
        </div>

        <div>
          <p class="text-base font-medium mb-2" style="color:#374151;">백업 파일 위치</p>
          <p class="text-base mb-3" style="color:#64748b;">
            데이터베이스 파일 <code class="font-mono px-1 py-0.5 rounded" style="background:#f1f5f9;color:#1e293b;">data.db</code>는
            프로그램 실행 파일과 같은 폴더에 있습니다.
          </p>
          <div
            class="rounded-lg p-3 text-base font-mono"
            style="background:#1e293b;color:#94a3b8;"
          >
            <span style="color:#64748b;"># 예시 경로 (Tauri 패키징 기준)</span><br />
            <span style="color:#e2e8f0;">C:\Users\사용자명\AppData\Local\principal-candidate-manager\</span><br />
            <span style="color:#86efac;">  └── data.db</span>  <span style="color:#64748b;">&lt;-- 이 파일을 복사하세요</span>
          </div>
        </div>

        <div>
          <p class="text-base font-medium mb-2" style="color:#374151;">백업 방법</p>
          <ol class="space-y-2">
            <li class="flex gap-2 text-base" style="color:#374151;">
              <span style="color:#94a3b8;flex-shrink:0;">①</span>
              프로그램을 완전히 종료합니다.
            </li>
            <li class="flex gap-2 text-base" style="color:#374151;">
              <span style="color:#94a3b8;flex-shrink:0;">②</span>
              탐색기에서 위 경로의 <code class="font-mono px-1 py-0.5 rounded" style="background:#f1f5f9;color:#1e293b;">data.db</code> 파일을 찾습니다.
            </li>
            <li class="flex gap-2 text-base" style="color:#374151;">
              <span style="color:#94a3b8;flex-shrink:0;">③</span>
              파일을 안전한 위치(바탕화면, 외부 드라이브 등)에 복사합니다.
            </li>
            <li class="flex gap-2 text-base" style="color:#374151;">
              <span style="color:#94a3b8;flex-shrink:0;">④</span>
              백업 완료 후 업데이트를 진행합니다.
            </li>
          </ol>
        </div>

        <div
          class="flex items-center gap-3 p-4 rounded-lg"
          style="background:#f0fdf4;border:1px solid #bbf7d0;"
        >
          <CheckCircle2 :size="16" style="color:#16a34a;flex-shrink:0;margin-top:2px;" />
          <p class="text-base" style="color:#15803d;">
            업데이트 후 문제가 발생하면, 백업한 <code class="font-mono px-1 py-0.5 rounded" style="background:#dcfce7;">data.db</code>를
            같은 위치에 복원하면 이전 데이터로 돌아갈 수 있습니다.
          </p>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue'
import axios from 'axios'
import {
  RefreshCw, CheckCircle2, AlertCircle, AlertTriangle,
  Download, Database, ExternalLink,
} from 'lucide-vue-next'

const GITHUB_API = 'https://api.github.com/repos/itmir913/principal-candidate-manager/releases/latest'

const loading        = ref(true)
const error          = ref('')
const currentVersion = ref('')
const latestVersion  = ref('')
const releaseNotes   = ref('')
const releaseUrl     = ref('')
const isLatest       = ref(true)

function stripV(v) {
  return (v ?? '').replace(/^v/i, '').trim()
}

async function checkUpdate() {
  loading.value = true
  error.value   = ''
  try {
    const [verRes, ghRes] = await Promise.all([
      axios.get('/api/version'),
      fetch(GITHUB_API, { headers: { Accept: 'application/vnd.github+json' } }),
    ])

    currentVersion.value = stripV(verRes.data.version)

    if (!ghRes.ok) throw new Error(`GitHub API 오류 (${ghRes.status})`)
    const gh = await ghRes.json()

    latestVersion.value = stripV(gh.tag_name)
    releaseNotes.value  = (gh.body ?? '').trim()
    releaseUrl.value    = gh.html_url ?? ''
    isLatest.value      = currentVersion.value === latestVersion.value
  } catch (e) {
    error.value = e.message || '알 수 없는 오류'
  } finally {
    loading.value = false
  }
}

onMounted(checkUpdate)

// 부모(AdminView)가 업데이트 여부를 읽을 수 있도록 노출
defineExpose({ isLatest, loading })
</script>
