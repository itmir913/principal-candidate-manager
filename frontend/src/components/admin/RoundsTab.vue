<template>
  <div class="py-8 px-4 sm:px-10">

    <!-- 페이지 헤더 -->
    <div class="mb-5">
      <p class="text-base mb-1" style="color: #94a3b8;">관리자</p>
      <h1 class="text-2xl font-semibold" style="color: #1e293b; margin: 0;">라운드 관리</h1>
    </div>

    <HelpBox
      v-if="rounds.length === 0"
      class="mb-5"
      storage-key="rounds-empty"
      :title="HELP_EMPTY.title"
      :intro="HELP_EMPTY.intro"
      :items="HELP_EMPTY.items"
    />

    <div class="flex flex-col lg:flex-row lg:items-start gap-6">

      <!-- ── 좌측: 라운드 목록 ────────────────────────────────── -->
      <div class="flex-shrink-0 flex flex-col w-full lg:w-[300px]">
        <div class="flex items-center justify-between mb-4">
          <h2 class="text-lg font-semibold" style="color: #1e293b;">라운드 목록</h2>
          <button
            class="text-base font-medium rounded-lg whitespace-nowrap disabled:opacity-40"
            style="padding: 7px 14px; border: none; background: #2563eb; color: white; cursor: pointer;"
            :disabled="hasOpenRound || loading"
            @click="handleOpenRound"
          >+ 라운드 열기</button>
        </div>

        <div class="flex flex-col gap-2">
          <div
            v-for="r in rounds"
            :key="r.id"
            class="rounded-xl transition-all"
            :style="{
              background: 'white',
              border: selected?.id === r.id ? '1px solid #93c5fd' : '1px solid #e2e8f0',
              boxShadow: '0 1px 4px rgba(0,0,0,0.07)',
            }"
          >
            <!-- 클릭 영역 -->
            <div class="cursor-pointer" style="padding: 14px 16px;" @click="selectRound(r)">
              <p class="text-lg font-semibold" style="color: #1e293b; margin: 0;">{{ r.id }}차 라운드</p>

              <div class="mt-1.5 flex items-center justify-start gap-2">
                <span
                  class="text-base font-medium"
                  style="padding: 2px 10px; border-radius: 999px; white-space: nowrap;"
                  :style="{
                    background: r.status === 'OPEN' ? '#dcfce7' : r.status === 'CLOSED' ? '#dbeafe' : '#f3e8ff',
                    color:      r.status === 'OPEN' ? '#15803d' : r.status === 'CLOSED' ? '#1d4ed8' : '#7c3aed',
                  }"
                >
                  {{ roundStatusLabel(r.status) }}
                </span>

                <span class="text-base" style="color: #94a3b8;">
                  <template v-if="r.status === 'OPEN'">{{ fmtDt(r.opened_at) }}</template>
                  <template v-else-if="r.status === 'CLOSED'">{{ fmtDt(r.closed_at) }}</template>
                  <template v-else-if="r.status === 'FINALIZED'">{{ fmtDt(r.finalized_at) }}</template>
                </span>

              </div>
            </div>
          </div>

          <!-- 로드 오류 — 서버 오류를 "라운드 없음" 빈 상태로 위장하지 않는다 -->
          <div v-if="roundsLoadError" class="text-base text-center" style="padding: 32px 12px; color: #991b1b;">
            라운드 목록을 불러오지 못했습니다:<br>{{ roundsLoadError }}
          </div>
          <div v-else-if="rounds.length === 0" class="text-base text-center" style="padding: 32px 0; color: #94a3b8;">
            라운드 없음
          </div>
        </div>
      </div>

      <!-- ── 우측: 라운드 상세 ──────────────────────────────────── -->
      <div class="flex-1 min-w-0">
        <div v-if="!selected" class="flex items-center justify-center" style="height: 240px;">
          <p class="text-base" style="color: #94a3b8;">라운드를 선택하거나 새 라운드를 열어주세요</p>
        </div>

        <template v-else>
          <div class="rounded-xl mb-5"
            style="padding: 18px 22px; background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04);">
            <div class="flex items-center gap-3 flex-wrap">
              <span class="text-xl font-bold" style="color: #1e293b;">{{ selected.id }}차 라운드</span>
              <span
                  class="text-base font-semibold"
                  style="padding: 4px 14px; border: 1px solid; border-radius: 999px;"
                  :style="{
                    background:  selected.status === 'OPEN' ? '#dcfce7' : selected.status === 'CLOSED' ? '#dbeafe' : '#f3e8ff',
                    color:       selected.status === 'OPEN' ? '#15803d' : selected.status === 'CLOSED' ? '#1d4ed8' : '#7c3aed',
                    borderColor: selected.status === 'OPEN' ? '#bbf7d0' : selected.status === 'CLOSED' ? '#bfdbfe' : '#e9d5ff'
                  }"
              >
                {{ roundStatusLabel(selected.status) }}</span>

              <!-- 상태 액션 버튼 -->
              <template v-if="selected.status === 'OPEN'">
                <button
                  class="text-base font-medium rounded-lg whitespace-nowrap disabled:opacity-40"
                  style="padding: 4px 14px; border: 1px solid #fca5a5; background: white; color: #ef4444; cursor: pointer;"
                  :disabled="roundActing"
                  @click="handleCloseRound(selected.id)"
                >종료하기</button>
              </template>
              <template v-else-if="selected.status === 'CLOSED'">
                <button
                  class="text-base font-medium rounded-lg whitespace-nowrap disabled:opacity-40"
                  style="padding: 4px 14px; border: 1px solid #e2e8f0; background: white; color: #64748b; cursor: pointer;"
                  :disabled="roundActing"
                  @click="handleReopenRound(selected.id)"
                >다시 열기</button>
                <button
                  class="text-base font-medium rounded-lg whitespace-nowrap disabled:opacity-40"
                  style="padding: 4px 14px; border: 1px solid #d8b4fe; background: white; color: #7c3aed; cursor: pointer;"
                  :disabled="roundActing"
                  @click="handleFinalizeRound(selected.id)"
                >마감하기</button>
              </template>

              <!-- 날짜 -->
              <div class="w-full flex gap-1 items-center flex-wrap">
              <span class="text-base" style="color: #94a3b8;">{{ fmtDt(selected.opened_at) }} 개시</span>
              <span v-if="selected.closed_at" class="text-base" style="color: #94a3b8;">→ {{ fmtDt(selected.closed_at) }} 입력 종료</span>
              <span v-if="selected.finalized_at" class="text-base" style="color: #94a3b8;">→ {{ fmtDt(selected.finalized_at) }} 최종 마감</span>
              </div>
            </div>
          </div>

          <!-- OPEN 라운드 담임 확정 현황 -->
          <div
            v-if="selected.status === 'OPEN' && confirmationStatus"
            class="rounded-xl mb-5"
            style="padding: 18px 22px; background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04);"
          >
            <p class="text-base font-semibold mb-3" style="color: #475569; text-transform: uppercase; letter-spacing: 0.05em;">담임 입력 확정 현황</p>
            <template v-if="confirmationStatus.classes.filter(c => !c.confirmed).length === 0">
              <p class="text-base font-semibold" style="color: #15803d;">✓ 모든 학급이 입력을 확정했습니다</p>
            </template>
            <template v-else>
              <p class="text-base mb-2" style="color: #475569;">
                확정:
                <span class="font-semibold" style="color: #1e293b;">
                  {{ confirmationStatus.classes.filter(c => c.confirmed).length }} / {{ confirmationStatus.classes.length }} 학급
                </span>
              </p>
              <p class="text-base" style="color: #d97706;">
                미확정: {{ confirmationStatus.classes.filter(c => !c.confirmed).map(classLabel).join(', ') }}
              </p>
            </template>
          </div>

          <HelpBox
            v-if="helpBox"
            :key="helpBox.key"
            class="mb-5"
            :storage-key="helpBox.key"
            :title="helpBox.title"
            :intro="helpBox.intro"
            :items="helpBox.items"
          />

          <!-- 서브탭 -->
          <div class="flex mb-5" style="border-bottom: 1px solid #e2e8f0;">
            <button
              v-for="t in subTabs"
              :key="t.key"
              class="text-base font-medium transition-colors"
              style="padding: 10px 20px; border: none; background: none; cursor: pointer; border-bottom: 2px solid transparent; margin-bottom: -1px;"
              :style="{
                borderBottomColor: view === t.key ? '#2563eb' : 'transparent',
                color: view === t.key ? '#2563eb' : '#64748b',
                fontWeight: view === t.key ? '600' : '400',
              }"
              @click="view = t.key"
            >{{ t.label }}</button>
          </div>

          <!-- ── 지원 현황 탭 ──────────────────────────────── -->
          <div v-if="view === 'apps'">
            <div class="flex items-center justify-between mb-4 flex-wrap gap-2">
              <span class="text-base" style="color: #64748b;">총 {{ apps.length }}건</span>
              <div v-if="selected.status === 'CLOSED'" class="flex items-center gap-3">
                <span v-if="calcMsg" class="text-base font-medium"
                  :style="{ color: calcMsg.ok ? '#16a34a' : '#ef4444' }">{{ calcMsg.text }}</span>
                <button
                  class="text-base font-semibold rounded-lg whitespace-nowrap disabled:opacity-40"
                  style="padding: 9px 18px; border: none; background: #4f46e5; color: white; cursor: pointer;"
                  :disabled="calcLoading || apps.length === 0"
                  @click="handleCalculate"
                >{{ calcLoading ? '계산 중…' : '점수 전체 재계산' }}</button>
              </div>
            </div>

            <div v-if="apps.length === 0" class="text-base text-center" style="padding: 48px 0; color: #94a3b8;">
              지원자가 없습니다
            </div>

            <div v-for="(group, key) in appsByUniv" :key="key" class="mb-6">
              <h4 class="text-base font-semibold mb-3" style="color: #1e293b;">{{ key }}</h4>
              <div class="rounded-xl overflow-hidden"
                style="background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04);">
                <div class="overflow-x-auto">
                  <table style="border-collapse: collapse; table-layout: fixed; width: 100%; min-width: 910px;">
                    <colgroup>
                      <col style="width: 160px;">
                      <col style="width: 100px;">
                      <col style="width: 90px;">
                      <col style="width: 120px;">
                      <col style="width: 150px;">
                      <col style="width: 110px;">
                      <col style="width: 110px;">
                      <col style="width: 70px;">
                    </colgroup>
                    <thead>
                      <tr style="background: #f8fafc; border-bottom: 1px solid #e2e8f0;">
                        <th class="text-base font-semibold text-left" style="padding: 13px 18px; color: #475569;">학번/학생코드</th>
                        <th class="text-base font-semibold text-left" style="padding: 13px 18px; color: #475569;">학생 이름</th>
                        <th class="text-base font-semibold text-left" style="padding: 13px 18px; color: #475569;">구분</th>
                        <th class="text-base font-semibold text-left" style="padding: 13px 18px; color: #475569;">모집단위</th>
                        <th class="text-base font-semibold text-left" style="padding: 13px 18px; color: #475569;">지원 학과</th>
                        <th class="text-base font-semibold text-center" style="padding: 13px 18px; color: #475569;">추천</th>
                        <th class="text-base font-semibold text-center" style="padding: 13px 18px; color: #475569;">포기처리</th>
                        <th class="text-base font-semibold text-right" style="padding: 13px 18px; color: #475569;">총점</th>
                      </tr>
                    </thead>
                    <tbody>
                      <tr v-for="app in group" :key="app.student_id"
                        class="hover:bg-slate-50"
                        style="border-bottom: 1px solid #f1f5f9; transition: background 0.1s;">
                        <td class="text-base" style="padding: 12px 18px; color: #475569;">
                          <span v-if="app.is_enrolled">{{ app.grade }}학년 {{ app.class_no }}반 {{ app.seq_no }}번</span>
                          <span v-else class="font-mono">{{ app.student_code }}</span>
                        </td>
                        <td class="text-base font-medium" style="padding: 12px 18px; color: #1e293b;">{{ app.name }}</td>
                        <td style="padding: 12px 18px;">
                          <span class="text-base font-medium"
                            :style="{ color: app.is_enrolled ? '#16a34a' : '#94a3b8' }">
                            {{ app.is_enrolled ? '재학생' : '졸업생' }}
                          </span>
                        </td>
                        <td class="text-base" style="padding: 12px 18px; color: #1e293b;">{{ app.track_name }}</td>
                        <td class="text-base" style="padding: 12px 18px; color: #475569;">{{ app.department_name }}</td>
                        <td class="text-base text-center" style="padding: 12px 18px;">
                          <span v-if="app.abandoned" style="color: #cbd5e1;">-</span>
                          <span v-else-if="selected.status === 'FINALIZED' && app.recommended"
                            class="text-base font-semibold" style="color: #16a34a;">추천 확정</span>
                          <span v-else-if="selected.status === 'FINALIZED' && !app.recommended"
                            class="text-base font-semibold" style="color: #ef4444;">미선발</span>
                          <span v-else style="color: #cbd5e1;">-</span>
                        </td>
                        <td class="text-center" style="padding: 12px 18px;">
                          <span v-if="app.abandoned" class="text-base font-semibold" style="color: #ef4444;">포기됨</span>
                          <button
                            v-else-if="selected.status === 'FINALIZED' && app.recommended"
                            class="text-base rounded-lg whitespace-nowrap"
                            style="padding: 5px 12px; border: 1px solid #fca5a5; background: white; color: #ef4444; cursor: pointer;"
                            @click="handleAbandon(app)"
                          >포기하기</button>
                          <span v-else style="color: #cbd5e1;">-</span>
                        </td>
                        <td class="text-base text-right font-semibold" style="padding: 12px 18px; color: #1e293b;">
                          {{ appTotalScore(app) }}
                        </td>
                      </tr>
                    </tbody>
                  </table>
                </div>
              </div>
            </div>
          </div>

          <!-- ── 결과 탭 ──────────────────────────────────── -->
          <div v-if="view === 'results'">
            <div class="sticky top-0 z-10" style="padding: 10px 0; margin: -10px 0 6px;">
              <div class="flex items-center gap-3 mb-3 flex-wrap">
                <select
                  v-model="selectedTrackId"
                  class="text-base rounded-lg focus:outline-none focus:ring-2 focus:ring-blue-400"
                  style="border: 1px solid #e2e8f0; padding: 9px 12px; color: #1e293b;"
                  @change="loadResults"
                >
                  <option value="">전체 대학</option>
                  <option v-for="t in tracksInRound" :key="t.id" :value="t.id">
                    {{ t.univ_name }} {{ t.track_name }}
                  </option>
                </select>
                <button
                  class="text-base font-medium rounded-lg whitespace-nowrap"
                  style="padding: 9px 16px; border: 1px solid #e2e8f0; background: white; color: #475569; cursor: pointer;"
                  @click="loadResults"
                >새로고침</button>
                <span style="color: #cbd5e1; user-select: none;">|</span>
                <button
                  v-if="selected.status === 'CLOSED'"
                  class="text-base font-semibold rounded-lg whitespace-nowrap disabled:opacity-40"
                  style="padding: 9px 16px; border: none; background: #d97706; color: white; cursor: pointer;"
                  :disabled="autoRecommendActing"
                  @click="handleAutoRecommend"
                >자동 추천 확정</button>
                <button
                  class="text-base font-medium rounded-lg whitespace-nowrap disabled:opacity-40"
                  style="padding: 9px 16px; border: none; background: #059669; color: white; cursor: pointer;"
                  :disabled="results.length === 0 || downloading"
                  @click="downloadExcel"
                >이 라운드 지원자 명단</button>
                <button
                  class="text-base font-medium rounded-lg whitespace-nowrap disabled:opacity-40"
                  style="padding: 9px 16px; border: none; background: #2563eb; color: white; cursor: pointer;"
                  :disabled="selected.status !== 'FINALIZED' || downloadingSummary"
                  @click="downloadSummary"
                >이 라운드 선발 현황</button>
              </div>

              <div v-if="results.length > 0" class="flex gap-2">
                <button
                  class="text-base font-medium rounded-lg whitespace-nowrap"
                  :style="{
                    padding: '6px 14px', cursor: 'pointer',
                    border: '1px solid',
                    borderColor: rankView === 'track' ? '#2563eb' : '#e2e8f0',
                    background: rankView === 'track' ? '#2563eb' : 'white',
                    color: rankView === 'track' ? 'white' : '#475569',
                  }"
                  @click="rankView = 'track'"
                >모집단위별 순위</button>
                <button
                  class="text-base font-medium rounded-lg whitespace-nowrap"
                  :style="{
                    padding: '6px 14px', cursor: 'pointer',
                    border: '1px solid',
                    borderColor: rankView === 'univ' ? '#2563eb' : '#e2e8f0',
                    background: rankView === 'univ' ? '#2563eb' : 'white',
                    color: rankView === 'univ' ? 'white' : '#475569',
                  }"
                  @click="rankView = 'univ'"
                >대학 전체 순위</button>
              </div>
            </div>

            <!-- 자동 추천 확정 결과 표시 -->
            <div v-if="autoRecommendResult" class="mb-5 rounded-xl" style="padding: 16px 20px; background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04);">
              <div v-if="autoRecommendScope" class="text-base mb-2" style="color: #64748b;">
                처리 범위: {{ autoRecommendScope }}
              </div>
              <div v-if="autoRecommendResult.confirmed.length > 0" class="text-base font-semibold mb-2" style="color: #15803d;">
                {{ autoRecommendResult.confirmed.length }}개 모집단위 {{ autoRecommendResult.confirmed.reduce((s, c) => s + c.count, 0) }}명 추천 확정
              </div>
              <div v-if="autoRecommendResult.confirmed.length === 0 && autoRecommendResult.manual.length === 0" class="text-base" style="color: #94a3b8;">
                자동 확정 대상 없음 (정원 소진 또는 후보 없음)
              </div>
              <div v-if="autoRecommendResult.manual.length > 0" class="rounded-lg mt-2" style="padding: 12px 16px; background: #fffbeb; border: 1px solid #fcd34d;">
                <p class="text-base font-semibold mb-2" style="color: #92400e;">수동 확인 필요</p>
                <div v-for="(m, i) in autoRecommendResult.manual" :key="i" class="text-base" style="color: #78350f;">
                  {{ m.univ_name }}<template v-if="m.track_name"> {{ m.track_name }}</template><template v-else> (대학 전체)</template> — {{ m.reason }}
                </div>
              </div>
            </div>

            <div v-if="results.length === 0" class="text-base text-center" style="padding: 48px 0; color: #94a3b8;">
              결과가 없습니다. 점수 계산을 먼저 실행하세요.
            </div>

            <div v-for="(group, key) in resultsByView" :key="key" class="mb-6">
              <div class="flex items-center gap-3 mb-3 flex-wrap">
                <h4 class="text-base font-semibold" style="color: #1e293b; margin: 0;">{{ key }}</h4>
                <span class="text-base" style="color: #94a3b8;">
                  <template v-if="group.totalQuota != null">
                    대학 정원 {{ group.totalQuota }}명 / 잔여 {{ group.univRemaining }}석
                  </template>
                  <template v-else>대학 정원 무제한</template>
                  <template v-if="rankView === 'track'">
                    <span style="margin: 0 6px; color: #e2e8f0;">|</span>
                    <template v-if="group.unitQuota != null">
                      모집단위 정원 {{ group.unitQuota }}명 / 잔여 {{ group.remaining }}석
                    </template>
                    <template v-else>모집단위 정원 무제한</template>
                  </template>
                </span>
                <button
                  v-if="selected.status === 'CLOSED' && univAutoButtonKeys.has(key)"
                  class="text-base font-medium rounded-lg whitespace-nowrap disabled:opacity-40"
                  style="padding: 6px 14px; border: 1px solid #fcd34d; background: #fffbeb; color: #92400e; cursor: pointer;"
                  :disabled="autoRecommendActing"
                  @click="handleAutoRecommendUniv(group)"
                >{{ group.univName }} 전체 자동 추천</button>
              </div>
              <div class="rounded-xl overflow-hidden"
                style="background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04);">
                <div class="overflow-x-auto">
                  <table style="border-collapse: collapse; table-layout: fixed; width: 100%; min-width: 1076px;">
                    <colgroup>
                      <col style="width: 36px;">
                      <col style="width: 70px;">
                      <col style="width: 160px;">
                      <col style="width: 100px;">
                      <col style="width: 90px;">
                      <col style="width: 140px;">
                      <col style="width: 90px;">
                      <col style="width: 120px;">
                      <col style="width: 110px;">
                      <col style="width: 160px;">
                    </colgroup>
                    <thead>
                      <tr style="background: #f8fafc; border-bottom: 1px solid #e2e8f0;">
                        <th style="padding: 13px 8px;"></th>
                        <th class="text-base font-semibold text-center" style="padding: 13px 16px; color: #475569;">{{ rankView === 'track' ? '모집단위 순위' : '대학 순위' }}</th>
                        <th class="text-base font-semibold text-left" style="padding: 13px 18px; color: #475569;">학번/학생코드</th>
                        <th class="text-base font-semibold text-left" style="padding: 13px 18px; color: #475569;">학생 이름</th>
                        <th class="text-base font-semibold text-left" style="padding: 13px 18px; color: #475569;">구분</th>
                        <th class="text-base font-semibold text-left" style="padding: 13px 18px; color: #475569;">지원 학과</th>
                        <th class="text-base font-semibold text-right" style="padding: 13px 18px; color: #475569;">총점</th>
                        <th class="text-base font-semibold text-center" style="padding: 13px 18px; color: #475569;">추천</th>
                        <th class="text-base font-semibold text-center" style="padding: 13px 18px; color: #475569;">포기처리</th>
                        <th class="text-base font-semibold text-center" style="padding: 13px 18px; color: #475569;">미선발</th>
                      </tr>
                    </thead>
                    <tbody>
                      <template v-for="r in group.results" :key="r.student_id">
                        <tr
                          class="cursor-pointer transition-colors"
                          :style="{
                            borderBottom: '1px solid #f1f5f9',
                            background:
                              selected.status === 'FINALIZED' && (r.abandoned || !r.recommended) ? '#fef2f2' :
                              selected.status === 'FINALIZED' && r.recommended && !r.abandoned ? '#f0fdf4' :
                              tieSet.has(`${r.student_id}-${r.track_id}`) ? '#fffbeb' :
                              undefined,
                          }"
                          @click="toggleRow(`${r.student_id}-${r.track_id}`)"
                        >
                          <td class="text-base text-center" style="padding: 12px 8px; color: #94a3b8; user-select: none;">
                            {{ expandedRows[`${r.student_id}-${r.track_id}`] ? '▼' : '▶' }}
                          </td>
                          <td class="text-base text-center" style="padding: 12px 16px; color: #475569;">{{ rankView === 'track' ? (r.track_rank ?? '-') : (r.ranking ?? '-') }}</td>
                          <td class="text-base" style="padding: 12px 18px; color: #475569; max-width: 160px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">
                            <span v-if="r.is_enrolled">{{ r.grade }}학년 {{ r.class_no }}반 {{ r.seq_no }}번</span>
                            <span v-else class="font-mono">{{ r.student_code }}</span>
                          </td>
                          <td class="text-base font-medium" style="padding: 12px 18px; color: #1e293b; max-width: 100px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">{{ r.name }}</td>
                          <td style="padding: 12px 18px;">
                            <span class="text-base font-medium"
                              :style="{ color: r.is_enrolled ? '#16a34a' : '#94a3b8' }">
                              {{ r.is_enrolled ? '재학생' : '졸업생' }}
                            </span>
                          </td>
                          <td class="text-base" style="padding: 12px 18px; color: #475569; max-width: 140px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">{{ r.department_name }}</td>
                          <td class="text-base text-right font-semibold" style="padding: 12px 18px; color: #1e293b;">
                            {{ formatScore(r.total_score) }}
                          </td>
                          <td class="text-center" style="padding: 12px 18px;" @click.stop>
                            <div class="flex flex-col items-center gap-1">
                            <span v-if="r.abandoned" class="text-base font-semibold" style="color: #ef4444;">포기됨</span>
                            <template v-else-if="r.recommended">
                              <span class="text-base font-semibold" style="color: #16a34a;">추천 확정됨</span>
                              <button
                                v-if="selected.status === 'CLOSED'"
                                class="text-base rounded-lg whitespace-nowrap"
                                style="padding: 3px 10px; border: 1px solid #fca5a5; background: white; color: #ef4444; cursor: pointer;"
                                @click="handleUnrecommend(r)"
                              >추천 취소</button>
                            </template>
                            <button
                              v-else-if="selected.status === 'CLOSED' && !r.excluded"
                              class="text-base font-semibold rounded-lg whitespace-nowrap disabled:opacity-40"
                              style="padding: 5px 12px; border: none; background: #16a34a; color: white; cursor: pointer;"
                              :disabled="resultActing"
                              @click="handleRecommend(r)"
                            >추천 확정</button>
                            <span v-else-if="selected.status === 'CLOSED' && r.excluded" style="color: #cbd5e1;">-</span>
                            <span v-else-if="selected.status === 'FINALIZED'" class="text-base font-semibold" style="color: #ef4444;">미선발</span>
                            <span v-else class="text-base font-semibold" style="color: #94a3b8;">-</span>
                            </div>
                          </td>
                          <td class="text-center" style="padding: 12px 18px;" @click.stop>
                            <button
                              v-if="r.recommended && !r.abandoned && selected.status === 'FINALIZED'"
                              class="text-base rounded-lg whitespace-nowrap"
                              style="padding: 5px 12px; border: 1px solid #fca5a5; background: white; color: #ef4444; cursor: pointer;"
                              @click="handleAbandon(r)"
                            >포기하기</button>
                            <span v-else style="color: #cbd5e1;">-</span>
                          </td>
                          <td class="text-center" style="padding: 12px 18px;" @click.stop>
                            <div class="flex flex-col items-center gap-1">
                            <template v-if="r.excluded">
                              <span class="text-base font-semibold" :title="r.excluded_reason" style="color: #d97706;">미선발</span>
                              <button
                                v-if="selected.status === 'CLOSED'"
                                class="text-base rounded-lg whitespace-nowrap disabled:opacity-40"
                                style="padding: 3px 10px; border: 1px solid #fcd34d; background: white; color: #92400e; cursor: pointer;"
                                :disabled="resultActing"
                                @click="handleClearExclusion(r)"
                              >미선발 해제</button>
                            </template>
                            <button
                              v-else-if="selected.status === 'CLOSED'"
                              class="text-base rounded-lg whitespace-nowrap"
                              style="padding: 5px 12px; border: 1px solid #fcd34d; background: white; color: #92400e; cursor: pointer;"
                              @click="startExclude(r)"
                            >미선발 처리</button>
                            <span v-else style="color: #cbd5e1;">-</span>
                            </div>
                          </td>
                        </tr>
                        <!-- 전형요소 점수 상세 -->
                        <tr v-if="expandedRows[`${r.student_id}-${r.track_id}`]"
                          style="border-bottom: 1px solid #f1f5f9; background: #f8fafc;">
                          <td colspan="10" style="padding: 14px 36px;">
                            <div class="flex flex-wrap gap-x-6 gap-y-2">
                              <div v-for="area in areas" :key="area.id" class="flex items-center gap-2">
                                <span class="text-base" style="color: #64748b;">{{ area.name }}</span>
                                <span class="text-base font-semibold" style="color: #1e293b;">{{ getAreaScore(r, area.id) }}</span>
                              </div>
                            </div>
                          </td>
                        </tr>
                      </template>
                    </tbody>
                  </table>
                </div>
              </div>
            </div>
          </div>

        </template>
      </div>
    </div>
  </div>

  <!-- 미선발 처리 모달 -->
  <Teleport to="body">
    <div
      v-if="showExcludeModal"
      class="fixed inset-0 flex items-center justify-center"
      style="background: rgba(0,0,0,0.35); z-index: 60;"
      role="dialog"
      aria-modal="true"
      @keydown.escape="showExcludeModal = false"
    >
      <div
        class="bg-white flex flex-col"
        style="border-radius: 14px; box-shadow: 0 8px 32px rgba(0,0,0,0.15); width: 100%; max-width: 480px; margin: 0 16px; padding: 1.5rem 1.75rem;"
      >
        <h2 class="text-lg font-semibold mb-1" style="margin: 0; color: #92400e;">미선발 처리</h2>
        <p class="text-base mb-4" style="color: #475569;">
          <span class="font-semibold" style="color: #1e293b;">{{ excludeTarget?.name }}</span> 학생을 이번 라운드에서 미선발 처리합니다.
        </p>
        <label class="block text-base font-medium mb-1.5" style="color: #64748b;">미선발 사유 <span style="color: #ef4444;">*</span></label>
        <input
          v-model="excludeReasonDraft"
          type="text"
          placeholder="미선발 사유를 입력하세요"
          class="text-base w-full"
          style="border: 1px solid #fcd34d; border-radius: 8px; padding: 9px 12px; box-sizing: border-box; outline: none;"
          @keyup.enter="confirmExclude"
        />
        <div class="flex justify-end gap-2 mt-5">
          <button
            class="text-base rounded-lg whitespace-nowrap"
            style="padding: 9px 18px; border: 1px solid #e2e8f0; background: white; color: #64748b; cursor: pointer;"
            @click="showExcludeModal = false"
          >취소</button>
          <button
            class="text-base font-semibold rounded-lg whitespace-nowrap disabled:opacity-40"
            style="padding: 9px 18px; border: none; background: #d97706; color: white; cursor: pointer;"
            :disabled="!excludeReasonDraft.trim() || resultActing"
            @click="confirmExclude"
          >{{ resultActing ? '처리 중...' : '미선발 확정' }}</button>
        </div>
      </div>
    </div>
  </Teleport>

  <!-- 미결정 지원자 안내 모달 -->
  <Teleport to="body">
    <div
      v-if="showUndecidedModal"
      class="fixed inset-0 flex items-center justify-center"
      style="background: rgba(0,0,0,0.35); z-index: 60;"
      role="dialog"
      aria-modal="true"
    >
      <div
        class="bg-white flex flex-col"
        style="border-radius: 14px; box-shadow: 0 8px 32px rgba(0,0,0,0.15); width: 100%; max-width: 680px; margin: 0 16px; padding: 1.5rem 1.75rem; max-height: 80vh;"
      >
        <h2 class="text-lg font-semibold mb-1" style="margin: 0; color: #b91c1c;">마감할 수 없습니다</h2>
        <p class="text-base mb-1" style="color: #475569; line-height: 1.6;">
          아래 지원자는 추천도 미선발도 결정되지 않았습니다.<br>
          각 지원자를 추천 확정하거나 미선발 처리한 후 다시 마감하세요.
        </p>
        <p class="text-base font-semibold mb-3" style="color: #1e293b;">총 {{ undecidedList.length }}명</p>
        <div class="overflow-y-auto" style="max-height: 380px;">
          <div class="overflow-x-auto">
            <table class="w-full min-w-max" style="border-collapse: collapse;">
              <thead>
                <tr style="background: #f8fafc; border-bottom: 1px solid #e2e8f0; position: sticky; top: 0;">
                  <th class="text-base font-semibold text-left" style="padding: 11px 16px; color: #475569; width: 110px;">학년/반</th>
                  <th class="text-base font-semibold text-left" style="padding: 11px 16px; color: #475569; width: 140px;">학번</th>
                  <th class="text-base font-semibold text-left" style="padding: 11px 16px; color: #475569; width: 100px;">이름</th>
                  <th class="text-base font-semibold text-left" style="padding: 11px 16px; color: #475569; width: 150px;">대학</th>
                  <th class="text-base font-semibold text-left" style="padding: 11px 16px; color: #475569; width: 150px;">모집단위</th>
                </tr>
              </thead>
              <tbody>
                <tr
                  v-for="u in undecidedList"
                  :key="`${u.student_code}-${u.univ_name}-${u.track_name}`"
                  style="border-bottom: 1px solid #f1f5f9;"
                >
                  <td class="text-base" style="padding: 10px 16px; color: #475569;">{{ u.grade }}학년 {{ u.class_no }}반</td>
                  <td class="text-base font-mono" style="padding: 10px 16px; color: #475569;">{{ u.student_code }}</td>
                  <td class="text-base font-medium" style="padding: 10px 16px; color: #1e293b;">{{ u.student_name }}</td>
                  <td class="text-base" style="padding: 10px 16px; color: #1e293b;">{{ u.univ_name }}</td>
                  <td class="text-base" style="padding: 10px 16px; color: #475569;">{{ u.track_name }}</td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>
        <div class="flex justify-end mt-5">
          <button
            class="text-base font-semibold rounded-lg"
            style="padding: 9px 20px; border: none; background: #2563eb; color: white; cursor: pointer;"
            @click="showUndecidedModal = false"
          >닫기</button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<script setup>
