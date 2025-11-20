#![allow(clippy::collapsible_if)]

use crate::config::{AppState, DEFAULT_VLM_MODEL};
use crate::utils::{get_env_key, is_dev, write_some_log};
use base64::{Engine, engine::general_purpose};
use reqwest::{Client, StatusCode};
use serde_json::{Value, json};
use std::{fmt, path::Path};
use tauri::{AppHandle, Emitter, Manager};
use tokio::time::{Duration, timeout};

#[derive(Debug)]
enum VlmError {
    Request(reqwest::Error),
    Status {
        code: StatusCode,
        body: String,
    },
    Chunk(reqwest::Error),
    StreamJson {
        raw: String,
        source: serde_json::Error,
    },
    StreamShape(String),
    Api(String),
    EmptyResponse,
    Timeout(&'static str),
}

impl fmt::Display for VlmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VlmError::Request(err) => write!(f, "请求接口失败: {err}"),
            VlmError::Status { code, body } => {
                write!(f, "接口返回错误状态 {code}: {body}")
            }
            VlmError::Chunk(err) => write!(f, "读取流式响应失败: {err}"),
            VlmError::StreamJson { raw, source } => {
                write!(f, "解析流式响应失败: {source}，原始数据: {raw}")
            }
            VlmError::StreamShape(msg) => write!(f, "响应结构异常: {msg}"),
            VlmError::Api(message) => write!(f, "API 错误: {message}"),
            VlmError::EmptyResponse => write!(f, "LLM 返回空内容"),
            VlmError::Timeout(context) => write!(f, "{context}，操作超时 (5s)"),
        }
    }
}

impl std::error::Error for VlmError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            VlmError::Request(err) | VlmError::Chunk(err) => Some(err),
            VlmError::StreamJson { source, .. } => Some(source),
            _ => None,
        }
    }
}

fn log_vlm_error(context: &str, err: &VlmError) {
    let message = format!("[VLM] {context}: {err}");
    if is_dev() {
        println!("{message}");
    } else {
        write_some_log(&message);
    }
}

fn append_delta_field(app_handle: &AppHandle, field: Option<&Value>, buffer: &mut String) -> bool {
    let mut appended = false;
    if let Some(value) = field {
        match value {
            Value::String(text) => appended |= append_segment(app_handle, buffer, text),
            Value::Array(items) => {
                for item in items {
                    if let Some(text) = item.get("text").and_then(|node| node.as_str()) {
                        appended |= append_segment(app_handle, buffer, text);
                    }
                }
            }
            _ => {}
        }
    }
    appended
}

fn append_plain_field(field: Option<&Value>, buffer: &mut String) -> Option<String> {
    if let Some(value) = field {
        let mut chunk = String::new();
        match value {
            Value::String(text) => {
                if !text.is_empty() {
                    chunk.push_str(text);
                }
            }
            Value::Array(items) => {
                for item in items {
                    if let Some(text) = item.get("text").and_then(|node| node.as_str()) {
                        if !text.is_empty() {
                            chunk.push_str(text);
                        }
                    }
                }
            }
            _ => {}
        }
        if !chunk.is_empty() {
            buffer.push_str(&chunk);
            return Some(chunk);
        }
    }
    None
}

fn append_segment(app_handle: &AppHandle, buffer: &mut String, text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    buffer.push_str(text);
    if let Err(err) = app_handle.emit("completion_stream", text) {
        let message = format!("completion_stream 事件发送失败: {err}");
        if is_dev() {
            println!("{message}");
        } else {
            write_some_log(&message);
        }
    }
    true
}

