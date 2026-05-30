import axios from 'axios'

export const getCurrentRound = () => axios.get('/api/rounds/current').then(r => r.data)

export const teacherGetStudents = () => axios.get('/api/teacher/students').then(r => r.data)

// 모집단위 목록 (대학명 포함) — 지원 등록 드롭다운용
export const teacherGetAllTracks = () => axios.get('/api/teacher/univ-tracks').then(r => r.data)

export const teacherGetApplications = (roundId) =>
  axios.get('/api/teacher/applications', { params: { round_id: roundId } }).then(r => r.data)

export const teacherCreateApplication = (body) =>
  axios.post('/api/teacher/applications', body)

export const teacherDeleteApplication = (sid, tid, rid) =>
  axios.delete(`/api/teacher/applications/${sid}/${tid}/${rid}`)

export const teacherChangePassword = (currentPassword, newPassword) =>
  axios.put('/api/teacher/password', { current_password: currentPassword, new_password: newPassword })

export const teacherGetResults = (roundId) =>
  axios.get('/api/teacher/results', { params: roundId != null ? { round_id: roundId } : {} }).then(r => r.data)
