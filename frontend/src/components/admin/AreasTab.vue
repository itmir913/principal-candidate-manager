<template>
  <div class="py-8 px-4 sm:px-10">

    <!-- 페이지 헤더 -->
    <div class="mb-5">
      <p class="text-base mb-1" style="color: #94a3b8;">관리자</p>
      <h1 class="text-2xl font-semibold" style="color: #1e293b; margin: 0;">전형요소 설정</h1>
    </div>

    <HelpBox class="mb-5" storage-key="areas-main" :title="HELP_MAIN.title" :intro="HELP_MAIN.intro" :items="HELP_MAIN.items" />

    <HelpBox
      v-if="hasLockedRound"
      class="mb-5"
      storage-key="areas-locked"
      variant="warning"
      :title="HELP_LOCKED.title"
      :intro="HELP_LOCKED.intro"
      :items="HELP_LOCKED.items"
    />

    <div class="flex flex-col lg:flex-row lg:items-start gap-6">

      <!-- ── 좌측: 전형요소 목록 ────────────────────────────────── -->
      <div class="flex flex-col w-full lg:w-[300px]">
        <div class="flex items-center justify-between mb-4">
          <h2 class="text-lg font-semibold" style="color: #1e293b;">전형요소 목록</h2>
          <button
            class="text-base font-medium rounded-lg"
            style="padding: 7px 14px; border: none; background: #2563eb; color: white; cursor: pointer;"
            @click="openAddForm">+ 전형요소 추가</button>
        </div>

        <p v-if="error" class="text-base mb-4" style="color: #ef4444;">{{ error }}</p>

        <!-- 전형요소 카드 목록 -->
        <div class="flex flex-col gap-2">
          <div
            v-for="area in areas" :key="area.id"
            class="rounded-xl transition-all"
            :style="{
              background: 'white',
              border: editingAreaId === area.id ? '1px solid #fbbf24' : (selected?.id === area.id ? '1px solid #93c5fd' : '1px solid #e2e8f0'),
              boxShadow: '0 1px 4px rgba(0,0,0,0.07)',
            }"
          >
            <!-- 보기 모드 -->
            <template v-if="editingAreaId !== area.id">
              <div class="cursor-pointer" style="padding: 14px 16px;" @click="selectArea(area)">
                <p class="text-lg font-semibold" style="color: #1e293b; margin: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">{{ area.name }}</p>
                <p class="text-base" style="margin: 4px 0 0; color: #64748b;">
                  {{ calcTypeLabel(area.calc_type) }} · {{ lookupScopeLabel(area.lookup_scope) }}
                </p>
                <div class="flex gap-3">
                  <button class="text-base font-medium"
                          style="color: #2563eb; background: none; border: none; cursor: pointer; padding: 0;"
                          @click.stop="startEditArea(area)">편집</button>
                  <button class="text-base font-medium"
                          style="color: #ef4444; background: none; border: none; cursor: pointer; padding: 0;"
                          @click.stop="removeArea(area.id)">삭제</button>
                </div>
              </div>
            </template>
            <!-- 편집 모드 -->
            <template v-else>
              <div style="padding: 14px 16px; background: #fefce8; border-radius: 10px;">
                <div class="space-y-3">
                  <div>
                    <label class="block text-base font-medium mb-1.5" style="color: #64748b;">전형요소 이름</label>
                    <input v-model="editArea.name" type="text"
                      class="w-full text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
                      style="border: 1px solid #e2e8f0; border-radius: 8px; padding: 9px 12px; box-sizing: border-box;" />
                  </div>
                  <!-- MANUAL은 단위가 항상 '점'(프로그램 강제)이므로 NUMERIC만 단위 입력 -->
                  <div v-if="area.calc_type === 'NUMERIC'">
                    <label class="block text-base font-medium mb-1.5" style="color: #64748b;">기준값 단위 <span style="color: #94a3b8;">(선택)</span></label>
                    <input v-model="editArea.unit" type="text" placeholder="예: 시간, 등급 (선택)"
                      class="w-full text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
                      style="border: 1px solid #e2e8f0; border-radius: 8px; padding: 9px 12px; box-sizing: border-box;" />
                  </div>
                  <div class="flex items-center gap-2">
                    <input v-model="editArea.teacher_editable" type="checkbox"
                      :id="`edit-te-${area.id}`" class="accent-blue-600 w-4 h-4" />
                    <label :for="`edit-te-${area.id}`" class="text-base" style="color: #475569;">담임교사 입력 허용</label>
                  </div>
                  <p v-if="editError" class="text-base" style="color: #ef4444;">{{ editError }}</p>
                </div>
                <div class="flex gap-2 mt-4">
                  <button
                    class="text-base font-semibold rounded-lg"
                    style="padding: 8px 18px; border: none; background: #2563eb; color: white; cursor: pointer;"
                    @click="saveEdit">저장</button>
                  <button
                    class="text-base rounded-lg"
                    style="padding: 8px 18px; border: 1px solid #e2e8f0; background: white; color: #64748b; cursor: pointer;"
                    @click="cancelEdit">취소</button>
                </div>
              </div>
            </template>
          </div>
          <div v-if="areas.length === 0" class="text-base text-center" style="padding: 32px 0; color: #94a3b8;">
            등록된 전형요소 없음
          </div>
        </div>

        <!-- 총점 -->
        <div v-if="areas.length > 0"
          class="flex items-center justify-end gap-2 mt-3 text-base font-semibold"
          style="border-top: 1px solid #e2e8f0; color: #1e293b;">
          <span>총점</span>
          <span>{{ displayScore(totalMaxScore) }}점</span>
        </div>
      </div>

      <!-- ── 우측 패널: 추가 폼 or 전형요소 상세 ────────────────── -->
      <div class="flex-1 min-w-0">

        <!-- 전형요소 추가 폼 -->
        <div v-if="showAddForm" class="rounded-xl @container"
             style="padding: 20px; background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04);">
          <h3 class="text-base font-semibold mb-4" style="color: #1e293b;">새 전형요소 추가</h3>

          <!-- ── 1. 기본 템플릿 (맨 위) ───────────────────────── -->
          <p class="text-base font-semibold mb-1" style="color: #1e293b;">템플릿으로 빠르게 시작</p>
          <p class="text-base mb-3" style="color: #94a3b8;">
            템플릿을 선택하면 아래 항목이 자동으로 채워집니다. 매뉴얼을 참고하여 적절한 전형요소 설정을 입력하세요.
          </p>
          <div class="grid grid-cols-1 @2xl:grid-cols-3 gap-3 mb-6">
            <div
              v-for="tpl in AREA_TEMPLATES" :key="tpl.id"
              class="template-btn rounded-xl text-left flex flex-col"
              style="padding: 14px 16px; background: white; border: 1px solid #e2e8f0; cursor: pointer; box-shadow: 0 1px 3px rgba(0,0,0,0.06);"
              @click="applyTemplate(tpl)">
              <p class="text-base font-semibold" style="color: #1e293b; margin: 0 0 4px;">{{ tpl.name }}</p>
              <p class="break-keep word-break-keep-all text-base" style="color: #64748b; margin: 0 0 8px; line-height: 1.5;">{{ tpl.description }}</p>
              <p class="break-keep word-break-keep-all text-base" style="color: #94a3b8; margin: 0 0 12px;">{{ tpl.hint }}</p>
              <button
                class="text-base mt-auto"
                style="padding: 0; border: none; background: none; color: #2563eb; cursor: pointer; text-align: left; text-decoration: underline; text-underline-offset: 2px;"
                :disabled="dlTemplateId === tpl.id"
                @click.stop="dlScoreTemplate(tpl)">
                {{ dlTemplateId === tpl.id ? '다운로드 중…' : '점수 기준 샘플 ↓' }}
              </button>
            </div>
          </div>

          <!-- ── 구분선 ────────────────────────────────────────── -->
          <hr style="margin: 0 0 20px; border: none; border-top: 1px solid #e2e8f0;" />

          <!-- ── 2. 세부 설정 폼 ──────────────────────────────── -->
          <p class="text-base font-semibold mb-3" style="color: #1e293b;">세부 설정</p>
          <div class="space-y-3">
            <div>
              <label class="block text-base font-medium mb-1.5" style="color: #64748b;">전형요소 이름 <span style="color: #ef4444;">*</span></label>
              <input v-model="newArea.name" type="text"
                     class="w-full text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
                     style="border: 1px solid #e2e8f0; border-radius: 8px; padding: 9px 12px; box-sizing: border-box;" />
            </div>
            <div>
              <label class="block text-base font-medium mb-1.5" style="color: #64748b;">만점(반영 비율) <span style="color: #ef4444;">*</span></label>
              <input v-model="newArea.max_score_display" type="number" step="0.00001"
                     class="w-full text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
                     style="border: 1px solid #e2e8f0; border-radius: 8px; padding: 9px 12px; box-sizing: border-box;" />
            </div>
            <div>
              <label class="block text-base font-medium mb-1.5" style="color: #64748b;">점수 산출 방식 <span style="color: #ef4444;">*</span></label>
              <select v-model="newArea.calc_type"
                      class="w-full text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
                      style="border: 1px solid #e2e8f0; border-radius: 8px; padding: 9px 12px;">
                <option value="NUMERIC">수치형 입력</option>
                <option value="CATEGORY">선택형 입력</option>
                <option value="MANUAL">점수 직접 입력</option>
              </select>
            </div>
            <div>
              <label class="block text-base font-medium mb-1.5" style="color: #64748b;">데이터 조회 기준 <span style="color: #ef4444;">*</span></label>
              <select v-model="newArea.lookup_scope"
                      class="w-full text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
                      style="border: 1px solid #e2e8f0; border-radius: 8px; padding: 9px 12px;">
                <option value="SIMPLE">기본 조회</option>
                <option value="COMPOSITE">대학별 환산점수 조회</option>
              </select>
            </div>
            <div v-if="newArea.calc_type === 'NUMERIC'">
              <label class="block text-base font-medium mb-1.5" style="color: #64748b;">구간 탐색 방향 <span style="color: #ef4444;">*</span></label>
              <select v-model="newArea.match_mode"
                      class="w-full text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
                      style="border: 1px solid #e2e8f0; border-radius: 8px; padding: 9px 12px;">
                <option value="">선택하세요</option>
                <option value="UPPER">▲ 기준값 이상(클수록 만점)</option>
                <option value="LOWER">▼ 기준값 이하(작을수록 만점)</option>
                <option value="EXACT">〓 정확히 일치</option>
              </select>
            </div>
            <div v-if="newArea.calc_type === 'CATEGORY'">
              <label class="block text-base font-medium mb-1.5" style="color: #64748b;">복수 활동 처리 방식 <span style="color: #ef4444;">*</span></label>
              <select v-model="newArea.category_agg"
                      class="w-full text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
                      style="border: 1px solid #e2e8f0; border-radius: 8px; padding: 9px 12px;">
                <option value="">선택하세요</option>
                <option value="SUM">중복 선택 가능 (점수 합산)</option>
                <option value="MAX">최대 1개만 인정 (최고점 반영)</option>
              </select>
            </div>
            <div v-if="newArea.calc_type === 'NUMERIC'">
              <label class="block text-base font-medium mb-1.5" style="color: #64748b;">기준값 단위 <span style="color: #94a3b8;">(선택)</span></label>
              <input v-model="newArea.unit" type="text" placeholder="예: 시간, 등급 (선택)"
                     class="w-full text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
                     style="border: 1px solid #e2e8f0; border-radius: 8px; padding: 9px 12px; box-sizing: border-box;" />
            </div>
            <div class="flex items-center gap-2">
              <input v-model="newArea.teacher_editable" type="checkbox" id="te" class="accent-blue-600 w-4 h-4" />
              <label for="te" class="text-base" style="color: #475569;">담임교사 입력 허용</label>
            </div>
            <p v-if="addError" class="text-base" style="color: #ef4444;">{{ addError }}</p>
            <div class="flex gap-2 pt-1">
              <button
                  class="text-base font-semibold rounded-lg"
                  style="padding: 8px 18px; border: none; background: #2563eb; color: white; cursor: pointer;"
                  @click="addArea">저장</button>
              <button
                  class="text-base rounded-lg"
                  style="padding: 8px 18px; border: 1px solid #e2e8f0; background: white; color: #64748b; cursor: pointer;"
                  @click="showAddForm = false">취소</button>
            </div>
          </div>

          <!-- ── 구분선 + 유형 안내 (참고용) ──────────────────── -->
          <hr style="margin: 24px 0; border: none; border-top: 1px solid #e2e8f0;" />

          <p class="text-base font-semibold mb-3" style="color: #1e293b;">유형 안내</p>

          <!-- 점수 산출 방식 -->
          <p class="text-base font-medium mb-2" style="color: #64748b;">점수 산출 방식</p>
          <div class="grid grid-cols-1 md:grid-cols-3 gap-3 mb-4">
            <div v-for="d in CALC_TYPE_DESCS" :key="d.key"
                 class="rounded-lg text-base"
                 style="padding: 12px 14px; background: #f8fafc; border: 1px solid #e2e8f0;">
              <p class="font-semibold" style="color: #1e293b; margin: 0 0 4px;">{{ d.label }}</p>
              <p style="color: #64748b; margin: 0; line-height: 1.5;">{{ d.desc }}</p>
            </div>
          </div>

          <!-- 데이터 조회 기준 -->
          <p class="text-base font-medium mb-2" style="color: #64748b;">데이터 조회 기준</p>
          <div class="grid grid-cols-1 md:grid-cols-2 gap-3 mb-4">
            <div v-for="d in LOOKUP_SCOPE_DESCS" :key="d.key"
                 class="rounded-lg text-base"
                 style="padding: 12px 14px; background: #f8fafc; border: 1px solid #e2e8f0;">
              <p class="font-semibold" style="color: #1e293b; margin: 0 0 4px;">{{ d.label }}</p>
              <p style="color: #64748b; margin: 0; line-height: 1.5;">{{ d.desc }}</p>
            </div>
          </div>

          <div class="rounded-lg text-base" style="padding: 10px 14px; background: #fffbeb; border: 1px solid #fcd34d; color: #92400e;">
            ⚠ 전형요소 등록 후에는 이름과 담임교사 입력 허용 여부만 변경할 수 있습니다.
          </div>
        </div>

        <!-- ── 전형요소 상세 ──────────────────────────────────── -->
        <div v-else-if="selected">

          <!-- 선택된 전형요소 헤더 -->
          <div class="flex items-center justify-between mb-4 flex-wrap gap-2">
            <div class="flex items-center gap-2 min-w-0 flex-wrap">
              <h3 class="text-lg font-semibold" style="color: #1e293b; margin: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">{{ selected.name }}</h3>
              <span class="text-base font-medium flex-shrink-0"
                style="padding: 3px 12px; background: #f1f5f9; color: #475569; border-radius: 999px;">{{ calcTypeLabel(selected.calc_type) }}</span>
              <span class="text-base font-medium flex-shrink-0"
                style="padding: 3px 12px; background: #f1f5f9; color: #475569; border-radius: 999px;">{{ lookupScopeLabel(selected.lookup_scope) }}</span>
            </div>
          </div>

          <!-- 기본 정보 카드 -->
          <div class="rounded-xl mb-5"
            style="padding: 18px 20px; background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04);">
            <div class="flex flex-wrap gap-x-8 gap-y-2">
              <span class="text-base"><span class="font-medium mr-2" style="color: #94a3b8;">만점</span><span style="color: #1e293b;">{{ displayScore(selected.max_score) }}점</span></span>
              <span class="text-base"><span class="font-medium mr-2" style="color: #94a3b8;">조회 기준</span><span style="color: #1e293b;">{{ lookupScopeLabel(selected.lookup_scope) }}</span></span>
              <span class="text-base"><span class="font-medium mr-2" style="color: #94a3b8;">계산 유형</span><span style="color: #1e293b;">{{ calcTypeLabel(selected.calc_type) }}</span></span>
              <span v-if="selected.calc_type === 'NUMERIC'" class="text-base"><span class="font-medium mr-2" style="color: #94a3b8;">탐색 방향</span><span style="color: #1e293b;">{{ matchModeLabel(selected.match_mode) }}</span></span>
              <span v-if="selected.calc_type === 'CATEGORY'" class="text-base"><span class="font-medium mr-2" style="color: #94a3b8;">범주 집계</span><span style="color: #1e293b;">{{ categoryAggLabel(selected.category_agg) }}</span></span>
              <span class="text-base"><span class="font-medium mr-2" style="color: #94a3b8;">담임교사 입력</span><span style="color: #1e293b;">{{ selected.teacher_editable ? '허용' : '불가' }}</span></span>
            </div>
            <p class="text-base" style="color: #94a3b8; margin: 0; padding-top: 12px; border-top: 1px solid #f1f5f9;">
              전형요소 등록 후에는 이름과 담임교사 입력 허용 여부만 변경할 수 있습니다.
            </p>
          </div>

          <!-- 서브탭 -->
          <div class="flex mb-5" style="border-bottom: 1px solid #e2e8f0;">
            <button v-if="selected.calc_type !== 'MANUAL'"
              class="text-base font-medium transition-colors"
              style="padding: 10px 20px; border: none; background: none; cursor: pointer; border-bottom: 2px solid transparent; margin-bottom: -1px;"
              :style="{
                borderBottomColor: activeTab === 'score' ? '#2563eb' : 'transparent',
                color: activeTab === 'score' ? '#2563eb' : '#64748b',
                fontWeight: activeTab === 'score' ? '600' : '400',
              }"
              @click="activeTab = 'score'">점수 기준</button>
            <button
              class="text-base font-medium transition-colors"
              style="padding: 10px 20px; border: none; background: none; cursor: pointer; border-bottom: 2px solid transparent; margin-bottom: -1px;"
              :style="{
                borderBottomColor: activeTab === 'base' ? '#2563eb' : 'transparent',
                color: activeTab === 'base' ? '#2563eb' : '#64748b',
                fontWeight: activeTab === 'base' ? '600' : '400',
              }"
              @click="activeTab = 'base'">기초 데이터</button>
          </div>

          <!-- ── 점수 기준 탭 ──────────────────────────────── -->
          <div v-if="activeTab === 'score'">
            <HelpBox class="mb-4" storage-key="areas-score" :title="HELP_SCORE.title" :intro="HELP_SCORE.intro" :items="HELP_SCORE.items" />
            <!-- 양식 예시 -->
            <div class="rounded-xl mb-4"
              style="padding: 16px 18px; background: #eff6ff; border: 1px solid #bfdbfe;">
              <p class="text-base font-semibold mb-3" style="color: #1d4ed8;">양식 예시 — {{ scoreEx.desc }}</p>
              <div class="overflow-x-auto">
                <table style="border-collapse: collapse;">
                  <thead>
                    <tr>
                      <th v-for="hd in scoreEx.headers" :key="hd"
                        class="text-base font-semibold text-left whitespace-nowrap"
                        style="padding: 8px 14px; background: #bfdbfe; border: 1px solid #93c5fd; color: #1d4ed8;">{{ hd }}</th>
                      <th style="background: transparent; padding: 8px 14px;"></th>
                    </tr>
                  </thead>
                  <tbody>
                    <tr v-for="(row, i) in scoreEx.rows" :key="i">
                      <td v-for="(cell, j) in row" :key="j"
                        class="text-base whitespace-nowrap"
                        style="padding: 7px 14px; border: 1px solid #bfdbfe; color: #1e293b;">{{ cell }}</td>
                      <td class="text-base whitespace-nowrap" style="padding: 7px 14px 7px 20px; color: #94a3b8;">{{ scoreEx.rowDescs[i] }}</td>
                    </tr>
                  </tbody>
                </table>
              </div>
            </div>
            <p class="text-base mb-2" :class="selected.lookup_scope === 'COMPOSITE' ? 'mb-1' : 'mb-4'" style="color: #64748b;">
              기준값·점수는 실제 값으로 작성 (예: 1.25, 30.5 / 소수점 최대 5자리)
            </p>
            <p v-if="selected.lookup_scope === 'COMPOSITE'" class="text-base mb-4" style="color: #2563eb;">
              대학명·모집단위명을 비워두면 모든 대학에 공통 적용됩니다.
            </p>

            <ExcelPanel
              :area-id="selected.id"
              :calc-type="selected.calc_type"
              :area-name="selected.name"
              panel="score"
              @result="onScoreResult" />
            <ImportResultBox v-if="scoreResult" :result="scoreResult" class="mt-3" />

            <!-- 점수 계산 테스트 카드 -->
            <ScoreDemoCard
              v-if="scorePage.rows.length > 0"
              class="mt-5"
              :area="selected"
              :rows="scorePage.rows"
              @highlight="onDemoHighlight" />

            <!-- 점수 기준 목록 -->
            <div class="mt-5 rounded-xl overflow-hidden"
              style="background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04); max-height: 400px; overflow-x: auto; overflow-y: auto;">
              <p v-if="scorePage.rows.length === 0" class="text-base text-center" style="padding: 32px; color: #94a3b8;">
                등록된 점수 기준 없음
              </p>
              <table v-else class="w-full" style="border-collapse: collapse; table-layout: fixed;" :style="selected.lookup_scope === 'COMPOSITE' ? 'min-width: 660px' : 'min-width: 300px'">
                <colgroup>
                  <template v-if="selected.lookup_scope === 'COMPOSITE'">
                    <col style="width: 140px;">
                    <col style="width: 100px;">
                    <col style="width: 160px;">
                    <col>
                  </template>
                  <template v-else>
                    <col>
                    <col>
                  </template>
                </colgroup>
                <thead style="position: sticky; top: 0; z-index: 1;">
                  <tr style="background: #f8fafc; border-bottom: 1px solid #e2e8f0;">
                    <th class="text-base font-semibold text-left" style="padding: 13px 18px; color: #475569;">
                      <template v-if="selected.calc_type === 'NUMERIC'">기준값<span v-if="selected.unit"> ({{ selected.unit }})</span></template>
                      <template v-else>범주</template>
                    </th>
                    <th class="text-base font-semibold text-left" style="padding: 13px 18px; color: #475569;">점수 (점)</th>
                    <template v-if="selected.lookup_scope === 'COMPOSITE'">
                      <th class="text-base font-semibold text-left" style="padding: 13px 18px; color: #475569;">대학명</th>
                      <th class="text-base font-semibold text-left" style="padding: 13px 18px; color: #475569;">모집단위명</th>
                    </template>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="(row, i) in scorePage.rows" :key="i"
                    :style="{
                      background: isDemoHighlighted(row) ? '#fefce8' : (i % 2 === 1 ? '#f8fafc' : 'white'),
                      borderBottom: '1px solid #f1f5f9',
                      fontWeight: isDemoHighlighted(row) ? '600' : '400',
                    }">
                    <td class="text-base" style="padding: 11px 18px; color: #1e293b;">
                      {{ selected.calc_type === 'NUMERIC' ? row.threshold : row.category }}
                    </td>
                    <td class="text-base" style="padding: 11px 18px; color: #1e293b;">{{ row.score }}</td>
                    <template v-if="selected.lookup_scope === 'COMPOSITE'">
                      <td class="text-base" style="padding: 11px 18px; color: #1e293b;">{{ row.univ_name }}</td>
                      <td class="text-base" style="padding: 11px 18px; color: #1e293b;">{{ row.track_name }}</td>
                    </template>
                  </tr>
                </tbody>
              </table>
            </div>
            <div v-if="scorePage.total > 0" class="mt-4 flex items-center justify-center gap-4">
              <button
                class="text-base rounded-lg disabled:opacity-40 disabled:cursor-not-allowed"
                style="padding: 8px 18px; border: 1px solid #e2e8f0; background: white; color: #475569; cursor: pointer;"
                :disabled="scorePage.page <= 1"
                @click="loadScoreRows(scorePage.page - 1)">&lt; 이전</button>
              <span class="text-base" style="color: #64748b;">
                {{ scorePage.page }} / {{ Math.ceil(scorePage.total / scorePage.per_page) }} 페이지
                (총 {{ scorePage.total }}행)
              </span>
              <button
                class="text-base rounded-lg disabled:opacity-40 disabled:cursor-not-allowed"
                style="padding: 8px 18px; border: 1px solid #e2e8f0; background: white; color: #475569; cursor: pointer;"
                :disabled="scorePage.page >= Math.ceil(scorePage.total / scorePage.per_page)"
                @click="loadScoreRows(scorePage.page + 1)">다음 &gt;</button>
            </div>
          </div>

          <!-- ── 기초 데이터 탭 ──────────────────────────────── -->
          <div v-if="activeTab === 'base'">
            <HelpBox class="mb-4" storage-key="areas-base" :title="HELP_BASE.title" :intro="HELP_BASE.intro" :items="HELP_BASE.items" />
            <!-- 양식 예시 -->
            <div class="rounded-xl mb-4"
              style="padding: 16px 18px; background: #eff6ff; border: 1px solid #bfdbfe;">
              <p class="text-base font-semibold mb-3" style="color: #1d4ed8;">양식 예시 — {{ baseEx.desc }}</p>
              <div class="overflow-x-auto">
                <table style="border-collapse: collapse;">
                  <thead>
                    <tr>
                      <th v-for="hd in baseEx.headers" :key="hd"
                        class="text-base font-semibold text-left whitespace-nowrap"
                        style="padding: 8px 14px; background: #bfdbfe; border: 1px solid #93c5fd; color: #1d4ed8;">{{ hd }}</th>
                    </tr>
                  </thead>
                  <tbody>
                    <tr v-for="(row, i) in baseEx.rows" :key="i">
                      <td v-for="(cell, j) in row" :key="j"
                        class="text-base whitespace-nowrap"
                        style="padding: 7px 14px; border: 1px solid #bfdbfe; color: #1e293b;">{{ cell }}</td>
                    </tr>
                  </tbody>
                </table>
              </div>
            </div>
            <p class="text-base mb-4" style="color: #64748b;">
              <template v-if="baseStudentType === 'enrolled'">학년·반·번호로 재학생을 찾아 값을 등록합니다.</template>
              <template v-else>학생코드로 졸업생을 찾아 값을 등록합니다.</template>
              소수점 최대 5자리까지 지원합니다.
            </p>

            <ExcelPanel
              :area-id="selected.id"
              :calc-type="selected.calc_type"
              :area-name="selected.name"
              panel="base"
              v-model:studentType="baseStudentType"
              @result="onBaseResult" />

            <!-- 외부 프로그램 가져오기 (COMPOSITE 전용, 재학생만) -->
            <div v-if="selected.lookup_scope === 'COMPOSITE'"
                 v-show="baseStudentType === 'enrolled'"
                 class="mt-3 flex flex-wrap gap-2">
              <label class="text-base rounded-lg cursor-pointer"
                style="padding: 9px 16px; border: 1px solid #e2e8f0; background: white; color: #475569;">
                대교협 석차연명부
                <input type="file" accept=".xlsx" class="hidden" @change="onExternalFile('daegyo', $event)" />
              </label>
              <label class="text-base rounded-lg cursor-pointer"
                style="padding: 9px 16px; border: 1px solid #e2e8f0; background: white; color: #475569;">
                유니브 석차연명부
                <input type="file" accept=".xls" class="hidden" @change="onExternalFile('univ', $event)" />
              </label>
            </div>

            <!-- 잘못된 파일을 올렸을 때 되돌리는 방법.
                 석차연명부 가져오기는 파일에 있는 대학·모집단위를 자동 생성하므로 파일을
                 잘못 고르면 엉뚱한 모집단위가 남는다. 올바른 파일을 다시 올려도 그 모집단위는
                 지워지지 않아(다른 track_id로 들어간다) 여기서 정리 경로를 알려 준다. -->
            <div v-if="selected.lookup_scope === 'COMPOSITE'"
                 v-show="baseStudentType === 'enrolled'"
                 class="mt-3 rounded-lg text-base"
                 style="padding: 10px 14px; background: #fffbeb; border: 1px solid #fcd34d; color: #92400e;">
              석차연명부를 가져오면 파일에 들어 있는 <strong>대학·모집단위가 자동으로 생성</strong>됩니다.
              파일을 잘못 골랐다면 올바른 파일을 다시 올리는 것만으로는 지워지지 않으므로,
              <strong>대학 설정</strong> 탭에서 잘못 생긴 모집단위(또는 대학)를 삭제하십시오.
              삭제하면 <strong>거기에 딸린 기초 데이터도 함께 지워집니다.</strong>
              다만 그 모집단위에 <strong>지원 기록이 하나라도 있으면 삭제할 수 없으니
              라운드를 열기 전에 정리</strong>하십시오.
            </div>

            <ImportResultBox v-if="baseResult" :result="baseResult" class="mt-3" />

            <!-- 기초 데이터 목록 -->
            <div class="mt-5 rounded-xl overflow-hidden"
              style="background: white; box-shadow: 0 1px 4px rgba(0,0,0,0.07), 0 0 0 1px rgba(0,0,0,0.04); max-height: 400px; overflow-x: auto; overflow-y: auto;">
              <p v-if="basePage.rows.length === 0" class="text-base text-center" style="padding: 32px; color: #94a3b8;">
                등록된 기초 데이터 없음
              </p>
              <table v-else class="w-full" style="border-collapse: collapse; table-layout: fixed;" :style="selected.lookup_scope === 'COMPOSITE' ? 'min-width: 680px' : 'min-width: 360px'">
                <colgroup>
                  <col style="width: 160px;">
                  <col style="width: 100px;">
                  <template v-if="selected.lookup_scope === 'COMPOSITE'">
                    <col style="width: 100px;">
                    <col style="width: 160px;">
                    <col>
                  </template>
                  <col v-else>
                </colgroup>
                <thead style="position: sticky; top: 0; z-index: 1;">
                  <tr style="background: #f8fafc; border-bottom: 1px solid #e2e8f0;">
                    <th class="text-base font-semibold text-left" style="padding: 13px 18px; color: #475569;">학생코드</th>
                    <th class="text-base font-semibold text-left" style="padding: 13px 18px; color: #475569;">이름</th>
                    <th class="text-base font-semibold text-left" style="padding: 13px 18px; color: #475569;">값</th>
                    <template v-if="selected.lookup_scope === 'COMPOSITE'">
                      <th class="text-base font-semibold text-left" style="padding: 13px 18px; color: #475569;">대학명</th>
                      <th class="text-base font-semibold text-left" style="padding: 13px 18px; color: #475569;">모집단위명</th>
                    </template>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="(row, i) in basePage.rows" :key="i"
                    :style="{ background: i % 2 === 1 ? '#f8fafc' : 'white', borderBottom: '1px solid #f1f5f9' }">
                    <td class="text-base font-mono" style="padding: 11px 18px; color: #475569;">{{ row.student_code }}</td>
                    <td class="text-base" style="padding: 11px 18px; color: #1e293b;">{{ row.name }}</td>
                    <td class="text-base" style="padding: 11px 18px; color: #1e293b;">{{ row.value }}</td>
                    <template v-if="selected.lookup_scope === 'COMPOSITE'">
                      <td class="text-base" style="padding: 11px 18px; color: #1e293b;">{{ row.univ_name }}</td>
                      <td class="text-base" style="padding: 11px 18px; color: #1e293b;">{{ row.track_name }}</td>
                    </template>
                  </tr>
                </tbody>
              </table>
            </div>
            <div v-if="basePage.total > 0" class="mt-4 flex items-center justify-center gap-4">
              <button
                class="text-base rounded-lg disabled:opacity-40 disabled:cursor-not-allowed"
                style="padding: 8px 18px; border: 1px solid #e2e8f0; background: white; color: #475569; cursor: pointer;"
                :disabled="basePage.page <= 1"
                @click="loadBaseRows(basePage.page - 1)">&lt; 이전</button>
              <span class="text-base" style="color: #64748b;">
                {{ basePage.page }} / {{ Math.ceil(basePage.total / basePage.per_page) }} 페이지
                (총 {{ basePage.total }}행)
              </span>
              <button
                class="text-base rounded-lg disabled:opacity-40 disabled:cursor-not-allowed"
                style="padding: 8px 18px; border: 1px solid #e2e8f0; background: white; color: #475569; cursor: pointer;"
                :disabled="basePage.page >= Math.ceil(basePage.total / basePage.per_page)"
                @click="loadBaseRows(basePage.page + 1)">다음 &gt;</button>
            </div>
          </div>
        </div>

        <!-- 비어있는 상태 -->
        <div v-else class="flex items-center justify-center" style="height: 240px;">
          <p class="text-base text-center" style="color: #94a3b8;">
            왼쪽에서 전형요소를 선택하면<br class="hidden md:block" />학교장추천전형의 영역별 반영비율과 만점을 관리할 수 있습니다.
          </p>
        </div>

      </div>
      <!-- ── 우측 패널 끝 ─────────────────────────────────────── -->

    </div>
  </div>

  <!-- ── 외부 가져오기 모달 ────────────────────────────────────── -->
  <Teleport to="body">
    <div v-if="extModal.open"
         class="fixed inset-0 z-50 flex items-center justify-center"
         style="background: rgba(0,0,0,0.35);">
      <div class="bg-white flex flex-col"
        style="border-radius: 14px; box-shadow: 0 8px 32px rgba(0,0,0,0.15); width: 85vw; height: 85vh; max-height: 90vh; overflow-y: auto; padding: 1.75rem;">
        <h3 class="text-lg font-semibold mb-1" style="color: #1e293b;">{{ extModal.title }}</h3>
        <p class="text-base" :class="extModal.headerInfo ? 'mb-2' : 'mb-5'" style="color: #475569;">{{ extModal.fileName }}</p>
        <p v-if="extModal.headerInfo" class="text-base mb-5"
          style="color: #475569; background: #f8fafc; border: 1px solid #e2e8f0; border-radius: 8px; padding: 10px 14px; word-break: break-all;">
          {{ extModal.headerInfo }}
        </p>

        <div class="space-y-4 mb-5">
          <div>
            <label class="block text-base font-medium mb-1.5" style="color: #64748b;">대학명 <span style="color: #ef4444;">*</span></label>
            <input v-model="extModal.univName" type="text"
              class="w-full text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
              style="border: 1px solid #e2e8f0; border-radius: 8px; padding: 10px 14px; box-sizing: border-box;"
              placeholder="예: 서울대학교" />
          </div>
          <div>
            <label class="block text-base font-medium mb-1.5" style="color: #64748b;">모집단위명 <span style="color: #ef4444;">*</span></label>
            <div class="flex gap-2 mb-1.5">
              <button v-for="p in TRACK_PRESETS" :key="p" type="button"
                class="text-base rounded-lg"
                style="padding: 6px 14px; border: 1px solid #e2e8f0; background: #f8fafc; color: #475569; cursor: pointer;"
                @click="extModal.trackName = p">{{ p }}</button>
            </div>
            <input v-model="extModal.trackName" type="text"
              class="w-full text-base focus:outline-none focus:ring-2 focus:ring-blue-400"
              style="border: 1px solid #e2e8f0; border-radius: 8px; padding: 10px 14px; box-sizing: border-box;"
              placeholder="예: 자연계열" />
          </div>
        </div>

        <div class="mb-5">
          <p class="text-base mb-2" style="color: #64748b;">
            미리보기 (상위 {{ extModal.preview.length }}행 / 총 {{ extModal.total }}행)
          </p>
          <div class="rounded-xl overflow-hidden" style="border: 1px solid #e2e8f0;">
            <div class="overflow-x-auto">
              <table class="w-full" style="border-collapse: collapse;">
                <thead>
                  <tr style="background: #f8fafc; border-bottom: 1px solid #e2e8f0;">
                    <th v-for="hd in ['학년','반','번호','이름', extModal.valueHeader]" :key="hd"
                      class="text-base font-semibold text-left whitespace-nowrap"
                      style="padding: 11px 16px; color: #475569;">{{ hd }}</th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="(row, i) in extModal.preview" :key="i"
                    :style="{ background: i % 2 === 1 ? '#f8fafc' : 'white', borderBottom: '1px solid #f1f5f9' }">
                    <td v-for="(cell, j) in row" :key="j"
                      class="text-base whitespace-nowrap"
                      style="padding: 10px 16px; color: #1e293b;">{{ cell }}</td>
                  </tr>
                </tbody>
              </table>
            </div>
          </div>
        </div>

        <p v-if="extModal.error" class="text-base mb-4" style="color: #ef4444;">{{ extModal.error }}</p>

        <div class="flex justify-end gap-2">
          <button
            class="text-base rounded-lg"
            style="padding: 10px 20px; border: 1px solid #e2e8f0; background: white; color: #64748b; cursor: pointer;"
            @click="closeExtModal">취소</button>
          <button
            class="text-base font-semibold rounded-lg disabled:opacity-50"
            style="padding: 10px 20px; border: none; background: #2563eb; color: white; cursor: pointer;"
            :disabled="extModal.importing || !extModal.univName.trim() || !extModal.trackName.trim()"
            @click="doExtImport">
            {{ extModal.importing ? '가져오는 중…' : '가져오기' }}
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<script setup>
import { ref, watch, onMounted, defineComponent, h, computed } from 'vue'
import {
  getAreas, createArea, updateArea, deleteArea, getRounds,
  downloadAreaScoreTemplate,
  downloadNumericTableTemplate, exportNumericTable, importNumericTable,
  downloadCategoryMapTemplate, exportCategoryMap, importCategoryMap,
  downloadBaseDataTemplate, exportBaseData, importBaseData,
  getNumericTableList, getCategoryMapList, getBaseDataList,
  previewDaegyoImport, importDaegyo, previewUnivImport, importUniv,
  blobErrMsg,
} from '../../api/admin.js'
import { getScoreExample, getBaseExample } from '../../data/areaSamples.js'
import HelpBox from '../common/HelpBox.vue'
import { dialog } from '../common/dialog.js'
import ScoreDemoCard from './ScoreDemoCard.vue'
import { isKeyMatched, formatScore } from '../../utils/scorePreviewShared.js'

