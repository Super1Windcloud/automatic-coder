import { invoke } from '@tauri-apps/api/core'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
import { logError } from '@/lib/logger.ts'

type ActivationAttemptPayload = {
  success: boolean
  status: string
  activated: boolean
}

const form = document.getElementById(
  'activation-form',
) as HTMLFormElement | null
const textarea = document.getElementById(
  'activation-code',
) as HTMLTextAreaElement | null
const statusBox = document.getElementById(
  'activation-status',
) as HTMLDivElement | null
const submitButton = document.querySelector<HTMLButtonElement>('button.submit')

function updateStatus(message: string, tone: 'info' | 'error' | 'success') {
  if (!statusBox) {
    return
  }
  statusBox.textContent = message
  statusBox.classList.remove('error', 'success')
  if (tone === 'error') {
    statusBox.classList.add('error')
  } else if (tone === 'success') {
    statusBox.classList.add('success')
  }
  statusBox.style.display = message ? 'block' : 'none'
}

async function closeAndLaunch() {
  try {
    await getCurrentWebviewWindow().close()
  } catch (err) {
    logError('failed to close activation window', err)
  }
}

async function ensureState() {
  try {
    const activated = await invoke<boolean>('get_activation_status')
    if (activated) {
      await closeAndLaunch()
    }
  } catch (err) {
    logError('failed to query activation status', err)
    updateStatus('无法验证激活状态，请重试或联系支持。', 'error')
  }
}

async function handleSubmit(event: Event) {
  event.preventDefault()
  if (!textarea) {
    return
  }
  const code = textarea.value.trim()
  console.log(code)
  if (!code) {
    updateStatus('请输入激活码。', 'error')
    return
  }
  try {
    submitButton?.setAttribute('disabled', 'true')
    updateStatus('正在验证，请稍候…', 'info')
    const payload = await invoke<ActivationAttemptPayload>(
      'submit_activation_code',
      {
        encryptedCode: code,
      },
    )

    if (payload.success && payload.activated) {
      updateStatus('激活成功，正在启动应用…', 'success')
      setTimeout(() => {
        closeAndLaunch().catch((err) =>
          logError('failed to close activation window', err),
        )
      }, 300)
      return
    }

    switch (payload.status) {
      case 'already_used':
        updateStatus('该激活码已被使用，请联系发行方获取新的激活码。', 'error')
        break
      case 'not_found':
        updateStatus('激活码无效，请确认后再次尝试。', 'error')
        break
      case 'pending_initialisation':
        updateStatus('激活系统尚未就绪，请稍后重试。', 'error')
        break
      case 'disabled':
        updateStatus('当前版本未启用激活校验。', 'info')
        setTimeout(() => {
          closeAndLaunch().catch((err) =>
            logError('failed to close activation window', err),
          )
        }, 200)
        break
      default:
        updateStatus('激活失败，请稍后重试或联系支持。', 'error')
        break
    }
  } catch (err) {
    logError('submit activation error', err)
    updateStatus('验证过程中出现错误，激活码无效。', 'error')
  } finally {
    submitButton?.removeAttribute('disabled')
  }
}

document.addEventListener('DOMContentLoaded', async () => {
  await ensureState()
  textarea?.focus()
  form?.addEventListener('submit', handleSubmit)
})
