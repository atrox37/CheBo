// ─── context_builder.rs ──────────────────────────────────────────────────────
// Context Builder（最终版）：按 IntentDecision 构建结构化上下文包
//
// 职责：
//   1. 根据 IntentDecision 决定召回哪些记忆
//   2. 自动向量召回（按意图设定阈值/去重/截断）
//   3. 构建 ContextPack → to_prompt_section() 渲染
// ─────────────────────────────────────────────────────────────────────────────

use sqlx::SqlitePool;

use crate::chat_intent::{ChatIntent, IntentDecision, RecallStrategy, ToolPolicy};
use crate::db;
use crate::llm::LlmConfig;
use crate::memory_vector;

// ─── 结构化上下文包 ───────────────────────────────────────────────────────────

pub struct ContextPack {
    pub working_memory: Option<String>,
    pub profile_items: Vec<String>,
    pub persona_items: Vec<String>,
    pub summaries: Vec<String>,
    pub long_term_memories: Vec<String>,
    pub vector_memories: Vec<String>,
    pub suggested_tools: Vec<String>,
    pub tool_policy: ToolPolicy,
}

impl ContextPack {
    pub fn is_empty(&self) -> bool {
        self.working_memory.is_none()
            && self.profile_items.is_empty()
            && self.persona_items.is_empty()
            && self.summaries.is_empty()
            && self.long_term_memories.is_empty()
            && self.vector_memories.is_empty()
    }

    /// 格式化为注入 system prompt 的文本块
    pub fn to_prompt_section(&self) -> String {
        let mut parts: Vec<String> = Vec::new();

        if let Some(wm) = &self.working_memory {
            parts.push(format!("【当前工作记忆】\n{wm}"));
        }

        if !self.profile_items.is_empty() {
            parts.push(format!("【用户画像】\n{}", self.profile_items.join("\n")));
        }

        if !self.persona_items.is_empty() {
            parts.push(format!("【Chebo 人格记忆】\n{}", self.persona_items.join("\n")));
        }

        if !self.summaries.is_empty() {
            parts.push(format!("【历史摘要】\n{}", self.summaries.join("\n")));
        }

        if !self.long_term_memories.is_empty() {
            parts.push(format!("【长期记忆】\n{}", self.long_term_memories.join("\n")));
        }

        if !self.vector_memories.is_empty() {
            parts.push(format!("【相关语义记忆】\n{}", self.vector_memories.join("\n")));
        }

        if parts.is_empty() {
            String::new()
        } else {
            format!("\n{}\n", parts.join("\n\n"))
        }
    }
}

// ─── 上下文需求（按 IntentDecision 决定）──────────────────────────────────────

struct ContextRequirements {
    need_profile: bool,
    need_persona: bool,
    need_summaries: bool,
    need_long_term: bool,
    need_vector_recall: bool,
    need_working_memory: bool,

    max_profile_items: usize,
    max_persona_items: usize,
    max_summaries: usize,
    max_long_term_items: usize,
    max_vector_items: usize,

    max_total_chars: usize,
}

