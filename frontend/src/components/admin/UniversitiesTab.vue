<template>
  <div class="py-8 px-4 sm:px-10">

    <!-- 페이지 헤더 -->
    <div class="flex items-start justify-between mb-5">
      <div>
        <p class="text-base mb-1" style="color: #94a3b8;">관리자</p>
        <h1 class="text-2xl font-semibold" style="color: #1e293b; margin: 0;">대학 설정</h1>
      </div>
      <div class="flex flex-wrap items-center gap-2">
        <button
            class="text-base font-medium rounded-lg disabled:opacity-40"
            style="padding: 8px 16px; border: none; background: #16a34a; color: white; cursor: pointer;"
            :disabled="downloading"
            @click="doExportQuotaStats(true)"
        >전체 명단·정원 현황 내보내기</button>
        <span style="color: #cbd5e1; user-select: none;">|</span>
        <button
          class="text-base font-medium rounded-lg disabled:opacity-40"
          style="padding: 8px 16px; border: 1px solid #e2e8f0; background: white; color: #475569; cursor: pointer;"
          :disabled="settingsBusy"
          @click="dlSettingsTemplate"
        >대학 설정 양식 다운로드</button>
        <button
            class="text-base font-medium rounded-lg disabled:opacity-40"
            style="padding: 8px 16px; border: 1px solid #e2e8f0; background: white; color: #475569; cursor: pointer;"
            :disabled="settingsBusy"
            @click="dlSettingsExport"
        >대학 설정 내보내기</button>
        <label
            class="text-base font-medium rounded-lg cursor-pointer"
            :class="settingsBusy ? 'opacity-60 pointer-events-none' : ''"
            style="padding: 8px 16px; background: #2563eb; color: white;"
        >
          {{ settingsBusy ? '불러오는 중…' : '대학 설정 가져오기' }}
          <input type="file" accept=".xlsx,.csv" class="hidden" :disabled="settingsBusy" @change="onSettingsFile" />
        </label>
      </div>
    </div>

    <HelpBox class="mb-5" storage-key="univs" :title="HELP.title" :intro="HELP.intro" :items="HELP.items" />

    <div class="flex flex-col lg:flex-row lg:items-start gap-6" style="min-height: 480px;">

      <!-- ── 좌측: 대학 목록 ─────────────────────────────────── -->
      <div class="flex flex-col flex-shrink-0 w-full lg:w-[300px]">
        <div class="flex items-center justify-between mb-4">
          <h2 class="text-lg font-semibold" style="color: #1e293b;">대학 목록</h2>
          <button
            class="text-base font-medium rounded-lg disabled:opacity-40"
            style="padding: 7px 14px; border: none; background: #2563eb; color: white; cursor: pointer;"
            :disabled="saving"
            @click="startAddUniv"
          >+ 대학 추가</button>
        </div>

        <p v-if="error" class="text-base mb-4" style="color: #ef4444;">{{ error }}</p>

        <!-- 대학 추가 폼 -->
        <div v-if="addingUniv" class="rounded-xl mb-3"
          style="padding: 16px 18px; background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04);">
          <h3 class="text-base font-semibold mb-3" style="color: #1e293b;">새 대학 추가</h3>
          <div class="space-y-3">
            <div>
              <label class="block text-base font-medium mb-1.5" style="color: #64748b;">대학명</label>
              <input v-model="univForm.univ_name" type="text"
                class="w-full text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
                style="border: 1px solid #e2e8f0; border-radius: 8px; padding: 9px 12px; box-sizing: border-box;"
                placeholder="예) 한국대학교" />
            </div>
            <div>
              <label class="block text-base font-medium mb-1.5" style="color: #64748b;">전체 정원</label>
              <QuotaInput v-model:unlimited="univForm.unlimited" v-model:quota="univForm.total_quota" />
            </div>
            <div class="flex items-center gap-2">
              <input v-model="univForm.prioritize_enrolled" type="checkbox" id="add-univ-pe" class="accent-blue-600 w-4 h-4" />
              <label for="add-univ-pe" class="text-base" style="color: #475569;">재학생 우선</label>
            </div>
          </div>
          <div class="flex gap-2 mt-4">
            <button
              class="text-base font-semibold rounded-lg disabled:opacity-40"
              style="padding: 8px 18px; border: none; background: #2563eb; color: white; cursor: pointer;"
              :disabled="saving || !univForm.univ_name.trim()"
              @click="saveAddUniv"
            >{{ saving ? '저장 중…' : '저장' }}</button>
            <button
              class="text-base rounded-lg"
              style="padding: 8px 18px; border: 1px solid #e2e8f0; background: white; color: #64748b; cursor: pointer;"
              :disabled="saving"
              @click="addingUniv = false"
            >취소</button>
          </div>
        </div>

        <!-- 대학 카드 목록 -->
        <div class="overflow-y-auto flex-1 space-y-2">
          <div
            v-for="u in univs"
            :key="u.id"
            class="rounded-xl transition-all"
            :style="{
              background: selectedUnivId === u.id ? '#eff6ff' : 'white',
              border: selectedUnivId === u.id ? '1px solid #93c5fd' : '1px solid #e2e8f0',
              boxShadow: '0 1px 4px rgba(0,0,0,0.07)',
            }"
          >
            <template v-if="editingUnivId !== u.id">
              <div class="cursor-pointer" style="padding: 14px 16px;" @click="selectUniv(u.id)">
                <p class="text-lg font-semibold" style="color: #1e293b; margin: 0;">{{ u.univ_name }}</p>
                <p class="text-base" style="margin: 4px 0 0; color: #64748b;">
                  대학 정원: <span class="font-medium">{{ u.total_quota != null ? u.total_quota + '명' : '무제한' }}</span>
                  &nbsp;·&nbsp;재학생 우선: {{ u.prioritize_enrolled ? '○' : '-' }}
                </p>
                <div class="flex gap-3">
                  <button class="text-base font-medium disabled:opacity-40"
                          style="color: #2563eb; background: none; border: none; cursor: pointer; padding: 0;"
                          :disabled="saving" @click.stop="startEditUniv(u)">편집</button>
                  <button class="text-base font-medium disabled:opacity-40"
                          style="color: #ef4444; background: none; border: none; cursor: pointer; padding: 0;"
                          :disabled="saving" @click.stop="removeUniv(u.id)">삭제</button>
                </div>
              </div>
            </template>

            <template v-else>
              <div style="padding: 14px 16px; background: #fefce8; border-radius: 10px;">
                <div class="space-y-3">
                  <div>
                    <label class="block text-base font-medium mb-1.5" style="color: #64748b;">대학명</label>
                    <input v-model="univForm.univ_name" type="text"
                      class="w-full text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
                      style="border: 1px solid #e2e8f0; border-radius: 8px; padding: 9px 12px; box-sizing: border-box;" />
                  </div>
                  <div>
                    <label class="block text-base font-medium mb-1.5" style="color: #64748b;">전체 정원</label>
                    <QuotaInput v-model:unlimited="univForm.unlimited" v-model:quota="univForm.total_quota" />
                  </div>
                  <div class="flex items-center gap-2">
                    <input v-model="univForm.prioritize_enrolled" type="checkbox"
                      :id="`edit-univ-pe-${u.id}`" class="accent-blue-600 w-4 h-4" />
                    <label :for="`edit-univ-pe-${u.id}`" class="text-base" style="color: #475569;">재학생 우선</label>
                  </div>
                </div>
                <div class="flex gap-2 mt-4">
                  <button
                    class="text-base font-semibold rounded-lg disabled:opacity-40"
                    style="padding: 8px 18px; border: none; background: #2563eb; color: white; cursor: pointer;"
                    :disabled="saving || !univForm.univ_name.trim()"
                    @click="saveEditUniv(u.id)"
                  >{{ saving ? '저장 중…' : '저장' }}</button>
                  <button class="text-base rounded-lg"
                    style="padding: 8px 18px; border: 1px solid #e2e8f0; background: white; color: #64748b; cursor: pointer;"
                    :disabled="saving" @click="editingUnivId = null">취소</button>
                </div>
              </div>
            </template>
          </div>

          <div v-if="univs.length === 0 && !addingUniv"
            class="text-base text-center" style="padding: 48px 0; color: #94a3b8;">
            등록된 대학이 없습니다.
          </div>
        </div>
      </div>

      <!-- ── 우측: 모집단위 ───────────────────────────────────── -->
      <div class="flex-1 min-w-0">
        <div v-if="!selectedUniv" class="flex items-center justify-center" style="height: 300px;">
          <p class="text-base" style="color: #94a3b8;">왼쪽에서 대학을 선택하면 모집단위를 관리할 수 있습니다.</p>
        </div>

        <template v-else>
          <!-- 헤더 -->
          <div class="flex items-start justify-between mb-4 flex-wrap gap-2">
            <div>
              <h2 class="text-lg font-semibold" style="color: #1e293b; margin: 0;">
                {{ selectedUniv.univ_name }} — 모집단위
              </h2>
              <p class="text-base" style="color: #64748b; margin: 4px 0 0;">
                대학 전체 정원:
                <span class="font-medium" style="color: #1e293b;">
                  {{ selectedUniv.total_quota != null ? selectedUniv.total_quota + '명' : '무제한' }}
                </span>
                <template v-if="selectedUnivStats">
                  &nbsp;·&nbsp;대학 전체 추천인원:
                  <span class="font-medium" style="color: #1e293b;">{{ selectedUnivStats.total_used }}명</span>
                  &nbsp;·&nbsp;대학 전체 잔여인원:
                  <span class="font-medium"
                    :style="{ color: selectedUniv.total_quota != null && selectedUnivStats.total_used >= selectedUniv.total_quota ? '#ef4444' : '#1e293b' }">
                    {{ remainingLabel(selectedUnivStats.total_used, selectedUniv.total_quota) }}
                  </span>
                </template>
              </p>
            </div>
            <div class="flex gap-2">
              <button
                class="text-base font-medium rounded-lg disabled:opacity-40"
                style="padding: 8px 16px; border: none; background: #16a34a; color: white; cursor: pointer;"
                :disabled="downloading"
                @click="doExportQuotaStats(false)"
              >이 대학 명단·정원 현황</button>
              <button
                class="text-base font-medium rounded-lg disabled:opacity-40"
                style="padding: 8px 16px; border: none; background: #2563eb; color: white; cursor: pointer;"
                :disabled="saving"
                @click="startAddTrack"
              >+ 모집단위 추가</button>
            </div>
          </div>

          <!-- 모집단위 테이블 -->
          <div class="rounded-xl overflow-hidden"
            style="background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04);">
            <div class="overflow-x-auto">
              <table style="border-collapse: collapse; table-layout: fixed; width: 100%; min-width: 780px;">
                <colgroup>
                  <col>
                  <col style="width: 110px;">
                  <col style="width: 100px;">
                  <col style="width: 100px;">
                  <col style="width: 130px;">
                  <col style="width: 160px;">
                </colgroup>
                <thead>
                  <tr style="background: #f8fafc; border-bottom: 1px solid #e2e8f0;">
                    <th class="text-base font-semibold text-left" style="padding: 14px 20px; color: #475569;">모집단위명</th>
                    <th class="text-base font-semibold text-left" style="padding: 14px 20px; color: #475569;">제한인원</th>
                    <th class="text-base font-semibold text-left" style="padding: 14px 20px; color: #475569;">추천인원</th>
                    <th class="text-base font-semibold text-left" style="padding: 14px 20px; color: #475569;">잔여인원</th>
                    <th class="text-base font-semibold text-center" style="padding: 14px 20px; color: #475569;">재학생 우선</th>
                    <th style="padding: 14px 20px;"></th>
                  </tr>
                </thead>
                <tbody>
                  <!-- 추가 행 -->
                  <tr v-if="addingTrack" style="background: #eff6ff; border-bottom: 1px solid #bfdbfe;">
                    <td style="padding: 10px 16px;">
                      <input v-model="trackForm.track_name" type="text"
                        class="text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
                        style="width: 100%; border: 1px solid #93c5fd; border-radius: 6px; padding: 8px 10px; box-sizing: border-box;"
                        placeholder="예) 자연계열" />
                    </td>
                    <td style="padding: 10px 16px;">
                      <QuotaInput v-model:unlimited="trackForm.unlimited" v-model:quota="trackForm.unit_quota" />
                    </td>
                    <td class="text-base" style="padding: 10px 16px; color: #94a3b8;">—</td>
                    <td class="text-base" style="padding: 10px 16px; color: #94a3b8;">—</td>
                    <td class="text-center" style="padding: 10px 16px;">
                      <input v-model="trackForm.prioritize_enrolled" type="checkbox" class="accent-blue-600 w-4 h-4" :disabled="univPrioritize" />
                    </td>
                    <td style="padding: 10px 16px;">
                      <div class="flex gap-2">
                        <button
                          class="text-base font-semibold rounded-lg disabled:opacity-40"
                          style="padding: 7px 14px; border: none; background: #2563eb; color: white; cursor: pointer;"
                          :disabled="saving || !trackForm.track_name.trim()"
                          @click="saveAddTrack"
                        >{{ saving ? '저장 중…' : '저장' }}</button>
                        <button class="text-base rounded-lg"
                          style="padding: 7px 14px; border: 1px solid #e2e8f0; background: white; color: #64748b; cursor: pointer;"
                          :disabled="saving" @click="addingTrack = false">취소</button>
                      </div>
                    </td>
                  </tr>

                  <template v-for="t in tracksWithStats" :key="t.id">
                    <!-- 보기 행 -->
                    <tr v-if="editingTrackId !== t.id"
                      class="hover:bg-slate-50"
                      style="border-bottom: 1px solid #f1f5f9; transition: background 0.1s;">
                      <td class="text-base" style="padding: 14px 20px; color: #1e293b;">{{ t.track_name }}</td>
                      <td class="text-base" style="padding: 14px 20px; color: #1e293b;">
                        {{ t.unit_quota != null ? t.unit_quota + '명' : '무제한' }}
                      </td>
                      <td style="padding: 14px 20px;">
                        <button
                          class="text-base font-medium underline"
                          style="color: #2563eb; background: none; border: none; cursor: pointer; padding: 0;"
                          @click="openRecommendedModal(t)"
                        >{{ t.unit_used }}명</button>
                      </td>
                      <td class="text-base font-medium" style="padding: 14px 20px;"
                        :style="{ color: t.unit_quota != null && t.unit_used >= t.unit_quota ? '#ef4444' : '#1e293b' }">
                        {{ remainingLabel(t.unit_used, t.unit_quota) }}
                      </td>
                      <td class="text-base text-center" style="padding: 14px 20px; color: #1e293b;">
                        {{ t.prioritize_enrolled ? '○' : '-' }}
                      </td>
                      <td style="padding: 14px 20px;">
                        <div class="flex gap-3">
                          <button class="text-base font-medium disabled:opacity-40"
                            style="color: #2563eb; background: none; border: none; cursor: pointer; padding: 0;"
                            :disabled="saving" @click="startEditTrack(t)">편집</button>
                          <button class="text-base font-medium disabled:opacity-40"
                            style="color: #ef4444; background: none; border: none; cursor: pointer; padding: 0;"
                            :disabled="saving" @click="removeTrack(t.id)">삭제</button>
                        </div>
                      </td>
                    </tr>
                    <!-- 편집 행 -->
                    <tr v-else style="background: #fefce8; border-bottom: 1px solid #fde68a;">
                      <td style="padding: 10px 16px;">
                        <input v-model="trackForm.track_name" type="text"
                          class="text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
                          style="width: 100%; border: 1px solid #fbbf24; border-radius: 6px; padding: 8px 10px; box-sizing: border-box;" />
                      </td>
                      <td style="padding: 10px 16px;">
                        <QuotaInput v-model:unlimited="trackForm.unlimited" v-model:quota="trackForm.unit_quota" />
                      </td>
                      <td class="text-base" style="padding: 10px 16px; color: #94a3b8;">—</td>
                      <td class="text-base" style="padding: 10px 16px; color: #94a3b8;">—</td>
                      <td class="text-center" style="padding: 10px 16px;">
                        <input v-model="trackForm.prioritize_enrolled" type="checkbox" class="accent-blue-600 w-4 h-4" :disabled="univPrioritize" />
                      </td>
                      <td style="padding: 10px 16px;">
                        <div class="flex gap-2">
                          <button
                            class="text-base font-semibold rounded-lg disabled:opacity-40"
                            style="padding: 7px 14px; border: none; background: #2563eb; color: white; cursor: pointer;"
                            :disabled="saving || !trackForm.track_name.trim()"
                            @click="saveEditTrack(t.id)"
                          >{{ saving ? '저장 중…' : '저장' }}</button>
                          <button class="text-base rounded-lg"
                            style="padding: 7px 14px; border: 1px solid #e2e8f0; background: white; color: #64748b; cursor: pointer;"
                            :disabled="saving" @click="editingTrackId = null">취소</button>
                        </div>
                      </td>
                    </tr>
                  </template>

                  <tr v-if="tracks.length === 0 && !addingTrack">
                    <td colspan="6" class="text-base text-center" style="padding: 48px 20px; color: #94a3b8;">
                      등록된 모집단위가 없습니다.
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>
          </div>
        </template>
      </div>
    </div>
  </div>

  <!-- ── 추천 확정 목록 모달 ──────────────────────────────────── -->
  <div v-if="modal.open"
    class="fixed inset-0 flex items-center justify-center z-50"
    style="background: rgba(0,0,0,0.35);"
    @click.self="modal.open = false"
    @keydown.escape.window="modal.open = false"
  >
    <div class="bg-white flex flex-col" style="border-radius: 14px; box-shadow: 0 8px 32px rgba(0,0,0,0.15); width: 100%; max-width: 560px; margin: 0 16px; max-height: 80vh;">
      <!-- 모달 헤더 -->
      <div class="flex items-center justify-between flex-shrink-0" style="padding: 18px 22px; border-bottom: 1px solid #f1f5f9;">
        <div>
          <h3 class="text-lg font-semibold" style="color: #1e293b; margin: 0;">{{ modal.trackName }} 추천 확정 목록</h3>
          <p class="text-base" style="color: #94a3b8; margin: 2px 0 0;">총 {{ modal.entries.length }}명</p>
        </div>
        <button
          class="text-xl leading-none"
          style="background: none; border: none; cursor: pointer; color: #94a3b8;"
          @click="modal.open = false">✕</button>
      </div>

      <!-- 모달 본문 -->
      <div class="overflow-y-auto flex-1" style="padding: 18px 22px;">
        <div v-if="modal.loading" class="text-base text-center" style="padding: 48px 0; color: #94a3b8;">로딩 중…</div>
        <div v-else-if="modal.entries.length === 0" class="text-base text-center" style="padding: 48px 0; color: #94a3b8;">
          추천 확정된 학생이 없습니다.
        </div>
        <template v-else>
          <div v-for="group in groupedByRound" :key="group.round_id" class="mb-6">
            <h4 class="text-base font-semibold mb-3" style="color: #64748b; letter-spacing: 0.04em; text-transform: uppercase;">
              {{ group.round_id }}차 ({{ group.entries.length }}명)
            </h4>
            <div class="rounded-xl overflow-hidden"
              style="border: 1px solid #e2e8f0;">
              <table class="w-full" style="border-collapse: collapse;">
                <thead>
                  <tr style="background: #f8fafc; border-bottom: 1px solid #e2e8f0;">
                    <th class="text-base font-semibold text-left" style="padding: 10px 14px; color: #475569; width: 50px;">순위</th>
                    <th class="text-base font-semibold text-left" style="padding: 10px 14px; color: #475569;">이름</th>
                    <th class="text-base font-semibold text-left" style="padding: 10px 14px; color: #475569;">학번</th>
                    <th class="text-base font-semibold text-center" style="padding: 10px 14px; color: #475569; width: 70px;">구분</th>
                    <th class="text-base font-semibold text-center" style="padding: 10px 14px; color: #475569; width: 70px;">상태</th>
                  </tr>
                </thead>
                <tbody>
                  <tr
                    v-for="e in group.entries"
                    :key="e.student_id"
                    :style="{
                      background: e.abandoned ? '#fef2f2' : '#f0fdf4',
                      borderBottom: '1px solid #f1f5f9',
                      textDecoration: e.abandoned ? 'line-through' : 'none',
                    }"
                  >
                    <td class="text-base text-center" style="padding: 11px 14px; color: #475569;">{{ e.ranking ?? '—' }}</td>
                    <td class="text-base" style="padding: 11px 14px; color: #1e293b;">{{ e.name }}</td>
                    <td class="text-base font-mono" style="padding: 11px 14px; color: #475569;">{{ e.student_code }}</td>
                    <td class="text-base text-center" style="padding: 11px 14px; color: #475569;">{{ e.is_enrolled ? '재학' : '졸업' }}</td>
                    <td class="text-base text-center font-medium" style="padding: 11px 14px;">
                      <span v-if="e.abandoned" style="color: #ef4444; text-decoration: none;">포기</span>
                      <span v-else style="color: #16a34a;">확정</span>
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>
          </div>
        </template>
      </div>

      <div class="flex justify-end flex-shrink-0" style="padding: 14px 22px; border-top: 1px solid #f1f5f9;">
        <button
          class="text-base rounded-lg"
          style="padding: 9px 20px; border: 1px solid #e2e8f0; background: white; color: #64748b; cursor: pointer;"
          @click="modal.open = false">닫기</button>
      </div>
    </div>
  </div>

  <!-- ── 설정 가져오기 미리보기(diff) 모달 ──────────────────────── -->
  <div v-if="settings.open"
    class="fixed inset-0 flex items-center justify-center z-50"
    style="background: rgba(0,0,0,0.4);"
    @keydown.escape.window="closeSettings"
  >
    <div class="bg-white flex flex-col"
      style="border-radius: 14px; box-shadow: 0 8px 32px rgba(0,0,0,0.2); width: 85vw; height: 85vh;">
      <!-- 헤더 -->
      <div class="flex items-center justify-between flex-shrink-0" style="padding: 18px 24px; border-bottom: 1px solid #f1f5f9;">
        <div>
          <h3 class="text-lg font-semibold" style="color: #1e293b; margin: 0;">대학 설정 가져오기 — 변경 내용 확인</h3>
          <p class="text-base" style="color: #94a3b8; margin: 3px 0 0;">
            파일: {{ settings.fileName }}
            <span v-if="!settings.loading">
              &nbsp;·&nbsp;변경 {{ settings.changes.length }}건, 유지 {{ settings.unchangedCount }}건
            </span>
          </p>
        </div>
        <button class="text-xl leading-none"
          style="background: none; border: none; cursor: pointer; color: #94a3b8;"
          @click="closeSettings">✕</button>
      </div>

      <!-- 본문 -->
      <div class="overflow-y-auto flex-1" style="padding: 20px 24px;">
        <div v-if="settings.loading" class="text-base text-center" style="padding: 60px 0; color: #94a3b8;">
          파일을 분석하는 중…
        </div>

        <template v-else>
          <!-- 오류 -->
          <div v-if="settings.errors.length" class="rounded-xl mb-4"
            style="padding: 16px 20px; border: 1px solid #fca5a5; background: #fef2f2;">
            <p class="text-base font-semibold mb-2" style="color: #991b1b;">
              오류 {{ settings.errors.length }}건 — 이대로는 적용할 수 없습니다. 파일을 고쳐 다시 가져오세요.
            </p>
            <ul class="list-disc list-inside space-y-1">
              <li v-for="(e, i) in settings.errors" :key="i" class="text-base" style="color: #991b1b;">{{ e }}</li>
            </ul>
          </div>

          <!-- 마감 라운드 차단 경고 -->
          <div v-else-if="settings.hasBlocked" class="rounded-xl mb-4"
            style="padding: 16px 20px; border: 1px solid #fca5a5; background: #fef2f2;">
            <p class="text-base font-semibold" style="color: #991b1b;">
              마감된 라운드({{ settings.closedLabels.join(', ') }})가 있어 아래 붉게 표시된 재학생 우선 변경을 적용할 수 없습니다.
            </p>
            <p class="text-base" style="color: #991b1b; margin: 4px 0 0;">
              정원만 바꾸려면 파일에서 재학생 우선 값을 원래대로 되돌린 뒤 다시 가져오세요. 재학생 우선을 바꾸려면 라운드를 다시 열고 설정을 고친 뒤 다시 마감하세요.
            </p>
          </div>

          <!-- 상시 안내 -->
          <p v-if="!settings.errors.length" class="text-base mb-4"
            style="padding: 12px 16px; border-radius: 10px; background: #fffbeb; border: 1px solid #fde68a; color: #92400e;">
            이 파일에 없는 대학·모집단위는 그대로 유지됩니다(삭제되지 않습니다). 아래 변경 내용을 확인한 뒤 "확인하고 적용"을 누르세요.
          </p>

          <!-- 변경 없음 -->
          <div v-if="!settings.errors.length && settings.changes.length === 0"
            class="text-base text-center" style="padding: 48px 0; color: #94a3b8;">
            현재 설정과 달라지는 항목이 없습니다.
          </div>

          <!-- 대학별 diff 그룹 -->
          <div v-for="grp in settingsGrouped" :key="grp.univ_name" class="mb-5">
            <h4 class="text-base font-semibold mb-2" style="color: #1e293b;">{{ grp.univ_name }}</h4>
            <div class="rounded-xl overflow-hidden" style="border: 1px solid #e2e8f0;">
              <table class="w-full" style="border-collapse: collapse; table-layout: fixed;">
                <!-- 대학마다 표가 달라도 열 위치가 어긋나지 않도록 너비를 고정한다 -->
                <colgroup>
                  <col style="width: 84px;" />
                  <col style="width: 280px;" />
                  <col />
                </colgroup>
                <tbody>
                  <tr v-for="(c, i) in grp.rows" :key="i"
                    :style="{
                      borderBottom: '1px solid #f1f5f9',
                      background: c.blocked ? '#fef2f2' : (c.op === 'create' ? '#f0fdf4' : '#fffbeb'),
                    }">
                    <td class="text-base" style="padding: 10px 16px; vertical-align: top;">
                      <span class="text-base font-semibold" :style="{ color: badgeColor(c) }">{{ badgeLabel(c) }}</span>
                    </td>
                    <td class="text-base" style="padding: 10px 16px; vertical-align: top; color: #1e293b; overflow-wrap: anywhere;">
                      <span v-if="c.track_name">모집단위 · {{ c.track_name }}</span>
                      <span v-else>대학 설정</span>
                    </td>
                    <td class="text-base" style="padding: 10px 16px; vertical-align: top; color: #475569; overflow-wrap: anywhere;">
                      <div v-for="(f, j) in c.fields" :key="j">
                        {{ f.field }}: <span style="color: #94a3b8;">{{ f.old }}</span>
                        <span style="color: #94a3b8;"> → </span>
                        <span class="font-medium" style="color: #1e293b;">{{ f.new }}</span>
                      </div>
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>
          </div>
        </template>
      </div>

      <!-- 푸터 -->
      <div class="flex items-center justify-between flex-shrink-0" style="padding: 14px 24px; border-top: 1px solid #f1f5f9;">
        <p v-if="settings.applyError" class="text-base" style="color: #ef4444; margin: 0;">{{ settings.applyError }}</p>
        <span v-else></span>
        <div class="flex gap-2">
          <button class="text-base rounded-lg"
            style="padding: 9px 20px; border: 1px solid #e2e8f0; background: white; color: #64748b; cursor: pointer;"
            :disabled="settings.applying"
            @click="closeSettings">취소</button>
          <button class="text-base font-semibold rounded-lg disabled:opacity-40"
            style="padding: 9px 20px; border: none; background: #2563eb; color: white; cursor: pointer;"
            :disabled="!canApplySettings"
            @click="applySettings"
          >{{ settings.applying ? '적용 중…' : '확인하고 적용' }}</button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, defineComponent, h } from 'vue'
