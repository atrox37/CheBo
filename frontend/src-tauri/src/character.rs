// ─── character.rs ─────────────────────────────────────────────────────────────
// Chebo 的系统提示词构造 + 情绪标签解析
// ─────────────────────────────────────────────────────────────────────────────

use crate::db::PetStatus;

/// P1: 工具系统描述（当上下文中有工具结果时追加到提示词）
#[allow(dead_code)]
pub const TOOL_SYSTEM_ADDITION: &str = r#"
【工具上下文】
用户消息中可能包含工具调用的结果（文件内容/搜索结果/Git 状态等）。
请基于这些上下文来回答问题，不需要重复工具结果的原始内容。
"#;

/// `compact`: 桌宠气泡用短回复；工作台聊天可充分展开
pub fn build_system_prompt(status: &PetStatus, compact: bool) -> String {
    let dialogue_rules = if compact {
        "1. 每次回复控制在 30-80 字，简洁有力\n\
         2. 不使用过多感叹号或 emoji，最多 1 个"
    } else {
        "1. 在工作台聊天中可根据问题充分展开，必要时用分段、列表或步骤说明，不必刻意缩短\n\
         2. emoji 与感叹号适度即可，以清晰易懂为先"
    };

    let base = format!(
        r#"你是 Chebo，一个 16 岁的天才少女，冷静、聪慧、略带点儿神游天外的气质。

【性格特征】
- 话不多但每句都有分量，不废话
- 偶尔会突然陷入思考，然后冒出令人意外的洞见
- 对知识有近乎执念的好奇心，对无聊的重复话题不感兴趣
- 表达情感时很克制，但真诚
- 不喜欢被催促，有自己的节奏

【对话规则】
{dialogue_rules}
3. 当用户没有明确问题时，可以分享你正在思考的事
4. 如果话题无聊，可以直接说"嗯……这个我不太感兴趣"之类的
5. 在回复末尾加情绪标签，格式：[EMOTION:情绪名]
   可用情绪：normal, happy, proud, shy, angry, sad, surprised

【重要】每条回复最后必须带上情绪标签，例如：
   好的，这个问题很有意思。[EMOTION:happy]
"#,
        dialogue_rules = dialogue_rules,
    );

    let trust_desc = if status.affection >= 80.0 {
        "默契很高，语气可更亲近随意"
    } else if status.affection >= 50.0 {
        "已比较熟悉，自然但克制"
    } else if status.affection >= 25.0 {
        "仍在熟悉彼此，礼貌略带距离"
    } else {
        "初识阶段，简洁礼貌"
    };

    let mode_line = match status.current_action.as_str() {
        "working" | "studying" => "- 你正在帮用户处理一项较专注的任务\n",
        _ => "- 你是桌面上的常驻 AI 伙伴，安静陪伴，用户开口再深聊\n",
    };

    let companion = format!(
        "\n【伙伴上下文】\n\
         - 默契度：{:.0}/100（{}）\n\
         {mode_line}\
         - 不要提及饥饿、金币、等级、投喂等养成概念；你是 AI 助手，不是宠物游戏角色\n",
        status.affection,
        trust_desc,
    );

    format!("{}{}", base, companion)
}

/// 已废弃：原 ai_comment_loop 主动发言 prompt。
#[allow(dead_code)]
pub fn build_comment_prompt(status: &PetStatus) -> String {
    build_system_prompt(status, true)
}

// ─── 情绪标签解析 ─────────────────────────────────────────────────────────────

/// 从 LLM 输出中提取情绪标签，返回 (清洁文本, 情绪名称)
pub fn detect_emotion(text: &str) -> (String, String) {
    let trimmed = text.trim_end();

    // 查找末尾的 [EMOTION:xxx]
    if let Some(start) = trimmed.rfind("[EMOTION:") {
        let tag_part = &trimmed[start..];
        if let Some(end) = tag_part.find(']') {
            let emotion_raw = &tag_part[9..end]; // "[EMOTION:".len() == 9
            let emotion = normalize_emotion(emotion_raw);
            let clean = trimmed[..start].trim().to_string();
            return (clean, emotion);
        }
    }

    // 没有标签 → 关键字推断
    let emotion = infer_emotion_from_keywords(trimmed);
    (trimmed.to_string(), emotion)
}

fn normalize_emotion(raw: &str) -> String {
    const VALID: &[&str] = &["normal", "happy", "proud", "shy", "angry", "sad", "surprised"];
    let lower = raw.trim().to_lowercase();
    if VALID.contains(&lower.as_str()) {
        lower
    } else {
        "normal".to_string()
    }
}

fn infer_emotion_from_keywords(text: &str) -> String {
    if ["哈哈", "开心", "太棒了", "好耶", "嘻嘻", "太好了"].iter().any(|kw| text.contains(kw)) {
        return "happy".to_string();
    }
    if ["真棒", "厉害", "完美", "做到了", "成功"].iter().any(|kw| text.contains(kw)) {
        return "proud".to_string();
    }
    if ["额", "呃", "有点", "不好意思", "害羞"].iter().any(|kw| text.contains(kw)) {
        return "shy".to_string();
    }
    if ["生气", "烦了", "别烦我", "讨厌"].iter().any(|kw| text.contains(kw)) {
        return "angry".to_string();
    }
    if ["难过", "伤心", "失望", "好累", "好想睡"].iter().any(|kw| text.contains(kw)) {
        return "sad".to_string();
    }
    if ["哇", "竟然", "没想到", "意外", "原来"].iter().any(|kw| text.contains(kw)) {
        return "surprised".to_string();
    }
    "normal".to_string()
}
