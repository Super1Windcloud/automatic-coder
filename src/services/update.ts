import { check } from '@tauri-apps/plugin-updater'
import { openUpdateWindow } from '@/components/AudoUpdater.tsx'
import { logError, logInfo } from '@/lib/logger.ts'
import { ignoreMouseEvents, startMouseEvents } from '@/lib/system.ts'

export async function checkCurrentAppUpdate() {
  try {
    startMouseEvents('updater').catch((err) => {
      logError('start mouse events for updater window failed', err)
    })
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
