// @vitest-environment jsdom
/**
 * 확인·경고 대화상자 — **열었을 때 실제로 그려지는지** 본다.
 *
 * 왜 따로 파일을 두나: `DialogHost.vue` 는 본문 전체가 `v-if="s.open"` 뒤에 있어,
 * 스모크 렌더에서 `textLen=0` / `html="<!--v-if-->"` 로 통과했다. 즉 이 앱의
 * **모든 확인 대화상자**(추천 포기, 삭제, 일괄 교체 같은 되돌리기 어려운 행위 포함)가
 * 어떤 테스트에도 닿지 않았다(3차 감사 중-D).
 *
 * `expect(wrapper.html()).toBeTruthy()` 는 `"<!--v-if-->"` 에도 참이다 — 그 단언이
 * 아무것도 지키지 못한다는 것을 여기서 못 박는다.
 *
 * 무엇을 지키나: 제목·본문·버튼 라벨이 화면에 닿는지, **danger 2단계**가 한 번 더 묻는지,
 * 취소·확인이 각각 올바른 값으로 약속을 끝내는지. 되돌리기 어려운 행위의 마지막 관문이다.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { mount } from '@vue/test-utils'
import DialogHost from '../src/components/common/DialogHost.vue'
import { dialog, dialogState, settleDialog } from '../src/components/common/dialog.js'
import { nextTick } from 'vue'

// **DialogHost 는 `<Teleport to="body">` 다.** 내용이 wrapper 밖(document.body)에
// 그려지므로 `wrapper.text()` 로는 영원히 빈 문자열이다 — 처음에 그렇게 헛돌았다.
// 그래서 body 를 직접 본다. 이것 자체가 스모크가 이 화면을 못 보던 또 다른 이유다.
const tick = async () => { await nextTick(); await nextTick() }
const bodyText = () => document.body.textContent
const btn = (label) =>
  [...document.body.querySelectorAll('button')].find(b => b.textContent.trim() === label)
const click = async (el) => { el.click(); await tick() }

describe('확인 대화상자', () => {
  beforeEach(() => { vi.spyOn(console, 'warn').mockImplementation(() => {}) })
  afterEach(() => {
    if (dialogState.open) settleDialog(false)   // 다음 테스트로 상태가 새지 않게
    vi.restoreAllMocks()
  })

  it('닫혀 있을 때는 아무것도 그리지 않는다', () => {
    const w = mount(DialogHost, { attachTo: document.body })
    expect(bodyText()).toBe('')
    // 스모크가 이 상태를 "통과"로 봤다 — html() 은 truthy 다.
    expect(w.html()).toBeTruthy()
    w.unmount()
  })

  it('열면 제목·본문·버튼이 그려진다', async () => {
    const w = mount(DialogHost, { attachTo: document.body })
    dialog.confirm({ title: '추천을 취소할까요?', message: '되돌릴 수 없습니다.',
                     confirmText: '취소하기', cancelText: '그만두기' })
    await tick()

    const t = bodyText()
    expect(t, '제목이 없다').toContain('추천을 취소할까요?')
    expect(t, '본문이 없다').toContain('되돌릴 수 없습니다.')
    expect(btn('취소하기'), '확인 버튼 라벨이 다르다').toBeTruthy()
    expect(btn('그만두기'), '취소 버튼 라벨이 다르다').toBeTruthy()
    w.unmount()
  })

  it('확인을 누르면 true, 취소를 누르면 false 로 끝난다', async () => {
    const w = mount(DialogHost, { attachTo: document.body })

    const yes = dialog.confirm({ title: 'A', message: 'B' })
    await tick()
    await click(btn('확인'))
    expect(await yes, '확인이 true 를 주지 않는다').toBe(true)

    const no = dialog.confirm({ title: 'A', message: 'B' })
    await tick()
    await click(btn('취소'))
    expect(await no, '취소가 false 를 주지 않는다').toBe(false)

    w.unmount()
  })

  it('danger 는 한 번 더 묻는다 — 1단계 확인만으로 끝나지 않는다', async () => {
    // 되돌리기 어려운 행위의 2단계 관문. 여기가 무너지면 한 번 누르면 실행된다.
    const w = mount(DialogHost, { attachTo: document.body })
    let settled = null
    dialog.confirm({
      title: '라운드를 마감할까요?', message: '마감 후에는 되돌릴 수 없습니다.',
      level: 'danger', dangerNotice: '추천이 확정되고 담임 입력이 잠깁니다.',
      finalConfirmText: '마감합니다',
    }).then(v => { settled = v })

    await tick()
    await click(btn('확인'))

    expect(settled, '1단계 확인만으로 끝나 버렸다').toBeNull()
    expect(bodyText(), '2단계 경고 문구가 없다').toContain('추천이 확정되고')
    expect(btn('마감합니다'), '최종 확인 버튼이 없다').toBeTruthy()

    await click(btn('마감합니다'))
    expect(settled, '최종 확인이 true 를 주지 않는다').toBe(true)
    w.unmount()
  })

  it('warnNotice 는 본문과 따로 그려진다', async () => {
    // 놓치면 안 되는 사실을 중립적인 본문에 섞으면 읽고 지나친다 — dialog.js 의 의도.
    const w = mount(DialogHost, { attachTo: document.body })
    dialog.confirm({ title: 'A', message: '본문입니다.', warnNotice: '미확정 학급이 2개 있습니다.' })
    await tick()
    expect(bodyText()).toContain('미확정 학급이 2개 있습니다.')
    w.unmount()
  })

  it('ESC 는 취소다 — 확인으로 동작하지 않는다', async () => {
    // 되돌리기 어려운 행위의 확인창에서 ESC 가 "확인"이 되면 한 번의 실수로 실행된다.
    // 이 단언이 없던 동안 `settleDialog(s.kind === 'alert')` -> `settleDialog(true)`
    // 변이가 전 검증을 통과했다(4차 감사 중-A).
    const w = mount(DialogHost, { attachTo: document.body })
    const answer = dialog.confirm({ title: '삭제할까요?', message: '되돌릴 수 없습니다.' })
    await tick()
    // 리스너는 window 에 붙는다(DialogHost.vue:142). document 로 쏘면 닿지 않는다.
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }))
    await tick()
    expect(await answer, 'ESC 가 확인으로 동작한다').toBe(false)
    w.unmount()
  })

  it('danger 2단계에서도 ESC 는 취소다', async () => {
    // DialogHost.vue:133 주석이 "2단계 상태에서도 즉시 취소"라고 보장하는데,
    // 1단계만 시험하던 동안 `settleDialog(s.kind === 'alert' || s.step === 2)` 변이가
    // 통과했다 — 마감 확정 화면에서 ESC 가 **실행**이 된다(5차 감사 중-2).
    const w = mount(DialogHost, { attachTo: document.body })
    const answer = dialog.confirm({
      title: '라운드를 마감할까요?', message: '되돌릴 수 없습니다.',
      level: 'danger', dangerNotice: '추천이 확정됩니다.', finalConfirmText: '마감합니다',
    })
    await tick()
    await click(btn('확인'))            // 1단계 통과 -> step 2
    expect(btn('마감합니다'), '2단계로 넘어가지 않았다').toBeTruthy()

    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }))
    await tick()
    expect(await answer, '2단계에서 ESC 가 확인으로 동작한다').toBe(false)
    w.unmount()
  })

  it('연속으로 열면 앞의 약속이 취소로 끝난다', async () => {
    // 끝나지 않은 약속이 남으면 `await dialog.confirm()` 뒤의 코드가 영원히 안 돌아
    // 버튼이 먹통이 된다(dialog.js:22 의 의도).
    const w = mount(DialogHost, { attachTo: document.body })
    const first = dialog.confirm({ title: 'A', message: 'A' })
    dialog.confirm({ title: 'B', message: 'B' })
    await tick()
    expect(await first, '앞의 다이얼로그가 끝나지 않는다').toBe(false)
    settleDialog(false)
    w.unmount()
  })

  it('alert 는 확인 버튼 하나뿐이다', async () => {
    const w = mount(DialogHost, { attachTo: document.body })
    dialog.alert({ title: '오류', message: '저장하지 못했습니다.', level: 'error' })
    await tick()
    expect(bodyText()).toContain('저장하지 못했습니다.')
    expect(btn('취소'), 'alert 에 취소 버튼이 있다').toBeFalsy()
    await click(btn('확인'))
    w.unmount()
  })
})
