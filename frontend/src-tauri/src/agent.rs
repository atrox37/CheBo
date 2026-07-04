// ─── agent.rs ────────────────────────────────────────────────────────────────
// Agent Runtime 状态机（P0 基础 + 架构强化扩展）
#![allow(dead_code)]
//
// 完整状态转移图：
//
//   Idle / Sleeping ──────────────────→ Thinking  （收到消息 / 主动发言）
//   Thinking        ──────────────────→ Talking   （第一个 token 到达）
//   Thinking        ──────────────────→ Idle      （LLM 出错 / 取消）
//   Talking         ──────────────────→ Idle      （流式输出完成）
//   Idle            ──────────────────→ Working   （任务启动）
//   Working         ──────────────────→ Idle      （任务完成 / 取消）
//   Idle            ──────────────────→ Sleeping  （空闲超 10 min）
//   Sleeping        ──────────────────→ Idle      （用户有活动）
//
//   任意可中断状态 ─────────────────→ Interrupted → （500ms后）→ Idle
//   Idle/Sleeping   ──────────────────→ Observing  （感知扫描启动）
//   Observing       ──────────────────→ Idle      （感知扫描结束）
//   Idle/Talking    ──────────────────→ WaitingConfirm （L2/L3 工具等确认）
//   WaitingConfirm  ──────────────────→ ExecutingTool  （用户确认）
//   WaitingConfirm  ──────────────────→ Idle       （用户拒绝）
//   ExecutingTool   ──────────────────→ Idle       （工具执行完成）
//   任意状态        ──────────────────→ ErrorRecover   （LLM/工具错误）
//   ErrorRecover    ──────────────────→ Idle       （恢复完成）
// ─────────────────────────────────────────────────────────────────────────────

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

// ─── 状态枚举 ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AgentState {
    // ── 原始 5 态 ──────────────────────────────────────────────────────────────
    /// 等待用户输入，默认状态
    Idle,
    /// LLM 处理中（已收到消息，未开始输出）
    Thinking,
    /// 流式输出中
    Talking,
    /// 执行计划任务（读书/工作 task）
    Working,
    /// 长时间无交互，低活跃模式
    Sleeping,

    // ── 扩展 5 态 ──────────────────────────────────────────────────────────────
    /// 感知系统正在扫描环境（窗口/剪贴板/空闲检测），未进入对话
    Observing,
    /// 等待用户对 L2/L3 工具调用的安全确认
    WaitingConfirm,
    /// 工具执行中（区别于 Working 任务系统，专指单次工具调用）
    ExecutingTool,
    /// 被用户打断（过渡态，自动 500ms 后恢复 Idle）
    Interrupted,
    /// 错误恢复中（LLM/Tool 失败后的短暂恢复状态）
    ErrorRecover,
}

// ─── 运行时 ───────────────────────────────────────────────────────────────────

pub struct AgentRuntime {
    state:         Mutex<AgentState>,
    last_activity: Mutex<Instant>,
}

impl AgentRuntime {
    pub fn new() -> Self {
        Self {
            state:         Mutex::new(AgentState::Idle),
            last_activity: Mutex::new(Instant::now()),
        }
    }

    // ── 只读查询 ───────────────────────────────────────────────────────────────

    /// 读取当前状态快照
    pub fn current(&self) -> AgentState {
        self.state.lock().unwrap().clone()
    }

    /// 记录最近活动时间（用户发消息时调用）
    pub fn mark_activity(&self) {
        *self.last_activity.lock().unwrap() = Instant::now();
    }

    /// 距上次活动的秒数（用于空闲/睡眠检测）
    pub fn idle_secs(&self) -> u64 {
        self.last_activity.lock().unwrap().elapsed().as_secs()
    }

    /// 是否可以接受新消息（Thinking/Talking/WaitingConfirm/ExecutingTool 时不可接受）
    pub fn can_receive_message(&self) -> bool {
        !matches!(
            self.current(),
            AgentState::Thinking | AgentState::Talking |
            AgentState::WaitingConfirm | AgentState::ExecutingTool
        )
    }

    /// 当前状态是否可被打断（Talking / Observing 可被打断）
    pub fn is_interruptible(&self) -> bool {
        matches!(self.current(), AgentState::Talking | AgentState::Observing)
    }

    /// 是否处于聊天生成流程（Thinking / Talking / 工具确认与执行）
    pub fn is_generating(&self) -> bool {
        matches!(
            self.current(),
            AgentState::Thinking
                | AgentState::Talking
                | AgentState::WaitingConfirm
                | AgentState::ExecutingTool
        )
    }

    /// 用户主动停止生成：立即回到 Idle（不走 Interrupted 动画）
    pub fn cancel_generation(&self, app: &AppHandle) {
        if self.is_generating() {
            self.set_idle(app);
        }
    }