// ── 도움말 문구 ────────────────────────────────────────────────
const HELP_MAIN = {
  title: '도움말 — 전형요소 설정',
  intro: '전형요소는 학생 점수를 이루는 항목(예: 교과 내신, 출결, 봉사 활동)입니다. 여기서 각 항목의 만점과 점수 계산 방식을 정합니다.',
  items: [
    '"+ 전형요소 추가"를 누르고 템플릿에서 골라 시작하는 것이 가장 쉽습니다.',
    '왼쪽 목록에서 전형요소를 클릭하면 오른쪽에서 점수 기준과 기초 데이터를 등록할 수 있습니다.',
    '목록 아래 총점이 학교 규정의 학교장 추천 전형 총점과 일치하는지 확인하세요.',
    { text: '라운드 종료 이후에는 전형요소 추가·수정·삭제가 차단됩니다. 반드시 첫 라운드를 열기 전에 전형요소 설정을 끝내세요. 첫 라운드 마감 이후에 전형요소는 수정할 수 없습니다.', warn: true },
    '점수 기준을 입력한 뒤 \'점수 계산 테스트\'에 값을 넣어 보면 실제 계산될 점수를 미리 확인할 수 있습니다.',
  ],
}

const HELP_LOCKED = {
  title: '지금은 전형요소를 수정할 수 없습니다',
  intro: '종료(입력 마감) 또는 마감된 라운드가 있어 전형요소 추가·수정·삭제와 점수 기준 업로드가 차단된 상태입니다. 이미 계산된 점수와 설정이 달라지는 것을 막기 위한 보호 장치입니다.',
  items: [
    '전형요소 목록과 등록된 점수 기준은 계속 볼 수 있습니다.',
    '학생별 기초 데이터의 등록·수정(가져오기)은 지금도 가능합니다.',
  ],
}