import {
  getUniversities, createUniversity, updateUniversity, deleteUniversity,
  getUnivTracks, createTrack, updateTrack, deleteTrack,
  getQuotaStats, exportQuotaStats, getTrackRecommendedList,
  downloadUnivSettingsTemplate, exportUnivSettings, previewUnivSettings, importUnivSettings,
  blobErrMsg,
} from '../../api/admin.js'
import HelpBox from '../common/HelpBox.vue'
import { dialog } from '../common/dialog.js'

const HELP = {
  title: '도움말 — 대학 설정',
  intro: '학생이 지원할 대학과 모집단위(예: 인문계열, 자연계열)를 등록하고, 학교장추천 가능 인원(정원)을 정하는 곳입니다.',
  items: [
    '"+ 대학 추가"로 대학을 만들고, 그 대학을 클릭한 뒤 "+ 모집단위 추가"로 모집단위를 등록하세요.',
    '정원 설정: 대학 전체 인원만 제한하는 대학이면 대학 정원만 입력하고 모집단위는 무제한으로 두세요. 모집단위별 인원 제한이 있으면 모집단위 정원을 입력하세요.',
    '"재학생 우선"을 켜면 추천 순위에서 재학생이 졸업생보다 항상 앞섭니다. 대학의 "재학생 우선"을 켜면 그 대학의 모든 모집단위도 함께 켜지고, 다시 끄면 모집단위도 함께 꺼집니다.',
    '대학은 끄고 특정 모집단위만 "재학생 우선"으로 둘 수 있습니다. 이 경우 그 모집단위 안에서만 재학생이 먼저 추천되며, 대학 전체 정원으로 인원을 줄일 때도 그 모집단위 내부 순서는 유지됩니다.',
    '라운드가 마감된 동안에는 "재학생 우선"을 바꿀 수 없습니다. 저장된 순위가 마감 시점 기준이라 설정만 바꾸면 순위와 어긋나기 때문입니다. 바꾸려면 라운드를 다시 열고 설정을 고친 뒤 다시 마감하세요. 정원 변경은 마감 중에도 가능합니다.',
    '표의 추천인원 숫자를 누르면 지금까지 그 모집단위로 추천 확정된 학생 목록을 볼 수 있습니다.',
    '"전형요소 - 기초 데이터"에서 "석차연명부" 버튼을 누르면 자동으로 대학별 모집단위도 추가됩니다.',
  ],
}

