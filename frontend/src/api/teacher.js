import axios from 'axios'

export const getCurrentRound = () => axios.get('/api/rounds/current').then(r => r.data)

export const teacherGetStudents = () => axios.get('/api/teacher/students').then(r => r.data)

// 대학 목록 (이름순 정렬)
export const teacherGetUniversities = () => axios.get('/api/teacher/universities').then(r => r.data)

// 특정 대학의 모집단위 목록 (이름순 정렬)
export const teacherGetUnivTracks = (univId) =>
  axios.get(`/api/teacher/universities/${univId}/tracks`).then(r => r.data)

// 전형요소 + 점수표 + 기저장 기초데이터
export const teacherGetAreaContext = (studentId, trackId) =>
  axios.get('/api/teacher/area-context', {
    params: { student_id: studentId, track_id: trackId },
  }).then(r => r.data)

// 실시간 점수 계산 (비저장)
export const teacherAreaScorePreview = (areaId, trackId, values) =>
  axios.post('/api/teacher/area-score-preview', {
    area_id: areaId,
    track_id: trackId,
    values,
  }).then(r => r.data)

// 모집단위 목록 전체 (대학명 포함) — 학급관리 탭용
export const teacherGetAllTracks = () => axios.get('/api/teacher/univ-tracks').then(r => r.data)

export const teacherGetApplications = (roundId) =>
  axios.get('/api/teacher/applications', { params: { round_id: roundId } }).then(r => r.data)

// body: { student_id, track_id, round_id, department_name, base_data_entries: [{area_id, values}] }
export const teacherCreateApplication = (body) =>
  axios.post('/api/teacher/applications', body)

export const teacherDeleteApplication = (sid, tid, rid) =>
  axios.delete(`/api/teacher/applications/${sid}/${tid}/${rid}`)

export const teacherChangePassword = (currentPassword, newPassword) =>
  axios.put('/api/teacher/password', { current_password: currentPassword, new_password: newPassword })

export const teacherGetResults = () =>
  axios.get('/api/teacher/results').then(r => r.data)

export const teacherAbandonApplication = (sid, tid, rid) =>
  axios.put(`/api/teacher/applications/${sid}/${tid}/${rid}/abandon`)
