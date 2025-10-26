import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";

export async function getScreenShotSolutionFromVLM(
  renderCallBack: (content: string) => void,
) {
  let content = "";
  const unlistenFn: UnlistenFn = await listen("completion_stream", (event) => {
    content += event.payload;
    content = content
      .replace("<|begin_of_box|>", "")
      .replace("<|end_of_box|>", "");

    renderCallBack(content);
  });

  invoke("create_screenshot_solution_stream")
    .then((res) => console.log("返回结果:", res))
    .catch((err) => {
      console.error("get solution error", err);
      unlistenFn();
    })
    .finally(() => {
      unlistenFn();
    });
  return unlistenFn;
}