import { ref, computed, onMounted, inject } from 'vue'
import {
  getRounds, openRound, closeRound, reopenRound, finalizeRound,
  calculateScores, getResults, recommendResult, unrecommendResult,
  getApplications, abandonApplication,
  excludeApplication, clearApplicationExclusion,
  getAreas,
  exportResultsExcel,
  exportRoundSummary,
  getQuotaStats,
  autoRecommend,
  autoRecommendUniv,
  blobErrMsg,
  getRoundConfirmationStatus,
} from '../../api/admin.js'
import HelpBox from '../common/HelpBox.vue'
import { dialog } from '../common/dialog.js'
import { roundStatusLabel } from '../../data/roundStatus.js'
import { formatScore } from '../../utils/scorePreviewShared.js'

const HELP_EMPTY = {
  title: '도움말 — 첫 라운드 열기 전 확인하세요',
  intro: '라운드는 한 번의 추천 진행 단위입니다. 라운드를 열면 담임교사가 지원자를 등록할 수 있게 됩니다.',
  items: [
    '라운드를 열기 전에 학급, 학생 명단, 전형요소, 대학 설정이 모두 끝났는지 확인하세요.',
    { text: '특히 전형요소는 라운드가 종료된 뒤에는 수정할 수 없으니 반드시 먼저 완성하세요.', warn: true },
    '준비가 끝났으면 "+ 라운드 열기"를 누르세요.',
  ],
}

