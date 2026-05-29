const { execSync } = require('child_process')
const path = require('path')

// ── 추가할 Cargo 도구는 여기에 ──────────────────────────────────
const CARGO_TOOLS = [
  'cargo-watch',
]

// ── 추가할 프론트엔드 전역 도구는 여기에 (npm -g install) ────────
const NPM_GLOBAL_TOOLS = [
  // 예: 'vite'
]

// ── 실행 헬퍼 ─────────────────────────────────────────────────
function run(cmd, opts = {}) {
  console.log(`\n> ${cmd}`)
  execSync(cmd, { stdio: 'inherit', ...opts })
}

const root = path.resolve(__dirname)

console.log('=== npm 패키지 설치 (루트) ===')
run('npm install', { cwd: root })

console.log('\n=== npm 패키지 설치 (frontend) ===')
run('npm install', { cwd: path.join(root, 'frontend') })

if (NPM_GLOBAL_TOOLS.length > 0) {
  console.log('\n=== npm 전역 도구 설치 ===')
  for (const tool of NPM_GLOBAL_TOOLS) {
    run(`npm install -g ${tool}`)
  }
}

console.log('\n=== Cargo 도구 설치 ===')
for (const tool of CARGO_TOOLS) {
  run(`cargo install ${tool}`)
}

console.log('\n✓ 셋업 완료. npm run dev 로 개발 서버를 시작하세요.')
