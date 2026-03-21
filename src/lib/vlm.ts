import { invoke } from '@tauri-apps/api/core'
import { listen, UnlistenFn } from '@tauri-apps/api/event'
import { createScopedLogger } from '@/lib/logger.ts'

const logger = createScopedLogger('vlm')

export async function getScreenShotSolutionFromVLM(
  renderCallBack: (content: string) => void,
) {
  let content = ''
  const unlistenFn: UnlistenFn = await listen('completion_stream', (event) => {
    content += event.payload
    content = content
      .replace('<|begin_of_box|>', '')
      .replace('<|end_of_box|>', '')

    renderCallBack(content)
  })

  invoke('create_screenshot_solution_stream')
    .then(() => logger.info('截图方案生成成功'))
    .catch((err) => {
      logger.error('get solution error', err)
      unlistenFn()
    })
    .finally(() => {
      unlistenFn()
    })
  return unlistenFn
}
