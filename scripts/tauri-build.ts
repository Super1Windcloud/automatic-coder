import { spawnSync } from 'node:child_process'

const command = process.platform === 'win32' ? 'pnpm.cmd' : 'pnpm'
const hostMachineId = resolveHostMachineId()
const env = {
  ...process.env,
  INTERVIEW_CODER_HOST_MACHINE_ID: hostMachineId,
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

function resolveHostMachineId() {
  const cargoCommand = process.platform === 'win32' ? 'cargo.exe' : 'cargo'
  const result = spawnSync(
    cargoCommand,
    ['run', '-p', 'license_manager', '--bin', 'host_machine_id', '--quiet'],
    {
      cwd: `${process.cwd()}/src-tauri`,
      env: process.env,
      encoding: 'utf-8',
      stdio: ['ignore', 'pipe', 'inherit'],
    },
  )

  const machineId = result.stdout?.trim()
  if (result.status !== 0 || !machineId) {
    process.exit(result.status ?? 1)
  }

  return machineId
}
