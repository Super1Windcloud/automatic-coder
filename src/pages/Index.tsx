import { MarkdownPreview } from "@/components/MarkdownPreview";

import { useEffect, useState, useSyncExternalStore } from "react";
import { useAppStateStoreWithNoHook } from "@/store";
import { showSolutionWindow } from "@/lib/system.ts";
import { getScreenShotSolutionFromVLM } from "@/lib/vlm.ts";

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
  const [solutionContent, setSolutionContent] = useState<string>("");

  useEffect(() => {
    setSolutionContent("");
    if (currentScreenShotPath) {
      setHasSolution(true);
    } else {
      setHasSolution(false);
    }
  }, [currentScreenShotPath]);

  useEffect(() => {
    showSolutionWindow();

    if (hasSolution && startShowSolution) {
      getScreenShotSolutionFromVLM((content: string) => {
        setSolutionContent(content);
        showSolutionWindow();
      });
    } else {
      setSolutionContent("");
    }
  }, [hasSolution, startShowSolution]);

  useEffect(() => {
    // 防抖
    const timeout = setTimeout(() => {
      showSolutionWindow();
    }, 300);
    return () => clearTimeout(timeout);
  }, [solutionContent]);

  return (
    <div
      style={{
        scrollbarWidth: "none",
        overflow: "hidden",
      }}
      className="bg-background"
    >
      <div
        style={{
          color: "lightgrey",
          display: "flex",
          flexDirection: "row",
          justifyContent: "space-evenly",
          alignItems: "center",
          scrollbarWidth: "none",
          overflow: "hidden",
        }}
      >
        <span>截图(Alt+1)</span>
        <span>答案(Ctrl+Enter)</span>
        <span>移动(Alt+↕↔)</span>
        <span>重置(Alt+2)</span>
        <span>隐藏(Ctrl+Alt+Q)</span>
      </div>
      {currentScreenShotPath && (
        <img
          style={{
            objectFit: "cover",
            height: 100,
            marginTop: 10,
            marginLeft: 40,
          }}
          src={currentScreenShotPath}
          alt="screenshot"
        />
      )}
      {hasSolution && solutionContent && (
        <MarkdownPreview content={solutionContent} />
      )}
    </div>
  );
};

export default Index;