const HELP_SCORE = {
  title: '도움말 — 점수 기준',
  intro: '점수 기준은 "어떤 값이면 몇 점"을 정하는 표입니다. 아래 양식 예시와 같은 모양의 엑셀 파일을 올리면 됩니다.',
  items: [
    '"양식 다운로드"로 빈 양식을 받아 기준값(또는 범주)과 점수를 채운 뒤 "가져오기"로 업로드하세요.',
    { text: '가져오기를 하면 이 전형요소의 기존 점수 기준이 전부 새 파일 내용으로 교체됩니다.', warn: true },
  ],
}

const HELP_BASE = {
  title: '도움말 — 기초 데이터',
  intro: '기초 데이터는 학생별 실제 값(예: 결석 횟수, 봉사 시간)입니다. 점수는 이 값을 점수 기준과 대조해 자동으로 계산됩니다.',
  items: [
    '재학생/졸업생을 선택하고 양식을 받아 학생별 값을 채워 업로드하세요.',
    '"담임교사 입력 허용"으로 설정된 전형요소는 담임교사가 지원자 등록 화면에서 직접 입력할 수도 있습니다. 관리자가 일괄 업로드해 두면 담임 화면에 미리 채워집니다.',
    '파일에 포함된 학생만 업데이트되고, 파일에 없는 학생의 기존 데이터는 그대로 유지됩니다.',
  ],
}

