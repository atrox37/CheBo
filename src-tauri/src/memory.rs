// ─── memory.rs ────────────────────────────────────────────────────────────────
// Memory System（P0 基础 + Batch D 四层分级强化）
#![allow(dead_code)]
//
// 四层记忆读写规则：
//   短期（messages 表，最近 20 条）     — 每次对话自动读写，load_history_for_context
//   中期（memory_summaries，每 20 条）  — maybe_summarize 自动触发
//   长期（long_term_memories，置信≥0.7）— save_long_term_memory_guarded 过滤写入
//   人格（persona_memory，Chebo 自身）  — update_persona_memory 专用函数写入
//
// Memory Policy：
//   - 长期记忆仅在 confidence >= 0.7 时写入（低置信丢弃）
//   - 同 key 写入新人格记忆时，旧值置信度自动衰减（resolve_memory_conflict）
//   - 人格记忆只通过 update_persona_memory / LLM 驱动，不由关键词规则直接写
//   - 中期摘要每 20 条消息触发一次，异步 fire-and-forget
// ─────────────────────────────────────────────────────────────────────────────

use std::sync::Arc;

use anyhow::Result;
use sqlx::SqlitePool;

use crate::db::{self, Message};
use crate::llm::{self, LlmConfig, LlmMessage};

// ─── 常量 ─────────────────────────────────────────────────────────────────────

/// 积累多少条新消息后触发自动摘要（从20降到10，增加摘要密度）
const SUMMARIZE_EVERY: i64 = 10;

// ─── v1 兼容接口（已有代码引用） ─────────────────────────────────────────────

/// 从数据库加载最近 N 条消息，转换为 LLM 消息格式（不含 system）
pub async fn load_history_for_context(
    pool:       &SqlitePool,
    session_id: &str,
    limit:      i64,
) -> Result<Vec<LlmMessage>> {
    let msgs = db::get_messages(pool, session_id, limit).await?;
    Ok(msgs
        .iter()
        .filter(|m| m.role == "user" || m.role == "assistant")
        .map(|m| LlmMessage {
            role:    m.role.clone(),
            content: m.content.clone(),
        })
        .collect())
}

/// 获取全部聊天记录（供前端历史页面展示）
pub async fn get_all_messages(pool: &SqlitePool) -> Result<Vec<Message>> {
    db::get_all_messages(pool).await
}

/// 保存一对对话（user + assistant）
pub async fn save_exchange(
    pool:              &SqlitePool,
    session_id:        &str,
    user_content:      &str,
    assistant_content: &str,
    emotion:           Option<&str>,
) -> Result<()> {
    db::save_message(pool, session_id, "user", user_content, None, None).await?;
    db::save_message(pool, session_id, "assistant", assistant_content, emotion, None).await?;
    Ok(())
}

// ─── P0 + Batch D: 富上下文构建（四层记忆）────────────────────────────────────

/// 构建完整的四层记忆上下文字符串，注入到 system prompt 中。
/// 层次：人格记忆 → 用户画像 → 历史对话摘要 → 近期长期记忆
pub async fn build_rich_context_string(
    pool:       &SqlitePool,
    session_id: &str,
) -> String {
    let mut parts: Vec<String> = Vec::new();

    // ── 0. Batch D: 人格记忆（Chebo 自身，高置信度优先）──────────────────────
    if let Ok(persona) = db::get_persona_memory_all(pool).await {
        let high_conf: Vec<_> = persona.iter().filter(|p| p.confidence >= 0.7).collect();
        if !high_conf.is_empty() {
            let lines: Vec<String> = high_conf
                .iter()
                .take(6)
                .map(|p| format!("  · [{}] {}", p.category, p.value))
                .collect();
            parts.push(format!("【Chebo 人格记忆】\n{}", lines.join("\n")));
        }
    }

    // ── 1. 用户画像 ──────────────────────────────────────────────────────────
    if let Ok(profile) = db::get_user_profile_all(pool).await {
        if !profile.is_empty() {
            let lines: Vec<String> = profile
                .iter()
                .take(8)
                .map(|e| format!("  · {} → {}", e.key, e.value))
                .collect();
            parts.push(format!("【用户画像】\n{}", lines.join("\n")));
        }
    }

    // ── 2. 中期：历史对话摘要（最近 10 条，从2提升到10以覆盖更长的对话历史）───
    if let Ok(summaries) = db::get_summaries(pool, 10).await {
        if !summaries.is_empty() {
            let lines: Vec<String> = summaries
                .iter()
                .rev()
                .map(|s| format!("  · {}", s.summary))
                .collect();
            parts.push(format!("【历史摘要】\n{}", lines.join("\n")));
        }
    }

    // ── 3. 长期记忆（跨会话最近 8 条，含用户「记下来」指令）────────────────
    if let Ok(mems) = db::get_recent_memories_global(pool, 8).await {
        if !mems.is_empty() {
            let lines: Vec<String> = mems.iter().map(|m| format!("  · {}", m.content)).collect();
            parts.push(format!("【记忆片段】\n{}", lines.join("\n")));
        }
    }

    if parts.is_empty() {
        String::new()
    } else {
        format!("\n{}\n", parts.join("\n"))
    }
}

