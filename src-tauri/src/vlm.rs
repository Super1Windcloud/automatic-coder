#![allow(clippy::collapsible_if)]

use crate::capture::capture_screen_png_bytes;
use crate::config::{AppState, DEFAULT_VLM_MODEL, alternate_vlm_model, persist_vlm_model};
use crate::utils::{get_custom_openai_config, get_env_key};
use crate::{app_debug, app_error, app_info, app_warn};
use base64::{Engine, engine::general_purpose};
use reqwest::{Client, StatusCode};
use serde_json::{Value, json};
use std::fmt;
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
    Config(String),
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
            VlmError::Config(message) => write!(f, "配置错误: {message}"),
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

impl VlmError {
    fn is_model_disabled(&self) -> bool {
        match self {
            VlmError::Status { code, body } => {
                *code == StatusCode::FORBIDDEN && body.contains("Model disabled")
            }
            VlmError::Api(message) => message.contains("Model disabled"),
            _ => false,
        }
    }
}

fn log_vlm_error(context: &str, model: &str, err: &VlmError) {
    app_error!("vlm", "{context}: model={model}, error={err}");
}

fn append_delta_field(app_handle: &AppHandle, field: Option<&Value>, buffer: &mut String) -> bool {
    let mut appended = false;
    if let Some(value) = field {
        match value {
            Value::String(text) => appended |= append_segment(app_handle, buffer, text),
            Value::Array(items) => {
                for item in items {
                    if let Some(text) = item
                        .get("text")
                        .or_else(|| item.get("output_text"))
                        .or_else(|| item.get("content"))
                        .and_then(|node| node.as_str())
                    {
                        appended |= append_segment(app_handle, buffer, text);
                    }
                }
            }
            _ => {}
        }
    }
    appended
}

fn collect_plain_chunk(field: Option<&Value>) -> Option<String> {
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
                    if let Some(text) = item
                        .get("text")
                        .or_else(|| item.get("output_text"))
                        .or_else(|| item.get("content"))
                        .and_then(|node| node.as_str())
                    {
                        if !text.is_empty() {
                            chunk.push_str(text);
                        }
                    }
                }
            }
            _ => {}
        }
        if !chunk.is_empty() {
            return Some(chunk);
        }
    }
    None
}

#[allow(unused)]
fn append_plain_field(field: Option<&Value>, buffer: &mut String) -> Option<String> {
    if let Some(chunk) = collect_plain_chunk(field) {
        buffer.push_str(&chunk);
        return Some(chunk);
    }
    None
}

fn append_segment(app_handle: &AppHandle, buffer: &mut String, text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    buffer.push_str(text);
    if let Err(err) = app_handle.emit("completion_stream", text) {
        app_error!("vlm", "completion_stream 事件发送失败: {err}");
    }
    true
}

fn is_stream_finished(choice: &Value) -> bool {
    choice
        .get("finish_reason")
        .and_then(|reason| reason.as_str())
        .is_some_and(|reason| !reason.is_empty() && reason != "null")
}

fn extract_sse_events(buffer: &mut String, flush_tail: bool) -> Vec<String> {
    let normalized = buffer.replace("\r\n", "\n").replace('\r', "\n");
    *buffer = normalized;

    let mut events = Vec::new();
    while let Some(pos) = buffer.find("\n\n") {
        let event = buffer[..pos].to_string();
        let rest = buffer[pos + 2..].to_string();
        if !event.trim().is_empty() {
            events.push(event);
        }
        *buffer = rest;
    }

    if flush_tail && !buffer.trim().is_empty() {
        events.push(buffer.trim().to_string());
        buffer.clear();
    }

    events
}

fn event_payload(event: &str) -> Option<String> {
    let mut data_lines = Vec::new();
    for line in event.lines() {
        if let Some(data) = line
            .strip_prefix("data: ")
            .or_else(|| line.strip_prefix("data:"))
        {
            data_lines.push(data.trim_end());
        }
    }

    if data_lines.is_empty() {
        None
    } else {
        Some(data_lines.join("\n").trim().to_string())
    }
}

