import { getVersion } from '@tauri-apps/api/app'
import { check } from '@tauri-apps/plugin-updater'
import { openUpdateWindow } from '@/components/AudoUpdater.tsx'
import { createScopedLogger } from '@/lib/logger.ts'
import { ignoreMouseEvents, startMouseEvents } from '@/lib/system.ts'

const logger = createScopedLogger('update-service')

export async function checkCurrentAppUpdate() {
  try {
    startMouseEvents('updater').catch((err) => {
      logger.error('start mouse events for updater window failed', err)
    })
    const currentVersion = await getVersion()
    const update = await check()

    if (!update) {
      logger.info(
        `✅ 当前已是最新版本，当前版本 ${currentVersion}，远程版本 ${currentVersion}`,
      )
      return
    }

    logger.info(
      `发现新版本 ${update.version}，当前版本 ${currentVersion}，远程版本 ${update.version}`,
    )
    openUpdateWindow()
      .catch((err) => {
        logger.error('打开更新窗口失败', err)
      })
      .finally(async () => {
        await ignoreMouseEvents('main')
      })
  } catch (err) {
    logger.error('检查更新失败', err)
  }
}