    /// 工具轮次结束后回到 Thinking，等待下一轮 LLM
    pub fn resume_thinking_after_tools(&self, app: &AppHandle) {
        let mut state = self.state.lock().unwrap();
        if matches!(
            *state,
            AgentState::ExecutingTool | AgentState::WaitingConfirm
        ) {
            *state = AgentState::Thinking;
            drop(state);
            let _ = app.emit("agent_state", AgentState::Thinking);
        }
    }

    // ── 原始状态转换 ──────────────────────────────────────────────────────────

    /// 尝试进入 Thinking（仅从 Idle/Sleeping 可转换）
    /// 返回 true 表示转换成功
    pub fn try_start_thinking(&self, app: &AppHandle) -> bool {
        let mut state = self.state.lock().unwrap();
        match *state {
            AgentState::Idle | AgentState::Sleeping => {
                *state = AgentState::Thinking;
                drop(state);
                let _ = app.emit("agent_state", AgentState::Thinking);
                true
            }
            _ => false,
        }
    }

    /// 进入 Talking（LLM 开始输出 token）
    pub fn set_talking(&self, app: &AppHandle) {
        *self.state.lock().unwrap() = AgentState::Talking;
        let _ = app.emit("agent_state", AgentState::Talking);
    }

    /// 回到 Idle
    pub fn set_idle(&self, app: &AppHandle) {
        *self.state.lock().unwrap() = AgentState::Idle;
        let _ = app.emit("agent_state", AgentState::Idle);
    }

    /// 进入 Working（计划任务开始）
    pub fn set_working(&self, app: &AppHandle) {
        *self.state.lock().unwrap() = AgentState::Working;
        let _ = app.emit("agent_state", AgentState::Working);
    }

    /// 进入 Sleeping（长时间无交互）
    pub fn set_sleeping(&self, app: &AppHandle) {
        *self.state.lock().unwrap() = AgentState::Sleeping;
        let _ = app.emit("agent_state", AgentState::Sleeping);
    }

    // ── 扩展状态转换 ──────────────────────────────────────────────────────────

    /// 进入 Observing（感知系统扫描，仅从 Idle/Sleeping 转换）
    /// 返回 true 表示转换成功
    pub fn set_observing(&self, app: &AppHandle) -> bool {
        let mut state = self.state.lock().unwrap();
        match *state {
            AgentState::Idle | AgentState::Sleeping => {
                *state = AgentState::Observing;
                drop(state);
                let _ = app.emit("agent_state", AgentState::Observing);
                true
            }
            _ => false,
        }
    }

    /// 进入 WaitingConfirm（等待 L2/L3 工具安全确认）
    pub fn set_waiting_confirm(&self, app: &AppHandle) {
        *self.state.lock().unwrap() = AgentState::WaitingConfirm;
        let _ = app.emit("agent_state", AgentState::WaitingConfirm);
    }

    /// 进入 ExecutingTool（用户确认后执行工具）
    pub fn set_executing_tool(&self, app: &AppHandle) {
        *self.state.lock().unwrap() = AgentState::ExecutingTool;
        let _ = app.emit("agent_state", AgentState::ExecutingTool);
    }

    /// 进入 Interrupted（被打断，异步 500ms 后自动恢复 Idle）
    /// 仅对可中断状态（Talking/Observing）有效
    pub fn set_interrupted(&self, app: &AppHandle) {
        if !self.is_interruptible() {
            return;
        }
        *self.state.lock().unwrap() = AgentState::Interrupted;
        let _ = app.emit("agent_state", AgentState::Interrupted);

        // 500ms 后自动恢复 Idle
        let app_clone = app.clone();
        let state_ptr = self as *const AgentRuntime as usize;
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(500)).await;
            // 安全：AgentRuntime 由 Arc 管理，生命周期超过此任务
            let runtime = unsafe { &*(state_ptr as *const AgentRuntime) };
            let mut s = runtime.state.lock().unwrap();
            if *s == AgentState::Interrupted {
                *s = AgentState::Idle;
                drop(s);
                let _ = app_clone.emit("agent_state", AgentState::Idle);
            }
        });
    }

    /// 进入 ErrorRecover（任何状态均可转换，表示发生了需恢复的错误）
    pub fn set_error_recover(&self, app: &AppHandle) {
        *self.state.lock().unwrap() = AgentState::ErrorRecover;
        let _ = app.emit("agent_state", AgentState::ErrorRecover);
    }
}

// AgentRuntime 内部使用 Mutex，可安全跨线程共享
unsafe impl Send for AgentRuntime {}
unsafe impl Sync for AgentRuntime {}
