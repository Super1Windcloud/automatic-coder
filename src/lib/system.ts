import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import { WebviewWindow } from '@tauri-apps/api/webviewWindow'
import {
  getCurrentWindow,
  LogicalPosition,
  LogicalSize,
} from '@tauri-apps/api/window'
import { createScopedLogger } from '@/lib/logger.ts'
import { useAppStateStoreWithNoHook } from '@/store'

let lastHeight = 0
const logger = createScopedLogger('system')

export async function resetWindow(offsetCallback: () => void) {
  await getCurrentWindow().setSize(new LogicalSize(800, 50))
  await getCurrentWindow().setPosition(new LogicalPosition(100, 50))
  offsetCallback()
  useAppStateStoreWithNoHook.getState().updateCurrentScreenShotPath('')
  useAppStateStoreWithNoHook.getState().updateStartShowSolution(false)
}

export async function showSolutionWindow() {
  const contentHeight = await getWebViewHeight()
  const window = getCurrentWindow()

  if (Math.abs(contentHeight - lastHeight) < 10) return // 🔥 忽略微小变化
  lastHeight = contentHeight

  await window.setSize(new LogicalSize(800, contentHeight))
}

async function resolveWindow(label?: string) {
  if (!label) {
    return getCurrentWindow()
  }
  try {
    const win = await WebviewWindow.getByLabel(label)
    if (win) {
      return win
    }
  } catch (error) {
    logger.error(`查找窗口 ${label ?? 'unknown'} 失败`, error)
  }
  return null
}

export async function ignoreMouseEvents(label?: string) {
  const win = await resolveWindow(label)
  if (!win) return
  await win.setIgnoreCursorEvents(true)
}

export async function startMouseEvents(label?: string) {
  const win = await resolveWindow(label)
  if (!win) return
  await win.setIgnoreCursorEvents(false)
}
async function getWebViewHeight() {
  return document.documentElement.scrollHeight
}

export async function enableMouseEventsForComponent(id: string) {
  const element = document.getElementById(id)
  if (element) {
    element.style.pointerEvents = 'auto' // 启用该组件的鼠标事件
  }
}

export async function getScreenCaptureToLocalPath() {
  try {
    const filePath = (await invoke<string>('get_screen_capture_to_path')) as string
    const imagePath = convertFileSrc(filePath.replace('\\', '/'))
    useAppStateStoreWithNoHook.getState().updateCurrentScreenShotPath(imagePath)
  } catch (error) {
    logger.error('截图失败', error)
    useAppStateStoreWithNoHook.getState().updateCurrentScreenShotPath('')
  }
}

export async function getScreenCaptureToBlobUrl(source: string = '截图') {
  try {
    logger.info(`${source} 请求开始`)
    const bytes = await invoke<number[]>('get_screen_capture_to_bytes')
    const blob = new Blob([new Uint8Array(bytes)], { type: 'image/png' })
    const url = URL.createObjectURL(blob)
    useAppStateStoreWithNoHook.getState().updateCurrentScreenShotPath(url)
  } catch (error) {
    logger.error(`${source} 失败`, error)
    useAppStateStoreWithNoHook.getState().updateCurrentScreenShotPath('')
  }
}
