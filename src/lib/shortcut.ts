import { register } from "@tauri-apps/plugin-global-shortcut";
import {
  getScreenCaptureToBlobUrl,
  resetWindow,
  showSolutionWindow,
} from "@/lib/system.ts";
import {
  getCurrentWindow,
  LogicalPosition,
  LogicalSize,
} from "@tauri-apps/api/window";
import { useAppStateStoreWithNoHook } from "@/store";
import { checkCurrentAppUpdate } from "@/services/update.ts";

let windowX = 0;
let windowY = 0;
let lastX = 0;
let lastY = 0;
let toggleFlag = false;
const result = await getCurrentWindow().innerPosition();
[windowX, windowY] = [result.x, result.y];

export async function registryGlobalShortcut() {
  await register("Alt+`", (event) => {
    if (event.state === "Released") {
      resetWindow(async () => {
        lastX = lastY = 0;
        const result = await getCurrentWindow().innerPosition();
        [windowX, windowY] = [result.x, result.y];
      });
    }
  });

  await register("Alt+2", async (event) => {
    if (event.state === "Released") {
      const result = await getCurrentWindow().innerPosition();
      [lastX, lastY] = [result.x, result.y];
      useAppStateStoreWithNoHook.getState().updateStartShowSolution(true);
      await showSolutionWindow();
    }
  });

  await register("Alt+1", async (event) => {
    if (event.state === "Released") {
      if (useAppStateStoreWithNoHook.getState().currentScreenShotPath) {
        await getCurrentWindow().setSize(new LogicalSize(800, 50));
        await getCurrentWindow().setPosition(new LogicalPosition(lastX, lastY));
        toggleFlag = false;
        useAppStateStoreWithNoHook.getState().updateCurrentScreenShotPath("");
        useAppStateStoreWithNoHook.getState().updateStartShowSolution(false);
      }
      await getScreenCaptureToBlobUrl();
    }
  });

  await register("Alt+Up", async (event) => {
    if (event.state === "Released") {
      if (useAppStateStoreWithNoHook.getState().startShowSolution) {
        toggleFlag = false;
        await moveWindow("down");
      } else {
        if (!toggleFlag) {
          const result = await getCurrentWindow().innerPosition();
          [windowX, windowY] = [result.x, result.y];
          toggleFlag = true;
        }
        await moveWindow("up");
      }
    }
  });

  await register("Alt+Down", async (event) => {
    if (event.state === "Released")
      if (useAppStateStoreWithNoHook.getState().startShowSolution) {
        toggleFlag = false;
        await moveWindow("up");
      } else {
        if (!toggleFlag) {
          const result = await getCurrentWindow().innerPosition();
          [windowX, windowY] = [result.x, result.y];
          toggleFlag = true;
        }
        await moveWindow("down");
      }
  });

  await register("Alt+Left", async (event) => {
    if (event.state === "Released") {
      if (!toggleFlag) {
        const result = await getCurrentWindow().innerPosition();
        [windowX, windowY] = [result.x, result.y];
        toggleFlag = true;
      }

      await moveWindow("left");
    }
  });
  await register("CommandOrControl+F11", async (event) => {
    if (event.state === "Released") {
      await checkCurrentAppUpdate();
    }
  });

  await register("Alt+Right", async (event) => {
    if (event.state === "Released") {
      if (!toggleFlag) {
        const result = await getCurrentWindow().innerPosition();
        [windowX, windowY] = [result.x, result.y];
        toggleFlag = true;
      }
      await moveWindow("right");
    }
  });

  async function moveWindow(direction: string) {
    const step = 100; // 步长
    switch (direction) {
      case "up":
        windowY -= step;
        break;
      case "down":
        windowY += step;
        break;
      case "left":
        windowX -= step;
        break;
      case "right":
        windowX += step;
        break;
      default:
        return;
    }
    await getCurrentWindow().setPosition(new LogicalPosition(windowX, windowY));
  }
}
