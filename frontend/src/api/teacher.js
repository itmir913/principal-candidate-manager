import axios from 'axios'

export const getCurrentRound = () => axios.get('/api/rounds/current').then(r => r.data)

export const teacherGetStudents = () => axios.get('/api/teacher/students').then(r => r.data)

export const teacherGetUniversities = () => axios.get('/api/teacher/universities').then(r => r.data)

export const teacherGetApplications = (roundId) =>
  axios.get('/api/teacher/applications', { params: { round_id: roundId } }).then(r => r.data)

export const teacherCreateApplication = (body) =>
  axios.post('/api/teacher/applications', body)

export const teacherDeleteApplication = (sid, uid, rid) =>
  axios.delete(`/api/teacher/applications/${sid}/${uid}/${rid}`)

export const teacherChangePassword = (newPassword) =>
  axios.put('/api/teacher/password', { new_password: newPassword })

export const teacherGetResults = (roundId) =>
  axios.get('/api/teacher/results', { params: roundId != null ? { round_id: roundId } : {} }).then(r => r.data)
