import * as fs from 'node:fs'
import axios from 'axios'
import dotenv from 'dotenv'
import FormData from 'form-data'
import pkg from '../package.json' with { type: 'json' }

dotenv.config({
  path: process.cwd() + '/.env',
})

const owner: string = 'SuperWindCloud'
const repo: string = 'rust_default_arg'
const path: string = 's3'
const token: string = process.env.GITEE_TOKEN || ''

interface PlatformInfo {
  signature: string
  url: string
}

interface Template {
  version: string
  platforms: Record<string, PlatformInfo>
}

const FILE_URL =
  'https://gitee.com/SuperWindcloud/rust_default_arg/raw/master/s3'

async function fetchTemplate(): Promise<Template> {
  const res = await axios.get(FILE_URL, { responseType: 'text' })
  const temp = res.data as string
  return JSON.parse(temp) as Template
}

const templateStr = await fetchTemplate()
console.log(templateStr)

templateStr.version = pkg.version

const signPath = process.cwd() + `/bundle/macos/Interview-Coder.app.tar.gz.sig`

const signContent = fs.readFileSync(signPath, 'utf-8')

templateStr.platforms['darwin-x86_64'].signature = signContent
templateStr.platforms['darwin-x86_64'].url =
  `https://gitee.com/SuperWindcloud/rust_default_arg/releases/download/${pkg.version}/Interview-Coder.app.tar.gz`

console.log(templateStr)

async function getFileInfo() {
  const url = `https://gitee.com/api/v5/repos/${owner}/${repo}/contents/${path}`
  const { data } = await axios.get(url, {
    params: token ? { access_token: token } : {},
  })
  return data.sha
}

async function updateS3File(newContent: string, sha: string) {
  const url = `https://gitee.com/api/v5/repos/${owner}/${repo}/contents/${path}`
  const message = 'publish new version'
  const encodedContent = Buffer.from(newContent, 'utf-8').toString('base64')

  const res = await axios.put(url, {
    access_token: token,
    content: encodedContent,
    sha,
    message,
  })

  return res.data
}

async function uploadAttach(releaseId: number, filePath: string) {
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

interface AttachFile {
  id: number
  name: string
  size: number
  browser_download_url: string
  download_count: number
  created_at: string
  updated_at: string
}

async function getReleaseAttachFilesAndDeleteExisted(
  releaseId: number,
): Promise<AttachFile[]> {
  const url = `https://gitee.com/api/v5/repos/${owner}/${repo}/releases/${releaseId}/attach_files`

  try {
    const response = await axios.get<AttachFile[]>(url, {
      params: {
        access_token: token,
      },
    })

    const data = response.data
    const existFiles = data.filter((item) =>
      item.name.includes('Interview-Coder.app.tar.gz'),
    )

    if (existFiles.length > 0) {
      console.log('❌ 已存在 Interview-Coder.app.tar.gz，删除...')
      for (const file of existFiles) {
        await axios.delete(
          `https://gitee.com/api/v5/repos/${owner}/${repo}/releases/${releaseId}/attach_files/${file.id}`,
          {
            params: {
              access_token: token,
            },
          },
        )
      }
    }

    return data
  } catch (error) {
    console.error('获取附件列表失败:', error)
    throw error
  }
}

async function uploadAttachInstallerAndCreateRelease() {
  const url = `https://gitee.com/api/v5/repos/${owner}/${repo}/releases`

  try {
    const res = await axios.post(
      url,
      {
        access_token: token,
        tag_name: pkg.version,
        name: pkg.version,
        body: 'enjoy it!',
        target_commitish: 'master',
      },
      {
        headers: { 'Content-Type': 'application/json' },
      },
    )

    console.log('✅ Release 创建成功：', res.data.name)
  } catch (error: unknown) {
    // eslint-disable-next-line @typescript-eslint/ban-ts-comment
    // @ts-expect-error
    console.error('❌ 创建失败：', error.response?.data || error.message)
  }

  const latestUrl = `https://gitee.com/api/v5/repos/${owner}/${repo}/releases/latest`
  const res = await axios.get(latestUrl, {
    params: token ? { access_token: token } : {},
  })
  const releaseId = res.data.id
  console.log(releaseId)
  await getReleaseAttachFilesAndDeleteExisted(releaseId)

  await uploadAttach(
    releaseId,
    process.cwd() + `/bundle/macos/Interview-Coder.app.tar.gz`,
  )
}

;(async () => {
  const sha = await getFileInfo()
  console.log(sha)
  const json = JSON.stringify(templateStr, null, 2)
  console.log(json)
  await updateS3File(json, sha)
  await uploadAttachInstallerAndCreateRelease()
})()
