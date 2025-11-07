import { UnlistenFn } from '@tauri-apps/api/event'

import { useEffect, useState, useSyncExternalStore } from 'react'
import { useAsync } from 'react-use'
import { MarkdownPreview } from '@/components/MarkdownPreview'
import { showSolutionWindow } from '@/lib/system.ts'
import { getScreenShotSolutionFromVLM } from '@/lib/vlm.ts'
import { useAppStateStoreWithNoHook } from '@/store'

const Index = ({
  hasSolution,
  setHasSolution,
}: {
  hasSolution: boolean
  setHasSolution: (value: boolean) => void
}) => {
  const currentScreenShotPath = useSyncExternalStore(
    useAppStateStoreWithNoHook.subscribe, // 订阅状态变化
    () => useAppStateStoreWithNoHook.getState().currentScreenShotPath, // selector
  )
  const startShowSolution = useSyncExternalStore(
    useAppStateStoreWithNoHook.subscribe, // 订阅状态变化
    () => useAppStateStoreWithNoHook.getState().startShowSolution, // selector
  )
  const [solutionContent, setSolutionContent] = useState<string>('')
  const [unlistenFn, setUnlistenFn] = useState<UnlistenFn | null>(null)

  useEffect(() => {
    setSolutionContent('')
    if (currentScreenShotPath) {
      setHasSolution(true)
    } else {
      setHasSolution(false)
    }
  }, [currentScreenShotPath])

  useAsync(async () => {
    await showSolutionWindow()

    if (hasSolution && startShowSolution) {
      const unlistener = await getScreenShotSolutionFromVLM(
        (content: string) => {
          setSolutionContent(content)
          showSolutionWindow()
        },
      )
      // 如果是 setUnlistenFn(unlistener); 会转换为 setUnlistenFn(prev => unlistener(prev)); 会立刻执行
      setUnlistenFn(() => unlistener)
    } else {
      setSolutionContent('')
    }
  }, [hasSolution, startShowSolution])

  useEffect(() => {
    if (!startShowSolution && unlistenFn) {
      console.warn('unlisten current callback')
      unlistenFn()
      setUnlistenFn(null)
    }
  }, [unlistenFn, startShowSolution])

  useAsync(async () => {
    // 防抖
    const timeout = setTimeout(() => {
      showSolutionWindow()
    }, 300)
    return () => clearTimeout(timeout)
  }, [solutionContent])

  return (
    <div
      style={{
        scrollbarWidth: 'none',
        width: '100vw',
        height: '100vh',
        background: 'linear-gradient(to bottom, #724766, #2C4F71)',
      }}
    >
      <div
        style={{
          color: 'lightgrey',
          display: 'flex',
          flexDirection: 'row',
          justifyContent: 'space-evenly',
          alignItems: 'center',
          scrollbarWidth: 'none',
          position: 'relative',
          paddingLeft: '1rem', // px-4
          paddingRight: '1rem',
          paddingTop: '0.5rem', // py-2
          paddingBottom: '0.5rem',
          boxShadow: '0 1px 2px rgba(0, 0, 0, 0.05)', // shadow-sm
          backdropFilter: 'blur(12px)', // backdrop-blur-md
          backgroundColor: 'rgba(255, 255, 255, 0.1)', // bg-white/10
          border: '1px solid rgba(255, 255, 255, 0.1)', // border border-white/10
        }}
      >
        <span>截图(Alt+1)</span>
        <span>答案(Alt+2)</span>
        <span>移动(Alt+↕↔)</span>
        <span>重置(Alt+`)</span>
        <span>隐藏(Ctrl+Shift+`)</span>
      </div>
      {currentScreenShotPath && (
        <div
          style={{
            margin: 0,
            padding: 0,
            scrollbarWidth: 'none',
          }}
        >
          (
          <img
            style={{
              objectFit: 'cover',
              height: 100,
              marginTop: 10,
              marginLeft: 40,
            }}
            src={currentScreenShotPath}
            alt="screenshot"
          />
          )
        </div>
      )}

      {hasSolution && solutionContent && (
        <MarkdownPreview content={solutionContent} />
      )}
    </div>
  )
}

export default Index
