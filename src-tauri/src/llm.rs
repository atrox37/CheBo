// ─── llm.rs ───────────────────────────────────────────────────────────────────
// 通过 reqwest 对 OpenAI-compatible API（默认 DeepSeek）做流式调用
// 每个 token chunk 通过 app.emit("assistant_chunk", ...) 推送到前端
// 支持多模态：当 images 不为空且模型支持 vision 时，以 multipart content 格式发送
// ─────────────────────────────────────────────────────────────────────────────
#![allow(dead_code)]

use anyhow::{anyhow, Result};
use futures_util::StreamExt;
use reqwest::{Client, Response, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

use crate::agent::AgentRuntime;
use crate::character::detect_emotion;

/// LLM 配置（从 AppConfig 传入，可克隆）
#[derive(Debug, Clone)]
pub struct LlmConfig {
    pub api_key:   String,
    pub base_url:  String,
    pub model:     String,
    pub temperature: f64,
    pub max_tokens:  u32,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            api_key:     String::new(),
            base_url:    "https://api.deepseek.com/v1".to_string(),
            model:       "deepseek-chat".to_string(),
            temperature: 0.8,
            max_tokens:  1000,
        }
    }
}

/// 发给 LLM 的单条消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmMessage {
    pub role:    String,
    pub content: String,
}

impl LlmMessage {
    pub fn system(content: &str) -> Self {
        Self { role: "system".to_string(), content: content.to_string() }
    }
    pub fn user(content: &str) -> Self {
        Self { role: "user".to_string(), content: content.to_string() }
    }
    pub fn assistant(content: &str) -> Self {
        Self { role: "assistant".to_string(), content: content.to_string() }
    }
}

// ─── 多模态消息构建 ───────────────────────────────────────────────────────────

/// 将 LlmMessage 列表转换为 API 请求的 messages JSON
///
/// 当 `images` 不为空时，最后一条 user 消息会转为 multipart content，
/// 其余消息保持纯文字格式（对视觉模型这是合法的混合格式）。
fn build_api_messages(messages: &[LlmMessage], images: &[String]) -> Value {
    let last_user_idx = messages
        .iter()
        .enumerate()
        .rev()
        .find(|(_, m)| m.role == "user")
        .map(|(i, _)| i);

    let arr: Vec<Value> = messages
        .iter()
        .enumerate()
        .map(|(i, msg)| {
            let inject_images = !images.is_empty()
                && Some(i) == last_user_idx;

            if inject_images {
                // multipart content: [text_part, image_part, ...]
                let mut parts = vec![
                    serde_json::json!({ "type": "text", "text": msg.content })
                ];
                for img_data_url in images {
                    parts.push(serde_json::json!({
                        "type": "image_url",
                        "image_url": { "url": img_data_url }
                    }));
                }
                serde_json::json!({ "role": msg.role, "content": parts })
            } else {
                serde_json::json!({ "role": msg.role, "content": msg.content })
            }
        })
        .collect();

    Value::Array(arr)
}

const LLM_MAX_RETRIES: u32 = 3;
const RETRYABLE_STATUS: &[u16] = &[429, 500, 502, 503];

fn http_client() -> Result<Client> {
    Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| anyhow!("创建 HTTP 客户端失败：{e}"))
}

fn chat_completions_url(base_url: &str) -> String {
    format!("{}/chat/completions", base_url.trim_end_matches('/'))
}

