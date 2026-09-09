import { createApp } from 'vue'
import { createPinia } from 'pinia'
import { VueQueryPlugin } from '@tanstack/vue-query'
import router from './router'
import './style.css'
import App from './App.vue'
import { setRouter } from './stores/auth.js'
import { useAppInfoStore } from './stores/appInfo.js'

createApp(App)
  .use(createPinia())
  .use(router)
  .use(VueQueryPlugin)
  .mount('#app')

setRouter(router)

// 제목·부제는 로그인 전 화면도 쓰므로 앱 시작 시 한 번 받아 둔다 (이슈 #23)
useAppInfoStore().load()
