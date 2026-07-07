// ─── working_memory.rs ───────────────────────────────────────────────────────
// Working Memory（P3）：维护当前正在推进的状态
//
// 职责：
//   1. 记录当前项目、话题、目标、决策、待办
//   2. 支持 scope（global / project:xxx / topic:xxx）
//   3. LLM 输出 patch 合并，不整行覆盖
//   4. ContextBuilder 注入 brief（非完整 JSON）
// ─────────────────────────────────────────────────────────────────────────────

use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::chat_intent::{ChatIntent, IntentDecision, MemoryAction};
use crate::db;
use crate::llm::{self, LlmConfig, LlmMessage};

// ─── 常量 ─────────────────────────────────────────────────────────────────────

const MAX_DECISIONS: usize = 12;
const MAX_OPEN_QUESTIONS: usize = 8;
const MAX_NEXT_ACTIONS: usize = 8;
const DEFAULT_SCOPE: &str = "global";

// ─── 业务结构体（Vec 版本） ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkingMemory {
    pub id: i64,
    pub scope: String,
    pub current_project: String,
    pub current_topic: String,
    pub user_goal: Option<String>,
    pub confirmed_decisions: Vec<String>,
    pub open_questions: Vec<String>,
    pub next_actions: Vec<String>,
    pub confidence: f64,
    pub updated_at: String,
}

impl WorkingMemory {
    /// 生成给 LLM 的简要文本（非完整 JSON）
    pub fn to_brief(&self, max_chars: usize) -> String {
        let mut parts = Vec::new();

        if !self.current_project.is_empty() {
            parts.push(format!("当前项目：{}", self.current_project));
        }
        if !self.current_topic.is_empty() {
            parts.push(format!("当前话题：{}", self.current_topic));
        }
        if let Some(ref goal) = self.user_goal {
            parts.push(format!("用户目标：{goal}"));
        }

        if !self.confirmed_decisions.is_empty() {
            let d: Vec<&str> = self.confirmed_decisions.iter().take(5).map(|s| s.as_str()).collect();
            parts.push("已确定：".to_string());
            for dec in d {
                parts.push(format!("- {dec}"));
            }
        }

        if !self.open_questions.is_empty() {
            let q: Vec<&str> = self.open_questions.iter().take(4).map(|s| s.as_str()).collect();
            parts.push("待解决：".to_string());
            for question in q {
                parts.push(format!("- {question}"));
            }
        }

        if !self.next_actions.is_empty() {
            let n: Vec<&str> = self.next_actions.iter().take(4).map(|s| s.as_str()).collect();
            parts.push("下一步：".to_string());
            for action in n {
                parts.push(format!("- {action}"));
            }
        }

        let text = parts.join("\n");
        if text.chars().count() > max_chars {
            text.chars().take(max_chars).collect::<String>() + "…"
        } else {
            text
        }
    }
}

// ─── Patch 结构（LLM 输出 → 合并） ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkingMemoryPatch {
    pub current_project: Option<String>,
    pub current_topic: Option<String>,
    pub user_goal: Option<String>,

    pub add_confirmed_decisions: Vec<String>,
    pub remove_confirmed_decisions: Vec<String>,

    pub add_open_questions: Vec<String>,
    pub resolve_open_questions: Vec<String>,

    pub add_next_actions: Vec<String>,
    pub complete_next_actions: Vec<String>,

    pub confidence: Option<f64>,
}

// ─── 加载 ─────────────────────────────────────────────────────────────────────

