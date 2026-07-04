// ─── tool_dispatcher.rs ────────────────────────────────────────────────────────
// Tool Dispatcher：解析 LLM 输出 → 查表执行 → 结果回写上下文
#![allow(dead_code)]
//
// 支持两种调用格式（参考 OpenHuman XmlToolDispatcher）：
//
// 格式 A（XML-JSON，推荐，兼容 DeepSeek/Ollama）：
//   <tool_call>
//   {"name":"web_search","arguments":{"query":"Rust tokio"}}
//   </tool_call>
//
// 格式 B（OpenAI native function calling）：
//   response.choices[0].message.tool_calls = [{type,function:{name,arguments}}]
//
// 主循环（turn loop）：
//   1. 调用 LLM 获得回复
//   2. dispatcher.parse_xml(text) 提取工具调用
//   3. 对 L2/L3 工具发送 pending 请求给前端确认
//   4. 执行工具，结果格式化
//   5. 把工具结果追加到 messages，继续调 LLM
//   6. 循环直到没有工具调用（最多 MAX_TURNS 轮）
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use std::sync::Arc;
use tauri::Emitter;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::tool_registry::ToolRegistry;
use crate::tool_trait::{ToolCallRequest, ToolCallResult, ToolPermission};

/// 最多允许 8 轮工具调用循环，防止死循环
pub const MAX_TOOL_TURNS: usize = 8;

// ─── 待确认工具调用（L2/L3 需用户审批）────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PendingToolCall {
    pub id:        String,
    pub name:      String,
    pub arguments: serde_json::Value,
    pub level:     u8,  // 2 or 3
    pub approved:  Option<bool>,
}

pub type PendingMap = Arc<Mutex<HashMap<String, PendingToolCall>>>;

// ─── Dispatcher 主体 ──────────────────────────────────────────────────────────

pub struct ToolDispatcher;

impl ToolDispatcher {
    // ── 解析 XML-JSON 格式 ─────────────────────────────────────────────────────

    /// 从 LLM 回复文本中提取所有 <tool_call>...</tool_call> 块
    pub fn parse_xml(text: &str) -> Vec<ToolCallRequest> {
        let mut results = Vec::new();
        let mut pos = 0;

        while let Some(start) = text[pos..].find("<tool_call>") {
            let abs_start = pos + start + "<tool_call>".len();
            if let Some(end_offset) = text[abs_start..].find("</tool_call>") {
                let json_str = text[abs_start..abs_start + end_offset].trim();
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
                    let name = val.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let arguments = val.get("arguments").cloned().unwrap_or(serde_json::json!({}));
                    if !name.is_empty() {
                        results.push(ToolCallRequest {
                            id: Uuid::new_v4().to_string(),
                            name,
                            arguments,
                        });
                    }
                }
                pos = abs_start + end_offset + "</tool_call>".len();
            } else {
                break;
            }
        }

