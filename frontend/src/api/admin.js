import axios from 'axios'

export const getClasses = () => axios.get('/api/classes').then(r => r.data)

export const upsertClass = (grade, classNo, body) =>
  axios.put(`/api/classes/${grade}/${classNo}`, body).then(r => r.data)

export const downloadClassTemplate = () =>
  axios.get('/api/classes/template', { responseType: 'blob' })

export const exportClasses = () =>
  axios.get('/api/classes/export', { responseType: 'blob' })

export const deleteClass = (grade, classNo) =>
  axios.delete(`/api/classes/${grade}/${classNo}`)

export const importClasses = (file) => {
  const fd = new FormData(); fd.append('file', file)
  return axios.post('/api/classes/import', fd).then(r => r.data)
}

export const getAreas = () => axios.get('/api/areas').then(r => r.data)

export const createArea = (body) => axios.post('/api/areas', body).then(r => r.data)

export const updateArea = (id, body) => axios.put(`/api/areas/${id}`, body).then(r => r.data)

export const deleteArea = (id) => axios.delete(`/api/areas/${id}`)

// ── 점수 기준 Excel (range-table) ──────────────────────────────
export const downloadRangeTableTemplate = (id) =>
  axios.get(`/api/areas/${id}/range-table/template`, { responseType: 'blob' })
export const exportRangeTable = (id) =>
  axios.get(`/api/areas/${id}/range-table/export`, { responseType: 'blob' })
export const importRangeTable = (id, file) => {
  const fd = new FormData(); fd.append('file', file)
  return axios.post(`/api/areas/${id}/range-table/import`, fd).then(r => r.data)
}

// ── 점수 기준 Excel (category-map) ─────────────────────────────
export const downloadCategoryMapTemplate = (id) =>
  axios.get(`/api/areas/${id}/category-map/template`, { responseType: 'blob' })
export const exportCategoryMap = (id) =>
  axios.get(`/api/areas/${id}/category-map/export`, { responseType: 'blob' })
export const importCategoryMap = (id, file) => {
  const fd = new FormData(); fd.append('file', file)
  return axios.post(`/api/areas/${id}/category-map/import`, fd).then(r => r.data)
}

// ── 점수 기준 / 기초 데이터 JSON 조회 ─────────────────────────
export const getRangeTableList = (id) =>
  axios.get(`/api/areas/${id}/range-table/list`).then(r => r.data)
export const getCategoryMapList = (id) =>
  axios.get(`/api/areas/${id}/category-map/list`).then(r => r.data)
export const getBaseDataList = (id) =>
  axios.get(`/api/areas/${id}/base-data/list`).then(r => r.data)

// ── 기초 데이터 Excel ──────────────────────────────────────────
export const downloadBaseDataTemplate = (id) =>
  axios.get(`/api/areas/${id}/base-data/template`, { responseType: 'blob' })
export const exportBaseData = (id) =>
  axios.get(`/api/areas/${id}/base-data/export`, { responseType: 'blob' })
export const importBaseData = (id, file) => {
  const fd = new FormData(); fd.append('file', file)
  return axios.post(`/api/areas/${id}/base-data/import`, fd).then(r => r.data)
}

// ── 학생 관리 ──────────────────────────────────────────────────
export const getStudents = (params = {}) =>
  axios.get('/api/students', { params }).then(r => r.data)

export const getStudentGradeOptions = () =>
  axios.get('/api/students/grade-options').then(r => r.data)

export const downloadStudentTemplate = () =>
  axios.get('/api/students/template', { responseType: 'blob' })
export const exportStudents = () =>
  axios.get('/api/students/export', { responseType: 'blob' })
export const importStudents = (file) => {
  const fd = new FormData(); fd.append('file', file)
  return axios.post('/api/students/import', fd).then(r => r.data)
}

export const downloadEnrolledTemplate = () =>
  axios.get('/api/students/enrolled/template', { responseType: 'blob' })
export const exportEnrolled = () =>
  axios.get('/api/students/enrolled/export', { responseType: 'blob' })
export const importEnrolled = (file) => {
  const fd = new FormData(); fd.append('file', file)
  return axios.post('/api/students/enrolled/import', fd).then(r => r.data)
}

export const downloadGraduatedTemplate = () =>
  axios.get('/api/students/graduated/template', { responseType: 'blob' })
export const exportGraduated = () =>
  axios.get('/api/students/graduated/export', { responseType: 'blob' })
export const importGraduated = (file) => {
  const fd = new FormData(); fd.append('file', file)
  return axios.post('/api/students/graduated/import', fd).then(r => r.data)
}

export const deleteStudent = (id) => axios.delete(`/api/students/${id}`)

// ── 대학 관리 ──────────────────────────────────────────────────
export const getUniversities = () => axios.get('/api/universities').then(r => r.data)

export const createUniversity = (body) =>
  axios.post('/api/universities', body).then(r => r.data)

export const updateUniversity = (id, body) =>
  axios.put(`/api/universities/${id}`, body).then(r => r.data)

export const deleteUniversity = (id) => axios.delete(`/api/universities/${id}`)

// ── 라운드 관리 ────────────────────────────────────────────────
export const getRounds = () => axios.get('/api/rounds').then(r => r.data)
export const openRound = () => axios.post('/api/rounds/open').then(r => r.data)
export const closeRound = (id) => axios.put(`/api/rounds/${id}/close`)
export const calculateScores = (roundId) =>
  axios.post(`/api/rounds/${roundId}/calculate`).then(r => r.data)
export const getResults = (roundId, univId) =>
  axios.get(`/api/rounds/${roundId}/results`, { params: univId ? { univ_id: univId } : {} }).then(r => r.data)
export const recommendResult = (sid, uid, rid) =>
  axios.put(`/api/results/${sid}/${uid}/${rid}/recommend`)

// ── 지원 관리 (admin) ──────────────────────────────────────────
export const getApplications = (roundId, univId) =>
  axios.get('/api/applications', { params: { round_id: roundId, univ_id: univId || undefined } }).then(r => r.data)
export const abandonApplication = (sid, uid, rid) =>
  axios.put(`/api/applications/${sid}/${uid}/${rid}/abandon`)

// ── 현재 라운드 (공용) ─────────────────────────────────────────
export const getCurrentRound = () => axios.get('/api/rounds/current').then(r => r.data)

// ── 결과 Excel 내보내기 ────────────────────────────────────────
export const exportResultsExcel = (roundId) =>
  axios.get(`/api/rounds/${roundId}/results/export`, { responseType: 'blob' })

// ── 관리자 비밀번호 변경 ───────────────────────────────────────
export const changeAdminPassword = (currentPassword, newPassword) =>
  axios.put('/api/auth/admin/password', { current_password: currentPassword, new_password: newPassword })

// ── 점수 미리보기 ──────────────────────────────────────────────
export const scorePreview = (studentId, univId) =>
  axios.get('/api/score-preview', { params: { student_id: studentId, univ_id: univId } }).then(r => r.data)
