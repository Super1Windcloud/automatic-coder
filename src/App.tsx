import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { useEffect, useRef, useState } from 'react'
// 非Web环境使用HashRouter 而不是 BrowserRouter
import { HashRouter, Route, Routes } from 'react-router-dom'
import AutoUpdater from '@/components/AudoUpdater.tsx'
import { Toaster } from '@/components/ui/sonner'
import { TooltipProvider } from '@/components/ui/tooltip'
import { registryGlobalShortcut } from '@/lib/shortcut.ts'
import { ignoreMouseEvents } from '@/lib/system.ts'
import UpdateWindow from '@/pages/UpdateWindow.tsx'
import Index from './pages/Index'
import NotFound from './pages/NotFound'

const queryClient = new QueryClient()

const MainApp = ({
  hasSolution,
  setHasSolution,
}: {
  hasSolution: boolean
  setHasSolution: (value: boolean) => void
}) => (
  <QueryClientProvider client={queryClient}>
    <TooltipProvider>
      <AutoUpdater />
      <Toaster />
      <HashRouter>
        <Routes>
          <Route
            path="/"
            element={
              <Index
                hasSolution={hasSolution}
                setHasSolution={setHasSolution}
              />
            }
          />
          <Route path="update" element={<UpdateWindow />} />
          <Route path="*" element={<NotFound />} />
        </Routes>
      </HashRouter>
    </TooltipProvider>
  </QueryClientProvider>
)

function App() {
  const [hasSolution, setHasSolution] = useState(false)

  const hasRegistered = useRef(false) // 使用 useRef 来确保只注册一次

  useEffect(() => {
    if (hasRegistered.current) {
      return
    }
    hasRegistered.current = true
    ignoreMouseEvents('main').catch((err) => {
      console.error('mouse err', err)
    })
    registryGlobalShortcut().catch((err) => {
      console.error('shortcut err', err)
    })
  }, [])

  return <MainApp hasSolution={hasSolution} setHasSolution={setHasSolution} />
}

export default App
