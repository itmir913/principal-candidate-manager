/// blob 응답으로 온 오류 메시지를 문자열로 바꾼다.
///
/// responseType: 'blob' 요청이 실패하면 response.data 가 Blob 이라 그대로 표시하면
/// "[object Blob]" 이 뜬다. 서버가 보낸 오류 문구를 사람이 읽을 수 있게 꺼낸다.
///
/// api/admin.js 에 있던 것을 옮겼다 — 담임 화면도 CSV 내려받기(이슈 #24)에서 쓰는데,
/// 담임 컴포넌트가 관리자 API 모듈을 참조하면 계층이 어긋난다.
export async function blobErrMsg(e) {
  const d = e.response?.data
  if (d instanceof Blob) {
    try { return await d.text() } catch { /* fall through */ }
  }
  return typeof d === 'string' ? d : (e.message ?? '오류가 발생했습니다')
}
