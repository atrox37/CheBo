// ─── chat_intent.rs ───────────────────────────────────────────────────────────
// Chat Intent 分类器（P1：三层路由）
//
// 决策流程：
//   硬信号层 (hard_signal_classify) — 零 LLM 开销，覆盖确定场景
//     └─ 未命中 → AI 分类层 (ai_classify) — 核心路径
//          └─ 失败/无效 → 规则兜底层 (rule_based_fallback)
//     └─ 应跳过 AI → 规则兜底层
// ─────────────────────────────────────────────────────────────────────────────

use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::llm::{self, LlmConfig, LlmMessage};

// ─── 意图枚举 ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatIntent {
    /// 普通聊天 / 陪伴 / 寒暄
    CasualChat,
    /// 技术问答 / 知识解释 / 概念说明
    TechnicalQa,
    /// 继续之前的话题、方案、任务
    ContinueTask,
    /// 项目设计 / 架构评审 / 方案优化
    ProjectReview,
    /// 用户明确要求记住某事
    RememberRequest,
    /// 需要读文件 / 搜索 / 执行命令 / 查看剪贴板等工具
    ToolOperation,
    /// 情绪陪伴 / 压力 / 疲惫 / 迷茫
    EmotionalSupport,
    /// 深度思考 / 复杂分析 / 多步骤推理
    DeepThink,
}

// ─── 决策附属策略 ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryAction {
    /// 不处理记忆
    None,
    /// 生成候选记忆，但不立即长期写入
    Candidate,
    /// 用户明确要求记住，直接高置信写入
    WriteExplicit,
    /// 更新当前任务状态 / 工作记忆
    UpdateWorkingMemory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecallStrategy {
    /// 不召回
    None,
    /// 只用最近消息
    RecentOnly,
    /// 加载 Working Memory
    WorkingMemory,
    /// 向量召回 TopK
    VectorTopK,
    /// 项目上下文召回
    ProjectContext,
    /// 混合召回：最近消息 + WorkingMemory + 摘要 + 向量
    FullHybrid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseMode {
    /// 桌宠模式短回复
    PetShort,
    /// 助手模式详细回复
    AssistantDetailed,
    /// 长期任务模式
    TaskMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolPolicy {
    /// 不主动注入工具
    None,
    /// 只允许只读工具，如 memory_recall / read_file / web_search
    ReadOnly,
    /// 允许 L0/L1 轻工具
    LightTools,
    /// 允许完整工具循环，但 L2/L3 仍需确认
    FullTools,
}

// ─── 输入数据结构 ─────────────────────────────────────────────────────────────

/// 本轮用户输入和运行状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentInput {
    pub content: String,
    pub deep_think: bool,
    pub assistant_mode: bool,
    pub has_images: bool,
}

/// 上一轮积累下来的系统上下文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentContext {
    /// 最近几条对话的简短文本（不要塞完整历史，每条 ≤ 80 字）
    pub recent_messages_brief: Vec<String>,
    /// 当前工作记忆摘要（项目、目标、未完成问题等）
    pub working_memory_brief: Option<String>,
}

// ─── 输出决策 ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentDecision {
    pub intent: ChatIntent,
    pub confidence: f32,

    /// 一句话解释，方便日志观察，不进入最终回复
    pub reason: String,

    pub recall_strategy: RecallStrategy,
    pub memory_action: MemoryAction,
    pub response_mode: ResponseMode,
    pub tool_policy: ToolPolicy,

    /// 是否建议创建长期 AgentTask
    pub should_start_task: bool,

    /// 建议注入或允许的工具名
    pub suggested_tools: Vec<String>,
}

// ─── 快捷构造器 ───────────────────────────────────────────────────────────────

impl IntentDecision {
    pub fn fallback() -> Self {
        Self {
            intent: ChatIntent::CasualChat,
            confidence: 0.5,
            reason: "fallback to casual chat".to_string(),
            recall_strategy: RecallStrategy::RecentOnly,
            memory_action: MemoryAction::None,
            response_mode: ResponseMode::PetShort,
            tool_policy: ToolPolicy::None,
            should_start_task: false,
            suggested_tools: vec![],
        }
    }

