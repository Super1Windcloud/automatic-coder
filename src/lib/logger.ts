import { isTauri } from '@tauri-apps/api/core'
import {
  error as tauriError,
  info as tauriInfo,
  warn as tauriWarn,
} from '@tauri-apps/plugin-log'

type LogLevel = 'info' | 'warn' | 'error'
type LogWriter = (message: string) => Promise<void>
type ScopedLogger = {
  info: (message: string, err?: unknown) => void
  warn: (message: string, err?: unknown) => void
  error: (message: string, err?: unknown) => void
}

const tauriLoggers: Record<LogLevel, LogWriter> = {
  info: tauriInfo,
  warn: tauriWarn,
  error: tauriError,
}

export function formatError(err: unknown): string {
  if (err instanceof Error) {
    return `${err.name}: ${err.message}${err.stack ? `\n${err.stack}` : ''}`
  }
  if (typeof err === 'string') {
    return err
  }
  if (typeof err === 'object' && err !== null) {
    try {
      return JSON.stringify(err)
    } catch (jsonError) {
      console.warn('无法序列化错误对象：', jsonError)
    }
  }
  try {
    return JSON.stringify(err)
  } catch (stringifyError) {
    console.warn('无法序列化未知错误：', stringifyError)
    return String(err)
  }
}

function buildMessage(message: string, err?: unknown) {
  return err ? `${message}\n${formatError(err)}` : message
}

function writeConsole(level: LogLevel, message: string, err?: unknown) {
  const consoleFn =
    level === 'error'
      ? console.error
      : level === 'warn'
        ? console.warn
        : console.info

  if (err !== undefined) {
    consoleFn(message, err)
    return
  }

  consoleFn(message)
}

function writeTauriLog(level: LogLevel, message: string, err?: unknown) {
  if (!isTauri()) {
    return
  }

  tauriLoggers[level](buildMessage(message, err)).catch((logErr) => {
    console.error('写入 Tauri 日志失败', logErr)
  })
}

function logWithLevel(level: LogLevel, message: string, err?: unknown) {
  writeConsole(level, message, err)
  writeTauriLog(level, message, err)
}

function withScope(scope: string, message: string) {
  return `[${scope}] ${message}`
}

export function logInfo(message: string, err?: unknown) {
  logWithLevel('info', message, err)
}

export function logWarn(message: string, err?: unknown) {
  logWithLevel('warn', message, err)
}

export function logError(message: string, err?: unknown) {
  logWithLevel('error', message, err)
}

export function createScopedLogger(scope: string): ScopedLogger {
  return {
    info: (message, err) => logInfo(withScope(scope, message), err),
    warn: (message, err) => logWarn(withScope(scope, message), err),
    error: (message, err) => logError(withScope(scope, message), err),
  }
}
