// ─── memory_controller.rs ────────────────────────────────────────────────────
// Memory Controller（P4）：统一记忆写入管理
//
// 职责：
//   1. 提取候选记忆（MemoryCandidate）
//   2. 分类（MemoryType）→ 决定存储目标
//   3. 评分（write_score）
//   4. 冲突检测与合并（ConflictResolver）
//   5. 生命周期管理（active → decay → archive）
// ─────────────────────────────────────────────────────────────────────────────

use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::chat_intent::IntentDecision;
use crate::db;
use crate::llm::LlmConfig;

// ─── 记忆类型枚举 ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryType {
    /// 稳定事实（用户姓名、职业、地点）
    Fact,
    /// 偏好（喜欢/不喜欢、风格偏好）
    Preference,
    /// 项目上下文（当前项目、技术栈）
    Project,
    /// 项目决策（已确定的方案、结论）
    Decision,
    /// 未完成任务/目标
    Task,
    /// 历史经历
    Episode,
    /// 交互规则（用户期望如何被对待）
    Procedure,
    /// 关系记忆（Chebo 与用户的关系状态）
    Relationship,
    /// 临时状态（不长期保存）
    TemporaryState,
}

impl MemoryType {
    pub fn storage_target(&self) -> &str {
        match self {
            MemoryType::Fact | MemoryType::Preference | MemoryType::Episode => "user_profile",
            MemoryType::Project | MemoryType::Decision | MemoryType::Task => "memory_items",
            MemoryType::Procedure | MemoryType::Relationship => "persona_memory",
            MemoryType::TemporaryState => "candidate",
        }
    }
}

// ─── 记忆来源枚举 ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemorySource {
    /// 用户明确表达（"记住..."）
    ExplicitUser,
    /// 对话中提取（LLM/关键词）
    Conversation,
    /// 系统推导
    System,
}

// ─── 候选记忆结构 ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCandidate {
    pub content: String,
    pub memory_type: MemoryType,
    pub scope: String,
    pub confidence: f64,
    pub importance: f64,
    pub stability: f64,
    pub source: MemorySource,
    pub explicitness: f64,
    pub source_message_id: Option<i64>,
    pub key: Option<String>,
}

impl MemoryCandidate {
    pub fn write_score(&self) -> f64 {
        self.confidence * 0.25
            + self.importance * 0.20
            + self.stability * 0.20
            + self.explicitness * 0.20
            + future_usefulness(&self.memory_type) * 0.15
    }
}

/// 基于记忆类型的"未来有用性"估计
fn future_usefulness(mt: &MemoryType) -> f64 {
    match mt {
        MemoryType::Decision => 0.95,
        MemoryType::Procedure => 0.90,
        MemoryType::Fact => 0.85,
        MemoryType::Preference => 0.80,
        MemoryType::Relationship => 0.75,
        MemoryType::Project => 0.70,
        MemoryType::Task => 0.60,
        MemoryType::Episode => 0.40,
        MemoryType::TemporaryState => 0.10,
    }
}

// ─── 记忆事件（统一入口） ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum MemoryEvent {
    UserMessage {
        content: String,
        session_id: String,
        decision: IntentDecision,
    },
}

// ─── 冲突解析器 ───────────────────────────────────────────────────────────────

struct ConflictResolver;

