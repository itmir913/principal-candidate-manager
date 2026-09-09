import { ref, computed } from 'vue'
import { defineStore } from 'pinia'
import axios from 'axios'

// 설정 전 기본 문구 — 백엔드 app_info::DEFAULT_TITLE / DEFAULT_DESC 와 같은 값이다.
// 서버가 항상 값을 채워 주므로 여기 값은 응답 도착 전 한 프레임과 조회 실패 시에만 쓰인다.
const FALLBACK_TITLE = '학교장 추천자'
const FALLBACK_DESC = '선발 관리 시스템'

// 사이드바는 한 줄짜리 고정 폭(240px)이라 title 만 쓴다. 그런데 미지정 상태의 title
// ('학교장 추천자')은 제품명이 아니라 문장 조각이라 단독으로는 어색하다.
// 지정 전에는 0.2.15 이전에 하드코딩돼 있던 문구를 그대로 쓴다 — 설정하지 않은 학교의
// 화면이 예전과 같아야 한다는 것이 이 기능의 전제다.
const SIDEBAR_FALLBACK = '학교장추천 선발 시스템'

/// 프로그램 제목·부제 (이슈 #23).
///
/// 로그인·시작·서버오류 화면이 인증 전에 제목을 그려야 해서 GET은 공개 엔드포인트다.
/// 한 번 받아 두고 앱 전체가 공유한다 — 화면마다 따로 요청하면 같은 값을 여러 번 묻게 된다.
export const useAppInfoStore = defineStore('appInfo', () => {
  const title = ref(FALLBACK_TITLE)
  const desc = ref(FALLBACK_DESC)
  const loaded = ref(false)
  // 관리자가 제목을 실제로 지정했는가. 앱 안의 설치본 이름 카드는 이 값이 true 일 때만 뜬다
  const configured = ref(false)

  /// 제목 + 부제를 한 줄로 — 브라우저 탭·문서 제목처럼 한 줄만 쓰는 자리용.
  const fullTitle = computed(() => (desc.value ? `${title.value} ${desc.value}` : title.value))

  /// 사이드바용 — 지정 전에는 예전 문구, 지정 후에는 학교가 붙인 이름.
  const sidebarTitle = computed(() => (configured.value ? title.value : SIDEBAR_FALLBACK))

  function _apply(data) {
    title.value = data.title
    desc.value = data.desc
    configured.value = data.configured
    loaded.value = true
    document.title = fullTitle.value
  }

  /// 앱 시작 시 1회. 실패해도 화면은 기본 문구로 그대로 뜬다 — 제목을 못 읽었다고
  /// 로그인을 막을 이유가 없다.
  async function load() {
    if (loaded.value) return
    try {
      const res = await axios.get('/api/app-info')
      _apply(res.data)
    } catch {
      document.title = fullTitle.value
    }
  }

  /// 설정 탭에서 저장한 직후 호출 — 저장 응답이 곧 새 값이라 다시 GET 하지 않는다.
  function set(data) {
    _apply(data)
  }

  return { title, desc, configured, loaded, fullTitle, sidebarTitle, load, set }
})
