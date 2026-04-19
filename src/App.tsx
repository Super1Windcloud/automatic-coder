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
import { createScopedLogger } from "@/lib/logger.ts";
import { registryGlobalShortcut } from "@/lib/shortcut.ts";
import { ignoreMouseEvents } from "@/lib/system.ts";
import UpdateWindow from "@/pages/UpdateWindow.tsx";
import { useAppStateStoreWithNoHook } from "@/store";
import Index from "./pages/Index";
import NotFound from "./pages/NotFound";

const queryClient = new QueryClient();
const logger = createScopedLogger("app");

function applyPageOpacity(opacity: number) {
  const normalized = Math.min(Math.max(opacity, 0.2), 1);
  document.documentElement.style.opacity = `${normalized}`;
  document.body.style.opacity = "1";
  document.documentElement.style.setProperty(
    "--page-text-color",
    normalized < 0.6 ? "#000000" : "#f4f4f5",
  );
  document.documentElement.style.setProperty(
    "--page-header-text-color",
    normalized < 0.6 ? "#000000" : "lightgrey",
  );
}

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
  const isActivatedRef = useRef(false);
  const blockReloadShortcut = (event: KeyboardEvent) => {
    if (event.key.toLowerCase() !== "r") {
      return;
    }
    if (event.ctrlKey || event.metaKey) {
      event.preventDefault();
      event.stopPropagation();
    }
  };

  const revealMainWindow = async () => {
    if (!isActivatedRef.current) {
      return;
    }
    try {
      await invoke("show_window");
    } catch (err) {
      logger.error("show window err", err);
    }
  };

  useEffect(() => {
    let unlistenActivation: UnlistenFn | null = null;
    let unlistenActivationRevoked: UnlistenFn | null = null;
    let unlistenOpacity: UnlistenFn | null = null;
    let unlistenBackgroundBroadcast: UnlistenFn | null = null;

    const registerShortcuts = async () => {
      if (hasRegistered.current) {
        return;
      }
      hasRegistered.current = true;
      try {
        await registryGlobalShortcut();
        await revealMainWindow();
      } catch (err) {
        logger.error("shortcut err", err);
      }
    };

    const waitForActivationAndRegister = async () => {
      try {
        const activated = await invoke<boolean>("get_activation_status");
        if (activated) {
          isActivatedRef.current = true;
          await registerShortcuts();
          return;
        }
        unlistenActivation = await listen("activation_granted", async () => {
          isActivatedRef.current = true;
          await registerShortcuts();
          if (unlistenActivation) {
            unlistenActivation();
            unlistenActivation = null;
          }
        });
        unlistenActivationRevoked = await listen("activation_revoked", async () => {
          isActivatedRef.current = false;
        });
      } catch (err) {
        logger.error("activation bootstrap err", err);
      }
    };

    ignoreMouseEvents("main").catch((err) => {
      logger.error("mouse err", err);
    });
    invoke<string>("get_store_config")
      .then((configStr) => {
        const config = JSON.parse(configStr) as { page_opacity?: number };
        applyPageOpacity(config.page_opacity ?? 1);
      })
      .catch((err) => {
        logger.error("load page opacity err", err);
      });
    invoke<string>("get_store_config")
      .then((configStr) => {
        const config = JSON.parse(configStr) as {
          background_broadcast?: boolean;
        };
        useAppStateStoreWithNoHook
          .getState()
          .updateBackgroundBroadcastEnabled(!!config.background_broadcast);
      })
      .catch((err) => {
        logger.error("load background broadcast err", err);
      });
    listen<number>("page-opacity-changed", (event) => {
      applyPageOpacity(event.payload);
    })
      .then((unlisten) => {
        unlistenOpacity = unlisten;
      })
      .catch((err) => {
        logger.error("listen page opacity err", err);
      });
    listen<boolean>("background-broadcast-changed", (event) => {
      useAppStateStoreWithNoHook
        .getState()
        .updateBackgroundBroadcastEnabled(event.payload);
      if (!event.payload) {
        void revealMainWindow();
      }
    })
      .then((unlisten) => {
        unlistenBackgroundBroadcast = unlisten;
      })
      .catch((err) => {
        logger.error("listen background broadcast err", err);
      });
    waitForActivationAndRegister().catch((err) => {
      logger.error("wait activation err", err);
    });

    return () => {
      if (unlistenActivation) {
        unlistenActivation();
        unlistenActivation = null;
      }
      if (unlistenOpacity) {
        unlistenOpacity();
        unlistenOpacity = null;
      }
      if (unlistenActivationRevoked) {
        unlistenActivationRevoked();
        unlistenActivationRevoked = null;
      }
      if (unlistenBackgroundBroadcast) {
        unlistenBackgroundBroadcast();
        unlistenBackgroundBroadcast = null;
      }
    };
  }, []);

  useEffect(() => {
    window.addEventListener("keydown", blockReloadShortcut, {
      capture: true,
    });
    return () => {
      window.removeEventListener("keydown", blockReloadShortcut, {
        capture: true,
      });
    };
  }, []);

  return <MainApp hasSolution={hasSolution} setHasSolution={setHasSolution} />;
}

export default App;
