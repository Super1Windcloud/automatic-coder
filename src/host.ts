import { invoke } from '@tauri-apps/api/core'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
import { createScopedLogger } from '@/lib/logger.ts'

const logger = createScopedLogger('host-management')

const publicKeyBox = document.getElementById('public-key')
const revocationUrlBox = document.getElementById('revocation-url')
const closeButton = document.getElementById('close-button')

const machineIdInput = document.getElementById('machine-id-input') as HTMLInputElement | null
const licenseIdInput = document.getElementById('license-id-input') as HTMLInputElement | null
const expiresDaysInput = document.getElementById('expires-days-input') as HTMLInputElement | null
const customerInput = document.getElementById('customer-input') as HTMLInputElement | null
const issueButton = document.getElementById('issue-button')
const issueCopyButton = document.getElementById('issue-copy-button')
const issuedLicenseOutput = document.getElementById('issued-license-output') as HTMLTextAreaElement | null
const issueStatus = document.getElementById('issue-status') as HTMLDivElement | null

const revocationVersionInput = document.getElementById('revocation-version-input') as HTMLInputElement | null
const revokedIdsInput = document.getElementById('revoked-ids-input') as HTMLTextAreaElement | null
const previewIssuedLicenseIdsButton = document.getElementById('preview-issued-license-ids') as HTMLButtonElement | null
const licensePreview = document.getElementById('license-preview') as HTMLDivElement | null
const licensePreviewList = document.getElementById('license-preview-list') as HTMLDivElement | null
const signRevocationButton = document.getElementById('sign-revocation-button')
const revocationCopyButton = document.getElementById('revocation-copy-button')
const signedRevocationsOutput = document.getElementById('signed-revocations-output') as HTMLTextAreaElement | null
const revocationStatus = document.getElementById('revocation-status') as HTMLDivElement | null

type IssuedLicensePreview = {
  issued_at: number
  license_id: string
  machine_id: string
  customer: string | null
}

function setStatus(target: HTMLDivElement | null, message: string, isError = false) {
  if (!target) {
    return
  }
  target.textContent = message
  target.className = isError ? 'status error' : 'status'
  target.style.display = message ? 'block' : 'none'
}

function defaultLicenseId() {
  const now = new Date()
  const date = `${now.getFullYear()}${`${now.getMonth() + 1}`.padStart(2, '0')}${`${now.getDate()}`.padStart(2, '0')}`
  const time = `${`${now.getHours()}`.padStart(2, '0')}${`${now.getMinutes()}`.padStart(2, '0')}${`${now.getSeconds()}`.padStart(2, '0')}`
  return `lic_${date}_${time}`
}

function appendRevokedLicenseId(licenseId: string) {
  if (!revokedIdsInput) {
    return
  }
  const existing = revokedIdsInput.value
    .split(/\r?\n/)
    .map((item) => item.trim())
    .filter(Boolean)
  if (existing.includes(licenseId)) {
    setStatus(revocationStatus, `许可证 ${licenseId} 已在吊销列表中。`)
    return
  }
  revokedIdsInput.value = [...existing, licenseId].join('\n')
  setStatus(revocationStatus, `已追加许可证 ${licenseId}。`)
}

function renderIssuedLicensePreview(items: IssuedLicensePreview[]) {
  if (!licensePreview || !licensePreviewList) {
    return
  }

  licensePreviewList.innerHTML = ''
  if (!items.length) {
    const empty = document.createElement('div')
    empty.className = 'license-preview-empty'
    empty.textContent = '本地还没有已保存的许可证记录。'
    licensePreviewList.append(empty)
    return
  }

  for (const item of items) {
    const chip = document.createElement('button')
    chip.type = 'button'
    chip.className = 'license-chip'
    chip.title = item.machine_id
    chip.innerHTML = `<span>${item.license_id}</span><small>${item.customer || '无客户标识'}</small>`
    chip.addEventListener('click', () => appendRevokedLicenseId(item.license_id))
    licensePreviewList.append(chip)
  }
}