// ── 정원 입력 서브컴포넌트 ────────────────────────────────────
const QuotaInput = defineComponent({
  props: {
    unlimited: Boolean,
    quota: { type: Number, default: 1 },
  },
  emits: ['update:unlimited', 'update:quota'],
  setup(props, { emit }) {
    return () => h('div', { class: 'flex items-center gap-2' }, [
      h('input', {
        type: 'checkbox',
        checked: props.unlimited,
        class: 'accent-blue-600 w-4 h-4',
        onChange: (e) => emit('update:unlimited', e.target.checked),
      }),
      h('span', { style: 'font-size: 16px; color: #475569; user-select: none;' }, '무제한'),
      !props.unlimited
        ? h('div', { class: 'flex items-center gap-1' }, [
            h('input', {
              type: 'number',
              value: props.quota,
              min: 1,
              style: 'width: 72px; border: 1px solid #e2e8f0; border-radius: 6px; padding: 7px 10px; font-size: 16px;',
              onInput: (e) => emit('update:quota', parseInt(e.target.value) || 1),
            }),
            h('span', { style: 'font-size: 16px; color: #64748b;' }, '명'),
          ])
        : null,
    ])
  },
})

// ── 상태 ──────────────────────────────────────────────────────
const univs        = ref([])
const tracks       = ref([])
const error        = ref('')
const saving       = ref(false)
const downloading  = ref(false)