    pub fn deep_think() -> Self {
        Self {
            intent: ChatIntent::DeepThink,
            confidence: 1.0,
            reason: "deep_think flag is enabled".to_string(),
            recall_strategy: RecallStrategy::FullHybrid,
            memory_action: MemoryAction::UpdateWorkingMemory,
            response_mode: ResponseMode::AssistantDetailed,
            tool_policy: ToolPolicy::FullTools,
            should_start_task: false,
            suggested_tools: vec!["memory_recall".to_string(), "web_search".to_string()],
        }
    }

    pub fn remember_request(reason: impl Into<String>) -> Self {
        Self {
            intent: ChatIntent::RememberRequest,
            confidence: 1.0,
            reason: reason.into(),
            recall_strategy: RecallStrategy::RecentOnly,
            memory_action: MemoryAction::WriteExplicit,
            response_mode: ResponseMode::PetShort,
            tool_policy: ToolPolicy::None,
            should_start_task: false,
            suggested_tools: vec![],
        }
    }

    pub fn casual_short(reason: impl Into<String>) -> Self {
        Self {
            intent: ChatIntent::CasualChat,
            confidence: 0.9,
            reason: reason.into(),
            recall_strategy: RecallStrategy::RecentOnly,
            memory_action: MemoryAction::None,
            response_mode: ResponseMode::PetShort,
            tool_policy: ToolPolicy::None,
            should_start_task: false,
            suggested_tools: vec![],
        }
    }
}

// ─── 对外主入口 ───────────────────────────────────────────────────────────────

/// 三层路由：硬信号 → AI 分类 → 规则兜底
pub async fn decide(
    input: IntentInput,
    context: IntentContext,
    llm_cfg: &Arc<LlmConfig>,
) -> IntentDecision {
    // 第 1 层：硬信号（零 LLM 开销）
    if let Some(decision) = hard_signal_classify(&input) {
        log::info!("chat_intent: hard_signal -> {:?}", decision.intent);
        return decision;
    }

    // 判断是否应跳过 AI 分类（短句、无强信号等）
    if should_skip_ai_classifier(&input) {
        let decision = rule_based_fallback(&input, &context);
        log::info!("chat_intent: skip_ai -> {:?}", decision.intent);
        return decision;
    }

    // 第 2 层：AI 分类
    match ai_classify(&input, &context, llm_cfg).await {
        Ok(decision) if validate_decision(&decision) => {
            log::info!(
                "chat_intent: ai -> {:?} (confidence={}, reason={})",
                decision.intent, decision.confidence, decision.reason
            );
            decision
        }
        Ok(decision) => {
            log::warn!("chat_intent: ai returned invalid/low-confidence decision: {:?}", decision);
            let fb = rule_based_fallback(&input, &context);
            log::info!("chat_intent: fallback after invalid ai -> {:?}", fb.intent);
            fb
        }
        Err(err) => {
            log::warn!("chat_intent: ai classify failed: {err}");
            let fb = rule_based_fallback(&input, &context);
            log::info!("chat_intent: fallback after ai error -> {:?}", fb.intent);
            fb
        }
    }
}

// ─── 第 1 层：硬信号分类 ─────────────────────────────────────────────────────

fn hard_signal_classify(input: &IntentInput) -> Option<IntentDecision> {
    let text = input.content.trim();

    // 深度思考标记
    if input.deep_think {
        return Some(IntentDecision::deep_think());
    }

    // 空输入
    if text.is_empty() {
        return Some(IntentDecision::casual_short("empty input"));
    }

    // 带图片 → 工具操作 / 视觉处理
    if input.has_images {
        return Some(IntentDecision {
            intent: ChatIntent::ToolOperation,
            confidence: 0.95,
            reason: "message contains images, should route to vision/tool handling".to_string(),
            recall_strategy: RecallStrategy::RecentOnly,
            memory_action: MemoryAction::None,
            response_mode: if input.assistant_mode { ResponseMode::AssistantDetailed } else { ResponseMode::PetShort },
            tool_policy: ToolPolicy::LightTools,
            should_start_task: false,
            suggested_tools: vec!["take_screenshot".to_string()],
        });
    }

    // 明确记忆请求
    if contains_explicit_remember_request(text) {
        return Some(IntentDecision::remember_request("user explicitly asked Chebo to remember something"));
    }

    // 极短寒暄（≤ 4 字、无问号、无强信号）→ 不走 AI
    if text.chars().count() <= 4 && !text.contains('?') && !text.contains('？')
        && !has_strong_intent_signal(text)
    {
        return Some(IntentDecision::casual_short("very short casual input"));
    }

    None
}

