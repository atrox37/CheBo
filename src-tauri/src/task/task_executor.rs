// ─── task/task_executor.rs ────────────────────────────────────────────────────
// Task 执行循环
//
// 核心流程：
//   loop {
//     读取当前 step
//     if requires_confirm → emit 等待确认, 暂停
//     else → 调 ToolDispatcher 执行
//     更新步骤状态
//     current_step + 1
//   }
//   all done → 生成总结 → 写入 Memory
// ──────────────────────────────────────────────────────────────────────────────

use std::sync::Arc;
use anyhow::Result;
use tauri::AppHandle;

use crate::llm::LlmConfig;
use crate::tool_dispatcher::{PendingMap, ToolDispatcher};
use crate::tool_registry::ToolRegistry;

use super::task::{AgentTask, StepStatus, TaskStatus};
use super::task_events;
use super::task_planner;
use super::task_store::TaskStore;

pub struct TaskExecutor {
    pub store:         Arc<TaskStore>,
    pub registry:      Arc<ToolRegistry>,
    pub agent_pending: PendingMap,
    pub llm_cfg:       Arc<LlmConfig>,
    pub app:           AppHandle,
}

impl TaskExecutor {
    /// 主循环：从当前步骤开始，逐步执行，直到完成/暂停/取消
    pub async fn run_task(&self, task_id: String) {
        loop {
            // 每轮重新从 DB 读取（确保外部暂停/取消立即生效）
            let mut task = match self.store.get(&task_id).await {
                Ok(Some(t)) => t,
                _ => break,
            };

            // 检查是否应停止
            match task.status {
                TaskStatus::Paused
                | TaskStatus::Cancelled
                | TaskStatus::Completed
                | TaskStatus::Failed => break,
                _ => {}
            }

            // 获取当前步骤
            let step = match task.steps.get(task.current_step).cloned() {
                Some(s) => s,
                None => {
                    // 所有步骤完成
                    self.finish_task(&mut task).await;
                    break;
                }
            };

            // 跳过已完成/跳过的步骤
            if step.status == StepStatus::Success || step.status == StepStatus::Skipped {
                task.current_step += 1;
                task.touch();
                let _ = self.store.save(&task).await;
                continue;
            }

            // 执行步骤
            match self.execute_step(&mut task, step).await {
                Ok(should_pause) => {
                    if should_pause {
                        // 步骤进入 WaitingConfirm，暂停循环等待外部恢复
                        break;
                    }
                }
                Err(e) => {
                    let step_idx = task.current_step;
                    if let Some(step) = task.steps.get_mut(step_idx) {
                        step.status = StepStatus::Failed;
                        step.error  = Some(e.to_string());
                    }
                    task.status        = TaskStatus::Failed;
                    task.error_message = Some(e.to_string());
                    task.touch();
                    let _ = self.store.save(&task).await;
                    task_events::emit_task_failed(&self.app, &task);
                    break;
                }
            }
        }
    }

    /// 执行单个步骤
    /// 返回 Ok(true) 表示需要暂停（等待用户确认）
    async fn execute_step(
        &self,
        task:      &mut AgentTask,
        mut step:  super::task::TaskStep,
    ) -> Result<bool> {
        // 更新步骤状态为 Running
        step.status = StepStatus::Running;
        let step_idx = step.step_index;
        if let Some(s) = task.steps.get_mut(step_idx) { *s = step.clone(); }
        task.touch();
        self.store.save(task).await?;
        task_events::emit_step_started(&self.app, &task.id, &step);

        // 步骤需要确认
        if step.requires_confirm {
            step.status = StepStatus::WaitingConfirm;
            task.status = TaskStatus::WaitingConfirm;
            if let Some(s) = task.steps.get_mut(step_idx) { *s = step.clone(); }
            task.touch();
            self.store.save(task).await?;
            task_events::emit_waiting_confirm(&self.app, &task.id, &step);
            task_events::emit_task_updated(&self.app, task);
            return Ok(true); // 暂停，等待确认
        }

        // 执行工具
        let result_text = if let Some(ref tool_name) = step.tool_name {
            task_events::emit_step_thinking(
                &self.app, &task.id, step_idx,
                &format!("调用工具 `{tool_name}`：{}", step.title),
            );
            let args = step.tool_args.clone().unwrap_or(serde_json::json!({}));
            let call = crate::tool_trait::ToolCallRequest {
                id:        uuid::Uuid::new_v4().to_string(),
                name:      tool_name.clone(),
                arguments: args,
            };

            let results = ToolDispatcher::execute_all(
                &self.registry,
                vec![call],
                &self.agent_pending,
                Some(&self.app),
            ).await;

            results.into_iter()
                .map(|r| r.to_context_text())
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            // 无工具的纯 LLM 分析步骤 — 先通知前端"思考中"
            task_events::emit_step_thinking(
                &self.app, &task.id, step_idx,
                &format!("正在思考：{}", step.title),
            );
            self.analyze_step_with_llm(task, &step).await
        };

        // 更新步骤成功
        step.status = StepStatus::Success;
        step.result = Some(result_text);
        if let Some(s) = task.steps.get_mut(step_idx) { *s = step.clone(); }
        task.current_step += 1;
        task.status = TaskStatus::Running;
        task.touch();
        self.store.save(task).await?;
        task_events::emit_step_finished(&self.app, &task.id, &step);
        task_events::emit_task_updated(&self.app, task);

        Ok(false)
    }

    /// 没有工具调用的步骤 → 用 LLM 分析并返回描述
    async fn analyze_step_with_llm(
        &self,
        task: &AgentTask,
        step: &super::task::TaskStep,
    ) -> String {
        use crate::llm::LlmMessage;

        // 收集已完成步骤的结果作为上下文
        let context = task.steps
            .iter()
            .filter(|s| s.status == StepStatus::Success)
            .map(|s| format!("- {}: {}", s.title, s.result.as_deref().unwrap_or("")))
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = format!(
            "任务：{}\n目标：{}\n\n已完成步骤：\n{}\n\n当前步骤：{}\n{}\n\n请执行这个步骤并给出结果。",
            task.title, task.goal, context, step.title, step.description
        );

        let messages = vec![
            LlmMessage::system("你是 Chebo，正在逐步完成用户交给的任务，请执行当前步骤。"),
            LlmMessage::user(&prompt),
        ];

        match crate::llm::call_silent(messages, &self.llm_cfg).await {
            Ok((result, _)) => result,
            Err(e) => format!("LLM 分析失败：{e}"),
        }
    }

    /// 所有步骤完成 → 生成总结 → 更新状态
    async fn finish_task(&self, task: &mut AgentTask) {
        let step_results: Vec<(String, String)> = task.steps
            .iter()
            .map(|s| (
                s.title.clone(),
                s.result.clone().unwrap_or_else(|| "已完成".to_string()),
            ))
            .collect();

        let summary = task_planner::summarize_task(
            &task.title,
            &task.goal,
            &step_results,
            &self.llm_cfg,
        ).await;

        task.status         = TaskStatus::Completed;
        task.result_summary = Some(summary.clone());
        task.touch();

        let _ = self.store.save(task).await;
        task_events::emit_task_completed(&self.app, task);
        task_events::emit_task_status_comment(
            &self.app,
            &format!("任务「{}」完成啦！{}", task.title, summary),
        );

        log::info!("任务完成: {} — {}", task.id, task.title);
    }
}
