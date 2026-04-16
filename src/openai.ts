import { invoke } from '@tauri-apps/api/core'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
import { createScopedLogger } from '@/lib/logger.ts'

const logger = createScopedLogger('openai-config')
const closeButton = document.getElementById('close-button')
const saveButton = document.getElementById('save-button')
const baseUrlInput = document.getElementById('base-url-input') as HTMLInputElement | null
const apiKeyInput = document.getElementById('api-key-input') as HTMLInputElement | null
const modelInput = document.getElementById('model-input') as HTMLInputElement | null
const statusBox = document.getElementById('status-box') as HTMLDivElement | null

function updateStatus(message: string, isError = false) {
  if (!statusBox) {
    return
  }
  statusBox.textContent = message
  statusBox.className = isError ? 'status error' : 'status'
  statusBox.style.display = message ? 'block' : 'none'
}

closeButton?.addEventListener('click', async () => {
  try {
    await getCurrentWebviewWindow().close()
  } catch (err) {
    logger.error('关闭自定义 OpenAI 配置窗口失败', err)
  }
})

saveButton?.addEventListener('click', async () => {
  const apiKey = apiKeyInput?.value.trim() || ''
  const baseUrl = baseUrlInput?.value.trim() || ''
  const model = modelInput?.value.trim() || ''

  try {
    await invoke('save_custom_openai_config', {
      apiKey,
      baseUrl,
      model,
    })
    updateStatus('保存成功。托盘开启“启用自定义 OpenAI 兼容 API”后生效。')
  } catch (err) {
    logger.error('保存自定义 OpenAI 配置失败', err)
    updateStatus('保存失败，请检查配置格式。', true)
  }
})

async function loadPreferences() {
  try {
    const configStr = await invoke<string>('get_store_config')
    const config = JSON.parse(configStr) as {
      custom_openai_api_key?: string
      custom_openai_base_url?: string
      custom_openai_model?: string
    }
    if (baseUrlInput) {
      baseUrlInput.value = config.custom_openai_base_url || 'https://api.openai.com/v1'
    }
    if (apiKeyInput) {
      apiKeyInput.value = config.custom_openai_api_key || ''
    }
    if (modelInput) {
      modelInput.value = config.custom_openai_model || 'gpt-4o'
    }
  } catch (err) {
    logger.error('加载自定义 OpenAI 配置失败', err)
    updateStatus('加载配置失败。', true)
  }
}

document.addEventListener('DOMContentLoaded', async () => {
  await loadPreferences()
})