/// 从 OpenAI 兼容 API 的错误体中提取可读信息
fn parse_api_error_message(body: &str) -> Option<String> {
    let json: Value = serde_json::from_str(body).ok()?;
    json.pointer("/error/message")
        .or_else(|| json.get("message"))
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn format_llm_error(status: StatusCode, body: &str) -> String {
    let code = status.as_u16();
    let detail = parse_api_error_message(body)
        .unwrap_or_else(|| body.chars().take(160).collect());

    match code {
        401 | 403 => format!("API Key 无效或无权访问（{code}）"),
        404 => format!("模型或接口不存在（{code}），请检查设置中的模型名称与 Base URL"),
        429 => {
            if detail.to_lowercase().contains("rate") {
                "请求过于频繁，请稍后再试".to_string()
            } else {
                format!("请求过于频繁（{code}）：{detail}")
            }
        }
        500 | 502 | 503 => {
            let busy = detail.to_lowercase();
            if busy.contains("busy") || busy.contains("unavailable") || busy.contains("overload") {
                format!("AI 服务当前繁忙，已自动重试仍失败，请稍后再试")
            } else {
                format!("AI 服务暂时不可用（{code}）：{detail}")
            }
        }
        _ => format!("LLM 请求失败（{code}）：{detail}"),
    }
}

fn retry_delay_ms(_status: StatusCode, attempt: u32) -> u64 {
    // 指数退避：1s → 2s → 4s
    1000u64.saturating_mul(2u64.saturating_pow(attempt))
}

fn is_retryable(status: StatusCode) -> bool {
    RETRYABLE_STATUS.contains(&status.as_u16())
}

async fn post_chat_completion(
    client: &Client,
    cfg: &LlmConfig,
    body: &Value,
) -> Result<Response> {
    let url = chat_completions_url(&cfg.base_url);

    for attempt in 0..=LLM_MAX_RETRIES {
        let resp = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", cfg.api_key))
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await?;

        if resp.status().is_success() {
            return Ok(resp);
        }

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();

        if is_retryable(status) && attempt < LLM_MAX_RETRIES {
            let delay = retry_delay_ms(status, attempt);
            log::warn!(
                "LLM {} 可重试，{delay}ms 后第 {} 次重试",
                status,
                attempt + 1
            );
            tokio::time::sleep(Duration::from_millis(delay)).await;
            continue;
        }

        return Err(anyhow!(format_llm_error(status, &text)));
    }

    Err(anyhow!("LLM 请求失败：超过最大重试次数"))
}

/// 非流式调用视觉模型，提取图片描述（用于视觉降级路由）
///
/// 返回图片描述文字，失败时返回错误。
pub async fn describe_images(
    images: &[String],
    user_text: &str,
    cfg: &LlmConfig,
) -> Result<String> {
    if images.is_empty() { return Ok(String::new()); }
    if cfg.api_key.is_empty() {
        return Err(anyhow!("视觉模型 API key 未配置"));
    }

    let prompt = if user_text.is_empty() {
        "请详细描述这些图片的内容，包括文字、图表、截图内容等。用中文回答。".to_string()
    } else {
        format!("用户问：{user_text}\n请根据图片内容回答，并描述图片中相关信息。用中文回答。")
    };

    let messages = vec![LlmMessage::user(&prompt)];
    let api_messages = build_api_messages(&messages, images);

    let client = http_client()?;
    let body = serde_json::json!({
        "model":       cfg.model,
        "messages":    api_messages,
        "temperature": 0.3,
        "max_tokens":  800,
        "stream":      false,
    });

    let resp = post_chat_completion(&client, cfg, &body).await?;

    let json: Value = resp.json().await?;
    let content = json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("（图片描述获取失败）")
        .to_string();

    Ok(content)
}

/// 流式对话核心函数
///
/// - `images`: base64 data URL 列表（如 "data:image/png;base64,..."）
///   当模型支持视觉时直接注入；不支持时调用方负责预处理（视觉降级）
/// - 每 token 通过 `app.emit("assistant_chunk", { content })` 推送
/// - 完成后通过 `app.emit("assistant_done", { full_content, emotion })` 推送
/// - 返回 (clean_text, emotion)
pub async fn stream_chat(
    app: &AppHandle,
    messages: Vec<LlmMessage>,
    images: &[String],
    cfg: &LlmConfig,
) -> Result<(String, String)> {
    if cfg.api_key.is_empty() {
        return Err(anyhow!("LLM API key 未配置，请在设置中填写"));
    }

    let client = http_client()?;
    let api_messages = build_api_messages(&messages, images);
    let body = serde_json::json!({
        "model":       cfg.model,
        "messages":    api_messages,
        "temperature": cfg.temperature,
        "max_tokens":  cfg.max_tokens,
        "stream":      true,
    });

    let resp = post_chat_completion(&client, cfg, &body).await?;

    let mut stream     = resp.bytes_stream();
    let mut full_text  = String::new();
    let mut line_buf   = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        let text  = String::from_utf8_lossy(&chunk);
        line_buf.push_str(&text);

        // SSE 以 \n 或 \r\n 分隔行，逐行处理
        loop {
            let pos = match line_buf.find('\n') {
                Some(p) => p,
                None    => break,
            };
            let line = line_buf[..pos].trim_end_matches('\r').to_string();
            line_buf  = line_buf[pos + 1..].to_string();

            if let Some(data) = line.strip_prefix("data: ") {
                if data.trim() == "[DONE]" {
                    continue;
                }
                if let Ok(json) = serde_json::from_str::<Value>(data) {
                    let content = json["choices"][0]["delta"]["content"]
                        .as_str()
                        .unwrap_or("");
                    if !content.is_empty() {
                        full_text.push_str(content);
                        // 推送 token 块到前端
                        let _ = app.emit(
                            "assistant_chunk",
                            serde_json::json!({ "content": content }),
                        );
                    }
                }
            }
        }
    }

    let (clean, emotion) = detect_emotion(&full_text);

    // 推送完成事件
    let _ = app.emit(
        "assistant_done",
        serde_json::json!({
            "full_content": clean,
            "emotion":      emotion,
        }),
    );

    Ok((clean, emotion))
}