/// 检测明确的"记住"请求
fn contains_explicit_remember_request(text: &str) -> bool {
    let keywords = [
        "记住", "帮我记住", "记下来", "你要记得",
        "以后你都", "以后回答", "我希望你以后", "以后别", "以后不要",
    ];
    keywords.iter().any(|kw| text.contains(kw))
}

/// 检测强意图信号（即使文本很短也不应跳过）
fn has_strong_intent_signal(text: &str) -> bool {
    // ContinueTask 信号
    if ["继续", "接着", "下一步", "然后"].iter().any(|kw| text.contains(kw)) {
        return true;
    }
    // RememberRequest 信号
    if ["记住", "记下来"].iter().any(|kw| text.contains(kw)) {
        return true;
    }
    // ToolOperation 信号
    if ["查", "搜", "读", "写", "执行", "打开", "搜索"].iter().any(|kw| text.contains(kw)) {
        return true;
    }
    // DeepThink 信号
    if ["分析", "设计", "完整实现"].iter().any(|kw| text.contains(kw)) {
        return true;
    }
    false
}

// ─── 判断是否跳过 AI 分类 ────────────────────────────────────────────────────

fn should_skip_ai_classifier(input: &IntentInput) -> bool {
    let text = input.content.trim();

    // 有强意图信号 → 不跳过（让硬信号层或规则层处理）
    if has_strong_intent_signal(text) {
        return false;
    }

    // 桌宠模式短句（≤ 15 字）且无强信号 → 跳过 AI
    if !input.assistant_mode && text.chars().count() <= 15 {
        return true;
    }

    // 助手模式极短（≤ 8 字）且无提问 → 跳过 AI
    if input.assistant_mode
        && text.chars().count() <= 8
        && !text.contains('?') && !text.contains('？')
    {
        return true;
    }

    false
}

// ─── 第 2 层：AI 分类器 ──────────────────────────────────────────────────────

const INTENT_CLASSIFIER_SYSTEM: &str = r#"
你是 Chebo 的聊天意图分类器。你的任务不是回答用户，而是判断本轮消息应该如何处理。

你必须只输出 JSON，不要输出 Markdown，不要解释。

可选 intent：
- casual_chat：普通聊天、陪伴、寒暄
- technical_qa：技术问答、知识解释、概念说明
- continue_task：继续之前的话题、方案、任务
- project_review：项目设计、架构评审、方案优化、产品设计
- remember_request：用户明确要求记住某事
- tool_operation：需要读取文件、搜索网页、执行命令、查看剪贴板、截图等工具
- emotional_support：情绪陪伴、压力、疲惫、迷茫、沮丧
- deep_think：复杂分析、深度思考、多步骤推理

可选 recall_strategy：
- none / recent_only / working_memory / vector_top_k / project_context / full_hybrid

可选 memory_action：
- none / candidate / write_explicit / update_working_memory

可选 response_mode：
- pet_short / assistant_detailed / task_mode

可选 tool_policy：
- none / read_only / light_tools / full_tools

输出 JSON 格式：
{
  "intent": "technical_qa",
  "confidence": 0.85,
  "reason": "一句话说明判断理由",
  "recall_strategy": "vector_top_k",
  "memory_action": "none",
  "response_mode": "assistant_detailed",
  "tool_policy": "read_only",
  "should_start_task": false,
  "suggested_tools": ["memory_recall"]
}
"#;

async fn ai_classify(
    input: &IntentInput,
    context: &IntentContext,
    llm_cfg: &Arc<LlmConfig>,
) -> anyhow::Result<IntentDecision> {
    let prompt = build_intent_prompt(input, context);

    let messages = vec![
        LlmMessage::system(INTENT_CLASSIFIER_SYSTEM),
        LlmMessage::user(&prompt),
    ];

    let (raw, _) = llm::call_silent(messages, llm_cfg).await?;

    parse_intent_decision(&raw)
}

