import { spawnSync } from 'node:child_process'

const args = process.argv.slice(2)
const isHostBuild = args.includes('--host')

const command = process.platform === 'win32' ? 'pnpm.cmd' : 'pnpm'
const env = {
  ...process.env,
  INTERVIEW_CODER_HOST_BUILD: isHostBuild ? '1' : '0',
}

const result = spawnSync(command, ['tauri', 'build'], {
  cwd: process.cwd(),
  env,
  stdio: 'inherit',
})

if (typeof result.status === 'number') {
  process.exit(result.status)
}

process.exit(1)
