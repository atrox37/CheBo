// ─── event_bus.rs ────────────────────────────────────────────────────────────
// 内部事件总线（P0 基础 + 架构强化扩展）
#![allow(dead_code)]
//   基于 tokio::sync::broadcast，供各后台模块订阅/发布，解耦模块间通信。
//
// 设计原则：
//   - 模块之间尽量通过事件通信，不互相直接调用
//   - 核心状态由 Agent Runtime 统一协调
//   - 注意：这是 Rust 内部通信用；Rust → 前端的通知仍使用 app.emit()
//
// 完整事件清单（15 个变体）：
//   用户交互：  UserMessage / AssistantDone
//   工具系统：  ToolCallRequested / ToolCallFinished
//   记忆系统：  MemoryReflect / MemoryUpdated
//   感知系统：  Perception / PerceptionChanged
//   宠物系统：  StatusChanged / PetMoodChanged / PetActionChanged / TaskComplete
//   发言控制：  ProactiveSpeech / ProactiveThrottled
//   异常处理：  SystemError
// ─────────────────────────────────────────────────────────────────────────────

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

/// 内部事件类型（完整版）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    // ── 用户交互 ───────────────────────────────────────────────────────────────

    /// 用户发送了消息（触发记忆提取等副作用）
    UserMessage { content: String, session_id: String },

    /// LLM 回复完成（供记忆/人格模块订阅）
    AssistantDone {
        text:       String,
        emotion:    String,
        session_id: String,
    },

    // ── 工具系统 ───────────────────────────────────────────────────────────────

    /// 工具调用请求（L2/L3 需等待前端确认）
    /// level: 0=L0只读 / 1=L1轻量 / 2=L2系统 / 3=L3高危
    ToolCallRequested {
        tool:  String,
        args:  String,
        level: u8,
        /// 前端确认 token（L2/L3 专用，L0/L1 为空字符串）
        token: String,
    },

    /// 工具调用完成（无论成功与否）
    ToolCallFinished {
        tool:    String,
        success: bool,
        /// 结果摘要（供 LLM 上下文使用，不超过 200 字）
        summary: String,
    },

    // ── 记忆系统 ───────────────────────────────────────────────────────────────

    /// 触发对话摘要（每 N 条消息后由消息保存逻辑发出）
    MemoryReflect { session_id: String },

    /// 记忆已更新（供感知/UI 模块感知记忆写入事件）
    /// layer: "short" | "mid" | "long" | "persona"
    MemoryUpdated { layer: String, key: String },

    // ── 感知系统 ───────────────────────────────────────────────────────────────

    /// 感知到环境变化（活跃窗口 / 剪贴板，原始事件）
    Perception { kind: String, data: String },

    /// 感知状态发生有意义的变化（经分类处理后发出）
    PerceptionChanged {
        /// 应用类型："coding" | "browsing" | "meeting" | "gaming" | "idle" | "other"
        app_type:  String,
        /// 用户空闲秒数
        idle_secs: u64,
        /// 是否全屏
        is_fullscreen: bool,
    },

    // ── 宠物系统 ───────────────────────────────────────────────────────────────

    /// 宠物核心数值发生变化（每 tick 由 tick_loop 发出）
    StatusChanged,

    /// 宠物情绪/好感度发生有意义的变化
    PetMoodChanged {
        mood:      f32,
        affection: f32,
        /// 变化原因：如 "feed" / "task_complete" / "ignored" / "praised"
        reason:    String,
    },

    /// 宠物动作状态变化（动画联动）
    PetActionChanged {
        /// 动作名称：如 "idle" / "eating" / "studying" / "working" / "sleeping"
        action: String,
    },

    /// 任务完成（供记忆/人格模块订阅）
    TaskComplete { task_id: String, task_name: String },

    // ── 主动发言 ───────────────────────────────────────────────────────────────

    /// 定时主动发言请求（由 ai_comment_loop 发出，实际执行由订阅者决定）
    ProactiveSpeech,

    /// 主动发言被节流跳过（供调试日志使用）
    ProactiveThrottled {
        /// 节流原因：如 "user_typing" | "fullscreen" | "too_frequent" | "meeting" | "rejected"
        reason: String,
    },

    // ── 异常处理 ───────────────────────────────────────────────────────────────

    /// 模块发生错误（供 Agent Runtime 进入 ErrorRecover 状态）
    SystemError {
        /// 出错模块名：如 "llm" / "tools" / "memory" / "perception"
        module: String,
        msg:    String,
    },
}

/// 事件总线：持有 broadcast sender，可低成本 clone
#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<AgentEvent>,
}

impl EventBus {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(256);
        Self { sender }
    }

    /// 发布事件（无接收者时静默忽略）
    pub fn publish(&self, event: AgentEvent) {
        let _ = self.sender.send(event);
    }

    /// 订阅事件（每个订阅者获得独立接收器）
    pub fn subscribe(&self) -> broadcast::Receiver<AgentEvent> {
        self.sender.subscribe()
    }
}