const hasLockedRound = ref(false)

async function loadRoundLockState() {
  try {
    const rounds = await getRounds()
    hasLockedRound.value = rounds.some(r => r.status === 'CLOSED' || r.status === 'FINALIZED')
  } catch {
    hasLockedRound.value = false // 조회 실패 시 경고 미표시 (백엔드 guard가 최종 방어선)
  }
}

// ── 전형요소 유형 안내 데이터 ────────────────────────────────────
const CALC_TYPE_DESCS = [
  {
    key: 'NUMERIC',
    label: '수치형 입력',
    desc: '기준값 범위에 따라 점수를 자동 산출합니다. 출결·성적·봉사 시간 등 연속적인 수치 데이터에 적합합니다.',
  },
  {
    key: 'CATEGORY',
    label: '선택형 입력',
    desc: '미리 정의한 범주에 해당하는 항목을 선택해 점수를 부여합니다. 자격증·수상·봉사 종류 등에 적합합니다.',
  },
  {
    key: 'MANUAL',
    label: '점수 직접 입력',
    desc: '담임교사 또는 관리자가 직접 점수를 입력합니다. 별도 기준표 없이 자유롭게 평가할 때 사용합니다.',
  },
]

const LOOKUP_SCOPE_DESCS = [
  {
    key: 'SIMPLE',
    label: '기본 조회',
    desc: '모든 대학의 모집단위에서 동일한 점수 기준표를 적용합니다. 대부분의 항목에 사용합니다.',
  },
  {
    key: 'COMPOSITE',
    label: '대학별 환산점수 조회',
    desc: '지원하는 대학·모집단위에 따라 서로 다른 기준표를 적용합니다. 대학별 환산 내신점수 반영 시 사용합니다.',
  },
]