fn build_intent_prompt(input: &IntentInput, context: &IntentContext) -> String {
    let recent = if context.recent_messages_brief.is_empty() {
        "无".to_string()
    } else {
        context.recent_messages_brief
            .iter()
            .take(5)
            .map(|m| format!("- {m}"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let wm = context.working_memory_brief
        .clone()
        .unwrap_or_else(|| "无".to_string());

    format!(
        r#"
【当前用户消息】
{content}

【运行环境】
assistant_mode: {assistant_mode}
deep_think: {deep_think}
has_images: {has_images}

【最近对话简述】
{recent}

【当前 Working Memory】
{working_memory}

请输出 IntentDecision JSON。
"#,
        content = input.content,
        assistant_mode = input.assistant_mode,
        deep_think = input.deep_think,
        has_images = input.has_images,
        recent = recent,
        working_memory = wm,
    )
}

// ─── JSON 解析与容错 ─────────────────────────────────────────────────────────

fn parse_intent_decision(raw: &str) -> anyhow::Result<IntentDecision> {
    let json = extract_json_object(raw)
        .ok_or_else(|| anyhow::anyhow!("no JSON object found in intent output"))?;

    let decision: IntentDecision = serde_json::from_str(json)?;
    Ok(decision)
}

fn extract_json_object(raw: &str) -> Option<&str> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    if end <= start { return None; }
    Some(&raw[start..=end])
}

fn validate_decision(decision: &IntentDecision) -> bool {
    // 置信度必须在 0-1 范围内
    if !(0.0..=1.0).contains(&decision.confidence) {
        return false;
    }
    // 低置信度不直接采用
    if decision.confidence < 0.65 {
        return false;
    }
    // 防止 AI 过度创建任务
    if decision.should_start_task {
        matches!(
            decision.intent,
            ChatIntent::DeepThink | ChatIntent::ProjectReview | ChatIntent::ContinueTask
        )
    } else {
        true
    }
}

// ─── 第 3 层：规则兜底 ───────────────────────────────────────────────────────

fn rule_based_fallback(input: &IntentInput, context: &IntentContext) -> IntentDecision {
    let text = input.content.trim();
    let lower = text.to_lowercase();

    if looks_like_continue_task(text) {
        return IntentDecision {
            intent: ChatIntent::ContinueTask,
            confidence: 0.7,
            reason: "fallback: message refers to previous context".to_string(),
            recall_strategy: RecallStrategy::WorkingMemory,
            memory_action: MemoryAction::UpdateWorkingMemory,
            response_mode: if input.assistant_mode { ResponseMode::AssistantDetailed } else { ResponseMode::PetShort },
            tool_policy: ToolPolicy::ReadOnly,
            should_start_task: false,
            suggested_tools: vec!["memory_recall".to_string()],
        };
    }

    if looks_like_tool_operation(text) {
        return IntentDecision {
            intent: ChatIntent::ToolOperation,
            confidence: 0.7,
            reason: "fallback: message appears to request tool operation".to_string(),
            recall_strategy: RecallStrategy::RecentOnly,
            memory_action: MemoryAction::None,
            response_mode: if input.assistant_mode { ResponseMode::AssistantDetailed } else { ResponseMode::PetShort },
            tool_policy: ToolPolicy::LightTools,
            should_start_task: false,
            suggested_tools: vec![],
        };
    }

    if looks_like_project_review(text) {
        return IntentDecision {
            intent: ChatIntent::ProjectReview,
            confidence: 0.7,
            reason: "fallback: message appears to be project or architecture discussion".to_string(),
            recall_strategy: RecallStrategy::ProjectContext,
            memory_action: MemoryAction::UpdateWorkingMemory,
            response_mode: ResponseMode::AssistantDetailed,
            tool_policy: ToolPolicy::ReadOnly,
            should_start_task: false,
            suggested_tools: vec!["memory_recall".to_string()],
        };
    }

    if looks_like_technical_qa(text, &lower) {
        return IntentDecision {
            intent: ChatIntent::TechnicalQa,
            confidence: 0.7,
            reason: "fallback: message appears to be technical question".to_string(),
            recall_strategy: RecallStrategy::VectorTopK,
            memory_action: MemoryAction::None,
            response_mode: if input.assistant_mode { ResponseMode::AssistantDetailed } else { ResponseMode::PetShort },
            tool_policy: ToolPolicy::ReadOnly,
            should_start_task: false,
            suggested_tools: vec!["memory_recall".to_string()],
        };
    }

    if looks_like_emotional_support(text) {
        return IntentDecision {
            intent: ChatIntent::EmotionalSupport,
            confidence: 0.7,
            reason: "fallback: message appears to express emotional state".to_string(),
            recall_strategy: RecallStrategy::RecentOnly,
            memory_action: MemoryAction::Candidate,
            response_mode: ResponseMode::PetShort,
            tool_policy: ToolPolicy::None,
            should_start_task: false,
            suggested_tools: vec![],
        };
    }

    // 默认：普通聊天
    IntentDecision {
        intent: ChatIntent::CasualChat,
        confidence: 0.6,
        reason: "fallback: default casual chat".to_string(),
        recall_strategy: RecallStrategy::RecentOnly,
        memory_action: MemoryAction::None,
        response_mode: if input.assistant_mode { ResponseMode::AssistantDetailed } else { ResponseMode::PetShort },
        tool_policy: ToolPolicy::None,
        should_start_task: false,
        suggested_tools: vec![],
    }
}

// ─── 规则检测函数 ─────────────────────────────────────────────────────────────

fn looks_like_continue_task(text: &str) -> bool {
    let keywords = [
        "继续", "接着", "刚才", "上面", "前面",
        "之前那个", "这个方案", "这个机制", "这个项目",
        "我们刚才", "按你说的", "下一步",
    ];
    keywords.iter().any(|kw| text.contains(kw))
}

fn looks_like_tool_operation(text: &str) -> bool {
    let keywords = [
        "查一下", "搜索", "搜一下", "帮我查",
        "读取", "读一下", "打开文件", "看一下文件",
        "列一下目录", "执行", "运行",
        "git status", "截图", "剪贴板",
    ];
    keywords.iter().any(|kw| text.contains(kw))
}

fn looks_like_project_review(text: &str) -> bool {
    let keywords = [
        "项目", "架构", "方案", "机制", "设计",
        "重构", "优化", "评审", "路线图", "模块", "系统", "产品",
    ];
    text.chars().count() >= 12 && keywords.iter().any(|kw| text.contains(kw))
}

fn looks_like_technical_qa(text: &str, lower: &str) -> bool {
    let question_words = [
        "什么", "怎么", "为什么", "如何", "区别",
        "原理", "介绍", "解释", "是什么", "?", "？",
    ];
    let tech_words = [
        "rust", "tauri", "vue", "react", "python", "docker",
        "agent", "llm", "rag", "sqlite", "sql", "api",
        "前端", "后端", "数据库", "向量", "记忆", "工具",
        "模型", "状态机", "架构", "prompt",
    ];

    let has_question = question_words.iter().any(|kw| text.contains(kw));
    let has_tech = tech_words.iter().any(|kw| lower.contains(&kw.to_lowercase()));

    has_question && has_tech
}

fn looks_like_emotional_support(text: &str) -> bool {
    let keywords = [
        "累", "焦虑", "烦", "难受", "压力",
        "不想干", "emo", "崩溃", "迷茫", "没动力", "好烦",
    ];
    keywords.iter().any(|kw| text.contains(kw))
}

// ─── 工具函数 ─────────────────────────────────────────────────────────────────

/// 兼容旧版 chat_router::should_run_agent_task（供外部调用）
pub fn should_run_agent_task(content: &str, deep_think: bool) -> bool {
    let c = content.trim();
    if c.len() < 10 { return false; }
    let strong = [
        "帮我整理", "帮我完成", "帮我处理", "帮我执行", "帮我写一份",
        "持续监控", "持续关注", "定期检查", "分步骤", "一步步",
        "多步", "整个文件夹", "整个项目", "批量处理", "制定计划",
        "完成以下", "按计划",
    ];
    if strong.iter().any(|s| c.contains(s)) { return true; }
    if deep_think {
        let action = ["帮我", "请帮", "整理", "分析", "总结", "生成", "导出"];
        if action.iter().any(|s| c.contains(s)) && c.len() >= 20 { return true; }
    }
    false
}

pub const DEEP_THINK_SYSTEM_ADDITION: &str = r#"
【深度思考】
用户希望更审慎地处理本条消息。请先拆解问题，必要时多轮调用工具，再给出结论。
"#;

pub const DEEP_THINK_MAX_TOOL_TURNS: usize = 14;