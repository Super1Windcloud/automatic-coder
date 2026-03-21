import { getVersion } from '@tauri-apps/api/app'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { openUrl } from '@tauri-apps/plugin-opener'
import { relaunch } from '@tauri-apps/plugin-process'
import { check, DownloadEvent, Update } from '@tauri-apps/plugin-updater'
import { useEffect, useMemo, useState } from 'react'
import { createScopedLogger } from '@/lib/logger.ts'
import { ignoreMouseEvents } from '@/lib/system.ts'

type UpdateStatus =
  | 'checking'
  | 'prompt'
  | 'downloading'
  | 'finished'
  | 'no-update'
  | 'error'

const logger = createScopedLogger('update-window')

export default function UpdateWindow() {
  const [status, setStatus] = useState<UpdateStatus>('checking')
  const [progress, setProgress] = useState(0)
  const [error, setError] = useState<string | null>(null)
  const [updateInfo, setUpdateInfo] = useState<Update | null>(null)

  useEffect(() => {
    const current = getCurrentWindow()
    current.center().catch((err) => {
      logger.error('更新窗口居中失败', err)
    })
    current.show().catch((err) => {
      logger.error('更新窗口显示失败', err)
    })
    current.setIgnoreCursorEvents(false).catch((err) => {
      logger.warn('无法启用更新窗口的鼠标事件', err)
    })

    let isMounted = true
    const run = async () => {
      try {
        logger.info('开始检查更新')
        const currentVersion = await getVersion()
        const update = await check()
        if (!isMounted) return

        if (!update) {
          setStatus('no-update')
          logger.info(
            `当前已是最新版本，当前版本 ${currentVersion}，远程版本 ${currentVersion}`,
          )
          return
        }

        setUpdateInfo(update)
        logger.info(
          `发现新版本 ${update.version}，当前版本 ${currentVersion}，远程版本 ${update.version}`,
        )
        setStatus('prompt')
      } catch (err) {
        if (!isMounted) return
        const message = '检查更新失败，请稍后重试。'
        setError(message)
        logger.error(message, err)
        setStatus('error')
      }
    }

    run().catch((err) => logger.error('检查更新任务执行失败', err))

    return () => {
      isMounted = false
    }
  }, [])

  const updateBody = useMemo(() => {
    if (!updateInfo?.rawJson) return ''
    const releaseNotes =
      typeof updateInfo.rawJson === 'object' && updateInfo.rawJson
        ? (updateInfo.rawJson as Record<string, unknown>).body
        : ''
    return typeof releaseNotes === 'string' ? releaseNotes : ''
  }, [updateInfo])

  const startDownload = async () => {
    if (!updateInfo) return

    setStatus('downloading')
    setProgress(0)
    setError(null)
    logger.info(`开始下载更新包 ${updateInfo.version}`)

    let downloaded = 0
    let total = 0

    try {
      await updateInfo.downloadAndInstall((event: DownloadEvent) => {
        switch (event.event) {
          case 'Started':
            total = event.data.contentLength ?? 0
            logger.info(
              `更新包下载开始，大小 ${total > 0 ? `${total} bytes` : '未知'}`,
            )
            break
          case 'Progress':
            downloaded += event.data.chunkLength
            if (total > 0) {
              setProgress(downloaded / total)
            }
            break
          case 'Finished':
            setProgress(1)
            setStatus('finished')
            logger.info('更新包下载完成')
            break
          default:
            break
        }
      })
      setTimeout(async () => {
        logger.info('更新安装完成，准备重启应用')
        try {
          await relaunch()
        } catch (relaunchError) {
          logger.error('应用重启失败', relaunchError)
        }
      }, 1200)
    } catch (err) {
      const message = '下载更新失败，请重试。'
      setStatus('error')
      setError(message)
      logger.error(message, err)
    }
  }

  const handleLater = async () => {
    try {
      logger.info('用户选择稍后提醒')
      await ignoreMouseEvents('main')
      const win = getCurrentWindow()
      await win.close()
      logger.info('更新窗口已关闭')
    } catch (err) {
      logger.error('关闭更新窗口失败', err)
    }
  }

  const renderContent = () => {
    switch (status) {
      case 'checking':
        return (
          <>
            <div className="pulse-dot" />
            <h2>正在为你查找新版本...</h2>
            <p>请稍候，这只需要几秒钟。</p>
          </>
        )
      case 'prompt':
        return (
          <>
            <h2 className={'found-version'}>
              发现新版本 {updateInfo?.version}
            </h2>
            <p>InterView Code 当前版本：{updateInfo?.currentVersion}</p>
            {updateBody && (
              <div
                className="release-note"
                dangerouslySetInnerHTML={{ __html: updateBody }}
              />
            )}
            <div className="actions">
              <button className="primary" onClick={startDownload}>
                立即更新
              </button>
              <button className="secondary" onClick={handleLater}>
                稍后提醒
              </button>
            </div>
            <h3
              onClick={async () => {
                await openUrl('https://github.com/Super1WindCloud')
              }}
              className={'author'}
            >
              SuperWindCloud
            </h3>
          </>
        )
      case 'downloading':
        return (
          <>
            <div className="spinner" />
            <h2>正在华丽升级...</h2>
            <p>请保持应用运行，更新会在完成后自动重启。</p>
            <div className="progress-bar">
              <div className="progress-track">
                <div
                  className="progress-fill"
                  style={{
                    width: `${Math.min(progress * 100, 100).toFixed(1)}%`,
                  }}
                />
              </div>
              <span>{Math.min(progress * 100, 100).toFixed(1)}%</span>
            </div>
          </>
        )
      case 'finished':
        return (
          <>
            <h2>✨ 完成！</h2>
            <p>新版本已就绪，应用即将自动重启。</p>
          </>
        )
      case 'no-update':
        return (
          <>
            <h2>暂无更新</h2>
            <p>你已经在使用最新版本啦。</p>
            <button className="secondary" onClick={handleLater}>
              关闭
            </button>
          </>
        )
      case 'error':
        return (
          <>
            <h2>更新遇到问题</h2>
            {error && <p>{error}</p>}
            <div className="actions">
              <button
                className="primary"
                onClick={startDownload}
                disabled={!updateInfo}
              >
                重试更新
              </button>
              <button className="secondary" onClick={handleLater}>
                关闭
              </button>
            </div>
          </>
        )
      default:
        return null
    }
  }

  return (
    <div className="update-wrapper">
      <div className="aurora" />
      <div className="update-card">{renderContent()}</div>

      <style>
        {`
          body {
            margin: 0;
            background: radial-gradient(120% 120% at 15% 15%, rgba(123, 0, 255, 0.6), transparent),
              radial-gradient(100% 100% at 85% 20%, rgba(0, 219, 222, 0.45), transparent),
              #0f172a;
            font-family: "Segoe UI", -apple-system, BlinkMacSystemFont, "Helvetica Neue", sans-serif;
            color: #f8fafc;
            border-radius: 15px;
          }
          .update-wrapper {
            position: relative;
            display: flex;
            align-items: center;
            justify-content: center;
            width: 100vw;
            height: 100vh;
            overflow: hidden;
          }
          .aurora {
            position: absolute;
            inset: 0;
            background: conic-gradient(from 180deg at 50% 50%, rgba(76, 29, 149, 0.4), transparent);
            filter: blur(120px);
            opacity: 0.6;
            animation: float 12s ease-in-out infinite alternate;
          }
          .update-card {
            position: relative;
            width: min(420px, 90vw);
            padding: 2.5rem 2rem;
            background: rgba(15, 23, 42, 0.55);
            border-radius: 22px;
            border: 1px solid rgba(148, 163, 184, 0.25);
            box-shadow: 0 24px 60px rgba(15, 23, 42, 0.55);
            backdrop-filter: blur(22px);
            text-align: center;
          }
          .update-card h2 {
            margin: 0 0 0.75rem;
            font-size: 1.6rem;
            letter-spacing: 0.04em;
          }
          .update-card p {
            margin: 0.25rem 0 0.75rem;
            color: rgba(226, 232, 240, 0.85);
            line-height: 1.6;
            font-size: 0.95rem;
          }
          .release-note {
            max-height: 160px;
            margin: 1rem 0;
            padding: 1rem;
            text-align: left;
            overflow-y: auto;
            border-radius: 14px;
            background: rgba(30, 41, 59, 0.65);
            border: 1px solid rgba(148, 163, 184, 0.15);
          }
          .actions {
            display: flex;
            justify-content: center;
            gap: 0.75rem;
            margin-top: 1.25rem;
          }
          button {
            border: none;
            border-radius: 999px;
            padding: 0.65rem 1.5rem;
            font-size: 0.95rem;
            cursor: pointer;
            transition: transform 0.22s ease, box-shadow 0.22s ease, opacity 0.22s ease;
          }
          button.primary {
            background: linear-gradient(135deg, #7c3aed, #2563eb);
            color: #fff;
            box-shadow: 0 10px 25px rgba(79, 70, 229, 0.35);
          }
          button.primary:hover {
            transform: translateY(-2px);
            box-shadow: 0 16px 32px rgba(59, 130, 246, 0.45);
          }
          button.secondary {
            background: rgba(148, 163, 184, 0.16);
            color: rgba(226, 232, 240, 0.88);
            border: 1px solid rgba(148, 163, 184, 0.25);
          }
          button.secondary:hover {
            opacity: 0.95;
            transform: translateY(-1px);
          }
          button:disabled {
            opacity: 0.6;
            cursor: not-allowed;
            box-shadow: none;
            transform: none;
          }
          .progress-bar {
            margin-top: 1.25rem;
            display: flex;
            align-items: center;
            gap: 0.75rem;
            justify-content: center;
          }
          .progress-track {
            flex: 1;
            height: 8px;
            border-radius: 999px;
            overflow: hidden;
            background: rgba(148, 163, 184, 0.18);
          }
          .progress-fill {
            height: 100%;
            background: linear-gradient(90deg, #38bdf8, #a855f7);
            transition: width 0.3s ease;
            border-radius: 999px;
          }
          .spinner {
            width: 64px;
            height: 64px;
            margin: 0 auto 1.5rem;
            border-radius: 50%;
            border: 4px solid rgba(148, 163, 184, 0.2);
            border-top-color: #7c3aed;
            animation: spin 1s linear infinite;
          }
             @keyframes glow {
        0% {
          text-shadow: 0 0 5px #00dbde, 0 0 10px #fc00ff;
        }
        50% {
          text-shadow: 0 0 20px #00dbde, 0 0 40px #fc00ff;
        }
        100% {
          text-shadow: 0 0 5px #00dbde, 0 0 10px #fc00ff;
        }
      }
      
      .author {
        cursor: pointer;
        background: linear-gradient(90deg, #00dbde, #fc00ff);
        -webkit-background-clip: text;
        color: transparent;
        font-weight: bold;
        font-size: 1.3rem;
        animation: glow 2s ease-in-out infinite;
      }
      
      .author:hover {
        animation-play-state: paused;
        transform: scale(1.05);
         text-decoration: underline;
      }


          .pulse-dot {
            width: 18px;
            height: 18px;
            margin: 0 auto 1.25rem;
            border-radius: 50%;
            background: radial-gradient(circle at center, #38bdf8, #0ea5e9);
            box-shadow: 0 0 18px rgba(56, 189, 248, 0.8);
            animation: pulse 1.6s ease-in-out infinite;
          }
          @keyframes spin {
            to {
              transform: rotate(360deg);
            }
          }
          @keyframes pulse {
            0%, 100% {
              transform: scale(1);
              opacity: 0.85;
            }
            50% {
              transform: scale(1.25);
              opacity: 0.35;
            }
          }
          @keyframes float {
            from {
              transform: translateX(-12px) translateY(-8px);
            }
            to {
              transform: translateX(12px) translateY(10px);
            }
          }
        `}
      </style>
    </div>
  )
}