/// 按 IntentDecision 结合 recall_strategy 生成需求
fn requirements_for_decision(decision: &IntentDecision) -> ContextRequirements {
    use ChatIntent::*;

    // recall_strategy 影响是否召回向量
    fn use_vector(decision: &IntentDecision) -> bool {
        matches!(
            decision.recall_strategy,
            RecallStrategy::VectorTopK
                | RecallStrategy::ProjectContext
                | RecallStrategy::FullHybrid
        )
    }

    // recall_strategy 影响是否需要 working memory
    fn use_wm(decision: &IntentDecision) -> bool {
        matches!(
            decision.recall_strategy,
            RecallStrategy::WorkingMemory
                | RecallStrategy::ProjectContext
                | RecallStrategy::FullHybrid
        )
    }

    match decision.intent {
        CasualChat => ContextRequirements {
            need_profile: true,
            need_persona: true,
            need_summaries: false,
            need_long_term: false,
            need_vector_recall: false,
            need_working_memory: false,
            max_profile_items: 2,
            max_persona_items: 4,
            max_summaries: 0,
            max_long_term_items: 0,
            max_vector_items: 0,
            max_total_chars: 1500,
        },

        TechnicalQa => ContextRequirements {
            need_profile: true,
            need_persona: true,
            need_summaries: true,
            need_long_term: true,
            need_vector_recall: use_vector(decision),
            need_working_memory: use_wm(decision),
            max_profile_items: 4,
            max_persona_items: 2,
            max_summaries: 3,
            max_long_term_items: 3,
            max_vector_items: 5,
            max_total_chars: 3500,
        },

        ContinueTask => ContextRequirements {
            need_profile: true,
            need_persona: true,
            need_summaries: true,
            need_long_term: true,
            need_vector_recall: use_vector(decision),
            need_working_memory: true,
            max_profile_items: 5,
            max_persona_items: 3,
            max_summaries: 5,
            max_long_term_items: 5,
            max_vector_items: 8,
            max_total_chars: 5500,
        },

        ProjectReview => ContextRequirements {
            need_profile: true,
            need_persona: true,
            need_summaries: true,
            need_long_term: true,
            need_vector_recall: use_vector(decision),
            need_working_memory: true,
            max_profile_items: 5,
            max_persona_items: 2,
            max_summaries: 6,
            max_long_term_items: 5,
            max_vector_items: 8,
            max_total_chars: 6000,
        },

        RememberRequest => ContextRequirements {
            need_profile: true,
            need_persona: true,
            need_summaries: false,
            need_long_term: true,
            need_vector_recall: false,
            need_working_memory: false,
            max_profile_items: 3,
            max_persona_items: 2,
            max_summaries: 0,
            max_long_term_items: 3,
            max_vector_items: 0,
            max_total_chars: 2000,
        },

        ToolOperation => ContextRequirements {
            need_profile: true,
            need_persona: false,
            need_summaries: false,
            need_long_term: false,
            need_vector_recall: use_vector(decision),
            need_working_memory: false,
            max_profile_items: 2,
            max_persona_items: 0,
            max_summaries: 0,
            max_long_term_items: 0,
            max_vector_items: 5,
            max_total_chars: 3000,
        },

        EmotionalSupport => ContextRequirements {
            need_profile: true,
            need_persona: true,
            need_summaries: false,
            need_long_term: false,
            need_vector_recall: false,
            need_working_memory: false,
            max_profile_items: 3,
            max_persona_items: 5,
            max_summaries: 0,
            max_long_term_items: 0,
            max_vector_items: 0,
            max_total_chars: 2200,
        },

        DeepThink => ContextRequirements {
            need_profile: true,
            need_persona: true,
            need_summaries: true,
            need_long_term: true,
            need_vector_recall: true,
            need_working_memory: true,
            max_profile_items: 8,
            max_persona_items: 4,
            max_summaries: 10,
            max_long_term_items: 8,
            max_vector_items: 10,
            max_total_chars: 9000,
        },
    }
}

// ─── 向量召回阈值（按意图） ───────────────────────────────────────────────────

fn min_score_for_intent(intent: ChatIntent) -> f32 {
    use ChatIntent::*;
    match intent {
        CasualChat | EmotionalSupport => 0.0,     // 不召回
        RememberRequest => 0.72,
        TechnicalQa => 0.72,
        ContinueTask => 0.62,                     // "继续刚才那个"句子短，放宽阈值
        ProjectReview => 0.65,
        DeepThink => 0.60,
        ToolOperation => 0.68,
    }
}

/// 截断单条记忆
fn truncate_memory(content: &str, max_chars: usize) -> String {
    if content.chars().count() <= max_chars {
        content.to_string()
    } else {
        content.chars().take(max_chars).collect::<String>() + "…"
    }
}