const selectedUnivId  = ref(null)
const addingUniv      = ref(false)
const editingUnivId   = ref(null)
const addingTrack     = ref(false)
const editingTrackId  = ref(null)

const univForm  = ref(emptyUnivForm())
const trackForm = ref(emptyTrackForm())

const quotaStats = ref(null)

// ── 모달 상태 ─────────────────────────────────────────────────
const modal = ref({ open: false, trackName: '', entries: [], loading: false })

// ── 설정 가져오기(diff) 모달 상태 ─────────────────────────────
const settings = ref({
  open: false, loading: false, applying: false,
  fileName: '', file: null,
  errors: [], changes: [], unchangedCount: 0, closedLabels: [], hasBlocked: false,
  applyError: '',
})
const settingsBusy = computed(() => settings.value.loading || settings.value.applying)
const canApplySettings = computed(() =>
  !settings.value.loading && !settings.value.applying &&
  settings.value.errors.length === 0 && !settings.value.hasBlocked &&
  settings.value.changes.length > 0)

const selectedUniv = computed(() => univs.value.find(u => u.id === selectedUnivId.value) ?? null)
const univPrioritize = computed(() => !!(selectedUniv.value?.prioritize_enrolled))

const selectedUnivStats = computed(() => {
  if (!quotaStats.value || !selectedUnivId.value) return null
  return quotaStats.value.univs.find(u => u.univ_id === selectedUnivId.value) ?? null
})

