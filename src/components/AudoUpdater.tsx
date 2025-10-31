import { useEffect, useRef } from "react";
import { check } from "@tauri-apps/plugin-updater";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { ignoreMouseEvents, startMouseEvents } from "@/lib/system.ts";

const UPDATE_WINDOW_LABEL = "updater";

export async function openUpdateWindow() {
  startMouseEvents().catch((err) => {
    console.error("start mouse", err as string);
  });
  if (typeof WebviewWindow.getByLabel === "function") {
    try {
      const existing = await WebviewWindow.getByLabel(UPDATE_WINDOW_LABEL);
      if (existing) {
        await existing.close();
      }
    } catch (error) {
      console.warn("关闭旧的更新窗口失败：", error);
    }
  }

  const updater = new WebviewWindow(UPDATE_WINDOW_LABEL, {
    title: "应用更新",
    url: "/#/update", // 👈 React Router 的路径
    width: 480,
    height: 320,
    resizable: true,
    center: true,
    decorations: false,
    alwaysOnTop: false,
    transparent: false,
    focus: true,
    visible: false,
  });

  await updater.once("tauri://created", () => {
    console.log("更新窗口已创建");
    updater.setFocus().catch(() => {});
  });

  await updater.once("tauri://error", (e) => {
    console.error("更新窗口创建失败：", e);
  });

  return updater;
}

export default function AutoUpdater() {
  const hasCheckedRef = useRef(false);

  useEffect(() => {
    const doCheck = async () => {
      try {
        const current = WebviewWindow.getCurrent();
        if (current.label !== "main") {
          return;
        }

        if (hasCheckedRef.current) {
          return;
        }
        hasCheckedRef.current = true;

        const update = await check();

        if (!update) {
          console.log("✅ 当前已是最新版本");
          return;
        }

        console.log(`发现新版本 ${update.version}`);
        openUpdateWindow()
          .catch((err) => {
            console.error("打开更新窗口失败：", err);
          })
          .finally(async () => {
            await ignoreMouseEvents();
          });
      } catch (err) {
        console.error("检查更新失败：", err);
      }
    };
    doCheck();
  }, []);

  return null;
}
