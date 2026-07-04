// ─── task/task_planner.rs ─────────────────────────────────────────────────────
// LLM 驱动的任务规划器
//
// 职责：
//   把用户的自然语言目标（goal）拆解成若干步骤（TaskStep）
//   通过 call_silent 调用 LLM，要求以 JSON 格式返回步骤列表
// ──────────────────────────────────────────────────────────────────────────────

use anyhow::{Context, Result};
use serde_json::Value;

use crate::llm::{self, LlmConfig, LlmMessage};
use super::task::TaskStep;

const PLAN_SYSTEM_PROMPT: &str = r#"
你是一个任务规划助手。你的任务是：
把用户给出的目标，拆分成可执行的步骤列表。

规则：
1. 步骤数量：3-8步，不要太多也不要太少
2. 每个步骤必须是具体、可操作的
3. 如果步骤需要调用工具，在 tool_name 和 tool_args 中指明
4. 可用工具：read_file / list_dir / web_search / memory_recall / git_status / clipboard_read / safe_shell
5. 如果步骤包含写操作或不可逆操作，设置 requires_confirm: true
6. 以 JSON 格式返回，格式如下：

{
  "title": "任务标题（10字以内）",
  "steps": [
    {
      "title": "步骤标题",
      "description": "详细说明这一步要做什么",
      "tool_name": "工具名（没有工具填null）",
      "tool_args": {"参数名": "参数值"},
      "requires_confirm": false
    }
  ]
}

不要解释，直接输出 JSON。
"#;

/// LLM 返回的规划结果
#[derive(Debug, serde::Deserialize)]
struct PlanResponse {
    title: String,
    steps: Vec<PlanStep>,
}

#[derive(Debug, serde::Deserialize)]
struct PlanStep {
    title:            String,
    description:      String,
    tool_name:        Option<String>,
    tool_args:        Option<Value>,
    requires_confirm: bool,
}

/// 调用 LLM 将 goal 拆解为步骤列表
/// 返回 (task_title, steps)
pub async fn plan_task(
    task_id:  &str,
    goal:     &str,
    llm_cfg:  &LlmConfig,
) -> Result<(String, Vec<TaskStep>)> {
    let messages = vec![
        LlmMessage::system(PLAN_SYSTEM_PROMPT),
        LlmMessage::user(&format!("目标：{goal}")),
    ];

    let (raw_text, _) = llm::call_silent(messages, llm_cfg)
        .await
        .context("LLM 任务规划调用失败")?;

    // 尝试从回复中提取 JSON（LLM 可能包裹在 ```json ... ``` 中）
    let json_str = extract_json(&raw_text)
        .unwrap_or_else(|| raw_text.trim().to_string());

    let plan: PlanResponse = serde_json::from_str(&json_str)
        .context(format!("解析规划 JSON 失败:\n{json_str}"))?;

    let steps: Vec<TaskStep> = plan.steps
        .into_iter()
        .enumerate()
        .map(|(i, s)| {
            // 如果工具是 safe_shell 或有写操作，自动 requires_confirm
            let needs_confirm = s.requires_confirm
                || s.tool_name.as_deref() == Some("safe_shell");

            TaskStep::new(
                task_id,
                i,
                s.title,
                s.description,
                s.tool_name,
                s.tool_args,
                needs_confirm,
            )
        })
        .collect();

    Ok((plan.title, steps))
}

/// 从 LLM 回复中提取 JSON 块（处理 ```json ... ``` 包裹）
fn extract_json(text: &str) -> Option<String> {
    // 优先提取 ```json ``` 块
    if let Some(start) = text.find("```json") {
        let after = &text[start + 7..];
        if let Some(end) = after.find("```") {
            return Some(after[..end].trim().to_string());
        }
    }
    // 其次提取 ``` ``` 块
    if let Some(start) = text.find("```") {
        let after = &text[start + 3..];
        if let Some(end) = after.find("```") {
            let inner = after[..end].trim();
            if inner.starts_with('{') {
                return Some(inner.to_string());
            }
        }
    }
    // 直接找 { } 块
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            if end > start {
                return Some(text[start..=end].to_string());
            }
        }
    }
    None
}

/// 任务完成后，调用 LLM 生成总结（写入 Memory）
pub async fn summarize_task(
    task_title: &str,
    goal:       &str,
    step_results: &[(String, String)], // (step_title, result)
    llm_cfg:    &LlmConfig,
) -> String {
    let steps_text = step_results
        .iter()
        .map(|(t, r)| format!("- {t}: {r}"))
        .collect::<Vec<_>>()
        .join("\n");

    let prompt = format!(
        "任务「{}」已完成。\n目标：{}\n执行结果：\n{}\n\n请用2-3句话总结这个任务的结果，语气自然。",
        task_title, goal, steps_text
    );

    let messages = vec![
        LlmMessage::system("你是 Chebo，总结任务完成情况，简洁自然。"),
        LlmMessage::user(&prompt),
    ];

    match llm::call_silent(messages, llm_cfg).await {
        Ok((summary, _)) => summary,
        Err(_) => format!("任务「{task_title}」已完成。"),
    }
}