fn extract_choice(json_chunk: &Value) -> Option<&Value> {
    json_chunk
        .get("choices")
        .and_then(|choices| choices.as_array())
        .and_then(|choices| choices.first())
}

fn append_choice_content(app_handle: &AppHandle, choice: &Value, result: &mut String) -> bool {
    let mut appended = false;
    if let Some(delta_obj) = choice.get("delta") {
        appended |= append_delta_field(app_handle, delta_obj.get("content"), result);
        if let Some(reasoning) = collect_plain_chunk(delta_obj.get("reasoning_content")) {
            app_debug!("vlm", "reasoning_content: {}", reasoning);
        }
    }
    if let Some(message_obj) = choice.get("message") {
        appended |= append_delta_field(app_handle, message_obj.get("content"), result);
        if let Some(reasoning) = collect_plain_chunk(message_obj.get("reasoning_content")) {
            app_debug!("vlm", "reasoning_content: {}", reasoning);
        }
    }
    if let Some(text) = json_chunk_text(choice) {
        appended |= append_segment(app_handle, result, &text);
    }
    appended
}

fn json_chunk_text(value: &Value) -> Option<String> {
    if let Some(text) = value.get("text").and_then(|node| node.as_str()) {
        if !text.is_empty() {
            return Some(text.to_string());
        }
    }
    if let Some(text) = value.get("output_text").and_then(|node| node.as_str()) {
        if !text.is_empty() {
            return Some(text.to_string());
        }
    }
    if let Some(content) = value.get("content") {
        return collect_plain_chunk(Some(content));
    }
    None
}