// ── 기본 템플릿 데이터 ─────────────────────────────────────────
// 템플릿 추가·수정·삭제는 이 배열만 편집하면 됩니다.
const AREA_TEMPLATES = [
  {
    id: 'grade',
    name: '교과 내신',
    description: '대학별 내신 환산등급을 기준으로 점수를 산출합니다. 등록된 점수 기준과 정확히 일치하는 경우에만 점수가 부여됩니다.',
    hint: '수치형 입력 · 정확히 일치 · 대학별 환산점수 조회 · 담임교사 입력 불가',
    defaults: {
      name: '교과 내신',
      max_score_display: 80,
      calc_type: 'NUMERIC',
      lookup_scope: 'COMPOSITE',
      match_mode: 'EXACT',
      category_agg: '',
      teacher_editable: false,
      unit: '등급',
    },
  },
  {
    id: 'attendance',
    name: '출결',
    description: '미인정 출결에 따라 점수를 산출합니다. 미인정 횟수가 작을수록 배점이 높습니다.',
    hint: '수치형 입력 · 기준값 이하(작을수록 만점) · 기본 조회 · 담임교사 입력 허용',
    defaults: {
      name: '출결',
      max_score_display: 10,
      calc_type: 'NUMERIC',
      lookup_scope: 'SIMPLE',
      match_mode: 'LOWER',
      category_agg: '',
      teacher_editable: true,
      unit: '일',
    },
  },
  {
    id: 'volunteer',
    name: '봉사 활동',
    description: '누적 봉사시간에 따라 점수를 산출합니다. 봉사시간이 많을수록 배점이 높습니다.',
    hint: '수치형 입력 · 기준값 이상(클수록 만점) · 기본 조회 · 담임교사 입력 허용',
    defaults: {
      name: '봉사 활동',
      max_score_display: 5,
      calc_type: 'NUMERIC',
      lookup_scope: 'SIMPLE',
      match_mode: 'UPPER',
      category_agg: '',
      teacher_editable: true,
      unit: '시간',
    },
  },
  {
    id: 'award',
    name: '수상 실적',
    description: '수상 실적을 등록하고 가장 높은 점수 1건만 반영합니다.',
    hint: '선택형 입력 · 최대 1개만 인정 · 기본 조회 · 담임교사 입력 허용',
    defaults: {
      name: '수상 실적',
      max_score_display: 3,
      calc_type: 'CATEGORY',
      lookup_scope: 'SIMPLE',
      match_mode: '',
      category_agg: 'MAX',
      teacher_editable: true,
      unit: '',
    },
  },
  {
    id: 'extracurricular',
    name: '교내 활동',
    description: '교내 활동 실적을 등록하고 점수를 합산하여 반영합니다.',
    hint: '선택형 입력 · 중복 선택 가능 · 기본 조회 · 담임교사 입력 허용',
    defaults: {
      name: '교내 활동',
      max_score_display: 2,
      calc_type: 'CATEGORY',
      lookup_scope: 'SIMPLE',
      match_mode: '',
      category_agg: 'SUM',
      teacher_editable: true,
      unit: '',
    },
  },
  {
    id: 'penalty',
    name: '생활태도',
    description: '징계·학교폭력·선도처분 미이수 등 감점 사유를 복수 선택하면 각 항목의 점수(음수)가 합산됩니다. 만점은 0점이며 사유가 없으면 감점 없이 0점입니다.',
    hint: '선택형 입력 · 중복 선택 가능(합산) · 기본 조회 · 담임교사 입력 허용',
    defaults: {
      name: '생활태도',
      max_score_display: 0,
      calc_type: 'CATEGORY',
      lookup_scope: 'SIMPLE',
      match_mode: '',
      category_agg: 'SUM',
      teacher_editable: true,
      unit: '',
    },
  },
]

