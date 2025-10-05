import { invoke } from "@tauri-apps/api/core";
import { getLLMPrompts } from "@/assets/constant.ts";

const selectButton = document.getElementById("select-button");
const languageSelect = document.getElementById("language-select");
const promptInput = document.getElementById("prompt-input");
const directionSelect = document.getElementById("direction-select");
const vlmKeyInput = document.getElementById("key-input");
selectButton?.addEventListener("click", async () => {
  const selectedLanguage = (languageSelect as HTMLSelectElement).value;
  const llmPrompt = (promptInput as HTMLInputElement).value.trim();
  const selectedDirection = (directionSelect as HTMLSelectElement).value;
  const prompt = llmPrompt || getLLMPrompts(selectedLanguage);
  const vlmKey = (vlmKeyInput as HTMLInputElement).value.trim();

  try {
    await invoke("set_capture_position", {
      position: selectedDirection,
    });
    await invoke("set_selected_language", {
      codeLanguage: selectedLanguage,
    });
    await invoke("set_vlm_key", {
      key: vlmKey,
    });
    await invoke("set_selected_language_prompt", {
      prompt,
    });
  } catch (err) {
    console.error("调用 Rust 命令失败:", err);
  }
});

async function loadPreferences() {
  try {
    const config_str = (await invoke("get_store_config")) as string;
    const config = JSON.parse(config_str) as {
      code_language: string;
      prompt: string;
      direction_enum: string;
    };
    if (config) {
      console.log("Loaded config:", config);
      const language = document.getElementById(
        "language-select",
      ) as HTMLSelectElement;
      if (!language) console.error("language error ");
      language.value = config.code_language || "TypeScript";
      (document.getElementById("direction-select") as HTMLSelectElement).value =
        config.direction_enum.toLowerCase() || "left_half";

      (document.getElementById("prompt-input") as HTMLInputElement).value =
        config.prompt || "";
    }
  } catch (err) {
    console.error("加载配置失败", err);
  }
}

document.addEventListener("DOMContentLoaded", async () => {
  await loadPreferences();
});