/// 加载指定 scope 的 working memory
pub async fn load_active(pool: &SqlitePool, scope: &str) -> Result<Option<WorkingMemory>> {
    let row = sqlx::query(
        "SELECT id, scope, current_project, current_topic, user_goal,
                confirmed_decisions, open_questions, next_actions,
                confidence, updated_at
         FROM working_memory WHERE scope = ? AND status = 'active'",
    )
    .bind(scope)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| {
        use sqlx::Row;
        WorkingMemory {
            id:                    r.get("id"),
            scope:                 r.get("scope"),
            current_project:       r.get("current_project"),
            current_topic:         r.get("current_topic"),
            user_goal:             r.get("user_goal"),
            confirmed_decisions:   serde_json::from_str(r.get::<String, _>("confirmed_decisions").as_str()).unwrap_or_default(),
            open_questions:        serde_json::from_str(r.get::<String, _>("open_questions").as_str()).unwrap_or_default(),
            next_actions:          serde_json::from_str(r.get::<String, _>("next_actions").as_str()).unwrap_or_default(),
            confidence:            r.get("confidence"),
            updated_at:            r.get("updated_at"),
        }
    }))
}

/// 获取指定 scope 的 brief 文本（供 ContextBuilder 使用）
pub async fn get_brief(
    pool: &SqlitePool,
    scope: &str,
    max_chars: usize,
) -> Result<Option<String>> {
    match load_active(pool, scope).await {
        Ok(Some(wm)) => {
            let brief = wm.to_brief(max_chars);
            if brief.is_empty() {
                Ok(None)
            } else {
                Ok(Some(brief))
            }
        }
        Ok(None) => Ok(None),
        Err(e) => Err(e),
    }
}

// ─── 合并 patch ───────────────────────────────────────────────────────────────

fn apply_patch(existing: &mut WorkingMemory, patch: WorkingMemoryPatch) {
    if let Some(project) = patch.current_project {
        if !project.is_empty() {
            existing.current_project = project;
        }
    }
    if let Some(topic) = patch.current_topic {
        if !topic.is_empty() {
            existing.current_topic = topic;
        }
    }
    if let Some(goal) = patch.user_goal {
        if !goal.is_empty() {
            existing.user_goal = Some(goal);
        }
    }

    // 添加决策（去重）
    for d in patch.add_confirmed_decisions {
        if !existing.confirmed_decisions.contains(&d) {
            existing.confirmed_decisions.push(d);
        }
    }
    // 移除决策
    existing.confirmed_decisions.retain(|d| !patch.remove_confirmed_decisions.contains(d));
    // 限制长度
    existing.confirmed_decisions.truncate(MAX_DECISIONS);

    // 添加未解决问题
    for q in patch.add_open_questions {
        if !existing.open_questions.contains(&q) {
            existing.open_questions.push(q);
        }
    }
    // 标记已解决
    existing.open_questions.retain(|q| !patch.resolve_open_questions.contains(q));
    existing.open_questions.truncate(MAX_OPEN_QUESTIONS);

    // 添加下一步
    for a in patch.add_next_actions {
        if !existing.next_actions.contains(&a) {
            existing.next_actions.push(a);
        }
    }
    // 标记已完成
    existing.next_actions.retain(|a| !patch.complete_next_actions.contains(a));
    existing.next_actions.truncate(MAX_NEXT_ACTIONS);

    if let Some(conf) = patch.confidence {
        existing.confidence = conf;
    }
}

// ─── LLM 驱动更新（仅在必要场景触发） ─────────────────────────────────────────

/// 判断是否应该更新 working memory
pub fn should_update(decision: &IntentDecision, _content: &str) -> bool {
    // 由 IntentDecision.memory_action 决定
    if decision.memory_action == MemoryAction::UpdateWorkingMemory {
        return true;
    }

    // 或者按意图判断（兜底）
    matches!(
        decision.intent,
        ChatIntent::ContinueTask | ChatIntent::ProjectReview | ChatIntent::DeepThink
    )
}