// 기존 tracks 목록에 통계(unit_used, by_round) 병합
const tracksWithStats = computed(() => {
  const statMap = {}
  if (selectedUnivStats.value) {
    for (const t of selectedUnivStats.value.tracks) {
      statMap[t.track_id] = t
    }
  }
  return tracks.value.map(t => ({
    ...t,
    unit_used: statMap[t.id]?.unit_used ?? 0,
    by_round:  statMap[t.id]?.by_round  ?? [],
  }))
})

// 모달에서 라운드별 그룹핑
const groupedByRound = computed(() => {
  const map = new Map()
  for (const e of modal.value.entries) {
    if (!map.has(e.round_id)) map.set(e.round_id, [])
    map.get(e.round_id).push(e)
  }
  return Array.from(map.entries())
    .sort(([a], [b]) => a - b)
    .map(([round_id, entries]) => ({ round_id, entries }))
})

function remainingLabel(used, quota) {
  if (quota == null) return '무제한'
  return Math.max(0, quota - used) + '명'
}

// ── 설정 diff: 대학별 그룹핑 + 배지 ───────────────────────────
const settingsGrouped = computed(() => {
  const map = new Map()
  for (const c of settings.value.changes) {
    if (!map.has(c.univ_name)) map.set(c.univ_name, [])
    map.get(c.univ_name).push(c)
  }
  return Array.from(map.entries()).map(([univ_name, rows]) => ({ univ_name, rows }))
})
function badgeLabel(c) {
  if (c.kind === 'cascade') return '자동'
  return c.op === 'create' ? '생성' : '변경'
}
function badgeColor(c) {
  if (c.blocked) return '#ef4444'
  if (c.kind === 'cascade') return '#7c3aed'
  return c.op === 'create' ? '#16a34a' : '#d97706'
}

