import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Toaster } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { BrowserRouter, Routes, Route } from "react-router-dom";
import Index from "./pages/Index";
import NotFound from "./pages/NotFound";
import { ignoreMouseEvents } from "@/lib/system.ts";
import { registryGlobalShortcut } from "@/lib/shortcut.ts";
import { useAsync } from "react-use";

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
      <Toaster />
      <BrowserRouter>
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
          <Route path="*" element={<NotFound />} />
        </Routes>
      </BrowserRouter>
    </TooltipProvider>
  </QueryClientProvider>
);

function App() {
  const [hasSolution, setHasSolution] = useState(false);

  const hasRegistered = useRef(false); // 使用 useRef 来确保只注册一次

  useEffect(() => {
    if (hasRegistered.current) {
      return;
    }
    hasRegistered.current = true;
    ignoreMouseEvents().catch((err) => {
      console.error("mouse err", err);
    });
    registryGlobalShortcut().catch((err) => {
      console.error("shortcut err", err);
    });
  }, []);

  useAsync(async () => {
    await invoke("show_window");
  }, []);

  return <MainApp hasSolution={hasSolution} setHasSolution={setHasSolution} />;
}

export default App;
