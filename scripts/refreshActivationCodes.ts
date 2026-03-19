import * as fs from 'node:fs'
import * as readline from 'node:readline'
import axios from 'axios'
import FormData from 'form-data'

type EnvBag = Record<string, string | undefined>

const runtimeEnv: EnvBag = (() => {
  if (typeof process !== 'undefined' && process.env) {
    return process.env
  }
  try {
    const meta =
      typeof import.meta !== 'undefined'
        ? (import.meta as ImportMeta & { env?: EnvBag })
        : undefined
    return meta?.env ?? {}
  } catch {
    return {}
  }
})()

const owner =
  runtimeEnv.ACTIVATION_REMOTE_OWNER ||
  runtimeEnv.GITHUB_OWNER ||
  'Super1Windcloud'
const repo =
  runtimeEnv.ACTIVATION_REMOTE_REPO ||
  runtimeEnv.GITHUB_REPO ||
  'automatic-coder'
const tag =
  runtimeEnv.ACTIVATION_REMOTE_TAG || runtimeEnv.GITHUB_RELEASE_TAG || 'v1.0.0'
const token =
  runtimeEnv.ACTIVATION_REMOTE_TOKEN ||
  runtimeEnv.GITHUB_TOKEN ||
  ''

// 定义响应类型
interface GithubAsset {
  id: number
  name: string
  browser_download_url: string
}

interface GithubRelease {
  id: number
  tag_name: string
  upload_url: string
  assets: GithubAsset[]
}

/**
 * 根据 tag 获取仓库的 release 信息
 * @param owner 仓库所属空间
 * @param repo 仓库名
 * @param tag 标签名
 * @param accessToken GitHub 授权码
 */
export async function getGithubReleaseByTag(
  owner: string,
  repo: string,
  tag: string,
  accessToken: string,
): Promise<GithubRelease> {
  const url = `https://api.github.com/repos/${owner}/${repo}/releases/tags/${tag}`

  const res = await axios.get<GithubRelease>(url, {
    headers: {
      Authorization: `Bearer ${accessToken}`,
      Accept: 'application/vnd.github.v3+json',
      'User-Agent': 'Axios-Client',
    },
    timeout: 30000,
  })

  return res.data
}

export async function getGithubReleaseID() {
  try {
    const release = await getGithubReleaseByTag(owner, repo, tag, token)
    return release.id
  } catch (err: unknown) {
    console.error('❌ 请求出错:', err)
  }
}

async function getGithubReleaseInfoByTag() {
  try {
    const release = await getGithubReleaseByTag(owner, repo, tag, token)
    if (release.assets.length > 0) {
      const data = release.assets[0] // 默认取第一个，或者根据名字找
      const activationAsset = release.assets.find(a => a.name === 'activation_codes.enc') || data
      return {
        name: activationAsset.name,
        id: activationAsset.id,
        url: activationAsset.browser_download_url,
        releaseId: release.id.toString(),
        uploadUrl: release.upload_url,
      }
    }
  } catch (err) {
    console.error('❌ 获取 Release 信息失败:', err)
  }
}

async function deleteAsset(assetId: number) {
  const url = `https://api.github.com/repos/${owner}/${repo}/releases/assets/${assetId}`
  await axios.delete(url, {
    headers: {
      Authorization: `Bearer ${token}`,
      Accept: 'application/vnd.github.v3+json',
      'User-Agent': 'Axios-Client',
    },
    timeout: 30000,
  })
  console.log('删除成功')
}

async function waitForEnter(promptMsg = '请按 Enter 键继续上传...') {
  return new Promise<void>((resolve) => {
    const rl = readline.createInterface({
      input: process.stdin,
      output: process.stdout,
    })

    rl.question(promptMsg, () => {
      rl.close()
      resolve()
    })
  })
}

export async function downloadActivateCodeFileAndDeleteAttach() {
  const data = await getGithubReleaseInfoByTag()
  if (!data) return

  const url = `https://api.github.com/repos/${owner}/${repo}/releases/assets/${data.id}`
  console.log(`正在下载: ${url}`)
  
  try {
    const res = await axios.get(url, {
      headers: {
        Authorization: `Bearer ${token}`,
        Accept: 'application/octet-stream',
        'User-Agent': 'Axios-Client',
      },
      responseType: 'arraybuffer',
      timeout: 20000,
    })

    const filePath = `./${data.name || 'download.bin'}`
    fs.writeFileSync(filePath, res.data)

    console.log(`✅ 文件下载成功：${filePath}`)

    await deleteAsset(data.id)
    await waitForEnter()
    return { data: res.data, filePath, releaseId: data.releaseId, uploadUrl: data.uploadUrl }
  } catch (err) {
    console.error('❌ 下载失败:', err)
  }
}

export async function updateActivationCodeFile(
  uploadUrl: string,
  filePath: string,
  fileName: string,
) {
  const base_url = uploadUrl.split('{')[0]
  const url = `${base_url}?name=${fileName}`
  const fileData = fs.readFileSync(filePath)

  try {
    await axios.post(url, fileData, {
      headers: {
        Authorization: `Bearer ${token}`,
        'Content-Type': 'application/octet-stream',
        'User-Agent': 'Axios-Client',
      },
      maxContentLength: Infinity,
      maxBodyLength: Infinity,
    })

    console.log('✅ 上传成功!')
  } catch (err) {
    console.error('❌ 上传失败:', err)
  }
}
