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
    // 렌더 테스트는 파일마다 큰 SFC 를 처음 import 하며 변환한다. 메모리가 적은 PC 에서
    // 전체를 돌리면 이 첫 import 가 기본 5초를 넘겨, 결함 없이 시간 초과로 빨개졌다
    // (2026-10-06 감사 F-9 — 같은 파일을 단독으로 돌리면 통과했다). 넉넉히 둔다.
    testTimeout: 30000,
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