// ── 상태 ──────────────────────────────────────────────────────
const areas    = ref([])
const selected = ref(null)
const error    = ref('')
const activeTab   = ref('score')
const scoreResult = ref(null)
const baseResult  = ref(null)
const scorePage   = ref({ rows: [], total: 0, page: 1, per_page: 50 })
const basePage    = ref({ rows: [], total: 0, page: 1, per_page: 50 })

const showAddForm = ref(false)
const newArea = ref(defaultNewArea())
const addError = ref('')

const editingAreaId = ref(null)
const editArea = ref({ name: '', teacher_editable: false, unit: '' })
const editError = ref('')

function defaultNewArea() {
  return { name: '', max_score_display: '', calc_type: 'NUMERIC',
           lookup_scope: 'SIMPLE', teacher_editable: true,
           match_mode: '', category_agg: '', unit: '' }
}


const CALC_TYPE_LABELS    = { NUMERIC: '수치형 입력', CATEGORY: '선택형 입력', MANUAL: '점수 직접 입력' }
const LOOKUP_SCOPE_LABELS = { SIMPLE: '기본 조회', COMPOSITE: '대학별 환산점수 조회' }
const MATCH_MODE_LABELS   = { UPPER: '▲ 이상(클수록 만점)', LOWER: '▼ 이하(작을수록 만점)', EXACT: '정확히 일치' }
const CATEGORY_AGG_LABELS = { SUM: '중복 선택 가능(합산)', MAX: '최대 1개 선택(최고점)' }
function calcTypeLabel(v)    { return CALC_TYPE_LABELS[v]    ?? v }
function lookupScopeLabel(v) { return LOOKUP_SCOPE_LABELS[v] ?? v }
function matchModeLabel(v)   { return v ? (MATCH_MODE_LABELS[v]   ?? v) : '—' }
function categoryAggLabel(v) { return v ? (CATEGORY_AGG_LABELS[v] ?? v) : '—' }
const displayScore = formatScore  // 공유 포맷터로 통일 (두 곳이 갈라지지 않도록)