// ─── Batch D: 人格记忆管理 API ───────────────────────────────────────────────

/// 更新一条人格记忆（写入前先对旧值降低置信度）
/// category: "trait" / "experience" / "relationship" / "mood_history"
pub async fn update_persona_memory(
    pool:       &SqlitePool,
    key:        &str,
    value:      &str,
    category:   &str,
    confidence: f64,
) {
    // 旧值置信度衰减（conflict resolution）
    let _ = db::decay_persona_confidence(pool, key, 0.15).await;
    // 写入新值
    if let Err(e) = db::upsert_persona_memory(pool, key, value, category, confidence).await {
        log::warn!("update_persona_memory: {e}");
    }
}

/// 读取人格记忆并生成注入段落（供 build_rich_context_string 以外的场景使用）
pub async fn get_persona_context(pool: &SqlitePool) -> String {
    match db::get_persona_memory_all(pool).await {
        Ok(persona) if !persona.is_empty() => {
            let lines: Vec<String> = persona
                .iter()
                .filter(|p| p.confidence >= 0.7)
                .map(|p| format!("  · [{}] {}", p.category, p.value))
                .collect();
            if lines.is_empty() {
                String::new()
            } else {
                format!("【Chebo 人格记忆】\n{}\n", lines.join("\n"))
            }
        }
        _ => String::new(),
    }
}

/// Memory Policy：评估一条记忆是否应写入长期记忆，并在置信度足够时写入
/// 返回 true 表示已写入，false 表示被置信度策略过滤
pub async fn maybe_save_long_term(
    pool:       &SqlitePool,
    session_id: &str,
    content:    &str,
    category:   &str,
    confidence: f64,
) -> bool {
    match db::save_long_term_memory_guarded(pool, session_id, content, category, confidence).await {
        Ok(saved) => saved,
        Err(e) => {
            log::warn!("maybe_save_long_term: {e}");
            false
        }
    }
}

// ─── P0: 自动对话摘要 ────────────────────────────────────────────────────────

