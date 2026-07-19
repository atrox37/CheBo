// ─── task/task_manager.rs ─────────────────────────────────────────────────────
// Task 高层管理器（供 Tauri 命令调用）
//
// 职责：
//   create_task   → 规划 + 保存 + 启动执行循环
//   pause_task    → 改状态为 Paused
//   resume_task   → 改状态为 Running + 重启执行循环
//   cancel_task   → 改状态为 Cancelled
//   approve_step  → 确认步骤 + 重启执行循环
// ──────────────────────────────────────────────────────────────────────────────

use std::sync::Arc;
use std::collections::HashMap;
use anyhow::{bail, Result};
use tauri::AppHandle;
use tokio::sync::oneshot;

use crate::llm::LlmConfig;
use crate::tool_dispatcher::PendingMap;
use crate::tool_registry::ToolRegistry;

use super::task::{AgentTask, StepStatus, TaskStatus};
use super::task_events;
use super::task_executor::TaskExecutor;
use super::task_planner;
use super::task_store::TaskStore;

pub struct TaskManager {
    pub store:            Arc<TaskStore>,
    pub registry:         Arc<ToolRegistry>,
    pub agent_pending:    PendingMap,
    pub confirm_channels: Arc<std::sync::Mutex<HashMap<String, oneshot::Sender<bool>>>>,  // 🆕 Ticket 02
    pub llm_cfg:          Arc<LlmConfig>,
    pub app:              AppHandle,
}

impl TaskManager {
    pub fn new(
        store:            Arc<TaskStore>,
        registry:         Arc<ToolRegistry>,
        agent_pending:    PendingMap,
        confirm_channels: Arc<std::sync::Mutex<HashMap<String, oneshot::Sender<bool>>>>,  // 🆕 Ticket 02
        llm_cfg:          Arc<LlmConfig>,
        app:              AppHandle,
    ) -> Self {
        Self { store, registry, agent_pending, confirm_channels, llm_cfg, app }
    }

    fn executor(&self) -> TaskExecutor {
        TaskExecutor {
            store:            self.store.clone(),
            registry:         self.registry.clone(),
            agent_pending:    self.agent_pending.clone(),
            confirm_channels: self.confirm_channels.clone(),  // 🆕 Ticket 02
            llm_cfg:          self.llm_cfg.clone(),
            app:              self.app.clone(),
        }
    }

    // ── 创建任务（规划 + 保存 + 启动）─────────────────────────────────────

    pub async fn create_task(
        &self,
        goal:       String,
        session_id: Option<String>,
    ) -> Result<AgentTask> {
        // 创建任务对象（初始状态 Created）
        let mut task = AgentTask::new(goal.clone(), session_id);
        task.status = TaskStatus::Planning;
        self.store.save(&task).await?;
        task_events::emit_task_created(&self.app, &task);
        task_events::emit_task_planning(&self.app, &task.id, &task.title);
        task_events::emit_task_status_comment(
            &self.app,
            &format!("好的，我来规划一下「{}」这个任务…", task.title),
        );

        // LLM 拆步骤
        match task_planner::plan_task(&task.id, &goal, &self.llm_cfg).await {
            Ok((title, steps)) => {
                if !title.is_empty() { task.title = title; }
                task.steps  = steps;
                task.status = TaskStatus::Running;
                task.touch();
                self.store.save(&task).await?;
                task_events::emit_task_updated(&self.app, &task);

                // 后台启动执行循环
                let executor    = self.executor();
                let task_id_bg  = task.id.clone();
                tauri::async_runtime::spawn(async move {
                    executor.run_task(task_id_bg).await;
                });
            }
            Err(e) => {
                task.status        = TaskStatus::Failed;
                task.error_message = Some(format!("规划失败：{e}"));
                task.touch();
                self.store.save(&task).await?;
                task_events::emit_task_failed(&self.app, &task);
                bail!("规划失败：{e}");
            }
        }

        Ok(task)
    }

    // ── 暂停任务 ───────────────────────────────────────────────────────────

    pub async fn pause_task(&self, task_id: &str) -> Result<()> {
        let mut task = self.require_task(task_id).await?;
        if task.status.is_terminal() {
            bail!("任务已终止，无法暂停");
        }
        task.status = TaskStatus::Paused;
        task.touch();
        self.store.save(&task).await?;
        task_events::emit_task_updated(&self.app, &task);
        task_events::emit_task_status_comment(&self.app, "任务已暂停，需要的时候叫我继续哦。");
        Ok(())
    }

    // ── 恢复任务 ───────────────────────────────────────────────────────────

