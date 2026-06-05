import { ref, computed } from 'vue'
import { defineStore } from 'pinia'
import axios from 'axios'

let interceptorRegistered = false
let _router = null

export function setRouter(router) {
  _router = router
}

export const useAuthStore = defineStore('auth', () => {
  const token = ref(localStorage.getItem('pcm_token') || null)
  const role = ref(localStorage.getItem('pcm_role') || null)
  const grade = ref(localStorage.getItem('pcm_grade') != null ? Number(localStorage.getItem('pcm_grade')) : null)
  const classNo = ref(localStorage.getItem('pcm_class_no') != null ? Number(localStorage.getItem('pcm_class_no')) : null)
  const teacherName = ref(localStorage.getItem('pcm_teacher_name') || null)

  // null = 아직 서버에 물어보지 않음 / false = 미설정 / true = 설정 완료
  const initialized = ref(null)

  const isAdmin = computed(() => role.value === 'admin')
  const isTeacher = computed(() => role.value === 'teacher')

  if (!interceptorRegistered) {
    interceptorRegistered = true
    axios.interceptors.request.use(config => {
      if (token.value) {
        config.headers.Authorization = `Bearer ${token.value}`
      }
      return config
    })
    axios.interceptors.response.use(
      res => res,
      err => {
        if (err.response?.status === 401) {
          token.value = null
          role.value = null
          grade.value = null
          classNo.value = null
          _persist()
          _router?.push('/login')
        }
        if (
          err.response?.status === 503 &&
          _router?.currentRoute.value.path !== '/server-error'
        ) {
          const d = err.response.data ?? {}
          _router?.replace({
            path: '/server-error',
            query: {
              code:    d.code    ?? 'SERVER_ERROR',
              message: d.message ?? '서버 오류가 발생했습니다.',
              ...(d.db_ver  != null ? { db_ver:  d.db_ver  } : {}),
              ...(d.app_ver != null ? { app_ver: d.app_ver } : {}),
            },
          })
        }
        return Promise.reject(err)
      }
    )
  }

  async function checkStatus() {
    if (initialized.value !== null) return  // 이미 알면 재요청 안 함
    try {
      const res = await axios.get('/api/auth/admin/status')
      initialized.value = res.data.initialized
    } catch {
      // 서버 오류 시 null 유지 → 가드가 통과 허용
    }
  }

  function _persist() {
    if (token.value) localStorage.setItem('pcm_token', token.value)
    else localStorage.removeItem('pcm_token')

    if (role.value) localStorage.setItem('pcm_role', role.value)
    else localStorage.removeItem('pcm_role')

    if (grade.value != null) localStorage.setItem('pcm_grade', grade.value)
    else localStorage.removeItem('pcm_grade')

    if (classNo.value != null) localStorage.setItem('pcm_class_no', classNo.value)
    else localStorage.removeItem('pcm_class_no')

    if (teacherName.value) localStorage.setItem('pcm_teacher_name', teacherName.value)
    else localStorage.removeItem('pcm_teacher_name')
  }

  async function loginAdmin(password) {
    const res = await axios.post('/api/auth/admin', { password })
    token.value = res.data.token
    role.value = 'admin'
    grade.value = null
    classNo.value = null
    initialized.value = true  // 로그인 성공 = 비밀번호 확실히 존재
    _persist()
  }

  async function loginTeacher(gradeVal, classNoVal, password) {
    const res = await axios.post('/api/auth/teacher', {
      grade: gradeVal,
      class_no: classNoVal,
      password,
    })
    token.value = res.data.token
    role.value = 'teacher'
    grade.value = res.data.grade
    classNo.value = res.data.class_no
    teacherName.value = res.data.teacher_name || null
    _persist()
  }

  function logout() {
    token.value = null
    role.value = null
    grade.value = null
    classNo.value = null
    teacherName.value = null
    _persist()
  }

  return {
    token, role, grade, classNo, teacherName, initialized,
    isAdmin, isTeacher,
    checkStatus, loginAdmin, loginTeacher, logout,
  }
})