/// 检查是否需要生成摘要；若累积了 SUMMARIZE_EVERY 条新消息则触发 LLM 摘要。
/// 此函数是 fire-and-forget，调用方应在后台 task 中调用。
pub async fn maybe_summarize(
    pool:       &SqlitePool,
    llm_cfg:    &Arc<LlmConfig>,
    session_id: &str,
) {
    // 查询上次摘要覆盖到的最后一条消息 ID
    let last_id = match db::get_last_summarized_msg_id(pool, session_id).await {
        Ok(id) => id,
        Err(e) => { log::warn!("maybe_summarize get_last_id: {e}"); return; }
    };

    // 统计新增消息条数
    let new_count = match db::count_messages_after(pool, session_id, last_id).await {
        Ok(c) => c,
        Err(e) => { log::warn!("maybe_summarize count: {e}"); return; }
    };

    if new_count < SUMMARIZE_EVERY {
        return; // 消息还不够多
    }

    // 获取待摘要的消息范围
    let end_id = match db::get_latest_message_id(pool, session_id).await {
        Ok(id) => id,
        Err(e) => { log::warn!("maybe_summarize get_latest_id: {e}"); return; }
    };

    let messages = match db::get_messages_in_range(pool, session_id, last_id, end_id).await {
        Ok(m) => m,
        Err(e) => { log::warn!("maybe_summarize get_range: {e}"); return; }
    };

    if messages.is_empty() {
        return;
    }

    // 构建摘要请求
    let conv_text: String = messages
        .iter()
        .filter(|m| m.role == "user" || m.role == "assistant")
        .map(|m| format!("{}: {}", if m.role == "user" { "用户" } else { "Chebo" }, m.content))
        .collect::<Vec<_>>()
        .join("\n");

    let prompt = format!(
        "请对以下对话进行详细摘要（200-300字），必须包含：\n\
         1. 用户当前在做什么/想做什么（项目、目标、问题）\n\
         2. 具体的技术栈、工具、路径等细节信息\n\
         3. 用户表达的个人信息（职业、兴趣、习惯等）\n\
         4. Chebo 给出的关键建议或信息\n\
         5. 用户的情感状态或偏好\n\n{conv_text}"
    );

    let llm_msgs = vec![
        LlmMessage::system("你是一个细致的对话记忆助手，请完整保留关键信息，不要过度压缩。"),
        LlmMessage::user(&prompt),
    ];

    match llm::call_silent(llm_msgs, llm_cfg).await {
        Ok((summary, _)) if !summary.is_empty() => {
            if let Err(e) = db::save_summary(pool, session_id, last_id, end_id, &summary).await {
                log::warn!("maybe_summarize save_summary: {e}");
            } else {
                log::info!("对话摘要已生成（msg {last_id}~{end_id}）: {}", &summary[..summary.len().min(50)]);
            }
        }
        Err(e) => log::warn!("maybe_summarize LLM: {e}"),
        _ => {}
    }
}

// ─── P0: 用户画像提取 ────────────────────────────────────────────────────────

/// 从触发词后截取内容（至句末或 max_chars）
fn extract_after_trigger(content: &str, trigger: &str, max_chars: usize) -> Option<String> {
    let pos = content.find(trigger)?;
    let rest = content[pos + trigger.len()..].trim_start_matches(['：', ':', '，', ',', ' ']);
    if rest.is_empty() {
        return None;
    }
    let end = rest
        .find(['。', '！', '？', '!', '?', '\n'])
        .unwrap_or(rest.len().min(max_chars));
    let value = rest[..end].trim().to_string();
    if value.is_empty() || value.chars().count() > max_chars {
        None
    } else {
        Some(value)
    }
}

/// 从多个触发词中尝试提取内容
fn extract_after_any_trigger(content: &str, triggers: &[&str], max_chars: usize) -> Option<String> {
    for trigger in triggers {
        if content.contains(trigger) {
            if let Some(v) = extract_after_trigger(content, trigger, max_chars) {
                return Some(v);
            }
        }
    }
    None
}

fn auto_user_note_key(fact: &str) -> String {
    let snippet: String = fact.chars().take(10).collect();
    let clean: String = snippet
        .chars()
        .filter(|c| !matches!(c, ' ' | '，' | '。' | '！' | '？' | '\n'))
        .collect();
    if clean.is_empty() {
        "用户备注".to_string()
    } else {
        format!("用户备注_{clean}")
    }
}

fn is_about_chebo_role(content: &str) -> bool {
    let markers = [
        "你就当", "你当", "你可以当", "我希望你", "你是我", "别当", "不要当",
        "扮演", "你不是", "当你是", "把你当", "对你来", "对你来说",
    ];
    markers.iter().any(|m| content.contains(m))
}

/// 用户明确要求「记下来 / 记住」时写入长期记忆，并同步画像
async fn extract_remember_requests(pool: &SqlitePool, session_id: &str, content: &str) {
    let remember_triggers = [
        "帮我记住", "请记住", "记下来", "记住", "帮我记", "别忘了",
        "要记得", "记一下", "备忘", "要记住",
    ];
    if !remember_triggers.iter().any(|t| content.contains(t)) {
        return;
    }

    let fact = extract_after_any_trigger(content, &remember_triggers, 120)
        .unwrap_or_else(|| content.to_string());

    let memory_text = format!("用户明确要求记住：{fact}");
    if let Err(e) = db::save_memory(pool, session_id, &memory_text, "user_note").await {
        log::warn!("extract_remember_requests save_memory: {e}");
    }

    if is_about_chebo_role(content) {
        update_persona_memory(
            pool,
            "user_defined_relationship",
            &fact,
            "relationship",
            1.0,
        )
        .await;
        log::info!("Chebo 画像更新（关系）: {fact}");
    } else {
        let key = auto_user_note_key(&fact);
        if let Err(e) = db::update_user_profile_entry(pool, &key, &fact).await {
            log::warn!("extract_remember_requests user_profile: {e}");
        } else {
            log::info!("用户画像更新（记住）: {key} = {fact}");
        }
    }
}