#[allow(dead_code)]
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
        "model": "stepfun-ai/step3",
        // "model": "zai-org/GLM-4.5V",
        "messages": messages
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
        println!("chunk: {}", text);
        for line in text.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                if data.trim() == "[DONE]" {
                    println!("流式完成事件");
                    break;
                }

                if let Ok(json_chunk) = serde_json::from_str::<Value>(data) {
                    if let Some(error) = json_chunk.get("error") {
                        return format!("API错误: {}", error);
                    }

                    if let Some(choice) = json_chunk
                        .get("choices")
                        .and_then(|choices| choices.as_array())
                        .and_then(|choices| choices.first())
                    {
                        let delta = choice.get("delta");
                        let message = choice.get("message");

                        if let Some(delta_obj) = delta {
                            if let Some(added) =
                                append_plain_field(delta_obj.get("content"), &mut result)
                            {
                                println!("content: {}", added);
                            }
                            if let Some(reasoning) =
                                append_plain_field(delta_obj.get("reasoning_content"), &mut reason)
                            {
                                println!("reasoning_content: {}", reasoning);
                            }
                        }

                        if let Some(message_obj) = message {
                            if let Some(added) =
                                append_plain_field(message_obj.get("content"), &mut result)
                            {
                                println!("content: {}", added);
                            }
                            if let Some(reasoning) = append_plain_field(
                                message_obj.get("reasoning_content"),
                                &mut reason,
                            ) {
                                println!("reasoning_content: {}", reasoning);
                            }
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
    model: &str,
    prompt: String,
    image_url: String,
) -> Result<String, VlmError> {
    dbg!(model);
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

    let body = if model == "zai-org/GLM-4.5V" {
        json!({
            "model": model,
            "stream": true,
            "messages": messages,
            "enable_thinking" :false
        })
    } else {
        json!({
            "model": model,
            "stream": true,
            "messages": messages,
        })
    };

    let send_future = client
        .post("https://api.siliconflow.cn/v1/chat/completions")
        .bearer_auth(api_key)
        .json(&body)
        .send();
    let mut res = timeout(Duration::from_secs(5), send_future)
        .await
        .map_err(|_| VlmError::Timeout("VLM 接口请求"))?
        .map_err(VlmError::Request)?;

    if let Err(status_err) = res.error_for_status_ref() {
        let status = status_err
            .status()
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let body = res.text().await.unwrap_or_default();
        return Err(VlmError::Status { code: status, body });
    }

    let mut result = String::new();
    let mut finished = false;
    while let Some(chunk) = timeout(Duration::from_secs(5), res.chunk())
        .await
        .map_err(|_| VlmError::Timeout("VLM 流式响应"))?
        .map_err(VlmError::Chunk)?
    {
        let text = String::from_utf8_lossy(&chunk);

        for line in text.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                let trimmed = data.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if trimmed == "[DONE]" {
                    let _ = app_handle.emit("completion_done", "");
                    finished = true;
                    break;
                }

                let json_chunk: Value =
                    serde_json::from_str(trimmed).map_err(|source| VlmError::StreamJson {
                        raw: trimmed.to_string(),
                        source,
                    })?;

                if let Some(error) = json_chunk.get("error") {
                    let message = error
                        .get("message")
                        .and_then(|val| val.as_str())
                        .map(|val| val.to_string())
                        .unwrap_or_else(|| error.to_string());
                    return Err(VlmError::Api(message));
                }

                let choice = json_chunk
                    .get("choices")
                    .and_then(|choices| choices.as_array())
                    .and_then(|choices| choices.first())
                    .ok_or_else(|| VlmError::StreamShape("响应缺少 choices 字段".into()))?;

                let delta = choice.get("delta");
                let message = choice.get("message");
                if delta.is_none() && message.is_none() {
                    return Err(VlmError::StreamShape(
                        "响应缺少 delta 或 message 字段".into(),
                    ));
                }

                let mut appended = false;
                if let Some(delta_obj) = delta {
                    appended |=
                        append_delta_field(app_handle, delta_obj.get("content"), &mut result);
                    appended |= append_delta_field(
                        app_handle,
                        delta_obj.get("reasoning_content"),
                        &mut result,
                    );
                }
                if let Some(message_obj) = message {
                    appended |=
                        append_delta_field(app_handle, message_obj.get("content"), &mut result);
                    appended |= append_delta_field(
                        app_handle,
                        message_obj.get("reasoning_content"),
                        &mut result,
                    );
                }

                if !appended {
                    continue;
                }
            }
        }
        if finished {
            break;
        }
    }
    if !finished {
        return Err(VlmError::StreamShape("LLM 流式响应未发送结束标记".into()));
    }
    if result.trim().is_empty() {
        return Err(VlmError::EmptyResponse);
    }
    Ok(result)
}

#[tauri::command]
pub async fn create_screenshot_solution_stream(app_handle: AppHandle) -> Result<String, String> {
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
    let entries: Vec<_> = std::fs::read_dir(&assets_path)
        .map_err(|err| format!("读取资源目录失败: {err}"))?
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
        let bytes =
            std::fs::read(first_image.path()).map_err(|err| format!("读取图片失败: {err}"))?;
        let base64_str = general_purpose::STANDARD.encode(&bytes);
        let base64 = format!("data:image/png;base64,{}", base64_str);
        let model_name = {
            let locked = state.vlm_model.lock().unwrap();
            if locked.is_empty() {
                DEFAULT_VLM_MODEL.to_string()
            } else {
                locked.clone()
            }
        };
        match request_chat_completion_stream(&app_handle, &model_name, prompt, base64).await {
            Ok(result) => Ok(result),
            Err(err) => {
                log_vlm_error("request_chat_completion_stream", &err);
                Err(err.to_string())
            }
        }
    } else {
        Err("没有找到图片文件".to_string())
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
    let image_path = Path::new("enc/test.png");
    let image_bytes = std::fs::read(image_path).unwrap();
    let base64 = general_purpose::STANDARD.encode(&image_bytes);
    let prompt = "图中是什么";
    let base64 = format!("data:image/png;base64,{}", base64);

    request_chat_completion_stream_thinking(prompt.into(), base64).await;
}
