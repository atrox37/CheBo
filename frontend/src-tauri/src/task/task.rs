// ─── task/task.rs ──────────────────────────────────────────────────────────────
// Task System 核心数据结构
#![allow(dead_code)]
//
// Task  = 长期目标（可暂停/恢复/重试）
// Step  = 单个执行步骤（可携带工具调用）
//
// Task 和 Step 的状态刻意分开：
//   Task:Running + Step:WaitingConfirm 表示"任务还在跑，但这一步等确认"
// ──────────────────────────────────────────────────────────────────────────────

use serde::{Deserialize, Serialize};

// ─── 状态枚举 ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Created,        // 刚创建，尚未规划
    Planning,       // LLM 正在拆步
    WaitingConfirm, // 整体需要用户确认（如确认执行计划）
    Running,        // 正在逐步执行
    Paused,         // 用户暂停
    Interrupted,    // 被中断（如应用关闭）
    Failed,         // 失败（可重试）
    Completed,      // 全部完成
    Cancelled,      // 用户取消
}

impl TaskStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled | Self::Failed)
    }
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Running | Self::Planning | Self::WaitingConfirm)
    }
    pub fn label(&self) -> &str {
        match self {
            Self::Created        => "已创建",
            Self::Planning       => "规划中",
            Self::WaitingConfirm => "等待确认",
            Self::Running        => "执行中",
            Self::Paused         => "已暂停",
            Self::Interrupted    => "已中断",
            Self::Failed         => "失败",
            Self::Completed      => "已完成",
            Self::Cancelled      => "已取消",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Pending,        // 等待执行
    Running,        // 正在执行
    WaitingConfirm, // L2/L3 工具等待确认
    Success,        // 执行成功
    Failed,         // 执行失败
    Skipped,        // 跳过（用户跳过或条件不满足）
}

impl StepStatus {
    pub fn label(&self) -> &str {
        match self {
            Self::Pending        => "待执行",
            Self::Running        => "执行中",
            Self::WaitingConfirm => "等待确认",
            Self::Success        => "已完成",
            Self::Failed         => "失败",
            Self::Skipped        => "已跳过",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskPriority {
    Low,
    Normal,
    High,
}

impl Default for TaskPriority {
    fn default() -> Self { Self::Normal }
}

// ─── 核心结构 ─────────────────────────────────────────────────────────────────

/// 长期任务（可持久化到 SQLite）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTask {
    pub id:                String,
    pub title:             String,
    pub goal:              String,
    pub status:            TaskStatus,
    pub steps:             Vec<TaskStep>,
    pub current_step:      usize,

    pub created_at:        i64,
    pub updated_at:        i64,

    pub retry_count:       u32,
    pub max_retries:       u32,

    pub priority:          TaskPriority,

    /// 发起这个任务的对话 session_id（用于关联记忆）
    pub source_session_id: Option<String>,
    /// 任务完成后的总结（写入 Memory）
    pub result_summary:    Option<String>,
    pub error_message:     Option<String>,
}

impl AgentTask {
    pub fn new(
        goal:              String,
        session_id:        Option<String>,
    ) -> Self {
        let now = chrono::Utc::now().timestamp();
        // 从 goal 提取标题（最多20字）
        let title = if goal.chars().count() > 20 {
            format!("{}…", goal.chars().take(20).collect::<String>())
        } else {
            goal.clone()
        };

        Self {
            id:                uuid::Uuid::new_v4().to_string(),
            title,
            goal,
            status:            TaskStatus::Created,
            steps:             Vec::new(),
            current_step:      0,
            created_at:        now,
            updated_at:        now,
            retry_count:       0,
            max_retries:       3,
            priority:          TaskPriority::Normal,
            source_session_id: session_id,
            result_summary:    None,
            error_message:     None,
        }
    }

    /// 当前执行中的步骤（如有）
    pub fn current_step_ref(&self) -> Option<&TaskStep> {
        self.steps.get(self.current_step)
    }

    /// 已完成步骤数
    pub fn completed_steps(&self) -> usize {
        self.steps.iter().filter(|s| s.status == StepStatus::Success).count()
    }

    /// 进度 0.0–1.0
    pub fn progress(&self) -> f64 {
        if self.steps.is_empty() { return 0.0; }
        self.completed_steps() as f64 / self.steps.len() as f64
    }

    pub fn touch(&mut self) {
        self.updated_at = chrono::Utc::now().timestamp();
    }
}

/// 单个执行步骤
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStep {
    pub id:              String,
    pub task_id:         String,
    pub step_index:      usize,

    pub title:           String,
    pub description:     String,

    pub status:          StepStatus,

    /// 如果本步骤需要调用工具，指定工具名
    pub tool_name:       Option<String>,
    /// 工具参数
    pub tool_args:       Option<serde_json::Value>,

    /// 工具执行结果（或 LLM 分析结果）
    pub result:          Option<String>,
    pub error:           Option<String>,

    /// 是否在执行前需要用户确认（即使工具本身是 L0/L1）
    pub requires_confirm: bool,
    pub retry_count:     u32,
}

impl TaskStep {
    pub fn new(
        task_id:          &str,
        step_index:       usize,
        title:            String,
        description:      String,
        tool_name:        Option<String>,
        tool_args:        Option<serde_json::Value>,
        requires_confirm: bool,
    ) -> Self {
        Self {
            id:              uuid::Uuid::new_v4().to_string(),
            task_id:         task_id.to_string(),
            step_index,
            title,
            description,
            status:          StepStatus::Pending,
            tool_name,
            tool_args,
            result:          None,
            error:           None,
            requires_confirm,
            retry_count:     0,
        }
    }
}

// ─── 前端展示 DTO ─────────────────────────────────────────────────────────────

/// 发给前端的任务摘要（列表视图）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSummaryDto {
    pub id:            String,
    pub title:         String,
    pub status:        String,
    pub status_label:  String,
    pub total_steps:   usize,
    pub done_steps:    usize,
    pub progress:      f64,
    pub created_at:    i64,
    pub updated_at:    i64,
}

impl From<&AgentTask> for TaskSummaryDto {
    fn from(t: &AgentTask) -> Self {
        Self {
            id:           t.id.clone(),
            title:        t.title.clone(),
            status:       serde_json::to_string(&t.status).unwrap_or_default()
                            .trim_matches('"').to_string(),
            status_label: t.status.label().to_string(),
            total_steps:  t.steps.len(),
            done_steps:   t.completed_steps(),
            progress:     t.progress(),
            created_at:   t.created_at,
            updated_at:   t.updated_at,
        }
    }
}
