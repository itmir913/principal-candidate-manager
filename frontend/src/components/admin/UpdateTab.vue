<template>
  <div class="py-8 px-4 sm:px-10">

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
      <div class="px-6 py-4 flex items-center justify-between" style="border-bottom: 1px solid #f1f5f9;">
        <h2 class="text-base font-semibold" style="color: #1e293b;">버전 정보</h2>
        <div class="flex items-center gap-2">
          <a
            v-if="!loading && !error && !isLatest && releaseUrl"
            :href="releaseUrl"
            target="_blank"
            rel="noopener noreferrer"
            class="flex items-center gap-1.5 text-base font-medium"
            style="background:#2563eb;border:none;border-radius:8px;padding:7px 14px;cursor:pointer;color:white;text-decoration:none;"
          >
            <Download :size="14" /> 최신 버전 다운로드
          </a>
          <button
            @click="checkUpdate"
            :disabled="loading"
            class="flex items-center gap-1.5 text-base disabled:opacity-40"
            style="background:none;border:1px solid #e2e8f0;border-radius:8px;padding:7px 14px;cursor:pointer;color:#64748b;"
          >
            <RefreshCw :size="14" /> 다시 확인
          </button>
        </div>
      </div>

      <div class="px-6 py-5">
        <!-- 로딩 -->
        <div v-if="loading" class="flex items-center gap-3" style="color: #94a3b8;">
          <div class="animate-spin rounded-full" style="width:18px;height:18px;border:2px solid #e2e8f0;border-top-color:#3b82f6;"></div>
          <span class="text-base">버전 정보를 불러오는 중...</span>
        </div>

        <!-- 오류 -->
        <div v-else-if="error" class="flex items-center gap-3 p-4 rounded-lg" style="background:#fef2f2; border: 1px solid #fecaca;">
          <AlertCircle :size="18" style="color:#dc2626; flex-shrink:0;" />
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
          <p class="text-base" style="color:#374151;">아래 <strong>DB 백업 다운로드</strong> 버튼을 눌러 데이터를 먼저 백업합니다.</p>
        </div>
        <div class="flex gap-3">
          <span
              class="flex-shrink-0 flex items-center justify-center rounded-full text-base font-bold"
              style="width:24px;height:24px;background:#dbeafe;color:#1d4ed8;font-size:12px;"
          >2</span>
          <p class="text-base" style="color:#374151;">맨 위 <strong>최신 버전 다운로드</strong> 버튼을 눌러 ZIP 파일을 내려받고 압축을 풉니다.</p>
        </div>
        <div class="flex gap-3">
          <span
              class="flex-shrink-0 flex items-center justify-center rounded-full text-base font-bold"
              style="width:24px;height:24px;background:#dbeafe;color:#1d4ed8;font-size:12px;"
          >3</span>
          <p class="text-base" style="color:#374151;">시스템 트레이 아이콘을 우클릭한 후 <strong>종료</strong>를 선택해 프로그램을 완전히 닫습니다.</p>
        </div>
        <div class="flex gap-3">
          <span
              class="flex-shrink-0 flex items-center justify-center rounded-full text-base font-bold"
              style="width:24px;height:24px;background:#dbeafe;color:#1d4ed8;font-size:12px;"
          >4</span>
          <p class="text-base" style="color:#374151;">기존 <code class="font-mono px-1 py-0.5 rounded" style="background:#f1f5f9;color:#1e293b;">principal-candidate-manager.exe</code>를 새 파일로 교체합니다.</p>
        </div>
        <div class="flex gap-3">
          <span
              class="flex-shrink-0 flex items-center justify-center rounded-full text-base font-bold"
              style="width:24px;height:24px;background:#dbeafe;color:#1d4ed8;font-size:12px;"
          >5</span>
          <p class="text-base" style="color:#374151;">프로그램을 다시 실행하면 업데이트가 적용됩니다.</p>
        </div>
      </div>
    </div>

    <!-- DB 백업 안내 -->
    <div
      class="rounded-xl"
      style="border: 1px solid #e2e8f0; background: white; overflow: hidden;"
    >
      <div class="px-6 py-4 flex items-center justify-between" style="border-bottom: 1px solid #f1f5f9;">
        <div class="flex items-center gap-2">
          <Database :size="16" style="color:#64748b;" />
          <h2 class="text-base font-semibold" style="color: #1e293b;">데이터 백업 안내</h2>
        </div>
        <button
          @click="downloadBackup"
          :disabled="downloading"
          class="flex items-center gap-1.5 text-base font-medium disabled:opacity-40"
          style="background:#2563eb;border:none;border-radius:8px;padding:7px 14px;cursor:pointer;color:white;"
        >
          <Download :size="14" />
          {{ downloading ? '다운로드 중...' : 'DB 백업 다운로드' }}
        </button>
      </div>
      <div class="px-6 py-5 space-y-5">
        <div>
          <p class="text-base font-medium mb-2" style="color:#374151;">백업 파일 위치</p>
          <p class="text-base mb-3" style="color:#64748b;">
            데이터베이스 파일 <code class="font-mono px-1 py-0.5 rounded" style="background:#f1f5f9;color:#1e293b;">data.db</code>는
            프로그램 실행 파일(<code class="font-mono px-1 py-0.5 rounded" style="background:#f1f5f9;color:#1e293b;">.exe</code>) 옆
            <code class="font-mono px-1 py-0.5 rounded" style="background:#f1f5f9;color:#1e293b;">pcm\</code> 폴더 안에 있습니다.
          </p>
          <div
            class="rounded-lg p-3 text-base font-mono"
            style="background:#1e293b;color:#94a3b8;"
          >
            <span style="color:#64748b;"># 예시 경로 (프로그램을 C:\PCM에 설치한 경우)</span><br />
            <span style="color:#e2e8f0;">C:\PCM\</span><br />
            <span style="color:#e2e8f0;">&nbsp;&nbsp;├── principal-candidate-manager.exe</span><br />
            <span style="color:#e2e8f0;">&nbsp;&nbsp;└── pcm\</span><br />
            <span style="color:#86efac;">&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;└── data.db</span>&nbsp;&nbsp;<span style="color:#64748b;">&lt;-- 이 파일을 복사하세요</span><br />
            <span style="color:#e2e8f0;">&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;└── config.json</span>
          </div>
        </div>

        <div>
          <p class="text-base font-medium mb-2" style="color:#374151;">백업 방법</p>
          <ol class="space-y-2">
            <li class="flex gap-2 text-base" style="color:#374151;">
              <span style="color:#94a3b8;flex-shrink:0;">①</span>
              <span>위 <strong>DB 백업 다운로드</strong> 버튼을 눌러 현재 DB를 저장하거나,
              탐색기에서 위 경로의 <code class="font-mono px-1 py-0.5 rounded" style="background:#f1f5f9;color:#1e293b;">data.db</code>를 직접 복사합니다.</span>
            </li>
            <li class="flex gap-2 text-base" style="color:#374151;">
              <span style="color:#94a3b8;flex-shrink:0;">②</span>
              백업 파일을 안전한 위치(바탕화면, 외부 드라이브 등)에 보관합니다.
            </li>
            <li class="flex gap-2 text-base" style="color:#374151;">
              <span style="color:#94a3b8;flex-shrink:0;">③</span>
              백업 완료 후 업데이트를 진행합니다.
            </li>
          </ol>
        </div>

        <div>
          <p class="text-base font-medium mb-2" style="color:#374151;">복원 방법</p>
          <ol class="space-y-2">
            <li class="flex gap-2 text-base" style="color:#374151;">
              <span style="color:#94a3b8;flex-shrink:0;">①</span>
              프로그램을 완전히 종료합니다.
            </li>
            <li class="flex gap-2 text-base" style="color:#374151;">
              <span style="color:#94a3b8;flex-shrink:0;">②</span>
              탐색기에서 위 경로로 이동합니다.
            </li>
            <li class="flex gap-2 text-base" style="color:#374151;">
              <span style="color:#94a3b8;flex-shrink:0;">③</span>
              <span>기존 <code class="font-mono px-1 py-0.5 rounded" style="background:#f1f5f9;color:#1e293b;">data.db</code>를 삭제하고,
              백업 파일을 같은 이름(<code class="font-mono px-1 py-0.5 rounded" style="background:#f1f5f9;color:#1e293b;">data.db</code>)으로 붙여넣습니다.</span>
            </li>
            <li class="flex gap-2 text-base" style="color:#374151;">
              <span style="color:#94a3b8;flex-shrink:0;">④</span>
              프로그램을 다시 실행합니다.
            </li>
          </ol>
        </div>
      </div>
    </div>

    <!-- About -->
    <div
      class="rounded-xl mt-6"
      style="border: 1px solid #e2e8f0; background: white; overflow: hidden;"
    >
      <div class="px-6 py-4" style="border-bottom: 1px solid #f1f5f9;">
        <h2 class="text-base font-semibold" style="color: #1e293b;">About</h2>
      </div>
      <div class="px-6 py-5 flex flex-col gap-1.5">
        <p class="text-xl font-semibold" style="color: #1e293b;">학교장 추천자 선발 관리 시스템</p>
        <div class="flex items-center gap-3">
          <p class="text-base" style="color: #64748b;">
            © 2026 luminousky ·
            <a href="https://luminousky.com" target="_blank" rel="noopener noreferrer"
               style="color:#3b82f6; text-decoration:none;">luminousky.com</a>
          </p>
          <a
            href="https://github.com/itmir913/principal-candidate-manager"
            target="_blank"
            rel="noopener noreferrer"
            class="flex items-center gap-1.5 text-base"
            style="color:#64748b; text-decoration:none;"
          >
            <svg width="15" height="15" viewBox="0 0 98 96" fill="currentColor" xmlns="http://www.w3.org/2000/svg">
              <path fill-rule="evenodd" clip-rule="evenodd" d="M48.854 0C21.839 0 0 22 0 49.217c0 21.756 13.993 40.172 33.405 46.69 2.427.49 3.316-1.059 3.316-2.362 0-1.141-.08-5.052-.08-9.127-13.59 2.934-16.42-5.867-16.42-5.867-2.184-5.704-5.42-7.17-5.42-7.17-4.448-3.015.324-3.015.324-3.015 4.934.326 7.523 5.052 7.523 5.052 4.367 7.496 11.404 5.378 14.235 4.074.404-3.178 1.699-5.378 3.074-6.6-10.839-1.141-22.243-5.378-22.243-24.283 0-5.378 1.94-9.778 5.014-13.2-.485-1.222-2.184-6.275.486-13.038 0 0 4.125-1.304 13.426 5.052a46.97 46.97 0 0 1 12.214-1.63c4.125 0 8.33.571 12.213 1.63 9.302-6.356 13.427-5.052 13.427-5.052 2.67 6.763.97 11.816.485 13.038 3.155 3.422 5.015 7.822 5.015 13.2 0 18.905-11.404 23.06-22.324 24.283 1.78 1.548 3.316 4.481 3.316 9.126 0 6.6-.08 11.897-.08 13.526 0 1.304.89 2.853 3.316 2.364 19.412-6.52 33.405-24.935 33.405-46.691C97.707 22 75.788 0 48.854 0z"/>
            </svg>
            GitHub
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
const downloading    = ref(false)

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

async function downloadBackup() {
  downloading.value = true
  try {
    const res = await axios.get('/api/auth/db-backup', { responseType: 'blob' })
    const url = URL.createObjectURL(res.data)
    const a = document.createElement('a')
    a.href = url
    a.download = res.headers['content-disposition']?.match(/filename="(.+)"/)?.[1] ?? 'data_backup.db'
    a.click()
    URL.revokeObjectURL(url)
  } catch (e) {
    alert('백업 다운로드 실패: ' + (e.response?.data ? await e.response.data.text() : e.message))
  } finally {
    downloading.value = false
  }
}

onMounted(checkUpdate)

// 부모(AdminView)가 업데이트 여부를 읽을 수 있도록 노출
defineExpose({ isLatest, loading })
</script>