const totalMaxScore = computed(() => areas.value.reduce((sum, a) => sum + a.max_score, 0))

const baseStudentType = ref('enrolled')
watch(baseStudentType, () => loadBaseRows(1))

// ── 점수 계산 테스트 하이라이트 ───────────────────────────────────
const demoHighlight = ref({ matchedKeys: [], trackId: null })

function onDemoHighlight({ matchedKeys, trackId }) {
  demoHighlight.value = { matchedKeys, trackId }
}

function isDemoHighlighted(row) {
  const { matchedKeys, trackId } = demoHighlight.value
  if (!matchedKeys.length) return false
  const rowKey = selected.value?.calc_type === 'NUMERIC' ? row.threshold : row.category
  if (selected.value?.lookup_scope === 'COMPOSITE') {
    if (trackId === 0) {
      if (row.track_id != null) return false
    } else {
      const hasTrackRows = scorePage.value.rows.some(r => r.track_id === trackId)
      if (hasTrackRows) {
        if (row.track_id !== trackId) return false
      } else {
        if (row.track_id != null) return false
      }
    }
  }
  return isKeyMatched(selected.value?.calc_type, matchedKeys, rowKey)
}

const scoreEx = computed(() => selected.value ? getScoreExample(selected.value) : null)
const baseEx  = computed(() => selected.value ? getBaseExample(selected.value, baseStudentType.value) : null)

// ── 전형요소 목록 ─────────────────────────────────────────────────
async function load() {
  try { areas.value = await getAreas() }
  catch (e) { error.value = e.response?.data ?? e.message }
}

function selectArea(area) {
  selected.value = area
  activeTab.value = area.calc_type === 'MANUAL' ? 'base' : 'score'
  editingAreaId.value = null
  showAddForm.value = false

  scoreResult.value = null
  baseResult.value  = null
  demoHighlight.value = { matchedKeys: [], trackId: null }
  loadScoreRows(1)
  loadBaseRows(1)
}

async function loadScoreRows(page = 1) {
  const area = selected.value
  const empty = { rows: [], total: 0, page: 1, per_page: 50 }
  if (!area || area.calc_type === 'MANUAL') { scorePage.value = empty; return }
  try {
    const data = area.calc_type === 'CATEGORY'
      ? await getCategoryMapList(area.id, page, scorePage.value.per_page)
      : await getNumericTableList(area.id, page, scorePage.value.per_page)
    scorePage.value = data
  } catch { scorePage.value = empty }
}

async function loadBaseRows(page = 1) {
  if (!selected.value) { basePage.value = { rows: [], total: 0, page: 1, per_page: 50 }; return }
  try {
    const data = await getBaseDataList(selected.value.id, page, basePage.value.per_page, baseStudentType.value)
    basePage.value = data
  } catch { basePage.value = { rows: [], total: 0, page: 1, per_page: 50 } }
}

function onScoreResult(evt) { scoreResult.value = evt; loadScoreRows(1) }
function onBaseResult(evt)  { baseResult.value = evt;  loadBaseRows(1)  }

async function addArea() {
  addError.value = ''
  const maxScore = parseFloat(String(newArea.value.max_score_display).trim())
  if (isNaN(maxScore) || maxScore < 0) {
    addError.value = '만점: 0 이상의 숫자를 입력하세요'
    return
  }
  const body = {
    name: newArea.value.name,
    max_score: maxScore,
    calc_type: newArea.value.calc_type,
    lookup_scope: newArea.value.lookup_scope,
    teacher_editable: newArea.value.teacher_editable,
    match_mode: newArea.value.match_mode || null,
    category_agg: newArea.value.category_agg || null,
    unit: newArea.value.calc_type === 'NUMERIC' ? (newArea.value.unit || null) : null,
  }
  try {
    await createArea(body)
    showAddForm.value = false
    selected.value = null
    await load()
  } catch (e) { addError.value = e.response?.data ?? e.message }
}

async function removeArea(id) {
  if (!(await dialog.confirm({
    title: '전형요소 삭제',
    message: '전형요소를 삭제하면 등록된 점수 기준과 기초 데이터도 함께 삭제됩니다. 계속할까요?',
    confirmText: '삭제',
    level: 'danger',
    dangerNotice: '삭제된 점수 기준과 기초 데이터는 복구할 수 없습니다.',
    finalConfirmText: '영구 삭제',
  }))) return
  try {
    await deleteArea(id)
    if (selected.value?.id === id) selected.value = null
    await load()
  } catch (e) { error.value = e.response?.data ?? e.message }
}

function startEditArea(area) {
  editArea.value = { name: area.name, teacher_editable: area.teacher_editable, unit: area.unit ?? '' }
  editError.value = ''
  editingAreaId.value = area.id
}

function cancelEdit() {
  editingAreaId.value = null
}

async function saveEdit() {
  editError.value = ''
  const area = areas.value.find(a => a.id === editingAreaId.value)
  const body = {
    name: editArea.value.name,
    teacher_editable: editArea.value.teacher_editable,
    // MANUAL은 단위 '점' 강제(프론트 표시 전용)라 unit을 보내지 않는다
    unit: area?.calc_type === 'NUMERIC' ? (editArea.value.unit ?? null) : undefined,
  }
  try {
    await updateArea(editingAreaId.value, body)
    const prevId = selected.value?.id
    await load()
    selected.value = prevId != null ? (areas.value.find(a => a.id === prevId) ?? null) : null
    editingAreaId.value = null
  } catch (e) {
    editError.value = e.response?.data ?? e.message
  }
}

function openAddForm() {
  newArea.value = defaultNewArea()
  addError.value = ''
  editingAreaId.value = null
  showAddForm.value = true
}

function applyTemplate(tpl) {
  newArea.value = { ...tpl.defaults }
}

const dlTemplateId = ref(null)

async function dlScoreTemplate(tpl) {
  if (dlTemplateId.value) return
  dlTemplateId.value = tpl.id
  try {
    const res = await downloadAreaScoreTemplate(tpl.id)
    const url = URL.createObjectURL(new Blob([res.data]))
    const a = document.createElement('a')
    a.href = url
    a.download = `${tpl.name}_점수기준_샘플.xlsx`
    a.click()
    URL.revokeObjectURL(url)
  } catch {
    await dialog.alert({ title: '다운로드 실패', message: '샘플 파일을 불러오지 못했습니다. 아직 파일이 준비되지 않았을 수 있습니다.', level: 'error' })
  } finally {
    dlTemplateId.value = null
  }
}

// ── 외부 가져오기 모달 ────────────────────────────────────────────
// 모집단위명 빠른 입력 — 매번 손으로 치는 수고를 줄이는 단순 채우기 버튼
const TRACK_PRESETS = ['인문계열', '자연계열']

const extModal = ref({
  open: false, format: '', title: '', file: null, fileName: '',
  univName: '', trackName: '', valueHeader: '', headerInfo: '',
  preview: [], total: 0, importing: false, error: '',
})

async function onExternalFile(format, evt) {
  const file = evt.target.files?.[0]
  evt.target.value = ''
  if (!file) return
  try {
    const data = format === 'daegyo'
      ? await previewDaegyoImport(selected.value.id, file)
      : await previewUnivImport(selected.value.id, file)
    extModal.value = {
      open: true,
      format,
      title: format === 'daegyo' ? '대교협 석차연명부 가져오기' : '유니브 석차연명부 가져오기',
      file,
      fileName: file.name,
      univName: data.univ_name,
      trackName: '',
      valueHeader: data.value_header,
      headerInfo: data.header_info,
      preview: data.preview,
      total: data.total,
      importing: false,
      error: '',
    }
  } catch (e) {
    await dialog.alert({ title: '가져오기 실패', message: e.response?.data ?? e.message ?? '파일 파싱 오류', level: 'error' })
  }
}

function closeExtModal() {
  extModal.value.open = false
}

