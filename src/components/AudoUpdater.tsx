import { WebviewWindow } from '@tauri-apps/api/webviewWindow'
import { check } from '@tauri-apps/plugin-updater'
import { useEffect, useRef } from 'react'
import { logError, logInfo, logWarn } from '@/lib/logger.ts'
import { ignoreMouseEvents, startMouseEvents } from '@/lib/system.ts'

const UPDATE_WINDOW_LABEL = 'updater'

export async function openUpdateWindow() {
  if (typeof WebviewWindow.getByLabel === 'function') {
    try {
      const existing = await WebviewWindow.getByLabel(UPDATE_WINDOW_LABEL)
      if (existing) {
        await existing.close()
      }
    } catch (error) {
      logWarn('关闭旧的更新窗口失败', error)
    }
  }

  const updater = new WebviewWindow(UPDATE_WINDOW_LABEL, {
    title: '应用更新',
    url: '/#/update', // hash router path
    width: 480,
    height: 320,
    resizable: false,
    center: true,
    decorations: false,
    alwaysOnTop: true,
    transparent: false,
    focus: true,
    visible: false,
    devtools: true,
  })

  await updater.once('tauri://created', async () => {
    logInfo('更新窗口已创建')
    try {
      await startMouseEvents(UPDATE_WINDOW_LABEL)
    } catch (error) {
      logWarn('无法开启更新窗口的鼠标事件', error)
    }
    await updater.show().catch((err) => {
      logError('更新窗口显示失败', err)
    })
    await updater.setFocus().catch((err) => {
      logError('更新窗口获取焦点失败', err)
    })
  })

  await updater.once('tauri://error', (e) => {
    logError('更新窗口创建失败', e)
  })

  return updater
}

export default function AutoUpdater() {
  const hasCheckedRef = useRef(false)

  useEffect(() => {
    const doCheck = async () => {
      try {
        const current = WebviewWindow.getCurrent()
        if (current.label !== 'main') {
          await startMouseEvents(current.label)
          return
        }

        if (hasCheckedRef.current) {
          return
        }
        hasCheckedRef.current = true

        const update = await check()

        if (!update) {
          logInfo('✅ 当前已是最新版本')
          return
        }

        logInfo(`发现新版本 ${update.version}`)
        openUpdateWindow()
          .catch((err) => {
            logError('打开更新窗口失败', err)
          })
          .finally(async () => {
            await ignoreMouseEvents('main')
          })
      } catch (err) {
        logError('检查更新失败', err)
      }
    }
    doCheck()
  }, [])

  return null
}
