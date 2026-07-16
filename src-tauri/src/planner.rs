// ─── planner.rs ──────────────────────────────────────────────────────────────
// 轻量对话规划器：为当前对话轮次生成执行计划，不持久化到 DB
//
// 与 task_planner.rs 的区别：
//   - task_planner：创建长期任务，持久化，逐步执行
//   - planner：仅当前轮次生效，轻量，不写 DB
//
// 触发场景：用户发送复杂请求（技术问答、项目分析、多步任务）时，
//           在工具循环之前先生成计划，让模型按计划执行。
// ──────────────────────────────────────────────────────────────────────────────

use std::sync::Arc;

use crate::chat_intent::ChatIntent;
use crate::llm::{self, LlmConfig, LlmMessage};

const PLAN_PROMPT: &str = r#"
你是一个轻量任务规划助手。你的任务：
根据用户的请求，生成一个 2-5 步的执行计划。

规则：
1. 步骤应具体、可操作
2. 每步用一句话描述
3. 按执行顺序排列
4. 不需要 JSON，直接列出即可

输出格式（不要解释，直接输出计划）：
【执行计划】
1. <第一步>
2. <第二步>
...
"#;

/// 判断当前意图是否需要事先规划
pub fn should_plan(intent: ChatIntent) -> bool {
    matches!(
        intent,
        ChatIntent::TechnicalQa
            | ChatIntent::ProjectReview
            | ChatIntent::DeepThink
            | ChatIntent::ContinueTask
    )
}

/// 为当前请求生成轻量执行计划
///
/// 返回计划文本（如 "【执行计划】\n1. ...\n2. ..."），
/// 如果规划失败返回 None，调用方应忽略并继续正常流程。
pub async fn quick_plan(
    goal: &str,
    intent: ChatIntent,
    llm_cfg: &Arc<LlmConfig>,
) -> Option<String> {
    if !should_plan(intent) {
        return None;
    }

    let intent_hint = match intent {
        ChatIntent::TechnicalQa => "技术问答",
        ChatIntent::ProjectReview => "项目分析/设计评审",
        ChatIntent::DeepThink => "深度分析任务",
        ChatIntent::ContinueTask => "延续之前的任务",
        _ => "一般任务",
    };

    let messages = vec![
        LlmMessage::system(PLAN_PROMPT),
        LlmMessage::user(&format!(
            "请求类型：{intent_hint}\n\n用户请求：{goal}"
        )),
    ];

    match llm::call_silent(messages, llm_cfg).await {
        Ok((plan, _)) => {
            let trimmed = plan.trim().to_string();
            if trimmed.is_empty() || trimmed.len() < 10 {
                return None;
            }
            // 确保计划以标题开头
            if trimmed.starts_with("【执行计划】") || trimmed.starts_with("1. ") {
                Some(trimmed)
            } else {
                Some(format!("【执行计划】\n{trimmed}"))
            }
        }
        Err(e) => {
            log::warn!("quick_plan failed: {e}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_plan() {
        assert!(should_plan(ChatIntent::TechnicalQa));
        assert!(should_plan(ChatIntent::ProjectReview));
        assert!(should_plan(ChatIntent::DeepThink));
        assert!(should_plan(ChatIntent::ContinueTask));
        assert!(!should_plan(ChatIntent::CasualChat));
        assert!(!should_plan(ChatIntent::EmotionalSupport));
        assert!(!should_plan(ChatIntent::RememberRequest));
        assert!(!should_plan(ChatIntent::ToolOperation));
    }
}