        results
    }

    /// 将 LLM 回复文本中的 <tool_call> 块替换为空（只保留文字部分）
    pub fn strip_tool_calls(text: &str) -> String {
        let mut result = text.to_string();
        while let Some(start) = result.find("<tool_call>") {
            if let Some(end) = result.find("</tool_call>") {
                result = format!("{}{}", &result[..start], &result[end + "</tool_call>".len()..]);
            } else {
                break;
            }
        }
        result.trim().to_string()
    }

    // ── 解析 OpenAI native tool_calls ─────────────────────────────────────────

    /// 解析 OpenAI native function calling 格式的 tool_calls
    pub fn parse_native(tool_calls: &serde_json::Value) -> Vec<ToolCallRequest> {
        let mut results = Vec::new();
        if let Some(arr) = tool_calls.as_array() {
            for tc in arr {
                let id = tc.get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let func = match tc.get("function") {
                    Some(f) => f,
                    None => continue,
                };
                let name = func.get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let arguments_str = func.get("arguments")
                    .and_then(|v| v.as_str())
                    .unwrap_or("{}");
                let arguments = serde_json::from_str(arguments_str)
                    .unwrap_or(serde_json::json!({}));

                if !name.is_empty() {
                    results.push(ToolCallRequest {
                        id: if id.is_empty() { Uuid::new_v4().to_string() } else { id },
                        name,
                        arguments,
                    });
                }
            }
        }
        results
    }

    // ── 执行所有工具调用 ──────────────────────────────────────────────────────

    /// 执行一批工具调用
    /// - L0/L1：直接执行
    /// - L2/L3：插入 pending map，等待前端确认（返回 pending 状态）
    pub async fn execute_all(
        registry:     &ToolRegistry,
        requests:     Vec<ToolCallRequest>,
        pending_map:  &PendingMap,
        app_handle:   Option<&tauri::AppHandle>,
    ) -> Vec<ToolCallResult> {
        let mut results = Vec::new();

        for req in requests {
            let tool = match registry.get(&req.name) {
                Some(t) => t.clone(),
                None => {
                    results.push(ToolCallResult::err(
                        &req.id,
                        &req.name,
                        ToolPermission::L0,
                        &format!("未知工具 \"{}\"", req.name),
                    ));
                    continue;
                }
            };

            let perm = tool.permission_level();

            if perm.needs_confirmation() {
                // L2/L3：发给前端确认
                let pending = PendingToolCall {
                    id:        req.id.clone(),
                    name:      req.name.clone(),
                    arguments: req.arguments.clone(),
                    level:     perm as u8 + 2, // L2=2, L3=3
                    approved:  None,
                };

                {
                    let mut map = pending_map.lock().await;
                    map.insert(req.id.clone(), pending.clone());
                }

                // 发送前端事件请求确认
                if let Some(app) = app_handle {
                    let _ = app.emit("tool_call_pending", &pending);
                }

                // 等待用户确认（最多 30 秒）
                let approved = wait_for_approval(&req.id, pending_map, 30).await;

                if approved {
                    let result = tool.execute(&req.id, req.arguments).await;
                    results.push(result);
                } else {
                    results.push(ToolCallResult::err(
                        &req.id,
                        &req.name,
                        perm,
                        "用户拒绝了此工具调用",
                    ));
                }

                // 清理 pending
                {
                    let mut map = pending_map.lock().await;
                    map.remove(&req.id);
                }
            } else {
                // L0/L1：直接执行
                let result = tool.execute(&req.id, req.arguments).await;
                results.push(result);
            }
        }

        results
    }

    /// 格式化工具结果为追加到对话的 message
    pub fn format_results_as_message(results: &[ToolCallResult]) -> String {
        results
            .iter()
            .map(|r| r.to_context_text())
            .collect::<Vec<_>>()
            .join("\n\n---\n\n")
    }
}

// ─── 等待用户确认 ─────────────────────────────────────────────────────────────

/// 轮询 pending_map 等待用户对某个工具调用作出决定（approved/rejected）
/// 最多等待 timeout_secs 秒
async fn wait_for_approval(
    id:          &str,
    pending_map: &PendingMap,
    timeout_secs: u64,
) -> bool {
    let deadline = tokio::time::Instant::now()
        + tokio::time::Duration::from_secs(timeout_secs);

    loop {
        if tokio::time::Instant::now() >= deadline {
            return false; // 超时默认拒绝
        }

        {
            let map = pending_map.lock().await;
            if let Some(call) = map.get(id) {
                if let Some(approved) = call.approved {
                    return approved;
                }
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
    }
}

// ─── 构建工具调用历史 message ─────────────────────────────────────────────────

/// 将工具结果整理成 "tool" 角色的 message（OpenAI chat 格式）
pub fn tool_results_to_messages(results: &[ToolCallResult]) -> Vec<serde_json::Value> {
    results
        .iter()
        .map(|r| serde_json::json!({
            "role": "tool",
            "tool_call_id": r.id,
            "content": r.to_context_text(),
        }))
        .collect()
}

/// 将工具结果整理成 user 消息（当模型不支持 tool role 时回退）
pub fn tool_results_to_user_message(results: &[ToolCallResult]) -> serde_json::Value {
    serde_json::json!({
        "role": "user",
        "content": ToolDispatcher::format_results_as_message(results),
    })
}