async function doExtImport() {
  const m = extModal.value
  if (!m.trackName.trim()) return
  m.importing = true
  m.error = ''
  try {
    const res = m.format === 'daegyo'
      ? await importDaegyo(selected.value.id, m.file, m.univName, m.trackName)
      : await importUniv(selected.value.id, m.file, m.univName, m.trackName)
    closeExtModal()
    onBaseResult(res.data)
  } catch (e) {
    const d = e.response?.data
    if (d != null && typeof d === 'object' && Array.isArray(d.errors)) {
      closeExtModal()
      onBaseResult(d)
    } else {
      m.error = typeof d === 'string' ? d : (e.message ?? '오류가 발생했습니다')
    }
  } finally {
    m.importing = false
  }
}

onMounted(() => { load(); loadRoundLockState() })

// ── 다운로드 헬퍼 ─────────────────────────────────────────────
function saveBlob(response, filename) {
  const url = URL.createObjectURL(new Blob([response.data]))
  const a = document.createElement('a')
  a.href = url; a.download = filename; a.click()
  URL.revokeObjectURL(url)
}

// ── ExcelPanel (인라인 컴포넌트) ──────────────────────────────
const ExcelPanel = defineComponent({
  props: {
    areaId:      { type: Number, required: true },
    calcType:    { type: String, required: true },
    areaName:    { type: String, required: true },
    panel:       { type: String, required: true }, // 'score' | 'base'
    studentType: { type: String, default: 'enrolled' },
  },
  emits: ['result', 'update:studentType'],
  setup(props, { emit }) {
    const err = ref('')
    const uploading = ref(false)
    const downloading = ref(false)

    async function dlTemplate() {
      err.value = ''
      downloading.value = true
      try {
        if (props.panel === 'score') {
          const res = props.calcType === 'CATEGORY'
            ? await downloadCategoryMapTemplate(props.areaId)
            : await downloadNumericTableTemplate(props.areaId)
          saveBlob(res, props.calcType === 'CATEGORY'
            ? `${props.areaName}_category_map_template.xlsx`
            : `${props.areaName}_numeric_table_template.xlsx`)
        } else {
          const res = await downloadBaseDataTemplate(props.areaId, props.studentType)
          saveBlob(res, `${props.areaName}_base_data_${props.studentType}_template.xlsx`)
        }
      } catch (e) { err.value = await blobErrMsg(e) }
      finally { downloading.value = false }
    }

    async function dlExport() {
      err.value = ''
      downloading.value = true
      try {
        if (props.panel === 'score') {
          const res = props.calcType === 'CATEGORY'
            ? await exportCategoryMap(props.areaId)
            : await exportNumericTable(props.areaId)
          saveBlob(res, props.calcType === 'CATEGORY'
            ? `${props.areaName}_category_map.xlsx`
            : `${props.areaName}_numeric_table.xlsx`)
        } else {
          // 선택된 재학생/졸업생 탭 기준으로 import와 동일 헤더로 내보낸다 (수정 후 재업로드 가능)
          const res = await exportBaseData(props.areaId, props.studentType)
          saveBlob(res, `${props.areaName}_base_data_${props.studentType}.xlsx`)
        }
      } catch (e) { err.value = await blobErrMsg(e) }
      finally { downloading.value = false }
    }

    async function onFile(evt) {
      const file = evt.target.files?.[0]
      if (!file) return
      err.value = ''
      uploading.value = true
      try {
        let result
        if (props.panel === 'score') {
          result = props.calcType === 'CATEGORY'
            ? await importCategoryMap(props.areaId, file)
            : await importNumericTable(props.areaId, file)
        } else {
          result = await importBaseData(props.areaId, file, props.studentType)
        }
        emit('result', result)
      } catch (e) {
        const d = e.response?.data
        if (d != null && typeof d === 'object' && Array.isArray(d.errors)) {
          emit('result', d)
        } else {
          err.value = typeof d === 'string' ? d : (e.message ?? '오류가 발생했습니다')
        }
      }
      finally { uploading.value = false; evt.target.value = '' }
    }

    const btnStyle = 'padding: 9px 16px; border: 1px solid #e2e8f0; background: white; color: #475569; border-radius: 8px; font-size: 16px; cursor: pointer;'
    const uploadStyle = (up) => `padding: 9px 16px; border-radius: 8px; font-size: 16px; cursor: pointer; color: white; background: ${up ? '#94a3b8' : '#2563eb'};`

    return () => h('div', { class: 'space-y-2' }, [
      h('div', { class: 'flex flex-wrap gap-2 items-center' }, [

        // ── 기초 데이터: 재학생/졸업생 라디오
        ...(props.panel === 'base' ? [
          h('label', { style: 'display: flex; align-items: center; gap: 6px; font-size: 16px; cursor: pointer; color: #475569;' }, [
            h('input', {
              type: 'radio',
              name: `st-${props.areaId}`,
              checked: props.studentType === 'enrolled',
              class: 'accent-blue-600',
              onChange: () => emit('update:studentType', 'enrolled'),
            }),
            '재학생',
          ]),
          h('label', { style: 'display: flex; align-items: center; gap: 6px; font-size: 16px; cursor: pointer; color: #475569;' }, [
            h('input', {
              type: 'radio',
              name: `st-${props.areaId}`,
              checked: props.studentType === 'graduated',
              class: 'accent-blue-600',
              onChange: () => emit('update:studentType', 'graduated'),
            }),
            '졸업생',
          ]),
          h('span', { style: 'color: #cbd5e1; user-select: none;' }, '|'),
        ] : []),

        h('button', { style: btnStyle, disabled: downloading.value, onClick: dlTemplate },
        props.panel === "base"
            ? (props.studentType === 'enrolled'
                ? '재학생 양식 다운로드'
                : '졸업생 양식 다운로드')
            : '양식 다운로드'
        ),

        h('label', { style: uploadStyle(uploading.value) }, [
          uploading.value
              ? '가져오는 중…'
              : props.panel === 'base'
                  ? (props.studentType === 'enrolled'
                      ? '재학생 가져오기'
                      : '졸업생 가져오기')
                  : '가져오기',
          h('input', { type: 'file', accept: '.xlsx,.csv', style: 'display: none;', onChange: onFile }),
        ]),

        h('span', { style: 'color: #cbd5e1; user-select: none;' }, '|'),
        h('button', { style: btnStyle, disabled: downloading.value, onClick: dlExport }, '전체 목록 다운로드'),
      ]),
      h('p', { style: 'font-size: 16px; color: #92400e;' },
        props.panel === 'base'
          ? `※ 파일에 포함된 학생의 기초 데이터만 업데이트됩니다. 파일에 없는 학생의 데이터는 유지됩니다.`
          : '※ 가져오기 시 기존 점수 기준이 모두 교체됩니다.'),
      err.value ? h('p', { style: 'font-size: 16px; color: #ef4444;' }, err.value) : null,
    ])
  },
})

// ── ImportResultBox (인라인 컴포넌트) ──────────────────────────
const ImportResultBox = defineComponent({
  props: { result: Object },
  setup(props) {
    return () => {
      const r = props.result
      const hasErrors = r.errors?.length > 0
      const hasWarnings = r.warnings?.length > 0
      const bgStyle = hasErrors
        ? 'padding: 14px 18px; border-radius: 12px; border: 1px solid #fca5a5; background: #fef2f2;'
        : 'padding: 14px 18px; border-radius: 12px; border: 1px solid #86efac; background: #f0fdf4;'
      const titleColor = hasErrors ? '#991b1b' : '#15803d'
      const countStr = r.rows != null ? `${r.rows}건` : r.inserted != null ? `신규 ${r.inserted}명, 수정 ${r.updated}명` : ''
      return h('div', { style: bgStyle }, [
        h('p', { style: `font-size: 16px; font-weight: 600; margin: 0 0 4px; color: ${titleColor};` },
          hasErrors ? '오류 발생 — 가져오기 실패' : `완료 — ${countStr} 처리됨`),
        hasWarnings
          ? h('ul', { style: 'font-size: 16px; color: #15803d; padding-left: 20px; margin: 0;' },
              r.warnings.map((w, i) => h('li', { key: i }, w)))
          : null,
        hasErrors
          ? h('ul', { style: 'font-size: 16px; color: #991b1b; padding-left: 20px; margin: 0;' },
              r.errors.map((e, i) => h('li', { key: i }, e)))
          : null,
      ])
    }
  },
})
</script>

<style scoped>
.template-btn {
  transition: border-color 0.15s;
}
.template-btn:hover {
  border-color: #93c5fd !important;
}
</style>