const TOOL_CALL_MARKER: &str = "<tool_call";

/// Agent 单轮流式输出结果
pub enum AgentStreamOutcome {
    /// 流式完成；`streamed` 为 true 表示已向 UI 推送 chunk 并发了 assistant_done
    Complete {
        raw:      String,
        emotion:  String,
        streamed: bool,
    },
    /// 用户取消；`partial` 为已生成的文本
    Cancelled { partial: String },
}

/// Agent 工具循环单轮：真流式读取 LLM，无工具时 token 直达前端；
/// 检测到 `<tool_call` 时停止 UI 推送并发出 assistant_stream_reset
pub async fn stream_agent_turn(
    app:    &AppHandle,
    messages: Vec<LlmMessage>,
    images:   &[String],
    cfg:      &LlmConfig,
    cancel:   Arc<AtomicBool>,
    agent:    &AgentRuntime,
) -> AgentStreamOutcome {
    if cfg.api_key.is_empty() {
        log::error!("LLM API key 未配置");
        return AgentStreamOutcome::Cancelled {
            partial: String::new(),
        };
    }

    let client = match http_client() {
        Ok(c) => c,
        Err(e) => {
            log::error!("http client: {e}");
            return AgentStreamOutcome::Cancelled {
                partial: String::new(),
            };
        }
    };

    let api_messages = build_api_messages(&messages, images);
    let body = serde_json::json!({
        "model":       cfg.model,
        "messages":    api_messages,
        "temperature": cfg.temperature,
        "max_tokens":  cfg.max_tokens,
        "stream":      true,
    });

    let resp = match post_chat_completion(&client, cfg, &body).await {
        Ok(r) => r,
        Err(e) => {
            let _ = app.emit("backend_error", format!("LLM 调用失败：{e}"));
            return AgentStreamOutcome::Cancelled {
                partial: String::new(),
            };
        }
    };

    let mut stream          = resp.bytes_stream();
    let mut full_text       = String::new();
    let mut line_buf        = String::new();
    let mut ui_open         = true;
    let mut talking_set     = false;
    let mut stream_reset    = false;
    let mut current_section = String::new(); // "thought" | "action" | "final" | ""

    /// 检测当前 full_text 进入了哪个章节
    fn detect_section(text: &str) -> &str {
        // 从末尾向前搜索最近的章节标记（按优先级）
        if text.contains("\nFinal Answer：") || text.starts_with("Final Answer：") {
            "final"
        } else if text.contains("\nAction：") || text.starts_with("Action：") {
            "action"
        } else if text.contains("\nThought：") || text.starts_with("Thought：") {
            "thought"
        } else {
            ""
        }
    }

    while let Some(chunk_result) = stream.next().await {
        if cancel.load(Ordering::Relaxed) {
            return AgentStreamOutcome::Cancelled {
                partial: full_text,
            };
        }

        let chunk = match chunk_result {
            Ok(c) => c,
            Err(e) => {
                let _ = app.emit("backend_error", format!("LLM 流读取失败：{e}"));
                return AgentStreamOutcome::Cancelled {
                    partial: full_text,
                };
            }
        };

        let text = String::from_utf8_lossy(&chunk);
        line_buf.push_str(&text);

        loop {
            let pos = match line_buf.find('\n') {
                Some(p) => p,
                None    => break,
            };
            let line = line_buf[..pos].trim_end_matches('\r').to_string();
            line_buf = line_buf[pos + 1..].to_string();

            if let Some(data) = line.strip_prefix("data: ") {
                if data.trim() == "[DONE]" {
                    continue;
                }
                if let Ok(json) = serde_json::from_str::<Value>(data) {
                    let content = json["choices"][0]["delta"]["content"]
                        .as_str()
                        .unwrap_or("");
                    if content.is_empty() {
                        continue;
                    }

                    full_text.push_str(content);

                    // ── 章节切换检测 ────────────────────────────────────────
                    let new_section = detect_section(&full_text);
                    if new_section != current_section.as_str() && !new_section.is_empty() {
                        current_section = new_section.to_string();
                        let _ = app.emit("assistant_section", serde_json::json!({
                            "section": current_section,
                        }));
                    }

                    if full_text.contains(TOOL_CALL_MARKER) {
                        if ui_open {
                            ui_open = false;
                            stream_reset = true;
                            let _ = app.emit("assistant_stream_reset", ());
                        }
                    } else if ui_open {
                        if !talking_set {
                            talking_set = true;
                            agent.set_talking(app);
                        }
                        let _ = app.emit(
                            "assistant_chunk",
                            serde_json::json!({ "content": content }),
                        );
                    }
                }
            }
        }
    }

    if cancel.load(Ordering::Relaxed) {
        return AgentStreamOutcome::Cancelled {
            partial: full_text,
        };
    }

    let (clean, emotion) = detect_emotion(&full_text);
    let streamed = ui_open && !full_text.is_empty();

    if streamed {
        let _ = app.emit(
            "assistant_done",
            serde_json::json!({
                "full_content": clean.clone(),
                "emotion":      emotion.clone(),
            }),
        );
    } else if stream_reset {
        let _ = stream_reset; // 已发 assistant_stream_reset
    }

    AgentStreamOutcome::Complete {
        raw: full_text,
        emotion,
        streamed,
    }
}

/// 非流式（静默）调用，用于后台任务（如主动评论、工具循环）
/// 不向前端推送 chunk，调用方按需处理结果
pub async fn call_silent(
    messages: Vec<LlmMessage>,
    cfg: &LlmConfig,
) -> Result<(String, String)> {
    call_silent_with_images(messages, &[], cfg).await
}

/// call_silent 带图片支持版本
pub async fn call_silent_with_images(
    messages: Vec<LlmMessage>,
    images: &[String],
    cfg: &LlmConfig,
) -> Result<(String, String)> {
    if cfg.api_key.is_empty() {
        return Err(anyhow!("LLM API key 未配置"));
    }

    let client = http_client()?;
    let api_messages = build_api_messages(&messages, images);
    let body = serde_json::json!({
        "model":       cfg.model,
        "messages":    api_messages,
        "temperature": cfg.temperature,
        "max_tokens":  cfg.max_tokens,
        "stream":      false,
    });

    let resp = post_chat_completion(&client, cfg, &body).await?;

    let json: Value = resp.json().await?;
    let content = json["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    Ok(detect_emotion(&content))
}