// ── 설정 양식/내보내기/가져오기 ───────────────────────────────
function saveSettingsBlob(response, fallback) {
  const url = URL.createObjectURL(new Blob([response.data]))
  const a = document.createElement('a')
  a.href = url; a.download = fallback; a.click()
  URL.revokeObjectURL(url)
}
async function dlSettingsTemplate() {
  try { saveSettingsBlob(await downloadUnivSettingsTemplate(), 'univ_settings_template.xlsx') }
  catch (e) { error.value = await blobErrMsg(e) }
}
async function dlSettingsExport() {
  try { saveSettingsBlob(await exportUnivSettings(), 'univ_settings.xlsx') }
  catch (e) { error.value = await blobErrMsg(e) }
}

async function onSettingsFile(evt) {
  const file = evt.target.files?.[0]
  evt.target.value = '' // 같은 파일 재선택 허용
  if (!file) return
  error.value = ''
  settings.value = {
    open: true, loading: true, applying: false,
    fileName: file.name, file,
    errors: [], changes: [], unchangedCount: 0, closedLabels: [], hasBlocked: false,
    applyError: '',
  }
  try {
    const p = await previewUnivSettings(file)
    settings.value.errors = p.errors ?? []
    settings.value.changes = p.changes ?? []
    settings.value.unchangedCount = p.unchanged_count ?? 0
    settings.value.closedLabels = p.closed_round_labels ?? []
    settings.value.hasBlocked = !!p.has_blocked
  } catch (e) {
    settings.value.errors = [e.response?.data ?? e.message ?? '미리보기에 실패했습니다']
  } finally {
    settings.value.loading = false
  }
}

