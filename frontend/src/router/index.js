import { createRouter, createWebHistory } from 'vue-router'
import { useAuthStore } from '../stores/auth'

const routes = [
  { path: '/', redirect: '/login' },
  {
    path: '/server-error',
    component: () => import('../views/ServerErrorView.vue'),
    // 인증·초기화 가드 없이 항상 표시 가능해야 함
  },
  {
    path: '/welcome',
    component: () => import('../views/WelcomeView.vue'),
    // 가드는 beforeEach에서 일괄 처리
  },
  {
    path: '/login',
    component: () => import('../views/LoginView.vue'),
    beforeEnter: () => {
      const auth = useAuthStore()
      if (auth.isAdmin) return '/admin'
      if (auth.isTeacher) return '/teacher'
    },
  },
  {
    path: '/admin',
    component: () => import('../views/AdminView.vue'),
    meta: { requiresAdmin: true },
  },
  {
    path: '/teacher',
    component: () => import('../views/TeacherView.vue'),
    meta: { requiresTeacher: true },
  },
]

const router = createRouter({
  history: createWebHistory(),
  routes,
})

router.beforeEach(async to => {
  // 서버 오류 뷰는 checkStatus API 호출 없이 즉시 표시
  // (checkStatus가 또 503을 받아 무한 리다이렉트 방지)
  if (to.path === '/server-error') return true

  const auth = useAuthStore()

  // 초기화 상태를 아직 모르면 서버에 확인
  await auth.checkStatus()

  // 미초기화 → /welcome 강제 이동
  if (auth.initialized === false && to.path !== '/welcome') {
    return '/welcome'
  }

  // 초기화 완료 상태에서 /welcome 접근 → /login
  if (auth.initialized === true && to.path === '/welcome') {
    return '/login'
  }

  // 인증 가드
  if (to.meta.requiresAdmin && !auth.isAdmin) return '/login'
  if (to.meta.requiresTeacher && !auth.isTeacher) return '/login'
})

export default router
