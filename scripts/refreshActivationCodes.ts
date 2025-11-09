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
  runtimeEnv.GITEE_OWNER ||
  'SuperWindcloud'
const repo =
  runtimeEnv.ACTIVATION_REMOTE_REPO ||
  runtimeEnv.GITEE_REPO ||
  'rust_default_arg'
const tag =
  runtimeEnv.ACTIVATION_REMOTE_TAG || runtimeEnv.GITEE_RELEASE_TAG || '0.1.0'
const token =
  runtimeEnv.ACTIVATION_REMOTE_TOKEN ||
  runtimeEnv.GITEE_TOKEN ||
  'ca4ea3ee8f000c59976334bd5455eda3'

// 定义响应类型
interface GiteeAuthor {
  id: number
  name: string
  email: string
  avatar_url?: string
}

interface GiteeAsset {
  name: string
  browser_download_url: string
}

interface GiteeRelease {
  id: number
  tag_name: string
  name: string
  body: string
  created_at: string
  prerelease: boolean
  target_commitish: string
  author: GiteeAuthor
  assets: GiteeAsset[]
}
interface AttachFile {
  name: string
  id: string
  size: number
  browser_download_url: string
}

/**
 * 根据 tag 获取仓库的 release 信息
 * @param owner 仓库所属空间
 * @param repo 仓库名
 * @param tag 标签名
 * @param accessToken Gitee 授权码
 */
export async function getGiteeReleaseByTag(
  owner: string,
  repo: string,
  tag: string,
  accessToken: string,
): Promise<GiteeRelease> {
  const url = `https://gitee.com/api/v5/repos/${owner}/${repo}/releases/tags/${tag}`

  const res = await axios.get<GiteeRelease>(url, {
    params: {
      access_token: accessToken,
    },
    headers: {
      Accept: 'application/json',
      'User-Agent': 'Axios-Client',
    },
    timeout: 30000,
  })

  return res.data
}

export async function getGiteeReleaseID() {
  try {
    const release = await getGiteeReleaseByTag(owner, repo, tag, token)
    return release.id
  } catch (err: unknown) {
    console.error('❌ 请求出错:', err)
  }
}

async function getGiteeReleaseInfoByID() {
  const id = await getGiteeReleaseID()
  if (id) {
    const url = `https://gitee.com/api/v5/repos/${owner}/${repo}/releases/${id}/attach_files`

    const res = await axios.get<AttachFile[]>(url, {
      params: {
        access_token: token,
      },
      headers: {
        Accept: 'application/json',
        'User-Agent': 'Axios-Client',
      },
      timeout: 30000,
    })

    const data = res.data[0]
    // console.log(data);
    return {
      name: data.name,
      id: data.id,
      url: data.browser_download_url,
      releaseId: id.toString(),
    }
  }
}
async function deleteAttachFile(deleteUrl: string) {
  await axios.delete(deleteUrl, {
    params: {
      access_token: token,
    },
    headers: {
      Accept: 'application/json',
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
  const data = await getGiteeReleaseInfoByID()
  const deleteUrl = `https://gitee.com/api/v5/repos/${owner}/${repo}/releases/${data!.releaseId}/attach_files/${data!.id}`
  const url = `https://gitee.com/api/v5/repos/${owner}/${repo}/releases/${data!.releaseId}/attach_files/${data!.id}/download`
  console.log(url)
  try {
    const res = await axios.get(url, {
      params: { access_token: token },
      headers: {
        Accept: 'application/octet-stream',
        'User-Agent': 'Axios-Client',
      },
      responseType: 'arraybuffer', // ✅ 关键点：下载二进制流
      timeout: 20000,
    })

    const filePath = `./${data!.name || 'download.bin'}`
    fs.writeFileSync(filePath, res.data)

    console.log(`✅ 文件下载成功：${filePath}`)

    await deleteAttachFile(deleteUrl)
    await waitForEnter()
    return { data: res.data, filePath, releaseId: data!.releaseId }
  } catch (err) {
    console.error('❌ 下载失败:', err)
  }
}

export async function updateActicationCodeFile(
  releaseId: number,
  filePath: string,
) {
  const formData = new FormData()
  formData.append('file', fs.createReadStream(filePath))

  try {
    const url = `https://gitee.com/api/v5/repos/${owner}/${repo}/releases/${releaseId}/attach_files`

    await axios.post(url, formData, {
      params: {
        access_token: token,
      },
      headers: formData.getHeaders(),
      maxContentLength: Infinity,
      maxBodyLength: Infinity,
    })

    console.log('✅ 上传成功!')
  } catch (err) {
    console.error('❌ 上传失败:', err)
  }
}
