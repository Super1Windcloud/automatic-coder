import fs from 'node:fs'
import path from 'node:path'
import { spawnSync } from 'node:child_process'

const rootDir = process.cwd()
const tauriCommand = resolveTauriCommand()
const hostMachineId = resolveHostMachineId()
const env = {
  ...process.env,
  INTERVIEW_CODER_HOST_MACHINE_ID: hostMachineId,
}

const result = spawnSync(tauriCommand.command, tauriCommand.args, {
  cwd: rootDir,
  env,
  stdio: 'inherit',
  shell: tauriCommand.shell,
})

if (result.status !== 0) {
  process.exit(result.status ?? 1)
}

syncBundleArtifacts()
process.exit(0)

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

function resolveTauriCommand() {
  const tauriBin = path.join(
    rootDir,
    'node_modules',
    '.bin',
    process.platform === 'win32' ? 'tauri.cmd' : 'tauri',
  )

  if (fs.existsSync(tauriBin)) {
    return {
      command: process.platform === 'win32' ? 'cmd.exe' : tauriBin,
      args: process.platform === 'win32' ? ['/d', '/s', '/c', tauriBin, 'build'] : ['build'],
      shell: false,
    }
  }

  return {
    command: process.platform === 'win32' ? 'pnpm.cmd' : 'pnpm',
    args: ['tauri', 'build'],
    shell: process.platform === 'win32',
  }
}

function syncBundleArtifacts() {
  const sourceDir = path.join(rootDir, 'src-tauri', 'target', 'release', 'bundle')
  const targetDir = path.join(rootDir, 'bundle')

  fs.rmSync(targetDir, { recursive: true, force: true })
  fs.cpSync(sourceDir, targetDir, { recursive: true })

  const macosApp = path.join(targetDir, 'macos', 'Interview-Coder.app')
  fs.rmSync(macosApp, { recursive: true, force: true })
}
