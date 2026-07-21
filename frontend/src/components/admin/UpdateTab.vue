<template>
  <div class="py-8 px-4 sm:px-10">

    <!-- 페이지 헤더 -->
    <div class="mb-5">
      <p class="text-base mb-1" style="color: #94a3b8;">업데이트</p>
      <h1 class="text-2xl font-semibold" style="color: #1e293b; margin: 0;">최신 버전 확인 및 데이터 백업·복원 안내</h1>
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
      <div
        class="px-6 py-5 prose prose-slate max-w-none
               prose-headings:font-semibold
               prose-h1:text-xl prose-h2:text-lg prose-h3:text-base
               prose-p:text-base prose-li:text-base prose-li:my-0
               prose-ul:my-2 prose-ol:my-2
               prose-hr:my-4"
        v-html="renderedNotes"
      />
    </div>

    <!-- 업데이트 방법 (업데이트가 있거나, 자동 확인 자체에 실패했을 때) -->
    <div
        v-if="!loading && (error || !isLatest)"
        class="rounded-xl mb-6"
        style="border: 1px solid #e2e8f0; background: white; overflow: hidden;"
    >
      <div class="px-6 py-4" style="border-bottom: 1px solid #f1f5f9;">
        <h2 class="text-base font-semibold" style="color: #1e293b;">업데이트 방법</h2>
      </div>
      <div class="px-6 py-5 space-y-3">
        <div v-if="error" class="rounded-lg text-base mb-2" style="padding: 10px 14px; background: #fffbeb; border: 1px solid #fcd34d; color: #92400e;">
          이 프로그램은 인터넷 연결이 필요 없는 학내 LAN에서 동작하도록 설계되었습니다. 이 화면의 자동 버전 확인은 인터넷이 되어야 동작하므로, 인터넷이 차단된 PC에서는 방금처럼 "버전 확인 실패"로 표시될 수 있습니다. 새 버전이 나왔다는 소식을 다른 경로로 전달받았다면, 아래 절차는 그대로 따르면 됩니다.
        </div>
        <div class="flex gap-3">
          <span
              class="flex-shrink-0 flex items-center justify-center rounded-full text-base font-bold"
              style="width:28px;height:28px;background:#dbeafe;color:#1d4ed8;font-size:16px;"
          >1</span>
          <p class="text-base" style="color:#374151;">아래 <strong>백업 파일 다운로드</strong> 버튼을 눌러 데이터를 먼저 백업합니다.</p>
        </div>
        <div class="flex gap-3">
          <span
              class="flex-shrink-0 flex items-center justify-center rounded-full text-base font-bold"
              style="width:28px;height:28px;background:#dbeafe;color:#1d4ed8;font-size:16px;"
          >2</span>
          <p class="text-base" style="color:#374151;">
            맨 위 <strong>최신 버전 다운로드</strong> 버튼(표시되지 않으면 아래 <strong>About</strong>의 <strong>GitHub</strong> 링크 → Releases)으로 이동해 최신 릴리스의 ZIP 파일을 내려받고 압축을 풉니다.
            학교 PC에 인터넷이 안 되면, 인터넷이 되는 다른 PC에서 내려받아 USB로 옮겨도 됩니다.
          </p>
        </div>
        <div class="flex gap-3">
          <span
              class="flex-shrink-0 flex items-center justify-center rounded-full text-base font-bold"
              style="width:28px;height:28px;background:#dbeafe;color:#1d4ed8;font-size:16px;"
          >3</span>
          <p class="text-base" style="color:#374151;">시스템 트레이 아이콘을 우클릭한 후 <strong>종료</strong>를 선택해 프로그램을 완전히 닫습니다.</p>
        </div>
        <div class="flex gap-3">
          <span
              class="flex-shrink-0 flex items-center justify-center rounded-full text-base font-bold"
              style="width:28px;height:28px;background:#dbeafe;color:#1d4ed8;font-size:16px;"
          >4</span>
          <p class="text-base" style="color:#374151;">기존 <code class="font-mono px-1 py-0.5 rounded" style="background:#f1f5f9;color:#1e293b;">principal-candidate-manager.exe</code>를 새 파일로 교체합니다. <code class="font-mono px-1 py-0.5 rounded" style="background:#f1f5f9;color:#1e293b;">pcm\</code> 폴더는 그대로 두세요 — 데이터는 그 폴더 안에 있고 실행 파일 교체와는 무관합니다.</p>
        </div>
        <div class="flex gap-3">
          <span
              class="flex-shrink-0 flex items-center justify-center rounded-full text-base font-bold"
              style="width:28px;height:28px;background:#dbeafe;color:#1d4ed8;font-size:16px;"
          >5</span>
          <p class="text-base" style="color:#374151;">프로그램을 다시 실행하면 업데이트가 적용됩니다.</p>
        </div>
      </div>
    </div>

    <!-- 데이터 백업 및 복원 -->
    <div
      class="rounded-xl"
      style="border: 1px solid #e2e8f0; background: white; overflow: hidden;"
    >
      <div class="px-6 py-4 flex items-center justify-between" style="border-bottom: 1px solid #f1f5f9;">
        <div class="flex items-center gap-2">
          <Database :size="16" style="color:#64748b;" />
          <h2 class="text-base font-semibold" style="color: #1e293b;">데이터 백업 및 복원</h2>
        </div>
        <button
          @click="downloadBackup"
          :disabled="downloading"
          class="flex items-center gap-1.5 text-base font-medium disabled:opacity-40"
          style="background:#2563eb;border:none;border-radius:8px;padding:7px 14px;cursor:pointer;color:white;"
        >
          <Download :size="14" />
          {{ downloading ? '다운로드 중...' : '백업 파일 다운로드' }}
        </button>
      </div>
      <div class="px-6 py-5 space-y-5">

        <div class="rounded-lg text-base" style="padding: 10px 14px; background: #eff6ff; border: 1px solid #bfdbfe; color: #1d4ed8;">
          이 프로그램은 학교 PC 한 대에만 데이터가 저장됩니다. 백업이 없으면 그 PC에 문제가 생겼을 때 되살릴 방법이 없습니다. <strong>라운드를 마감한 직후</strong>처럼 중요한 시점마다 백업해 두세요.
        </div>

        <div>
          <p class="text-base font-medium mb-2" style="color:#374151;">데이터 폴더 구조</p>
          <p class="text-base mb-3" style="color:#64748b;">
            모든 데이터는 프로그램 실행 파일(<code class="font-mono px-1 py-0.5 rounded" style="background:#f1f5f9;color:#1e293b;">.exe</code>) 옆
            <code class="font-mono px-1 py-0.5 rounded" style="background:#f1f5f9;color:#1e293b;">pcm\</code> 폴더 안에 있습니다.
            아래 파일들은 서로 묶여 있는 한 세트입니다 — <code class="font-mono px-1 py-0.5 rounded" style="background:#f1f5f9;color:#1e293b;">data.db</code> 하나만 복사하면 아직 파일에 반영되지 않은 최근 저장 내용이 빠질 수 있습니다.
          </p>
          <div
            class="rounded-lg p-3 text-base font-mono"
            style="background:#1e293b;color:#94a3b8;"
          >
            <span style="color:#64748b;"># 예시 경로 (프로그램을 C:\PCM에 설치한 경우)</span><br />
            <span style="color:#e2e8f0;">C:\PCM\</span><br />
            <span style="color:#e2e8f0;">&nbsp;&nbsp;├── principal-candidate-manager.exe</span><br />
            <span style="color:#e2e8f0;">&nbsp;&nbsp;└── pcm\</span><br />
            <span style="color:#86efac;">&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;├── data.db</span>&nbsp;&nbsp;<span style="color:#64748b;">&lt;-- 실제 데이터</span><br />
            <span style="color:#86efac;">&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;├── data.db-wal</span>&nbsp;&nbsp;<span style="color:#64748b;">&lt;-- 있을 수 있음(최근 저장 내용)</span><br />
            <span style="color:#86efac;">&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;├── data.db-shm</span><br />
            <span style="color:#e2e8f0;">&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;├── config.json</span><br />
            <span style="color:#e2e8f0;">&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;└── logs\</span>&nbsp;&nbsp;<span style="color:#64748b;">&lt;-- 진단용 로그(복원 대상 아님)</span>
          </div>
        </div>

        <div>
          <p class="text-base font-medium mb-2" style="color:#374151;">백업 방법 — 상황에 따라 둘 중 하나</p>
          <p class="text-base mb-3" style="color:#64748b;">
            두 방법 모두 복원 절차는 동일합니다. 차이는 <strong>사용 상황</strong>뿐입니다.
          </p>
          <ol class="space-y-3">
            <li class="text-base" style="color:#374151;">
              <p class="font-medium" style="color:#1e293b;">① 프로그램이 켜져 있을 때 (권장)</p>
              <p>위 <strong>백업 파일 다운로드</strong> 버튼을 클릭합니다. 프로그램을 끄지 않아도, 원격 PC에서 접속한 상태에서도 안전하게 백업 파일(zip)이 만들어집니다.</p>
            </li>
            <li class="text-base" style="color:#374151;">
              <p class="font-medium" style="color:#1e293b;">② 프로그램이 켜지지 않을 때</p>
              <p>
                프로그램을 완전히 종료한 뒤(트레이 아이콘 우클릭 → 종료) 탐색기에서 위 <code class="font-mono px-1 py-0.5 rounded" style="background:#f1f5f9;color:#1e293b;">pcm\</code> 폴더를 통째로 압축(zip)합니다.
                <strong>data.db 파일 하나만 복사하지 마세요</strong> — 함께 있는 data.db-wal 파일에 아직 반영되지 않은 최근 내용이 남아 있으면 그 내용이 빠집니다.
              </p>
            </li>
          </ol>
          <div class="rounded-lg mt-3" style="padding: 10px 14px; background: #f0fdf4; border: 1px solid #bbf7d0; color: #166534;">
            ✓ 백업 파일은 USB, 외부 드라이브 등 안전한 곳에 보관하세요. 전교생의 개인정보와 성적 자료가 담겨 있으므로 보관·폐기에 주의해야 합니다.
          </div>
        </div>

        <div>
          <p class="text-base font-medium mb-2" style="color:#374151;">복원 방법</p>
          <ol class="space-y-2">
            <li class="flex gap-2 text-base" style="color:#374151;">
              <span style="color:#94a3b8;flex-shrink:0;">①</span>
              프로그램을 완전히 종료합니다 (트레이 아이콘 우클릭 → 종료).
            </li>
            <li class="flex gap-2 text-base" style="color:#374151;">
              <span style="color:#94a3b8;flex-shrink:0;">②</span>
              <span>백업 zip의 압축을 풉니다. 탐색기의 기본 "압축 풀기"는 zip과 같은 이름의 폴더를 새로 만들고 그 안에 <code class="font-mono px-1 py-0.5 rounded" style="background:#f1f5f9;color:#1e293b;">pcm\</code> 폴더를 넣습니다 — 압축을 푼 뒤 그 안쪽에서 pcm 폴더를 찾으세요.</span>
            </li>
            <li class="flex gap-2 text-base" style="color:#374151;">
              <span style="color:#94a3b8;flex-shrink:0;">③</span>
              <span><code class="font-mono px-1 py-0.5 rounded" style="background:#f1f5f9;color:#1e293b;">principal-candidate-manager.exe</code>가 있는 폴더에서, 기존 <code class="font-mono px-1 py-0.5 rounded" style="background:#f1f5f9;color:#1e293b;">pcm</code> 폴더의 이름을 <code class="font-mono px-1 py-0.5 rounded" style="background:#f1f5f9;color:#1e293b;">pcm_old</code>처럼 바꿉니다. <strong>지우지 말고 이름만 바꾸세요</strong> — 복원이 잘못되어도 원래 데이터가 남아 있습니다.</span>
            </li>
            <li class="flex gap-2 text-base" style="color:#374151;">
              <span style="color:#94a3b8;flex-shrink:0;">④</span>
              <span>②에서 찾은 <code class="font-mono px-1 py-0.5 rounded" style="background:#f1f5f9;color:#1e293b;">pcm</code> 폴더를 exe 옆(방금 이름을 바꾼 자리)으로 옮깁니다. <strong>기존 pcm 폴더 위에 덮어쓰지 마세요</strong> — 옛 데이터의 흔적 파일이 남아 있으면 새 데이터가 손상될 수 있습니다. 반드시 ③처럼 통째로 치운 뒤에 넣어야 합니다.</span>
            </li>
            <li class="flex gap-2 text-base" style="color:#374151;">
              <span style="color:#94a3b8;flex-shrink:0;">⑤</span>
              프로그램을 다시 실행해 데이터가 정상인지 확인한 뒤, pcm_old 폴더를 지웁니다.
            </li>
          </ol>
          <div class="rounded-lg mt-3" style="padding: 10px 14px; background: #fffbeb; border: 1px solid #fcd34d; color: #92400e;">
            ⚠ 백업 파일 다운로드 버튼으로 받은 zip 안에는 같은 복원 절차가 적힌 <strong>복원방법.txt</strong>가 함께 들어 있습니다. 이 화면을 다시 찾기 어려우면 그 파일을 보세요.
          </div>
        </div>

        <div>
          <p class="text-base font-medium mb-2" style="color:#374151;">문제가 생겼을 때 (진단)</p>
          <p class="text-base" style="color:#64748b;">
            <code class="font-mono px-1 py-0.5 rounded" style="background:#f1f5f9;color:#1e293b;">pcm\logs\</code> 폴더에 날짜별 로그 파일(예: <code class="font-mono px-1 py-0.5 rounded" style="background:#f1f5f9;color:#1e293b;">pcm.2026-07-21.log</code>)이 쌓입니다.
            프로그램이 이상하게 동작하거나 오류가 반복되면 이 폴더의 최근 로그 파일을 개발자에게 함께 전달해 주세요.
            (백업 zip에는 로그가 포함되지 않으므로, 로그는 <code class="font-mono px-1 py-0.5 rounded" style="background:#f1f5f9;color:#1e293b;">pcm\logs\</code> 폴더에서 직접 복사해야 합니다.)
          </p>
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
import { ref, computed, onMounted } from 'vue'
import axios from 'axios'
import { marked } from 'marked'
import { blobErrMsg } from '../../api/admin.js'
import { dialog } from '../common/dialog.js'
import {
  RefreshCw, CheckCircle2, AlertCircle,
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

const renderedNotes = computed(() => marked.parse(releaseNotes.value || ''))

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
    a.download = res.headers['content-disposition']?.match(/filename="(.+)"/)?.[1] ?? 'pcm_backup.zip'
    a.click()
    URL.revokeObjectURL(url)
  } catch (e) {
    await dialog.alert({ title: '백업 다운로드 실패', message: await blobErrMsg(e), level: 'error' })
  } finally {
    downloading.value = false
  }
}

onMounted(checkUpdate)

// 부모(AdminView)가 업데이트 여부를 읽을 수 있도록 노출
defineExpose({ isLatest, loading })
</script>