function fmtDt(s) {
  if (!s) return ''
  const d = new Date(s)
  const pad = n => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${pad(d.getMonth()+1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`
}

const refreshSidebarRound = inject('refreshRound', () => {})

const rounds  = ref([])
const selected = ref(null)
const view    = ref('apps')
const loading = ref(false)

const apps    = ref([])
const results = ref([])
const areas   = ref([])

const roundsLoadError    = ref('')
const roundActing        = ref(false)
// 결과 행 단위 조작(추천 확정/취소, 미선발 처리/해제) 공용 진행 플래그.
// 정원 마지막 한 자리에서 두 번 클릭하면 첫 요청은 성공하는데 두 번째가
// 자기 자신을 센 정원 카운트 때문에 409("정원이 이미 찼습니다")를 띄운다.
const resultActing       = ref(false)
const calcLoading        = ref(false)
const calcMsg            = ref(null)
const downloading        = ref(false)
const downloadingSummary = ref(false)
const expandedRows       = ref({})
const quotaStats         = ref(null)
const autoRecommendActing = ref(false)
const autoRecommendResult = ref(null)
const autoRecommendScope  = ref('')

const selectedTrackId   = ref('')
const confirmationStatus = ref(null)  // { classes: [...] } | null

const showExcludeModal   = ref(false)
const excludeTarget      = ref(null)   // ResultRow | null
const excludeReasonDraft = ref('')

const showUndecidedModal = ref(false)
const undecidedList      = ref([])

function rowKey(r) { return `${r.student_id}-${r.track_id}` }

const subTabs = [
  { key: 'apps',    label: '지원 현황' },
  { key: 'results', label: '결과' },
]

const hasOpenRound = computed(() => rounds.value.some(r => r.status === 'OPEN' || r.status === 'CLOSED'))

const helpBox = computed(() => {
  if (!selected.value) return null
  const s = selected.value.status
  if (s === 'OPEN') {
    return {
      key: 'rounds-open',
      title: '도움말 — 라운드 진행 중',
      intro: '담임교사들이 지원자를 등록하고 있는 단계입니다.',
      items: [
        '모든 담임의 입력이 끝나면 "종료하기"를 눌러 라운드를 종료하세요.',
        '라운드를 종료하면 담임 입력이 차단되고 모든 지원자의 점수가 자동 계산됩니다.',
        '라운드 종료할 때 기초 데이터가 누락되어 점수 계산을 할 수 없는 학생이 있으면 오류 목록이 표시되고 종료되지 않습니다. 해당 학생의 데이터를 채운 뒤 다시 시도하세요.',
      ],
    }
  }
  if (s === 'CLOSED') {
    if (view.value === 'apps') {
      return {
        key: 'rounds-closed-apps',
        title: '도움말 — 지원 현황 확인',
        intro: '라운드 종료 시 모든 지원자의 점수가 자동 계산되어 있습니다.',
        items: [
          '총점이 비어 있거나("-") 이상하면 "점수 전체 재계산"을 눌러 다시 계산하세요.',
          '종료 후에 기초 데이터를 수정했다면 반드시 "점수 전체 재계산"을 눌러 변경 내용을 반영하세요.',
          '점수 확인이 끝나면 [결과] 탭으로 이동해 추천을 확정하세요.',
        ],
      }
    }
    return {
      key: 'rounds-closed-results',
      title: '도움말 — 추천 확정',
      intro: '순위를 확인하고 추천자를 확정하는 단계입니다.',
      items: [
        '"자동 추천 확정"을 누르면 모든 모집단위에서 순위 순으로 잔여 정원까지 자동 확정됩니다.',
        '동점 등으로 자동 확정하지 못한 모집단위는 노란색 "수동 확인 필요" 목록에 표시됩니다. 해당 모집단위에서 학생을 직접 골라 "추천 확정"을 누르세요.',
        '잘못 확정했으면 "추천 취소"로 되돌릴 수 있습니다.',
        { text: '확정이 모두 끝나면 위의 "마감하기"를 누르세요. 마감은 되돌릴 수 없으며, 마감하면 결과가 담임교사에게 공개됩니다.', warn: true },
      ],
    }
  }
  return {
    key: 'rounds-finalized',
    title: '도움말 — 마감된 라운드',
    intro: '이 라운드는 마감되어 결과가 확정되었고 담임교사에게 공개되었습니다.',
    items: [
      '[결과] 탭에서 "이 라운드 지원자 명단"(지원 학생 전원의 결과)과 "이 라운드 선발 현황"(모집단위별 지원·추천·포기·잔여석)을 엑셀로 내려받을 수 있습니다.',
      { text: '추천 확정 학생이 추천을 포기하면 "포기하기"를 눌러 처리하세요. 포기는 되돌릴 수 없습니다.', warn: true },
      '학생의 지원 포기 등으로 빈자리가 생겨 추가 추천이 필요하면 "+ 라운드 열기"로 다음 차수를 시작하세요.',
    ],
  }
})

const appsByUniv = computed(() => {
  const map = {}
  for (const app of apps.value) {
    const key = app.univ_name
    if (!map[key]) map[key] = []
    map[key].push(app)
  }
  for (const key of Object.keys(map)) {
    map[key].sort((a, b) => {
      if (a.is_enrolled !== b.is_enrolled) return a.is_enrolled ? -1 : 1
      const code = (a.student_code ?? '').localeCompare(b.student_code ?? '')
      if (code !== 0) return code
      return a.track_name.localeCompare(b.track_name, 'ko')
    })
  }
  return map
})

function appTotalScore(app) {
  const r = results.value.find(r => r.student_id === app.student_id && r.track_id === app.track_id)
  return r ? formatScore(r.total_score) : '-'
}

const tracksInRound = computed(() => {
  const seen = new Set()
  return results.value
    .filter(r => { if (seen.has(r.track_id)) return false; seen.add(r.track_id); return true })
    .map(r => ({ id: r.track_id, univ_name: r.univ_name, track_name: r.track_name }))
})

const trackQuotaMap = computed(() => {
  const map = {}
  if (!quotaStats.value) return map
  for (const u of quotaStats.value.univs) {
    for (const t of u.tracks) {
      map[t.track_id] = {
        univId: u.univ_id,
        univName: u.univ_name,
        unitQuota: t.unit_quota,
        unitUsed: t.unit_used,
        totalQuota: u.total_quota,
        totalUsed: u.total_used,
      }
    }
  }
  return map
})

const rankView = ref('track')

const resultsByUniv = computed(() => {
  const map = {}
  for (const r of results.value) {
    const key = `${r.univ_name} ${r.track_name}`
    if (!map[key]) {
      const q = trackQuotaMap.value[r.track_id]
      const unitQuota = q?.unitQuota ?? null
      const totalQuota = q?.totalQuota ?? null
      map[key] = {
        univId: q?.univId ?? null,
        univName: r.univ_name,
        unitQuota,
        totalQuota,
        remaining: unitQuota != null ? Math.max(0, unitQuota - (q?.unitUsed ?? 0)) : null,
        univRemaining: totalQuota != null ? Math.max(0, totalQuota - (q?.totalUsed ?? 0)) : null,
        results: [],
      }
    }
    map[key].results.push(r)
  }
  return map
})

const resultsByUnivOnly = computed(() => {
  const map = {}
  for (const r of results.value) {
    const key = r.univ_name
    if (!map[key]) {
      const q = trackQuotaMap.value[r.track_id]
      const totalQuota = q?.totalQuota ?? null
      map[key] = {
        univId: q?.univId ?? null,
        univName: r.univ_name,
        totalQuota,
        univRemaining: totalQuota != null ? Math.max(0, totalQuota - (q?.totalUsed ?? 0)) : null,
        results: [],
      }
    }
    map[key].results.push(r)
  }
  for (const g of Object.values(map)) {
    g.results.sort((a, b) => {
      if (a.ranking == null && b.ranking == null) return 0
      if (a.ranking == null) return 1
      if (b.ranking == null) return -1
      return a.ranking - b.ranking
    })
  }
  return map
})

const resultsByView = computed(() => rankView.value === 'track' ? resultsByUniv.value : resultsByUnivOnly.value)

// 대학별 자동 추천 버튼은 대학 단위 동작이다. 모집단위별 보기에서는 그룹이 모집단위마다
// 나뉘므로 각 대학의 첫 그룹에만 노출한다 — 같은 버튼이 모집단위 수만큼 반복되어
// "이 모집단위만 처리"로 오해되는 것을 막는다.
const univAutoButtonKeys = computed(() => {
  const seen = new Set()
  const keys = new Set()
  for (const [key, g] of Object.entries(resultsByView.value)) {
    if (g.univId == null || seen.has(g.univId)) continue
    seen.add(g.univId)
    keys.add(key)
  }
  return keys
})

const tieSet = computed(() => {
  const set = new Set()
  if (rankView.value === 'track') {
    const counts = {}
    for (const r of results.value) {
      if (r.track_rank == null) continue
      const k = `${r.track_id}-${r.round_id}-${r.track_rank}`
      if (!counts[k]) counts[k] = []
      counts[k].push(r)
    }
    for (const rows of Object.values(counts)) {
      if (rows.length > 1) for (const r of rows) set.add(`${r.student_id}-${r.track_id}`)
    }
  } else {
    const counts = {}
    for (const r of results.value) {
      if (r.ranking == null) continue
      const k = `${r.univ_name}-${r.round_id}-${r.ranking}`
      if (!counts[k]) counts[k] = []
      counts[k].push(r)
    }
    for (const rows of Object.values(counts)) {
      if (rows.length > 1) for (const r of rows) set.add(`${r.student_id}-${r.track_id}`)
    }
  }
  return set
})

function getAreaScore(r, areaId) {
  try {
    const detail = typeof r.score_detail === 'string'
      ? JSON.parse(r.score_detail)
      : r.score_detail
    const v = detail[String(areaId)]
    return v !== undefined ? formatScore(v) : '-'
  } catch {
    return '-'
  }
}

async function loadRounds() {
  roundsLoadError.value = ''
  try {
    rounds.value = await getRounds()
  } catch (e) {
    rounds.value = []
    roundsLoadError.value = e.response?.data ?? e.message ?? '오류가 발생했습니다'
  }
}

function classLabel(c) {
  if (c.grade === 0 && c.class_no === 0) return '졸업생 담당'
  const base = `${c.grade}학년 ${c.class_no}반`
  return c.teacher_name ? `${base} (${c.teacher_name})` : base
}

async function loadConfirmationStatus() {
  if (!selected.value || selected.value.status !== 'OPEN') {
    confirmationStatus.value = null
    return
  }
  try {
    confirmationStatus.value = await getRoundConfirmationStatus(selected.value.id)
  } catch {
    confirmationStatus.value = null
  }
}

async function selectRound(r) {
  selected.value = r
  calcMsg.value = null
  autoRecommendResult.value = null
  autoRecommendScope.value = ''
  confirmationStatus.value = null
  await Promise.all([loadApps(), loadResults(), loadAreas(), loadConfirmationStatus()])
}

async function loadApps() {
  if (!selected.value) return
  apps.value = await getApplications(selected.value.id)
}

async function loadResults() {
  if (!selected.value) return
  ;[results.value, quotaStats.value] = await Promise.all([
    getResults(selected.value.id, selectedTrackId.value || null),
    getQuotaStats(),
  ])
  expandedRows.value = {}
}

function toggleRow(key) {
  const next = { ...expandedRows.value }
  if (next[key]) delete next[key]
  else next[key] = true
  expandedRows.value = next
}

async function loadAreas() {
  areas.value = await getAreas()
}

async function handleOpenRound() {
  if (!(await dialog.confirm({
    title: '라운드 열기',
    message: '새 라운드를 열겠습니까?\n라운드를 열면 담임교사의 지원 입력이 시작됩니다.',
    confirmText: '라운드 열기',
  }))) return
  loading.value = true
  try {
    await openRound()
    await loadRounds()
    const open = rounds.value.find(r => r.status === 'OPEN')
    if (open) await selectRound(open)
    await refreshSidebarRound()
  } catch (e) {
    await dialog.alert({ title: '오류', message: e.response?.data || e.message, level: 'error' })
  } finally {
    loading.value = false
  }
}

async function handleCloseRound(id) {
  if (roundActing.value) return

  // 미확정 학급 조회
  let closeMsg = '라운드를 종료하시겠습니까?\n담임교사의 입력이 차단되고, 모든 지원자의 점수가 계산됩니다.\n필요하면 "다시 열기"로 되돌릴 수 있습니다.'
  try {
    const status = await getRoundConfirmationStatus(id)
    const unconfirmed = status.classes.filter(c => !c.confirmed)
    if (unconfirmed.length > 0) {
      const labels = unconfirmed.slice(0, 10).map(classLabel)
      const extra = unconfirmed.length > 10 ? `\n외 ${unconfirmed.length - 10}곳` : ''
      closeMsg += `\n\n⚠ 아직 입력을 확정하지 않은 학급이 있습니다:\n${labels.join(', ')}${extra}`
    }
  } catch {
    closeMsg += '\n\n확정 현황을 불러오지 못했습니다'
  }

  if (!(await dialog.confirm({
    title: '라운드 종료',
    message: closeMsg,
    confirmText: '종료하기',
    level: 'warn',
  }))) return
  roundActing.value = true
  try {
    await closeRound(id)
    await loadRounds()
    if (selected.value?.id === id) {
      const updated = rounds.value.find(r => r.id === id)
      if (updated) selected.value = updated
      await loadResults()
    }
    await refreshSidebarRound()
  } catch (e) {
    await dialog.alert({ title: '오류', message: e.response?.data || e.message, level: 'error' })
  } finally {
    roundActing.value = false
  }
}

async function handleReopenRound(id) {
  if (roundActing.value) return
  if (!(await dialog.confirm({
    title: '라운드 다시 열기',
    message: '라운드를 다시 여시겠습니까?\n지금까지 확정한 추천 표시가 모두 초기화됩니다.',
    confirmText: '다시 열기',
    level: 'warn',
  }))) return
  roundActing.value = true
  try {
    await reopenRound(id)
    await loadRounds()
    if (selected.value?.id === id) {
      const updated = rounds.value.find(r => r.id === id)
      if (updated) selected.value = updated
      await loadConfirmationStatus()
    }
    await refreshSidebarRound()
  } catch (e) {
    await dialog.alert({ title: '오류', message: e.response?.data || e.message, level: 'error' })
  } finally {
    roundActing.value = false
  }
}

async function handleFinalizeRound(id) {
  if (roundActing.value) return
  if (!(await dialog.confirm({
    title: '라운드 마감',
    message: '라운드를 마감하시겠습니까?\n추천 확정이 고정되고, 결과가 담임교사에게 공개됩니다.',
    confirmText: '마감하기',
    level: 'danger',
    dangerNotice: '한번 마감된 라운드는 절대로 되돌릴 수 없습니다.',
    finalConfirmText: '마감 확정',
  }))) return
  roundActing.value = true
  try {
    await finalizeRound(id)
    await loadRounds()
    if (selected.value?.id === id) {
      const updated = rounds.value.find(r => r.id === id)
      if (updated) selected.value = updated
    }
    await refreshSidebarRound()
  } catch (e) {
    const d = e.response?.data
    if (d != null && typeof d === 'object' && Array.isArray(d.undecided)) {
      if (d.undecided.length === 0) {
        await dialog.alert({ title: '마감할 수 없습니다', message: d.error ?? '미결정 지원자 오류가 발생했습니다', level: 'error' })
      } else {
        undecidedList.value = d.undecided
        showUndecidedModal.value = true
      }
    } else {
      await dialog.alert({ title: '마감할 수 없습니다', message: finalizeErrMsg(e), level: 'error' })
    }
  } finally {
    roundActing.value = false
  }
}

// finalize 422는 JSON 바디 {error, track_violations, univ_violations} — 위반 목록을 사람이 읽을 수 있게 펼친다
function finalizeErrMsg(e) {
  const d = e.response?.data
  if (d != null && typeof d === 'object' && (Array.isArray(d.track_violations) || Array.isArray(d.univ_violations))) {
    const lines = [d.error ?? '정원 초과로 라운드를 확정할 수 없습니다']
    for (const v of d.track_violations ?? []) {
      lines.push(`- ${v.univ_name} ${v.track_name}: 모집단위 정원 ${v.unit_quota}명, 추천 확정 ${v.total_recommended}명`)
    }
    for (const v of d.univ_violations ?? []) {
      lines.push(`- ${v.univ_name} (대학 전체): 정원 ${v.total_quota}명, 추천 확정 ${v.total_recommended}명`)
    }
    return lines.join('\n')
  }
  return typeof d === 'string' ? d : (e.message ?? '오류가 발생했습니다')
}

async function handleCalculate() {
  if (!selected.value) return
  const roundId = selected.value.id
  calcLoading.value = true
  calcMsg.value = null
  try {
    const res = await calculateScores(roundId)
    if (selected.value?.id !== roundId) return
    calcMsg.value = { ok: true, text: `점수 재계산 완료: ${res.calculated}건` }
    await loadResults()
  } catch (e) {
    calcMsg.value = { ok: false, text: e.response?.data || e.message }
  } finally {
    calcLoading.value = false
  }
}

async function handleAbandon(app) {
  if (!(await dialog.confirm({
    title: '지원 포기 처리',
    message: `${app.name} 학생의 지원을 포기 처리하시겠습니까?`,
    confirmText: '포기 처리',
    level: 'danger',
    dangerNotice: '한 번 포기하면 다시 되돌릴 수 없습니다. 재추천을 희망하면 다음 라운드에서 재지원해야 합니다.',
    finalConfirmText: '포기 확정',
  }))) return
  try {
    await abandonApplication(app.student_id, app.track_id, app.round_id)
    await Promise.all([loadApps(), loadResults()])
  } catch (e) {
    await dialog.alert({ title: '오류', message: e.response?.data || e.message, level: 'error' })
  }
}

function startExclude(r) {
  excludeTarget.value = r
  excludeReasonDraft.value = ''
  showExcludeModal.value = true
}

async function confirmExclude() {
  if (resultActing.value) return
  const r = excludeTarget.value
  if (!r) return
  const reason = excludeReasonDraft.value.trim()
  if (!reason) return
  resultActing.value = true
  try {
    await excludeApplication(r.student_id, r.track_id, r.round_id, reason)
    showExcludeModal.value = false
    await loadResults()
  } catch (e) {
    await dialog.alert({ title: '오류', message: e.response?.data || e.message, level: 'error' })
  } finally {
    resultActing.value = false
  }
}

async function handleClearExclusion(r) {
  if (resultActing.value) return
  if (!(await dialog.confirm({
    title: '미선발 해제',
    message: `${r.name} 학생의 미선발 처리를 해제하시겠습니까?`,
    confirmText: '해제',
    level: 'warn',
  }))) return
  resultActing.value = true
  try {
    await clearApplicationExclusion(r.student_id, r.track_id, r.round_id)
    await loadResults()
  } catch (e) {
    await dialog.alert({ title: '오류', message: e.response?.data || e.message, level: 'error' })
  } finally {
    resultActing.value = false
  }
}

async function downloadExcel() {
  if (!selected.value) return
  downloading.value = true
  try {
    const res = await exportResultsExcel(selected.value.id)
    const url = URL.createObjectURL(res.data)
    const a = document.createElement('a')
    a.href = url
    a.download = `round_${selected.value.id}_applicants.xlsx`
    a.click()
    URL.revokeObjectURL(url)
  } catch (e) {
    await dialog.alert({ title: '다운로드 실패', message: await blobErrMsg(e), level: 'error' })
  } finally {
    downloading.value = false
  }
}

async function downloadSummary() {
  if (!selected.value) return
  downloadingSummary.value = true
  try {
    const res = await exportRoundSummary(selected.value.id)
    const url = URL.createObjectURL(res.data)
    const a = document.createElement('a')
    a.href = url
    a.download = `round_${selected.value.id}_summary.xlsx`
    a.click()
    URL.revokeObjectURL(url)
  } catch (e) {
    await dialog.alert({ title: '다운로드 실패', message: await blobErrMsg(e), level: 'error' })
  } finally {
    downloadingSummary.value = false
  }
}

const AUTO_RECOMMEND_NOTE =
  '모집단위 정원까지 순위순으로 채운 뒤, 대학 정원이 있으면 대학 전체 순위로 상위까지만 남깁니다.\n'
  + '동점이 정원 경계를 가르는 지점은 자동 확정하지 않고 수동 확인 목록으로 알려드립니다.'

async function handleAutoRecommend() {
  if (!selected.value) return
  if (!(await dialog.confirm({
    title: '자동 추천 확정',
    message: `모든 대학에 대해 추천을 자동 확정할까요?\n${AUTO_RECOMMEND_NOTE}`,
    confirmText: '자동 확정',
  }))) return
  await runAutoRecommend(() => autoRecommend(selected.value.id), '전체 대학')
}

async function handleAutoRecommendUniv(group) {
  if (!selected.value || group.univId == null) return
  if (!(await dialog.confirm({
    title: '이 대학 자동 추천',
    message: `${group.univName}의 모집단위만 자동 확정할까요?\n${AUTO_RECOMMEND_NOTE}`,
    confirmText: '자동 확정',
  }))) return
  await runAutoRecommend(
    () => autoRecommendUniv(selected.value.id, group.univId),
    group.univName,
  )
}

async function runAutoRecommend(call, scopeLabel) {
  autoRecommendActing.value = true
  autoRecommendResult.value = null
  autoRecommendScope.value = ''
  try {
    const res = await call()
    autoRecommendResult.value = res
    autoRecommendScope.value = scopeLabel
    await loadResults()
  } catch (e) {
    await dialog.alert({ title: '오류', message: e.response?.data || e.message, level: 'error' })
  } finally {
    autoRecommendActing.value = false
  }
}

async function handleRecommend(r) {
  if (resultActing.value) return
  resultActing.value = true
  try {
    await recommendResult(r.student_id, r.track_id, r.round_id)
    await loadResults()
  } catch (e) {
    await dialog.alert({ title: '오류', message: e.response?.data || e.message, level: 'error' })
  } finally {
    resultActing.value = false
  }
}

async function handleUnrecommend(r) {
  if (resultActing.value) return
  if (!(await dialog.confirm({
    title: '추천 취소',
    message: `${r.name} 학생의 추천을 취소하시겠습니까?`,
    confirmText: '추천 취소',
    level: 'warn',
  }))) return
  resultActing.value = true
  try {
    await unrecommendResult(r.student_id, r.track_id, r.round_id)
    await loadResults()
  } catch (e) {
    await dialog.alert({ title: '오류', message: e.response?.data || e.message, level: 'error' })
  } finally {
    resultActing.value = false
  }
}

onMounted(loadRounds)
</script>
