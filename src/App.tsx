import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
// 非Web环境使用HashRouter 而不是 BrowserRouter
import { HashRouter, Route, Routes } from "react-router-dom";
import AutoUpdater from "@/components/AudoUpdater.tsx";
import { Toaster } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
import { logError } from "@/lib/logger.ts";
import { registryGlobalShortcut } from "@/lib/shortcut.ts";
import { ignoreMouseEvents } from "@/lib/system.ts";
import UpdateWindow from "@/pages/UpdateWindow.tsx";
import Index from "./pages/Index";
import NotFound from "./pages/NotFound";

const queryClient = new QueryClient();

const MainApp = ({
  hasSolution,
  setHasSolution,
}: {
  hasSolution: boolean;
  setHasSolution: (value: boolean) => void;
}) => (
  <QueryClientProvider client={queryClient}>
    <TooltipProvider>
      {/*暂时跳过更新检测*/}
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
);

function App() {
  const [hasSolution, setHasSolution] = useState(false);

  const hasRegistered = useRef(false); // 使用 useRef 来确保只注册一次

  const revealMainWindow = async () => {
    try {
      await invoke("show_window");
    } catch (err) {
      logError("show window err", err);
    }
  };

  useEffect(() => {
    let unlistenActivation: UnlistenFn | null = null;

    const registerShortcuts = async () => {
      if (hasRegistered.current) {
        return;
      }
      hasRegistered.current = true;
      try {
        await registryGlobalShortcut();
        await revealMainWindow();
      } catch (err) {
        logError("shortcut err", err);
      }
    };

    const waitForActivationAndRegister = async () => {
      try {
        const activated = await invoke<boolean>("get_activation_status");
        if (activated) {
          await registerShortcuts();
          return;
        }
        unlistenActivation = await listen("activation_granted", async () => {
          await registerShortcuts();
          if (unlistenActivation) {
            unlistenActivation();
            unlistenActivation = null;
          }
        });
      } catch (err) {
        logError("activation bootstrap err", err);
      }
    };

    ignoreMouseEvents("main").catch((err) => {
      logError("mouse err", err);
    });
    waitForActivationAndRegister().catch((err) => {
      logError("wait activation err", err);
    });

    return () => {
      if (unlistenActivation) {
        unlistenActivation();
        unlistenActivation = null;
      }
    };
  }, []);

  return <MainApp hasSolution={hasSolution} setHasSolution={setHasSolution} />;
}

export default App;
