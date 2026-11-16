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

  // 담임 신원 — 화면의 졸업생/일반 담임 분기는 이 값 하나만 본다(이슈 #49).
  // 'graduate' = 백엔드가 준 grade 0·class_no 0, 'homeroom' = 둘 다 양의 정수,
  // 'invalid' = role 은 teacher 인데 그 외(null 포함). teacher 가 아니면 null.
  // grade 가 0 이 아니라는 이유만으로 일반 담임으로 떨어지지 않게 한다(Fail-Fast).
  // 탭 안의 분기는 graduate 아니면 일반 담임 쪽이다 — invalid·null 상태를 막는 것은
  // TeacherView 의 신원 관문 하나다.
  const teacherKind = computed(() => {
    if (role.value !== 'teacher') return null
    const g = grade.value, c = classNo.value
    if (g === 0 && c === 0) return 'graduate'
    if (Number.isInteger(g) && g > 0 && Number.isInteger(c) && c > 0) return 'homeroom'
    return 'invalid'
  })

  // 복원 검증: 저장소가 반쪽(role=teacher 인데 학년·반이 없거나 어긋남)이면 인증 상태를
  // 전부 지운다. 반쪽 상태로 담임 화면에 들어가지 않는다.
  if (teacherKind.value === 'invalid') _clear()

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
          logout()
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
    // 계약 기록이지 방어선이 아니다 — 백엔드 TeacherTokenResponse 는 grade·class_no 가
    // i64 라 여기 닿지 않는다. 닿는다면 상태를 쓰지 않고 실패시킨다.
    if (!Number.isInteger(res.data?.grade) || !Number.isInteger(res.data?.class_no)) {
      throw new Error('로그인 응답에 학년·반 정보가 없습니다.')
    }
    token.value = res.data.token
    role.value = 'teacher'
    grade.value = res.data.grade
    classNo.value = res.data.class_no
    teacherName.value = res.data.teacher_name || null
    _persist()
  }

  function logout() {
    _clear()
  }

  function _clear() {
    token.value = null
    role.value = null
    grade.value = null
    classNo.value = null
    teacherName.value = null
    _persist()
  }

  return {
    token, role, grade, classNo, teacherName, initialized,
    isAdmin, isTeacher, teacherKind,
    checkStatus, loginAdmin, loginTeacher, logout,
  }
})
