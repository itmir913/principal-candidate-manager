import axios from 'axios'

export const getClasses = () => axios.get('/api/classes').then(r => r.data)

export const upsertClass = (grade, classNo, body) =>
  axios.put(`/api/classes/${grade}/${classNo}`, body).then(r => r.data)

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

export const downloadStudentTemplate = () =>
  axios.get('/api/students/template', { responseType: 'blob' })

export const exportStudents = () =>
  axios.get('/api/students/export', { responseType: 'blob' })

export const importStudents = (file) => {
  const fd = new FormData()
  fd.append('file', file)
  return axios.post('/api/students/import', fd).then(r => r.data)
}

// ── 대학 관리 ──────────────────────────────────────────────────
export const getUniversities = () => axios.get('/api/universities').then(r => r.data)

export const createUniversity = (body) =>
  axios.post('/api/universities', body).then(r => r.data)

export const updateUniversity = (id, body) =>
  axios.put(`/api/universities/${id}`, body).then(r => r.data)

export const deleteUniversity = (id) => axios.delete(`/api/universities/${id}`)