/// 从对话中生成 patch，合并到 existing working memory
pub async fn update_from_conversation(
    pool: &SqlitePool,
    llm_cfg: &Arc<LlmConfig>,
    scope: &str,
    recent_messages: &[db::Message],
    decision: &IntentDecision,
) -> Result<()> {
    // 构建对话文本
    let conv_text: String = recent_messages
        .iter()
        .filter(|m| m.role == "user" || m.role == "assistant")
        .map(|m| format!("{}: {}", if m.role == "user" { "用户" } else { "Chebo" }, m.content))
        .collect::<Vec<_>>()
        .join("\n");

    if conv_text.trim().is_empty() {
        return Ok(());
    }

    // 读取现有 working memory
    let existing = load_active(pool, scope).await?;
    let existing_brief = existing.as_ref().map(|wm| wm.to_brief(800)).unwrap_or_default();

    let prompt = format!(
        r#"根据最近的对话，更新 Working Memory。

当前 intent: {:?}
用户目标/意图：{}

对话内容：
{conv_text}

当前 Working Memory：
{existing_brief}

请分析对话并输出 patch JSON：
{{
  "current_project": "更新或null",
  "current_topic": "更新或null",
  "user_goal": "更新或null",
  "add_confirmed_decisions": ["新决策"],
  "remove_confirmed_decisions": ["已无效的决策"],
  "add_open_questions": ["新问题"],
  "resolve_open_questions": ["已解决的问题"],
  "add_next_actions": ["新待办"],
  "complete_next_actions": ["已完成的待办"],
  "confidence": 0.9
}}

只输出 JSON，不要解释。"#,
        intent = decision.intent,
        conv_text = conv_text,
        existing_brief = existing_brief,
    );

    let llm_msgs = vec![
        LlmMessage::system("你是 Chebo 的工作记忆更新助手。根据对话内容，决定当前项目/话题/目标/决策/待办如何变化。"),
        LlmMessage::user(&prompt),
    ];

    let (raw, _) = llm::call_silent(llm_msgs, llm_cfg).await?;

    // 解析 JSON
    let json_start = raw.find('{');
    let json_end = raw.rfind('}');
    let patch: WorkingMemoryPatch = match (json_start, json_end) {
        (Some(start), Some(end)) if end > start => {
            serde_json::from_str(&raw[start..=end])?
        }
        _ => {
            log::warn!("working_memory: no valid JSON patch found in LLM output");
            return Ok(());
        }
    };

    // 合并
    let mut wm = existing.unwrap_or(WorkingMemory {
        id: 0,
        scope: scope.to_string(),
        current_project: String::new(),
        current_topic: String::new(),
        user_goal: None,
        confirmed_decisions: vec![],
        open_questions: vec![],
        next_actions: vec![],
        confidence: 0.8,
        updated_at: String::new(),
    });

    apply_patch(&mut wm, patch);

    // 保存
    save(pool, &wm).await?;

    Ok(())
}

// ─── 保存到数据库 ─────────────────────────────────────────────────────────────

async fn save(pool: &SqlitePool, wm: &WorkingMemory) -> Result<()> {
    use sqlx::Row;

    let decisions = serde_json::to_string(&wm.confirmed_decisions)?;
    let questions = serde_json::to_string(&wm.open_questions)?;
    let actions   = serde_json::to_string(&wm.next_actions)?;

    // UPSERT by scope
    sqlx::query(
        "INSERT INTO working_memory (scope, current_project, current_topic, user_goal,
                confirmed_decisions, open_questions, next_actions, confidence, status)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'active')
         ON CONFLICT(scope) DO UPDATE SET
            current_project      = excluded.current_project,
            current_topic        = excluded.current_topic,
            user_goal            = excluded.user_goal,
            confirmed_decisions  = excluded.confirmed_decisions,
            open_questions       = excluded.open_questions,
            next_actions         = excluded.next_actions,
            confidence           = excluded.confidence,
            status               = 'active',
            updated_at           = datetime('now','localtime')",
    )
    .bind(&wm.scope)
    .bind(&wm.current_project)
    .bind(&wm.current_topic)
    .bind(&wm.user_goal)
    .bind(&decisions)
    .bind(&questions)
    .bind(&actions)
    .bind(wm.confidence)
    .execute(pool)
    .await?;

    Ok(())
}

/// 可用 scope 列表（用于后续扩展）
pub fn default_scope() -> String {
    DEFAULT_SCOPE.to_string()
}