    pub async fn resume_task(&self, task_id: &str) -> Result<()> {
        let mut task = self.require_task(task_id).await?;
        if task.status != TaskStatus::Paused && task.status != TaskStatus::Interrupted {
            bail!("任务当前状态无法恢复：{}", task.status.label());
        }
        task.status = TaskStatus::Running;
        task.touch();
        self.store.save(&task).await?;
        task_events::emit_task_updated(&self.app, &task);
        task_events::emit_task_status_comment(
            &self.app,
            &format!("好的，继续执行「{}」！", task.title),
        );

        // 重启执行循环
        let executor   = self.executor();
        let task_id_bg = task_id.to_string();
        tauri::async_runtime::spawn(async move {
            executor.run_task(task_id_bg).await;
        });

        Ok(())
    }

    // ── 取消任务 ───────────────────────────────────────────────────────────

    pub async fn cancel_task(&self, task_id: &str) -> Result<()> {
        let mut task = self.require_task(task_id).await?;
        if task.status.is_terminal() {
            bail!("任务已终止");
        }
        task.status = TaskStatus::Cancelled;
        task.touch();
        self.store.save(&task).await?;
        task_events::emit_task_updated(&self.app, &task);
        Ok(())
    }

    // ── 确认步骤（WaitingConfirm → 继续执行）─────────────────────────────

    pub async fn approve_step(
        &self,
        task_id: &str,
        step_id: &str,
        approved: bool,
    ) -> Result<()> {
        let mut task = self.require_task(task_id).await?;

        // 找到对应步骤并更新状态
        let step_idx = task.steps
            .iter()
            .position(|s| s.id == step_id)
            .ok_or_else(|| anyhow::anyhow!("步骤不存在: {step_id}"))?;

        if approved {
            // 用户同意 → 清除 requires_confirm 标记，让执行器正常运行
            task.steps[step_idx].requires_confirm = false;
            task.steps[step_idx].status = StepStatus::Pending;
            task.status = TaskStatus::Running;
            task.touch();
            self.store.save(&task).await?;
            task_events::emit_task_updated(&self.app, &task);

            // 重启执行循环
            let executor   = self.executor();
            let task_id_bg = task_id.to_string();
            tauri::async_runtime::spawn(async move {
                executor.run_task(task_id_bg).await;
            });
        } else {
            // 用户拒绝 → 跳过这个步骤
            task.steps[step_idx].status  = StepStatus::Skipped;
            task.steps[step_idx].error   = Some("用户跳过".to_string());
            task.current_step            = step_idx + 1;
            task.status                  = TaskStatus::Running;
            task.touch();
            self.store.save(&task).await?;
            task_events::emit_task_updated(&self.app, &task);

            // 继续执行剩余步骤
            let executor   = self.executor();
            let task_id_bg = task_id.to_string();
            tauri::async_runtime::spawn(async move {
                executor.run_task(task_id_bg).await;
            });
        }

        Ok(())
    }

    // ── 重试失败任务 ───────────────────────────────────────────────────────

    pub async fn retry_task(&self, task_id: &str) -> Result<()> {
        let mut task = self.require_task(task_id).await?;
        if task.status != TaskStatus::Failed {
            bail!("只有失败的任务才能重试");
        }
        if task.retry_count >= task.max_retries {
            bail!("已达最大重试次数（{}）", task.max_retries);
        }

        task.retry_count   += 1;
        task.status        = TaskStatus::Running;
        task.error_message = None;
        // 将失败的步骤重置为 Pending
        for step in task.steps.iter_mut() {
            if step.status == StepStatus::Failed {
                step.status      = StepStatus::Pending;
                step.error       = None;
                step.retry_count += 1;
            }
        }
        task.touch();
        self.store.save(&task).await?;
        task_events::emit_task_updated(&self.app, &task);

        let executor   = self.executor();
        let task_id_bg = task_id.to_string();
        tauri::async_runtime::spawn(async move {
            executor.run_task(task_id_bg).await;
        });

        Ok(())
    }

    // ── 应用启动时恢复被中断的任务 ────────────────────────────────────────

    pub async fn resume_interrupted_on_startup(&self) {
        match self.store.list_interrupted().await {
            Ok(tasks) => {
                for mut task in tasks {
                    log::info!("恢复中断任务: {} ({})", task.title, task.id);
                    task.status = TaskStatus::Running;
                    task.touch();
                    let _ = self.store.save(&task).await;
                    task_events::emit_task_updated(&self.app, &task);
                    task_events::emit_task_status_comment(
                        &self.app,
                        &format!("应用重启了，我来继续上次的任务「{}」。", task.title),
                    );

                    let executor   = self.executor();
                    let task_id_bg = task.id.clone();
                    tauri::async_runtime::spawn(async move {
                        executor.run_task(task_id_bg).await;
                    });
                }
            }
            Err(e) => log::warn!("恢复中断任务失败: {e}"),
        }
    }

    // ── 工具函数 ───────────────────────────────────────────────────────────

    async fn require_task(&self, task_id: &str) -> Result<AgentTask> {
        self.store.get(task_id).await?
            .ok_or_else(|| anyhow::anyhow!("任务不存在: {task_id}"))
    }
}
