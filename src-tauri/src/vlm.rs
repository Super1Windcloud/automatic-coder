use crate::config::AppState;
use crate::utils::{get_env_key, is_dev, write_some_log};
use base64::{engine::general_purpose, Engine};
use reqwest::Client;
use serde_json::json;
use std::path::Path;
use tauri::{AppHandle, Emitter, Manager};

async fn request_chat_completion(prompt: String, image_url: String) -> String {
    let messages = json!([
        {
            "role": "system",
            "content": prompt
        },
        {
            "role": "user",
            "content": [
                {
                    "type": "image_url",
                    "image_url": {
                        "url": image_url,
                        "detail": "high"
                    }
                }

            ]
        }
    ]);

    let api_key = get_env_key("SiliconflowVLM");
    let client = Client::new();

    let body = json!({
        "model": "Qwen/Qwen3-VL-235B-A22B-Instruct",
        "thinking_budget": 4096,
        "top_p": 0.7,
        "temperature": 0.7,
        "top_k": 50,
        "frequency_penalty": 0.5,
        "n": 1,
        "stream": false,
        "stop": [],
        "messages": messages
    });

    let res = client
        .post("https://api.siliconflow.cn/v1/chat/completions")
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .expect("请求发送失败");

    let text = res.text().await.expect("读取响应失败");

    let json_res = serde_json::from_str::<serde_json::Value>(&text).expect("解析 JSON 失败");
    let choices = json_res["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("");
    println!("choices: {}", choices);
    let input_tokens = match &json_res["usage"]["prompt_tokens"] {
        serde_json::Value::String(s) => s.parse::<usize>().unwrap_or(0),
        serde_json::Value::Number(n) => n.as_u64().unwrap_or(0) as usize,
        _ => 0,
    };

    let output_tokens = match &json_res["usage"]["completion_tokens"] {
        serde_json::Value::String(s) => s.parse::<usize>().unwrap_or(0),
        serde_json::Value::Number(n) => n.as_u64().unwrap_or(0) as usize,
        _ => 0,
    };

    println!("input_tokens: {}", input_tokens);
    println!("output_tokens: {}", output_tokens);
    calc_cost(input_tokens, output_tokens);
    text
}

async fn request_chat_completion_stream_thinking(prompt: String, image_url: String) -> String {
    let start_time = std::time::Instant::now();
    let messages = json!([
        {
            "role": "system",
            "content": prompt
        },
        {
            "role": "user",
            "content": [
                {
                    "type": "image_url",
                    "image_url": {
                        "url": image_url,
                        "detail": "high"
                    }
                }

            ]
        }
    ]);

    let api_key = get_env_key("SiliconflowVLM");
    let client = Client::new();
    let body = json!({
        // "model": "stepfun-ai/step3",
        "model":"Qwen/Qwen3-VL-235B-A22B-Instruct", 
        // "model": "zai-org/GLM-4.5V",
        "top_p": 0.7,
        "temperature": 0.7,
        "top_k": 50,
        "frequency_penalty": 0.5,
        "n": 1,
        "stream": true,
        "stop": [],
        "messages": messages,
        "enable_thinking" :false,
    });

    let mut res = client
        .post("https://api.siliconflow.cn/v1/chat/completions")
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .expect("请求发送失败");

    let mut result = String::new();
    let mut reason = String::new();

    while let Some(chunk) = res.chunk().await.expect("读取 chunk 失败") {
        let text = String::from_utf8_lossy(&chunk);

        for line in text.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                if data.trim() == "[DONE]" {
                    println!("流式完成事件");
                    break;
                }

                if let Ok(json_chunk) = serde_json::from_str::<serde_json::Value>(data) {
                    if let Some(error) = json_chunk.get("error") {
                        return format!("API错误: {}", error);
                    }

                    if let Some(content) = json_chunk["choices"][0]["delta"]["content"].as_str() {
                        if !content.is_empty() {
                            println!("content: {}", content);
                            result.push_str(content);
                        }
                    } else if let Some(content) =
                        json_chunk["choices"][0]["delta"]["reasoning_content"].as_str()
                    {
                        if !content.is_empty() {
                            println!("reasoning_content: {}", content);

                            reason.push_str(content);
                        }
                    }
                }
            }
        }
    }

    println!("reason: {}", reason);
    println!("result: {}", result);

    let end_time = std::time::Instant::now();
    calc_cost(prompt.len(), result.len());
    println!("time: {} s", (end_time - start_time).as_secs());
    result
}
async fn request_chat_completion_stream(
    app_handle: &AppHandle,
    prompt: String,
    image_url: String,
    thinking: bool,
) -> String {
    let messages = json!([
        {
            "role": "system",
            "content": prompt
        },
        {
            "role": "user",
            "content": [
                {
                    "type": "image_url",
                    "image_url": {
                        "url": image_url,
                        "detail": "high"
                    }
                }

            ]
        }
    ]);

    let api_key = get_env_key("SiliconflowVLM");
    let client = Client::new();

    // Qwen/Qwen3-VL-235B-A22B-Instruct
    let body = json!({
        "model": if !thinking{  "zai-org/GLM-4.5V" } else { "stepfun-ai/step3" },
        "thinking_budget": 4096,
        "top_p": 0.7,
        "temperature": 0.7,
        "top_k": 50,
        "frequency_penalty": 0.5,
        "n": 1,
        "stream": true,
        "stop": [],
        "messages": messages,
        "enable_thinking" :false
    });

    let mut res = client
        .post("https://api.siliconflow.cn/v1/chat/completions")
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .expect("请求发送失败");

    let mut result = String::new();
    while let Some(chunk) = res.chunk().await.expect("读取 chunk 失败") {
        let text = String::from_utf8_lossy(&chunk);

        for line in text.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                if data.trim() == "[DONE]" {
                    let _ = app_handle.emit("completion_done", "").ok();
                    break;
                }

                if let Ok(json_chunk) = serde_json::from_str::<serde_json::Value>(data) {
                    if let Some(error) = json_chunk.get("error") {
                        return format!("API错误: {}", error);
                    }

                    if let Some(content) = json_chunk["choices"][0]["delta"]["content"].as_str() {
                        result.push_str(content);
                        let _ = app_handle.emit("completion_stream", content);
                    } else if let Some(content) =
                        json_chunk["choices"][0]["delta"]["reasoning_content"].as_str()
                    {
                        let _ = app_handle.emit("completion_stream", content);
                        result.push_str(content);
                    }
                }
            }
        }
    }
    result
}