async function toggleIssuedLicensePreview() {
  if (!licensePreview) {
    return
  }

  if (licensePreview.classList.contains('open')) {
    licensePreview.classList.remove('open')
    return
  }

  try {
    const items = await invoke<IssuedLicensePreview[]>('host_list_issued_licenses')
    renderIssuedLicensePreview(items)
    licensePreview.classList.add('open')
    setStatus(revocationStatus, items.length ? '已加载本地许可证 ID。' : '没有可预览的许可证记录。')
  } catch (err) {
    logger.error('load issued licenses failed', err)
    setStatus(revocationStatus, '读取本地许可证记录失败。', true)
  }
}

async function copyText(value: string, successTarget: HTMLDivElement | null) {
  if (!value.trim()) {
    setStatus(successTarget, '没有可复制的内容。', true)
    return
  }

  try {
    await navigator.clipboard.writeText(value)
    setStatus(successTarget, '已复制到剪贴板。')
  } catch (err) {
    logger.error('copy failed', err)
    setStatus(successTarget, '复制失败，请手动复制。', true)
  }
}

closeButton?.addEventListener('click', async () => {
  try {
    await getCurrentWebviewWindow().close()
  } catch (err) {
    logger.error('关闭宿主管理窗口失败', err)
  }
})

issueButton?.addEventListener('click', async () => {
  const machineId = machineIdInput?.value.trim() || ''
  const licenseId = licenseIdInput?.value.trim() || ''
  const expiresDays = expiresDaysInput?.value.trim()
  const customer = customerInput?.value.trim() || ''

  if (!machineId || !licenseId) {
    setStatus(issueStatus, '机器码和许可证 ID 不能为空。', true)
    return
  }

  try {
    const license = await invoke<string>('host_issue_license', {
      machineId,
      licenseId,
      expiresDays: expiresDays ? Number(expiresDays) : null,
      customer: customer || null,
    })
    if (issuedLicenseOutput) {
      issuedLicenseOutput.value = license
    }
    setStatus(issueStatus, '许可证已生成。')
  } catch (err) {
    logger.error('issue license failed', err)
    setStatus(issueStatus, '生成许可证失败，请检查当前机器是否为宿主机构建机器。', true)
  }
})

issueCopyButton?.addEventListener('click', async () => {
  await copyText(issuedLicenseOutput?.value || '', issueStatus)
})

signRevocationButton?.addEventListener('click', async () => {
  const version = Number(revocationVersionInput?.value || '1')
  const revoked = (revokedIdsInput?.value || '')
    .split(/\r?\n/)
    .map((item) => item.trim())
    .filter(Boolean)

  try {
    const payload = await invoke<string>('host_sign_revocations', {
      version,
      revoked,
    })
    if (signedRevocationsOutput) {
      signedRevocationsOutput.value = payload
    }
    setStatus(revocationStatus, '撤销列表已签名。')
  } catch (err) {
    logger.error('sign revocations failed', err)
    setStatus(revocationStatus, '生成撤销列表失败。', true)
  }
})

revocationCopyButton?.addEventListener('click', async () => {
  await copyText(signedRevocationsOutput?.value || '', revocationStatus)
})

previewIssuedLicenseIdsButton?.addEventListener('click', async () => {
  await toggleIssuedLicensePreview()
})

async function loadContext() {
  try {
    const context = await invoke<{
      public_key: string
      revocation_url: string
    }>('host_get_management_context')

    if (publicKeyBox) {
      publicKeyBox.textContent = context.public_key
    }
    if (revocationUrlBox) {
      revocationUrlBox.textContent = context.revocation_url || '未配置'
    }
    if (licenseIdInput && !licenseIdInput.value) {
      licenseIdInput.value = defaultLicenseId()
    }
  } catch (err) {
    logger.error('load host context failed', err)
    setStatus(issueStatus, '加载宿主管理配置失败，当前机器可能没有宿主权限。', true)
  }
}

document.addEventListener('DOMContentLoaded', async () => {
  await loadContext()
})