function closeSettings() {
  if (settings.value.applying) return
  settings.value.open = false
  settings.value.file = null
}

async function applySettings() {
  if (!canApplySettings.value || !settings.value.file) return
  settings.value.applying = true
  settings.value.applyError = ''
  try {
    await importUnivSettings(settings.value.file)
    settings.value.open = false
    settings.value.file = null
    await Promise.all([loadUnivs(), loadQuotaStats()])
    if (selectedUnivId.value) await loadTracks(selectedUnivId.value)
  } catch (e) {
    // import 가 preview 이후 상태 변화로 거부되면(422/409) 오류 목록을 그대로 보여준다
    const d = e.response?.data
    if (d && typeof d === 'object' && Array.isArray(d.errors) && d.errors.length) {
      settings.value.applyError = d.errors.join(' / ')
    } else {
      settings.value.applyError = typeof d === 'string' ? d : (e.message ?? '적용에 실패했습니다')
    }
  } finally {
    settings.value.applying = false
  }
}

// ── 통계 로드 ─────────────────────────────────────────────────
async function loadQuotaStats() {
  try {
    quotaStats.value = await getQuotaStats()
  } catch (_) {
    // 통계 로드 실패는 조용히 무시 (탭 자체를 막지 않음)
  }
}

// ── 내보내기 ──────────────────────────────────────────────────
async function doExportQuotaStats(all = false) {
  if (downloading.value) return
  downloading.value = true
  try {
    const res = await exportQuotaStats(all ? null : selectedUnivId.value)
    const url = URL.createObjectURL(res.data)
    const a = document.createElement('a')
    a.href = url
    const date = new Date().toISOString().slice(0, 10).replace(/-/g, '')
    a.download = all
      ? `전체_명단_정원현황_${date}.xlsx`
      : `${selectedUniv.value?.univ_name ?? '대학'}_명단_정원현황_${date}.xlsx`
    a.click()
    URL.revokeObjectURL(url)
  } catch (e) {
    // blob 요청 오류 — response.data가 Blob이므로 blobErrMsg로 문자열화
    error.value = await blobErrMsg(e)
  } finally {
    downloading.value = false
  }
}

// ── 추천 확정 목록 모달 ───────────────────────────────────────
async function openRecommendedModal(track) {
  modal.value = { open: true, trackName: track.track_name, entries: [], loading: true }
  try {
    modal.value.entries = await getTrackRecommendedList(track.id)
  } catch (e) {
    error.value = e.response?.data ?? e.message
    modal.value.open = false
  } finally {
    modal.value.loading = false
  }
}

