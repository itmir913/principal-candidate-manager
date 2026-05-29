import axios from 'axios'

export const getClasses = () => axios.get('/api/classes').then(r => r.data)

export const upsertClass = (grade, classNo, body) =>
  axios.put(`/api/classes/${grade}/${classNo}`, body).then(r => r.data)

export const getAreas = () => axios.get('/api/areas').then(r => r.data)

export const createArea = (body) => axios.post('/api/areas', body).then(r => r.data)

export const updateArea = (id, body) => axios.put(`/api/areas/${id}`, body).then(r => r.data)

export const deleteArea = (id) => axios.delete(`/api/areas/${id}`)

export const getRangeTable = (id) =>
  axios.get(`/api/areas/${id}/range-table`).then(r => r.data)

export const putRangeTable = (id, rows) =>
  axios.put(`/api/areas/${id}/range-table`, rows)

export const getCategoryMap = (id) =>
  axios.get(`/api/areas/${id}/category-map`).then(r => r.data)

export const putCategoryMap = (id, rows) =>
  axios.put(`/api/areas/${id}/category-map`, rows)

export const getUniversities = () => axios.get('/api/universities').then(r => r.data)

export const createUniversity = (body) =>
  axios.post('/api/universities', body).then(r => r.data)

export const updateUniversity = (id, body) =>
  axios.put(`/api/universities/${id}`, body).then(r => r.data)

export const deleteUniversity = (id) => axios.delete(`/api/universities/${id}`)