/// 去重（取前 80 字做 key）
fn dedup_vector_hits(hits: Vec<memory_vector::RecallHit>) -> Vec<memory_vector::RecallHit> {
    let mut seen = std::collections::HashSet::new();
    hits.into_iter()
        .filter(|h| {
            let key: String = h.content.chars().take(80).collect();
            seen.insert(key)
        })
        .collect()
}

// ─── 自动向量召回 ─────────────────────────────────────────────────────────────

async fn load_vector_memories(
    pool: &SqlitePool,
    llm_cfg: &LlmConfig,
    content: &str,
    req: &ContextRequirements,
    decision: &IntentDecision,
) -> Vec<String> {
    if !req.need_vector_recall || req.max_vector_items == 0 || content.trim().is_empty() {
        return vec![];
    }

    let min_score = min_score_for_intent(decision.intent);

    // 多取一些，过滤和去重后足够
    let fetch_k = req.max_vector_items.saturating_mul(2).max(4);

    match memory_vector::recall_semantic(pool, llm_cfg, content, fetch_k).await {
        Ok(hits) if !hits.is_empty() => {
            let deduped = dedup_vector_hits(hits);
            deduped
                .into_iter()
                .filter(|h| h.score >= min_score)
                .take(req.max_vector_items)
                .map(|h| {
                    format!(
                        "  · {}（相关度 {:.0}%）",
                        truncate_memory(&h.content, 220),
                        h.score * 100.0
                    )
                })
                .collect()
        }
        Ok(_) => vec![],
        Err(e) => {
            log::debug!("load_vector_memories failed: {e}");
            vec![]
        }
    }
}

// ─── 主入口 ───────────────────────────────────────────────────────────────────

/// 根据 IntentDecision 构建 ContextPack
pub async fn build_context_pack(
    pool: &SqlitePool,
    session_id: &str,
    content: &str,
    decision: &IntentDecision,
    llm_cfg: &LlmConfig,
) -> ContextPack {
    let req = requirements_for_decision(decision);

    // ── 用户画像 ──────────────────────────────────────────────────────────────
    let profile_items = if req.need_profile {
        if let Ok(profile) = db::get_user_profile_all(pool).await {
            profile
                .iter()
                .take(req.max_profile_items)
                .map(|e| format!("  · {} → {}", e.key, e.value))
                .collect()
        } else {
            vec![]
        }
    } else {
        vec![]
    };

    // ── 人格记忆 ──────────────────────────────────────────────────────────────
    let persona_items = if req.need_persona {
        if let Ok(persona) = db::get_persona_memory_all(pool).await {
            persona
                .iter()
                .filter(|p| p.confidence >= 0.7)
                .take(req.max_persona_items)
                .map(|p| format!("  · [{}] {}", p.category, p.value))
                .collect()
        } else {
            vec![]
        }
    } else {
        vec![]
    };

    // ── 历史摘要 ──────────────────────────────────────────────────────────────
    let summaries = if req.need_summaries {
        if let Ok(sums) = db::get_summaries(pool, req.max_summaries as i64).await {
            sums.iter()
                .rev()
                .map(|s| format!("  · {}", truncate_memory(&s.summary, 300)))
                .collect()
        } else {
            vec![]
        }
    } else {
        vec![]
    };

    // ── 长期记忆 ──────────────────────────────────────────────────────────────
    let long_term_memories = if req.need_long_term {
        if let Ok(mems) = db::get_recent_memories_global(pool, req.max_long_term_items as i64).await
        {
            mems.iter()
                .map(|m| format!("  · {}", truncate_memory(&m.content, 200)))
                .collect()
        } else {
            vec![]
        }
    } else {
        vec![]
    };

    // ── 自动向量召回（按意图阈值/去重/截断） ────────────────────────────────
    let vector_memories =
        load_vector_memories(pool, llm_cfg, content, &req, decision).await;

    let _ = session_id; // P3: working memory 查询

    ContextPack {
        working_memory: None,
        profile_items,
        persona_items,
        summaries,
        long_term_memories,
        vector_memories,
        suggested_tools: decision.suggested_tools.clone(),
        tool_policy: decision.tool_policy,
    }
}