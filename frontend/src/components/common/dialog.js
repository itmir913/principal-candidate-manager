import { reactive } from 'vue'

// 전역 다이얼로그 상태 — DialogHost.vue가 렌더링한다
export const dialogState = reactive({
  open: false,
  kind: 'confirm',        // 'confirm' | 'alert'
  level: 'normal',        // confirm: 'normal' | 'warn' | 'danger' / alert: 'normal' | 'error'
  title: '',
  message: '',
  confirmText: '확인',
  cancelText: '취소',
  dangerNotice: '',       // danger 2단계 경고 패널에 표시할 결과 설명 문장
  finalConfirmText: '',   // danger 2단계 빨간 버튼 라벨
  step: 1,                // danger 전용: 1(1차 확인) → 2(최종 확인)
  resolver: null,
})

function openDialog(opts) {
  return new Promise((resolve) => {
    // 동시 호출 시 이전 다이얼로그는 취소로 종결
    if (dialogState.open && dialogState.resolver) dialogState.resolver(false)
    dialogState.kind             = opts.kind
    dialogState.level            = opts.level ?? 'normal'
    dialogState.title            = opts.title ?? ''
    dialogState.message          = opts.message ?? ''
    dialogState.confirmText      = opts.confirmText ?? '확인'
    dialogState.cancelText       = opts.cancelText ?? '취소'
    dialogState.dangerNotice     = opts.dangerNotice ?? ''
    dialogState.finalConfirmText = opts.finalConfirmText ?? opts.confirmText ?? '확인'
    dialogState.step             = 1
    dialogState.resolver         = resolve
    dialogState.open             = true
  })
}

// result: confirm은 true/false, alert는 항상 true
export function settleDialog(result) {
  if (!dialogState.open) return
  const resolve = dialogState.resolver
  dialogState.open = false
  dialogState.resolver = null
  if (resolve) resolve(result)
}

export const dialog = {
  /**
   * 확인/취소 다이얼로그. 확인=true, 취소·ESC=false.
   * level: 'normal'(파란 확인 버튼) | 'warn'(흰 배경+빨간 테두리) | 'danger'(2단계 검증)
   * danger는 1차 확인 후 경고 패널 + 빨간 배경 버튼(finalConfirmText)으로 한 번 더 묻는다.
   */
  confirm(opts) { return openDialog({ ...opts, kind: 'confirm' }) },
  /** 확인 버튼 하나짜리 알림. level: 'normal' | 'error'(빨간 제목+경고 아이콘) */
  alert(opts) { return openDialog({ ...opts, kind: 'alert', level: opts?.level ?? 'normal' }) },
}