fn handle_openai_event(
    app_handle: &AppHandle,
    trimmed: &str,
    result: &mut String,
    finished: &mut bool,
    strict_choices: bool,
) -> Result<(), VlmError> {
    if trimmed.is_empty() {
        return Ok(());
    }
    if trimmed == "[DONE]" {
        let _ = app_handle.emit("completion_done", "");
        *finished = true;
        return Ok(());
    }

    let json_chunk: Value = serde_json::from_str(trimmed).map_err(|source| VlmError::StreamJson {
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

    let Some(choice) = extract_choice(&json_chunk) else {
        if strict_choices {
            return Err(VlmError::StreamShape(format!(
                "响应缺少 choices 字段，原始片段: {}",
                trimmed
            )));
        }
        if let Some(text) = json_chunk_text(&json_chunk) {
            append_segment(app_handle, result, &text);
        }
        return Ok(());
    };

    if is_stream_finished(choice) {
        let _ = app_handle.emit("completion_done", "");
        *finished = true;
        return Ok(());
    }

    let appended = append_choice_content(app_handle, choice, result);
    if !appended && strict_choices && choice.get("delta").is_none() && choice.get("message").is_none()
    {
        return Err(VlmError::StreamShape(format!(
            "响应缺少 delta 或 message 字段，原始片段: {}",
            trimmed
        )));
    }

    Ok(())
}

async fn request_chat_completion_stream(
    app_handle: &AppHandle,
    model: &str,
    prompt: String,
    image_url: String,
) -> Result<String, VlmError> {
    let (custom_openai_enabled, custom_api_key, custom_base_url, custom_model) =
        get_custom_openai_config();
    if custom_openai_enabled {
        return request_custom_openai_stream(
            app_handle,
            &sanitize_custom_model(&custom_model),
            &sanitize_custom_base_url(&custom_base_url),
            &custom_api_key,
            prompt,
            image_url,
        )
        .await;
    }

    app_info!("vlm", "request started with model: {model}");
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
    if api_key.trim().is_empty() {
        return Err(VlmError::Config("请先在设置中填写 API Key".into()));
    }
    let client = Client::new();

    let body = if model == "zai-org/GLM-4.5V"
        || model == "Qwen/Qwen3.5-122B-A10B"
        || model == "Qwen/Qwen3.5-397B-A17B"
        || model == "Pro/moonshotai/Kimi-K2.5"
    {
        json!({
            "model": model,
            "stream": true,
            "messages": messages,
            "enable_thinking" :false
        })
    } else if model == "Qwen/Qwen3-VL-235B-A22B-Instruct" {
        json!({
            "model": model,
            "stream": true,
            "messages": messages
        })
    } else {
        json!(null)
    };
    if body.is_null() {
        return Err(VlmError::Api(String::from("暂不支持该模型")));
    }

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
    let mut pending = String::new();
    while let Some(chunk) = timeout(Duration::from_secs(5), res.chunk())
        .await
        .map_err(|_| VlmError::Timeout("VLM 流式响应"))?
        .map_err(VlmError::Chunk)?
    {
        let text = String::from_utf8_lossy(&chunk);
        pending.push_str(&text);

        for event in extract_sse_events(&mut pending, false) {
            if let Some(payload) = event_payload(&event) {
                handle_openai_event(app_handle, &payload, &mut result, &mut finished, true)?;
                if finished {
                    break;
                }
            }
        }
        if finished {
            break;
        }
    }
    if !pending.trim().is_empty() && !finished {
        for event in extract_sse_events(&mut pending, true) {
            if let Some(payload) = event_payload(&event) {
                handle_openai_event(app_handle, &payload, &mut result, &mut finished, true)?;
                if finished {
                    break;
                }
            }
        }
    }
    if !finished && !result.trim().is_empty() {
        app_warn!("vlm", "stream ended without explicit end marker, treating buffered content as complete");
        let _ = app_handle.emit("completion_done", "");
        finished = true;
    }
    if !finished {
        return Err(VlmError::StreamShape("LLM 流式响应未发送结束标记".into()));
    }
    if result.trim().is_empty() {
        return Err(VlmError::EmptyResponse);
    }
    Ok(result)
}

fn sanitize_custom_base_url(base_url: &str) -> String {
    let normalized = base_url.trim().trim_end_matches('/');
    if normalized.is_empty() {
        "https://api.openai.com/v1".to_string()
    } else {
        normalized.to_string()
    }
}

fn sanitize_custom_model(model: &str) -> String {
    let normalized = model.trim();
    if normalized.is_empty() {
        "gpt-4o".to_string()
    } else {
        normalized.to_string()
    }
}

async fn request_custom_openai_stream(
    app_handle: &AppHandle,
    model: &str,
    base_url: &str,
    api_key: &str,
    prompt: String,
    image_url: String,
) -> Result<String, VlmError> {
    app_info!(
        "vlm",
        "request started with custom openai-compatible api: model={}, base_url={}",
        model,
        base_url
    );

    if api_key.trim().is_empty() {
        return Err(VlmError::Config(
            "已启用自定义 OpenAI 兼容 API，但尚未填写 API Key".into(),
        ));
    }

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

    let body = json!({
        "model": model,
        "stream": true,
        "messages": messages
    });

    let endpoint = format!("{}/chat/completions", base_url);
    let client = Client::new();
    let send_future = client
        .post(endpoint)
        .bearer_auth(api_key)
        .json(&body)
        .send();

    let mut res = timeout(Duration::from_secs(5), send_future)
        .await
        .map_err(|_| VlmError::Timeout("自定义 OpenAI 接口请求"))?
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
    let mut pending = String::new();
    while let Some(chunk) = timeout(Duration::from_secs(5), res.chunk())
        .await
        .map_err(|_| VlmError::Timeout("自定义 OpenAI 流式响应"))?
        .map_err(VlmError::Chunk)?
    {
        let text = String::from_utf8_lossy(&chunk);
        pending.push_str(&text);
        for event in extract_sse_events(&mut pending, false) {
            if let Some(payload) = event_payload(&event) {
                handle_openai_event(app_handle, &payload, &mut result, &mut finished, false)?;
                if finished {
                    break;
                }
            }
        }
        if finished {
            break;
        }
    }

    if !pending.trim().is_empty() && !finished {
        for event in extract_sse_events(&mut pending, true) {
            if let Some(payload) = event_payload(&event) {
                handle_openai_event(app_handle, &payload, &mut result, &mut finished, false)?;
                if finished {
                    break;
                }
            }
        }
    }

    if !finished && !result.trim().is_empty() {
        app_warn!(
            "vlm",
            "custom openai stream ended without explicit end marker, treating buffered content as complete"
        );
        let _ = app_handle.emit("completion_done", "");
        finished = true;
    }
    if !finished {
        return Err(VlmError::StreamShape(
            "自定义 OpenAI 流式响应未发送结束标记".into(),
        ));
    }
    if result.trim().is_empty() {
        return Err(VlmError::EmptyResponse);
    }
    Ok(result)
}

#[tauri::command]
pub async fn create_screenshot_solution_stream(app_handle: AppHandle) -> Result<String, String> {
    let state = app_handle.state::<AppState>();
    let (custom_openai_enabled, _, _, custom_model) = get_custom_openai_config();
    let prompt = state.prompt.lock().unwrap().clone();
    let direction = *state
        .capture_position
        .lock()
        .map_err(|_| "capture position lock poisoned".to_string())?;
    let bytes = capture_screen_png_bytes(direction)?;
    app_info!(
        "vlm",
        "using in-memory screenshot {} bytes ({:.2} KiB)",
        bytes.len(),
        bytes.len() as f64 / 1024.0
    );

    let base64_str = general_purpose::STANDARD.encode(&bytes);
    let base64 = format!("data:image/png;base64,{}", base64_str);
    let model_name = if custom_openai_enabled {
        sanitize_custom_model(&custom_model)
    } else {
        let locked = state.vlm_model.lock().unwrap();
        if locked.is_empty() {
            DEFAULT_VLM_MODEL.to_string()
        } else {
            locked.clone()
        }
    };
    app_info!("vlm", "create solution using model: {model_name}");
    match request_chat_completion_stream(&app_handle, &model_name, prompt.clone(), base64.clone())
        .await
    {
        Ok(result) => Ok(result),
        Err(err) if err.is_model_disabled() && !custom_openai_enabled => {
            let fallback_model = alternate_vlm_model(&model_name).to_string();
            if fallback_model == model_name {
                log_vlm_error("request_chat_completion_stream", &model_name, &err);
                return Err(format!("模型 {model_name}: {err}"));
            }
            app_warn!(
                "vlm",
                "model {model_name} disabled, switching to fallback model {fallback_model}"
            );
            persist_vlm_model(&app_handle, &fallback_model)
                .map_err(|persist_err| format!("模型切换失败: {persist_err}"))?;

            match request_chat_completion_stream(&app_handle, &fallback_model, prompt, base64).await
            {
                Ok(result) => Ok(result),
                Err(retry_err) => {
                    log_vlm_error(
                        "request_chat_completion_stream fallback",
                        &fallback_model,
                        &retry_err,
                    );
                    Err(format!("模型 {fallback_model}: {retry_err}"))
                }
            }
        }
        Err(err) => {
            log_vlm_error("request_chat_completion_stream", &model_name, &err);
            Err(format!("模型 {model_name}: {err}"))
        }
    }
}


#[allow(unused)]
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

#[allow(unused)]
fn calc_cost<T: ToF64, U: ToF64>(input_tokens: T, output_tokens: U) -> f64 {
    let input_price_per_m = 4.0; // ¥1 / M tokens
    let output_price_per_m = 10.0; // ¥6 / M tokens

    let result = (input_tokens.to_f64() * input_price_per_m
        + output_tokens.to_f64() * output_price_per_m)
        / 1_000_000.0;
    app_info!("vlm", "cost: {} ¥", result);
    result
}
