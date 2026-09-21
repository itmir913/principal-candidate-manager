import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import tailwindcss from '@tailwindcss/vite'

export default defineConfig({
  plugins: [
    vue(),
    tailwindcss(),
  ],
  build: {
    rollupOptions: {
      input: {
        main: 'index.html',
        manual: 'manual.html',
      },
    },
  },
  // vitest — 순수 로직(점수 표기·오류 문자열·라벨) 전용이라 DOM 이 필요 없다.
  // 컴포넌트 렌더 테스트는 의도적으로 도입하지 않는다(화면 확인은 사람 몫 —
  // src/docs/13_frontend_pitfalls.md).
  test: {
    environment: 'node',
    include: ['src/**/*.test.js', 'tests/**/*.test.js'],
  },
  server: {
    port: 5173,
    host: true,
    proxy: {
      '/api': {
        target: 'http://127.0.0.1:8080',
        changeOrigin: true,
      },
    },
  },
})