impl ConflictResolver {
    /// 检测是否存在冲突，返回应使用的值
    async fn resolve(
        pool: &SqlitePool,
        candidate: &MemoryCandidate,
    ) -> Result<ResolveResult> {
        let key = match &candidate.key {
            Some(k) => k.clone(),
            None => return Ok(ResolveResult::New(candidate.clone())),
        };

        // 检查现有记忆
        let existing = match candidate.memory_type.storage_target() {
            "user_profile" => {
                sqlx::query("SELECT value, confidence, source FROM user_profile WHERE key = ?")
                    .bind(&key)
                    .fetch_optional(pool)
                    .await?
                    .map(|r| {
                        use sqlx::Row;
                        ResolvedMemory {
                            value: r.get::<String, _>("value"),
                            confidence: r.get::<f64, _>("confidence"),
                            source: r.get::<String, _>("source"),
                        }
                    })
            }
            "persona_memory" => {
                sqlx::query("SELECT value, confidence FROM persona_memory WHERE key = ?")
                    .bind(&key)
                    .fetch_optional(pool)
                    .await?
                    .map(|r| {
                        use sqlx::Row;
                        ResolvedMemory {
                            value: r.get::<String, _>("value"),
                            confidence: r.get::<f64, _>("confidence"),
                            source: "auto".to_string(),
                        }
                    })
            }
            _ => None,
        };

        match existing {
            Some(ex) => {
                // 冲突规则：explicit > recent > inferred
                let new_is_explicit = candidate.source == MemorySource::ExplicitUser;
                let old_is_explicit = ex.source == "user";
                let new_score = candidate.write_score();
                let old_score = ex.confidence;

                if new_is_explicit && !old_is_explicit {
                    // 用户明确表达 > 系统推断
                    Ok(ResolveResult::Overwrite(candidate.clone()))
                } else if !new_is_explicit && old_is_explicit {
                    // 保留旧值
                    Ok(ResolveResult::Keep)
                } else if new_score > old_score {
                    Ok(ResolveResult::Overwrite(candidate.clone()))
                } else {
                    Ok(ResolveResult::Keep)
                }
            }
            None => Ok(ResolveResult::New(candidate.clone())),
        }
    }
}

struct ResolvedMemory {
    value: String,
    confidence: f64,
    source: String,
}

enum ResolveResult {
    New(MemoryCandidate),
    Overwrite(MemoryCandidate),
    Keep,
}

// ─── 主入口：处理记忆事件 ───────────────────────────────────────────────────

/// 处理记忆事件（由 send_message 后处理调用）
pub async fn process_event(
    pool: &SqlitePool,
    llm_cfg: &Arc<LlmConfig>,
    event: MemoryEvent,
) {
    match event {
        MemoryEvent::UserMessage { content, session_id, decision } => {
            // 1. 提取候选记忆
            let candidates = extract_candidates(&content, &decision).await;

            for candidate in candidates {
                let score = candidate.write_score();

                // 2. 低分丢弃
                if score < 0.45 {
                    // 写入 candidates 表，供后续展示"Chebo 发现了一些关于你的信息"
                    if let Err(e) = save_candidate(pool, &candidate, score).await {
                        log::warn!("memory_controller save_candidate: {e}");
                    }
                    continue;
                }

                // 3. 冲突检测
                match ConflictResolver::resolve(pool, &candidate).await {
                    Ok(ResolveResult::New(c) | ResolveResult::Overwrite(c)) => {
                        // 4. 持久化
                        if let Err(e) = persist_candidate(pool, &c, score, &session_id).await {
                            log::warn!("memory_controller persist: {e}");
                        }
                    }
                    Ok(ResolveResult::Keep) => {
                        // 更新 existing 的 last_used_at
                    }
                    Err(e) => {
                        log::warn!("memory_controller resolve conflict: {e}");
                    }
                }
            }
        }
    }
}

// ─── 候选记忆提取 ─────────────────────────────────────────────────────────────

async fn extract_candidates(content: &str, decision: &IntentDecision) -> Vec<MemoryCandidate> {
    let mut candidates = Vec::new();

    // 规则提取（零 LLM）
    extract_by_rules(content, &mut candidates);

    // 用户明确"记住" → 高优先级
    if decision.memory_action == crate::chat_intent::MemoryAction::WriteExplicit {
        // TODO: P5 接入 LLM 深度提取
    }

    candidates
}

