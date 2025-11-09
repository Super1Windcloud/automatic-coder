import { invoke } from '@tauri-apps/api/core'

type LogLevel = 'info' | 'warn' | 'error'

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

function appendLog(message: string, err?: unknown) {
  const payload = err ? `${message} | detail: ${formatError(err)}` : message
  invoke('append_app_log', { message: payload }).catch((logErr) => {
    console.error('写入日志失败：', logErr)
  })
}

function logWithLevel(level: LogLevel, message: string, err?: unknown) {
  const consoleFn =
    level === 'error'
      ? console.error
      : level === 'warn'
        ? console.warn
        : console.log
  if (err !== undefined) {
    consoleFn(message, err)
  } else {
    consoleFn(message)
  }
  appendLog(message, err)
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