// ── 폼 초기값 ─────────────────────────────────────────────────
function emptyUnivForm() {
  return { univ_name: '', unlimited: true, total_quota: 1, prioritize_enrolled: true }
}
function emptyTrackForm() {
  return { track_name: '', unlimited: true, unit_quota: 1, prioritize_enrolled: false }
}
function univToForm(u) {
  return { univ_name: u.univ_name, unlimited: u.total_quota == null, total_quota: u.total_quota ?? 1, prioritize_enrolled: !!u.prioritize_enrolled }
}
function trackToForm(t) {
  return { track_name: t.track_name, unlimited: t.unit_quota == null, unit_quota: t.unit_quota ?? 1, prioritize_enrolled: !!t.prioritize_enrolled }
}
function univFormToBody(f) {
  return { univ_name: f.univ_name.trim(), total_quota: f.unlimited ? null : f.total_quota, prioritize_enrolled: f.prioritize_enrolled }
}
function trackFormToBody(f) {
  return { track_name: f.track_name.trim(), unit_quota: f.unlimited ? null : f.unit_quota, prioritize_enrolled: f.prioritize_enrolled }
}

// ── 로드 ──────────────────────────────────────────────────────
async function loadUnivs() {
  try { univs.value = await getUniversities() }
  catch (e) { error.value = e.response?.data ?? e.message }
}
async function loadTracks(univId) {
  try { tracks.value = await getUnivTracks(univId) }
  catch (e) { error.value = e.response?.data ?? e.message }
}

// ── 대학 선택 ─────────────────────────────────────────────────
function selectUniv(id) {
  if (selectedUnivId.value === id) return
  selectedUnivId.value = id
  editingTrackId.value = null
  addingTrack.value    = false
  tracks.value         = []
  loadTracks(id)
}

// ── 대학 CRUD ─────────────────────────────────────────────────
function startAddUniv() { univForm.value = emptyUnivForm(); editingUnivId.value = null; addingUniv.value = true }

async function saveAddUniv() {
  saving.value = true; error.value = ''
  try { await createUniversity(univFormToBody(univForm.value)); addingUniv.value = false; await loadUnivs() }
  catch (e) { error.value = e.response?.data ?? e.message }
  finally { saving.value = false }
}

function startEditUniv(u) { addingUniv.value = false; editingUnivId.value = u.id; univForm.value = univToForm(u) }

async function saveEditUniv(id) {
  const body = univFormToBody(univForm.value)
  const current = univs.value.find(u => u.id === id)
  // 대학 재학생 우선 값이 바뀌면 그 대학 모든 모집단위에 cascade 된다(양방향).
  // 실제로 값이 달라지는 모집단위가 있을 때만 확인 — 무변경엔 유령 확인을 띄우지 않는다.
  if (current && !!current.prioritize_enrolled !== body.prioritize_enrolled) {
    const uTracks = selectedUnivId.value === id ? tracks.value : await getUnivTracks(id)
    const changing = uTracks.filter(t => !!t.prioritize_enrolled !== body.prioritize_enrolled).length
    if (changing > 0) {
      const ok = await dialog.confirm({
        title: '재학생 우선 설정 변경',
        message: `이 대학의 모든 모집단위 재학생 우선 설정이 대학 설정값으로 통일됩니다.\n개별 지정한 모집단위 설정 ${changing}곳이 사라집니다.`,
        confirmText: '변경',
        level: 'warn',
      })
      if (!ok) return
    }
  }
  saving.value = true; error.value = ''
  try { await updateUniversity(id, body); editingUnivId.value = null; await loadUnivs() }
  catch (e) { error.value = e.response?.data ?? e.message }
  finally { saving.value = false }
}

async function removeUniv(id) {
  if (!(await dialog.confirm({
    title: '대학 삭제',
    message: '이 대학과 모든 모집단위를 삭제하시겠습니까?',
    confirmText: '삭제',
    level: 'danger',
    dangerNotice: '삭제된 대학·모집단위 정보는 복구할 수 없습니다. 이 대학과 연관된 기초 데이터(석차연명부 포함)도 함께 삭제됩니다.',
    finalConfirmText: '영구 삭제',
  }))) return
  saving.value = true; error.value = ''
  try {
    await deleteUniversity(id)
    if (selectedUnivId.value === id) { selectedUnivId.value = null; tracks.value = [] }
    await loadUnivs()
  } catch (e) { error.value = e.response?.data ?? e.message }
  finally { saving.value = false }
}

// ── 모집단위 CRUD ─────────────────────────────────────────────
function startAddTrack() {
  trackForm.value = emptyTrackForm()
  if (univPrioritize.value) trackForm.value.prioritize_enrolled = true
  editingTrackId.value = null
  addingTrack.value = true
}

async function saveAddTrack() {
  if (!selectedUnivId.value) return
  saving.value = true; error.value = ''
  try { await createTrack(selectedUnivId.value, trackFormToBody(trackForm.value)); addingTrack.value = false; await loadTracks(selectedUnivId.value) }
  catch (e) { error.value = e.response?.data ?? e.message }
  finally { saving.value = false }
}

function startEditTrack(t) {
  addingTrack.value = false
  editingTrackId.value = t.id
  trackForm.value = trackToForm(t)
  if (univPrioritize.value) trackForm.value.prioritize_enrolled = true
}

async function saveEditTrack(id) {
  saving.value = true; error.value = ''
  try { await updateTrack(id, trackFormToBody(trackForm.value)); editingTrackId.value = null; await loadTracks(selectedUnivId.value) }
  catch (e) { error.value = e.response?.data ?? e.message }
  finally { saving.value = false }
}

async function removeTrack(id) {
  if (!(await dialog.confirm({
    title: '모집단위 삭제',
    message: '이 모집단위를 삭제하시겠습니까?',
    confirmText: '삭제',
    level: 'danger',
    dangerNotice: '삭제된 모집단위 정보는 복구할 수 없습니다.',
    finalConfirmText: '영구 삭제',
  }))) return
  saving.value = true; error.value = ''
  try { await deleteTrack(id); await loadTracks(selectedUnivId.value) }
  catch (e) { error.value = e.response?.data ?? e.message }
  finally { saving.value = false }
}

onMounted(() => { loadUnivs(); loadQuotaStats() })
</script>
