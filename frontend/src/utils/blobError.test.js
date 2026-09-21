import { describe, it, expect } from 'vitest'
import { blobErrMsg } from './blobError.js'

// Blob 은 Node 18+ 전역이라 jsdom 없이 그대로 쓴다.

describe('blobErrMsg', () => {
  it('Blob 응답에서 서버 오류 문구를 꺼낸다', async () => {
    // 이 분기가 빠지면 화면에 "[object Blob]" 이 뜬다.
    const e = { response: { data: new Blob(['정원을 초과했습니다']) } }
    expect(await blobErrMsg(e)).toBe('정원을 초과했습니다')
  })

  it('Blob 읽기가 실패하면 조용히 죽지 않고 다음 후보로 넘어간다', async () => {
    const broken = new Blob(['x'])
    broken.text = () => Promise.reject(new Error('read failed'))
    const e = { response: { data: broken }, message: 'Network Error' }
    expect(await blobErrMsg(e)).toBe('Network Error')
  })

  it('문자열 본문은 그대로 쓴다', async () => {
    const e = { response: { data: '라운드가 마감되었습니다' } }
    expect(await blobErrMsg(e)).toBe('라운드가 마감되었습니다')
  })

  it('본문이 없으면 axios 메시지를 쓴다', async () => {
    expect(await blobErrMsg({ message: 'timeout of 0ms exceeded' }))
      .toBe('timeout of 0ms exceeded')
  })

  it('아무 단서도 없으면 기본 문구를 준다', async () => {
    expect(await blobErrMsg({})).toBe('오류가 발생했습니다')
  })

  it('JSON 본문은 문자열이 아니므로 메시지로 대체한다', async () => {
    // 객체를 그대로 표시하면 "[object Object]" 가 된다.
    const e = { response: { data: { error: 'x' } }, message: 'Request failed' }
    expect(await blobErrMsg(e)).toBe('Request failed')
  })
})