fn extract_by_rules(content: &str, candidates: &mut Vec<MemoryCandidate>) {
    // 规则：检测用户明确表达
    let explicit_triggers = [
        ("我喜欢", MemoryType::Preference, MemorySource::Conversation, 0.6),
        ("我不喜欢", MemoryType::Preference, MemorySource::Conversation, 0.6),
        ("我习惯", MemoryType::Procedure, MemorySource::Conversation, 0.55),
        ("我是", MemoryType::Fact, MemorySource::Conversation, 0.5),
        ("我住在", MemoryType::Fact, MemorySource::Conversation, 0.6),
        ("我的工作", MemoryType::Fact, MemorySource::Conversation, 0.6),
        ("我在学", MemoryType::Task, MemorySource::Conversation, 0.55),
        ("这个项目", MemoryType::Project, MemorySource::Conversation, 0.5),
        (
            "记住",
            MemoryType::Fact,
            MemorySource::ExplicitUser,
            0.95,
        ),
    ];

    for (trigger, mtype, source, confidence) in explicit_triggers {
        if let Some(pos) = content.find(trigger) {
            let rest = content[pos + trigger.len()..]
                .trim_start_matches(['：', ':', '，', ',', ' ']);
            let end = rest
                .find(['。', '！', '？', '!', '?', '\n'])
                .unwrap_or(rest.len().min(60));
            let value = rest[..end].trim().to_string();
            if value.is_empty() || value.chars().count() > 60 {
                continue;
            }

            let key = match mtype {
                MemoryType::Fact => match trigger {
                    "我是" => Some("自我描述".to_string()),
                    "我住在" => Some("居住地".to_string()),
                    "我的工作" => Some("职业".to_string()),
                    _ => None,
                },
                MemoryType::Preference => match trigger {
                    "我喜欢" => Some("兴趣爱好".to_string()),
                    "我不喜欢" => Some("讨厌事物".to_string()),
                    _ => None,
                },
                MemoryType::Task => match trigger {
                    "我在学" => Some("学习内容".to_string()),
                    _ => None,
                },
                _ => None,
            };

            candidates.push(MemoryCandidate {
                content: value,
                memory_type: mtype,
                scope: "global".to_string(),
                confidence,
                importance: 0.6,
                stability: 0.5,
                source,
                explicitness: if source == MemorySource::ExplicitUser {
                    0.95
                } else {
                    0.5
                },
                source_message_id: None,
                key,
            });
        }
    }
}

// ─── 持久化 ───────────────────────────────────────────────────────────────────

async fn persist_candidate(
    pool: &SqlitePool,
    candidate: &MemoryCandidate,
    score: f64,
    _session_id: &str,
) -> Result<()> {
    let target = candidate.memory_type.storage_target();

    match target {
        "user_profile" => {
            if let Some(key) = &candidate.key {
                crate::memory::core_memory_store::set_user_profile(pool, key, &candidate.content).await?;
            } else {
                // 无 key 的 Episode/自由文本，自动生成 key
                let key = format!("epi_{}", candidate.content.chars().take(10).collect::<String>());
                crate::memory::core_memory_store::set_user_profile(pool, &key, &candidate.content).await?;
            }
        }
        "persona_memory" => {
            if let Some(key) = &candidate.key {
                crate::memory::core_memory_store::upsert_persona_memory(
                    pool,
                    key,
                    &candidate.content,
                    "trait",
                    candidate.confidence,
                )
                .await?;
            }
        }
        _ => {}
    }

    // 记录到 memory_events 审计
    let _ = sqlx::query(
        "INSERT INTO pet_events (type, payload) VALUES ('memory_persist', ?)",
    )
    .bind(serde_json::json!({
        "content": candidate.content.chars().take(60).collect::<String>(),
        "type": format!("{:?}", candidate.memory_type),
        "score": score,
        "target": target,
    }).to_string())
    .execute(pool)
    .await;

    log::info!(
        "memory_controller: {:.2} [{:?}] {} → {}",
        score,
        candidate.memory_type,
        candidate.content.chars().take(40).collect::<String>(),
        target,
    );

    Ok(())
}

async fn save_candidate(pool: &SqlitePool, candidate: &MemoryCandidate, score: f64) -> Result<()> {
    sqlx::query(
        "INSERT INTO memory_candidates (content, memory_type, scope, score, status)
         VALUES (?, ?, ?, ?, 'candidate')",
    )
    .bind(&candidate.content)
    .bind(format!("{:?}", candidate.memory_type))
    .bind(&candidate.scope)
    .bind(score)
    .execute(pool)
    .await?;
    Ok(())
}