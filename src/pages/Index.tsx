import { invoke } from "@tauri-apps/api/core";
import { UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useState, useSyncExternalStore } from "react";
import { MarkdownPreview } from "@/components/MarkdownPreview";
import {
  hideCurrentWindow,
  showSolutionWindow,
  speakAnswer,
} from "@/lib/system.ts";
import { getScreenShotSolutionFromVLM } from "@/lib/vlm.ts";
import { useAppStateStoreWithNoHook } from "@/store";

const DIRECTION_LABEL_MAP: Record<string, string> = {
  lefthalf: "屏幕左半边",
  righthalf: "屏幕右半边",
  uphalf: "屏幕上半边",
  downhalf: "屏幕下半边",
  fullscreen: "全屏",
};

type PreferencesSummary = {
  language: string;
  direction: string;
  prompt: string;
  model: string;
  opacity: string;
};

const Index = ({
  hasSolution,
  setHasSolution,
}: {
  hasSolution: boolean;
  setHasSolution: (value: boolean) => void;
}) => {
  const currentScreenShotPath = useSyncExternalStore(
    useAppStateStoreWithNoHook.subscribe, // 订阅状态变化
    () => useAppStateStoreWithNoHook.getState().currentScreenShotPath, // selector
  );
  const startShowSolution = useSyncExternalStore(
    useAppStateStoreWithNoHook.subscribe, // 订阅状态变化
    () => useAppStateStoreWithNoHook.getState().startShowSolution, // selector
  );
  const backgroundBroadcastEnabled = useSyncExternalStore(
    useAppStateStoreWithNoHook.subscribe,
    () => useAppStateStoreWithNoHook.getState().backgroundBroadcastEnabled,
  );
  const [solutionContent, setSolutionContent] = useState<string>("");
  const [unlistenFn, setUnlistenFn] = useState<UnlistenFn | null>(null);
  const [preferenceSummary, setPreferenceSummary] =
    useState<PreferencesSummary | null>(null);

  useEffect(() => {
    setSolutionContent("");
    if (currentScreenShotPath) {
      setHasSolution(true);
    } else {
      setHasSolution(false);
    }
  }, [currentScreenShotPath]);

  useEffect(() => {
    if (!currentScreenShotPath) {
      return;
    }
    if (useAppStateStoreWithNoHook.getState().backgroundBroadcastEnabled) {
      return;
    }

    const timer = window.setTimeout(() => {
      void showSolutionWindow();
    }, 80);

    return () => window.clearTimeout(timer);
  }, [currentScreenShotPath]);

  useEffect(() => {
    if (backgroundBroadcastEnabled) {
      void hideCurrentWindow();
    } else if (hasSolution && startShowSolution) {
      void showSolutionWindow();
    }
  }, [backgroundBroadcastEnabled, hasSolution, startShowSolution]);

  useEffect(() => {
    if (!backgroundBroadcastEnabled) {
      return;
    }
    if (!solutionContent.trim()) {
      return;
    }

    speakAnswer(solutionContent);
  }, [backgroundBroadcastEnabled, solutionContent]);

  useEffect(() => {
    if (!hasSolution || !startShowSolution) {
      setSolutionContent("");
      return;
    }

    let disposed = false;

    const startRequest = async () => {
      const unlistener = await getScreenShotSolutionFromVLM(
        (content: string) => {
          if (disposed) {
            return;
          }
          setSolutionContent(content);
          if (!useAppStateStoreWithNoHook.getState().backgroundBroadcastEnabled) {
            void showSolutionWindow();
          }
        },
        (content: string) => {
          if (
            !disposed &&
            useAppStateStoreWithNoHook.getState().backgroundBroadcastEnabled
          ) {
            speakAnswer(content);
          }
        },
      );

      if (disposed) {
        unlistener();
        return;
      }

      setUnlistenFn(() => unlistener);
    };

    void startRequest();

    return () => {
      disposed = true;
    };
  }, [hasSolution, startShowSolution]);

  useEffect(() => {
    if (!startShowSolution && unlistenFn) {
      console.warn("unlisten current callback");
      unlistenFn();
      setUnlistenFn(null);
    }
  }, [unlistenFn, startShowSolution]);

  useEffect(() => {
    const timeout = setTimeout(() => {
      if (!useAppStateStoreWithNoHook.getState().backgroundBroadcastEnabled) {
        void showSolutionWindow();
      }
    }, 300);
    return () => clearTimeout(timeout);
  }, [solutionContent]);

  useEffect(() => {
    const loadPreferencesSummary = async () => {
      try {
        const configStr = await invoke<string>("get_store_config");
        const config = JSON.parse(configStr) as {
          code_language?: string;
          direction_enum?: string;
          prompt?: string;
          vlm_model?: string;
          page_opacity?: number;
        };
        const normalizedDirection = config.direction_enum
          ? config.direction_enum.toLowerCase()
          : "";
        setPreferenceSummary({
          language: config.code_language || "未设置",
          direction:
            DIRECTION_LABEL_MAP[normalizedDirection] ||
            config.direction_enum ||
            "未设置",
          prompt: config.prompt || "默认提示词",
          model: config.vlm_model || "zai-org/GLM-4.5V",
          opacity: `${Math.round((config.page_opacity ?? 1) * 100)}%`,
        });
      } catch (error) {
        console.error("加载配置摘要失败", error);
      }
    };

    loadPreferencesSummary();
  }, [currentScreenShotPath]);

  useEffect(() => {
    if (!preferenceSummary) {
      return;
    }
    if (!currentScreenShotPath) {
      return;
    }
    if (useAppStateStoreWithNoHook.getState().backgroundBroadcastEnabled) {
      return;
    }

    const timer = window.setTimeout(() => {
      void showSolutionWindow();
    }, 80);

    return () => window.clearTimeout(timer);
  }, [currentScreenShotPath, preferenceSummary]);

  return (
    <div
      style={{
        scrollbarWidth: "none",
        width: "100vw",
        height: "100vh",
        background: "linear-gradient(to bottom, #724766, #2C4F71)",
      }}
    >
      <div
        style={{
          color: "var(--page-header-text-color, lightgrey)",
          display: "flex",
          flexDirection: "row",
          justifyContent: "space-evenly",
          alignItems: "center",
          scrollbarWidth: "none",
          position: "relative",
          paddingLeft: "1rem", // px-4
          paddingRight: "1rem",
          paddingTop: "0.5rem", // py-2
          paddingBottom: "0.5rem",
          boxShadow: "0 1px 2px rgba(0, 0, 0, 0.05)", // shadow-sm
          backdropFilter: "blur(12px)", // backdrop-blur-md
          backgroundColor: "rgba(255, 255, 255, 0.1)", // bg-white/10
          border: "1px solid rgba(255, 255, 255, 0.1)", // border border-white/10
        }}
      >
        <span>截图(Alt+1)</span>
        <span>答案(Alt+2)</span>
        <span>暂停/恢复(Alt+Space)</span>
        <span>移动(Alt+↕↔)</span>
        <span>重置(Alt+`)</span>
        <span>隐藏(Ctrl+Shift+`)</span>
      </div>
      {currentScreenShotPath && (
        <div
          style={{
            margin: 0,
            padding: 0,
            scrollbarWidth: "none",
            display: "flex",
            flexDirection: "row",
            alignItems: "center",
            gap: 24,
            marginLeft: 40,
            marginTop: 10,
          }}
        >
          <img
            style={{
              objectFit: "cover",
              height: 100,
            }}
            src={currentScreenShotPath}
            alt="screenshot"
          />
          {preferenceSummary && (
            <div
              style={{
                color: "var(--page-text-color, #f4f4f5)",
                fontSize: 14,
                lineHeight: 1.5,
                display: "flex",
                flexDirection: "column",
                gap: 4,
                maxWidth: 420,
              }}
            >
              <div>当前编程语言：{preferenceSummary.language}</div>
              <div>当前截屏方位：{preferenceSummary.direction}</div>
              <div>
                当前提示词：
                <span style={{ wordBreak: "break-word" }}>
                  {preferenceSummary.prompt}
                </span>
              </div>
              <div>当前大模型：{preferenceSummary.model}</div>
              <div>当前页面透明度：{preferenceSummary.opacity}</div>
            </div>
          )}
        </div>
      )}

      {hasSolution && solutionContent && (
        <MarkdownPreview content={solutionContent} />
      )}
    </div>
  );
};

export default Index;
