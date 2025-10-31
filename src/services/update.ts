import { check } from "@tauri-apps/plugin-updater";
import { ignoreMouseEvents, startMouseEvents } from "@/lib/system.ts";
import { openUpdateWindow } from "@/components/AudoUpdater.tsx";

export async function checkCurrentAppUpdate() {
  try {
    startMouseEvents("updater").catch((err) => {
      console.error("start mouse", err as string);
    });
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
        await ignoreMouseEvents("main");
      });
  } catch (err) {
    console.error("检查更新失败：", err);
  }
}