#[tauri::command]
pub async fn create_screenshot_solution_stream(app_handle: AppHandle) -> String {
    let assets_path = if cfg!(target_os = "windows") {
        Path::new("assets").to_path_buf()
    } else if cfg!(target_os = "macos") {
        let log_dir = dirs::data_dir().unwrap().join("interview_coder_app");
        let assets = log_dir.join("assets");
        assets.to_path_buf()
    } else {
        write_some_log("unknown platform to support");
        std::process::exit(1);
    };
    let state = app_handle.state::<AppState>();
    let prompt = state.prompt.lock().unwrap().clone();
    if !is_dev() {
        write_some_log(assets_path.to_str().unwrap())
    }
    let entries: Vec<_> = std::fs::read_dir(assets_path)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| {
            let path = e.path();
            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
            matches!(ext, "png" | "jpg" | "jpeg")
        })
        .collect();

    if let Some(first_image) = entries.first() {
        if !is_dev() {
            write_some_log(first_image.path().to_str().unwrap())
        }
        let bytes = std::fs::read(first_image.path()).unwrap();
        let base64_str = general_purpose::STANDARD.encode(&bytes);
        let base64 = format!("data:image/png;base64,{}", base64_str);
        let result = request_chat_completion_stream(&app_handle, prompt, base64, false).await;
        result
    } else {
        "没有找到图片文件".to_string()
    }
}

trait ToF64 {
    fn to_f64(&self) -> f64;
}

impl ToF64 for &str {
    fn to_f64(&self) -> f64 {
        self.trim().parse::<f64>().unwrap_or(0.0)
    }
}

impl ToF64 for String {
    fn to_f64(&self) -> f64 {
        self.trim().parse::<f64>().unwrap_or(0.0)
    }
}

impl ToF64 for usize {
    fn to_f64(&self) -> f64 {
        *self as f64
    }
}

impl ToF64 for u32 {
    fn to_f64(&self) -> f64 {
        *self as f64
    }
}

fn calc_cost<T: ToF64, U: ToF64>(input_tokens: T, output_tokens: U) -> f64 {
    let input_price_per_m = 4.0; // ¥1 / M tokens
    let output_price_per_m = 10.0; // ¥6 / M tokens

    let result = (input_tokens.to_f64() * input_price_per_m
        + output_tokens.to_f64() * output_price_per_m)
        / 1_000_000.0;
    println!("cost: {} ¥", result);
    result
}

///cargo test vlm::test_request_chat_completion_stream
#[tokio::test]
async fn test_request_chat_completion_stream() {
    use dotenv::dotenv;

    dotenv().ok();
    let image_path = Path::new("assets/test.png");
    let image_bytes = std::fs::read(image_path).unwrap();
    let base64 = general_purpose::STANDARD.encode(&image_bytes);
    let prompt = "请用Java完成图中的算法题,给出解题思路和最终代码";
    let base64 = format!("data:image/png;base64,{}", base64);

    request_chat_completion(prompt.into(), base64).await;
}
