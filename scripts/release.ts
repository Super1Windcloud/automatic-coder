import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

type SupportedPlatform = 'windows' | 'macos';

const rootDir = process.cwd();
const packageJsonPath = path.join(rootDir, 'package.json');
const cargoTomlPath = path.join(rootDir, 'src-tauri', 'Cargo.toml');
const tauriConfigPath = path.join(rootDir, 'src-tauri', 'tauri.conf.json');

function readJson<T>(filePath: string): T {
  return JSON.parse(fs.readFileSync(filePath, 'utf-8')) as T;
}

function writeJson(filePath: string, value: unknown) {
  fs.writeFileSync(filePath, `${JSON.stringify(value, null, 2)}\n`, 'utf-8');
}

function bumpPatchVersion(version: string) {
  const segments = version.split('.');
  if (segments.length !== 3) {
    throw new Error(`Unsupported version format: ${version}`);
  }

  const [major, minor, patch] = segments.map((segment) => Number(segment));
  if ([major, minor, patch].some((segment) => Number.isNaN(segment))) {
    throw new Error(`Version contains non-numeric segments: ${version}`);
  }

  return `${major}.${minor}.${patch + 1}`;
}

function updateCargoTomlVersion(fileContent: string, nextVersion: string) {
  const versionLinePattern = /^version = "(.+)"$/m;

  if (!versionLinePattern.test(fileContent)) {
    throw new Error('Unable to find version field in src-tauri/Cargo.toml');
  }

  return fileContent.replace(versionLinePattern, `version = "${nextVersion}"`);
}

function resolvePlatform(): SupportedPlatform {
  const arg = process.argv[2]?.toLowerCase();
  if (arg === 'windows' || arg === 'macos') {
    return arg;
  }

  if (process.platform === 'win32') {
    return 'windows';
  }

  if (process.platform === 'darwin') {
    return 'macos';
  }

  throw new Error(
    'Unsupported platform. Pass "windows" or "macos" explicitly.',
  );
}

function run(command: string, args: string[]) {
  execFileSync(command, args, {
    cwd: rootDir,
    stdio: 'inherit',
    shell: process.platform === 'win32',
  });
}

function getPublishScript(platform: SupportedPlatform) {
  return platform === 'windows'
    ? 'scripts/publish_windows.ts'
    : 'scripts/publish_macos.ts';
}

function syncVersions(nextVersion: string) {
  const packageJson = readJson<Record<string, unknown>>(packageJsonPath);
  packageJson.version = nextVersion;
  writeJson(packageJsonPath, packageJson);

  const tauriConfig = readJson<Record<string, unknown>>(tauriConfigPath);
  tauriConfig.version = nextVersion;
  writeJson(tauriConfigPath, tauriConfig);

  const cargoToml = fs.readFileSync(cargoTomlPath, 'utf-8');
  const updatedCargoToml = updateCargoTomlVersion(cargoToml, nextVersion);
  fs.writeFileSync(cargoTomlPath, updatedCargoToml, 'utf-8');
}

function main() {
  const platform = resolvePlatform();
  const packageJson = readJson<{ version: string }>(packageJsonPath);
  const currentVersion = packageJson.version;
  const nextVersion = bumpPatchVersion(currentVersion);
  const publishScript = getPublishScript(platform);

  console.log(`Releasing ${platform} build: ${currentVersion} -> ${nextVersion}`);
  syncVersions(nextVersion);

  run('pnpm', ['tb']);
  run('pnpm', ['tsx', publishScript]);
}

main();
