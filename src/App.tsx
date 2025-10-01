import {useEffect} from "react";
import {invoke} from "@tauri-apps/api/core";
import "./App.css";

import {Toaster} from "@/components/ui/sonner";
import {TooltipProvider} from "@/components/ui/tooltip";
import {QueryClient, QueryClientProvider} from "@tanstack/react-query";
import {BrowserRouter, Routes, Route} from "react-router-dom";
import Index from "./pages/Index";
import NotFound from "./pages/NotFound";

const queryClient = new QueryClient();

const MainApp = () => (
    <QueryClientProvider client={queryClient}>
        <TooltipProvider>
            <Toaster/>
            <BrowserRouter>
                <Routes>
                    <Route path="/" element={<Index/>}/>
                    <Route path="*" element={<NotFound/>}/>
                </Routes>
            </BrowserRouter>
        </TooltipProvider>
    </QueryClientProvider>
);

function App() {

    useEffect(() => {
        invoke("show_window")
    }, []);

    return <MainApp/>
}

export default App;
