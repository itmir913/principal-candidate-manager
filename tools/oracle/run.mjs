// 독립 오라클 대조 러너 — `npm run test:oracle` 의 진입점.
//
// 네 단계를 순서대로 돌린다. 하나라도 실패하면 즉시 비정상 종료한다.
//   1) generate.py   시나리오 생성 (scenarios.json)
//   2) audit_oracle_dump  구현 실측값 덤프 (actual.json)
//        - PCM_ORACLE_DIR 이 없으면 이 테스트는 no-op 이다. 여기서만 설정한다.
//          (일반 `cargo test` 가 파일을 만들지 않게 하려는 의도된 설계)
//   3) compare.py    오라클 예측 vs 실측 대조 (불일치 시 exit 1)
//   4) front_check.mjs  프론트 파생값 대조 (알려진 결함은 XFAIL)
//
// scenarios.json / actual.json 은 생성물이라 저장소에 넣지 않는다(.gitignore).
// 이 러너를 거치면 항상 현재 코드 기준으로 다시 만들어진다.

import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import path from 'node:path'

const HERE = path.dirname(fileURLToPath(import.meta.url))
const REPO = path.resolve(HERE, '..', '..')

/** 파이썬 실행 파일 이름은 환경마다 다르다(Windows: python, 대부분의 CI: python3). */
function pythonCmd() {
  for (const cmd of ['python', 'python3']) {
    const r = spawnSync(cmd, ['--version'], { stdio: 'ignore' })
    if (r.status === 0) return cmd
  }
  console.error('python 을 찾을 수 없습니다. 오라클 대조에는 Python 3 가 필요합니다.')
  process.exit(1)
}

// Windows 러너의 파이썬은 stdout 인코딩이 cp1252 라서 한국어를 print 하는 순간
// UnicodeEncodeError 로 죽는다(GitHub Actions windows-latest 에서 실제로 터졌다).
// 로컬 콘솔이 UTF-8 이면 재현되지 않으므로 러너에서만 드러난다 — 여기서 못 박는다.
const PY_ENV = { PYTHONUTF8: '1', PYTHONIOENCODING: 'utf-8' }

function run(label, cmd, args, opts = {}) {
  console.log(`\n=== ${label} ===`)
  const r = spawnSync(cmd, args, { stdio: 'inherit', shell: false, ...opts })
  if (r.error) {
    console.error(`${label} 실행 실패: ${r.error.message}`)
    process.exit(1)
  }
  if (r.status !== 0) {
    console.error(`\n${label} 실패 (exit ${r.status})`)
    process.exit(r.status ?? 1)
  }
}

const py = pythonCmd()

run('1/4 시나리오 생성', py, [path.join(HERE, 'generate.py')], {
  cwd: HERE,
  env: { ...process.env, ...PY_ENV },
})
run('2/4 구현 실측 덤프', 'cargo', ['test', '--test', 'audit_oracle_dump', '--', '--nocapture'], {
  cwd: REPO,
  env: { ...process.env, PCM_ORACLE_DIR: HERE },
})
run('3/4 오라클 대조', py, [path.join(HERE, 'compare.py')], {
  cwd: HERE,
  env: { ...process.env, ...PY_ENV },
})
run('4/4 프론트 대조', process.execPath, [path.join(HERE, 'front_check.mjs')], { cwd: HERE })

console.log('\n오라클 대조 전 단계 통과')
