import {
  getCurrentWindow,
  LogicalPosition,
  LogicalSize,
} from "@tauri-apps/api/window";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { useAppStateStoreWithNoHook } from "@/store";
let lastHeight = 0;

export async function resetWindow(offsetCallback: () => void) {
  await getCurrentWindow().setSize(new LogicalSize(800, 50));
  await getCurrentWindow().setPosition(new LogicalPosition(100, 50));
  offsetCallback();
  useAppStateStoreWithNoHook.getState().updateCurrentScreenShotPath("");
  useAppStateStoreWithNoHook.getState().updateStartShowSolution(false);
}

export async function showSolutionWindow() {
  const contentHeight = await getWebViewHeight();
  const window = getCurrentWindow();

  if (Math.abs(contentHeight - lastHeight) < 10) return; // 🔥 忽略微小变化
  lastHeight = contentHeight;

  await window.setSize(new LogicalSize(800, contentHeight));
}

export async function ignoreMouseEvents() {
  await getCurrentWindow().setIgnoreCursorEvents(true);
}

async function getWebViewHeight() {
  return document.documentElement.scrollHeight;
}

export async function enableMouseEventsForComponent(id: string) {
  const element = document.getElementById(id);
  if (element) {
    element.style.pointerEvents = "auto"; // 启用该组件的鼠标事件
  }
}

export async function getScreenCaptureToLocalPath() {
  const filePath = (await invoke("get_screen_capture_to_path")) as string;
  console.log(filePath);
  const imagePath = convertFileSrc(filePath.replace("\\", "/"));
  useAppStateStoreWithNoHook.getState().updateCurrentScreenShotPath(imagePath);
}

export async function getScreenCaptureToBlobUrl() {
  const bytes = await invoke<number[]>("get_screen_capture_to_bytes");
  const blob = new Blob([new Uint8Array(bytes)], { type: "image/png" });
  const url = URL.createObjectURL(blob);
  useAppStateStoreWithNoHook.getState().updateCurrentScreenShotPath(url);
}