/// 从对话中提取用户对 Chebo 角色/关系的期望（无需「记住」关键词）
async fn extract_persona_hints(pool: &SqlitePool, content: &str) {
    let rules: &[(&str, &str, &str)] = &[
        ("你就当", "user_expectation", "relationship"),
        ("你当", "user_expectation", "relationship"),
        ("你可以当", "user_expectation", "relationship"),
        ("我希望你", "user_role", "relationship"),
        ("你是我", "relationship_with_user", "relationship"),
        ("把我当", "relationship_with_user", "relationship"),
        ("不要叫我", "address_preference", "relationship"),
        ("叫我", "address_preference", "relationship"),
        ("你不是AI", "identity_note", "trait"),
        ("你不是 AI", "identity_note", "trait"),
        ("像朋友一样", "interaction_style", "trait"),
    ];

    for (trigger, key, category) in rules {
        if content.contains(trigger) {
            if let Some(value) = extract_after_trigger(content, trigger, 80) {
                update_persona_memory(pool, key, &value, category, 0.95).await;
                log::debug!("Chebo 画像提示: [{category}] {key} = {value}");
            }
        }
    }
}

/// 从用户消息中提取个人信息，写入 user_profile 表。
/// 采用关键词匹配 + 简单规则（无需 LLM 调用，零延迟）。
pub async fn extract_user_profile(pool: &SqlitePool, user_content: &str) {
    // (触发词, 画像字段 key)
    let rules: &[(&str, &str)] = &[
        ("我叫", "姓名"),
        ("我的名字是", "姓名"),
        ("我名字叫", "姓名"),
        ("我是", "自我描述"),
        ("我住在", "居住地"),
        ("我在", "位置"),
        ("我喜欢", "兴趣爱好"),
        ("我热爱", "兴趣爱好"),
        ("我讨厌", "讨厌事物"),
        ("我不喜欢", "讨厌事物"),
        ("我的工作是", "职业"),
        ("我是做", "职业"),
        ("我在学", "学习内容"),
        ("我正在学", "学习内容"),
        ("我在写", "当前项目"),
        ("我正在做", "当前项目"),
        ("我的目标是", "目标"),
        ("我的生日", "生日"),
        ("我有", "拥有"),
    ];

    for (trigger, key) in rules {
        if user_content.contains(trigger) {
            if let Some(value) = extract_after_trigger(user_content, trigger, 30) {
                let _ = db::set_user_profile(pool, key, &value).await;
                log::debug!("用户画像更新: {key} = {value}");
            }
            break; // 每条消息只提取第一个匹配
        }
    }
}

/// 加载最近 N 条长期记忆，拼成文本插入 system prompt（v1 兼容接口）
pub async fn load_memory_context(pool: &SqlitePool, session_id: &str) -> Result<String> {
    Ok(build_rich_context_string(pool, session_id).await)
}

/// 从用户消息中检测并保存关键信息（v1 兼容接口）
pub async fn maybe_extract_memory(
    pool:         &SqlitePool,
    session_id:   &str,
    user_content: &str,
) -> Result<()> {
    extract_user_profile(pool, user_content).await;
    extract_persona_hints(pool, user_content).await;
    extract_remember_requests(pool, session_id, user_content).await;

    // 保留旧版简单关键词记忆
    let triggers = ["我叫", "我的名字", "我住在", "我喜欢", "我讨厌", "我的工作", "我在学"];
    for trigger in triggers {
        if user_content.contains(trigger) {
            let memory_text = format!("用户说：{}", user_content);
            db::save_memory(pool, session_id, &memory_text, "user_info").await?;
            break;
        }
    }
    Ok(())
}
