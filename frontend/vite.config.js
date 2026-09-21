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
  // vitest — 기본은 DOM 없는 node 환경(순수 로직·소스 규칙 검사).
  // 렌더 테스트(tests/*.render.test.js, tests/smoke-render.test.js)만 파일 첫 줄의
  // `// @vitest-environment jsdom` 으로 jsdom 을 쓴다. 외관은 단언하지 않고
  // "화면이 열리는가"만 본다 — src/docs/13_frontend_pitfalls.md §4.
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
