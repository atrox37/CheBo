// ─── task/task_events.rs ──────────────────────────────────────────────────────
// Task System 对外广播的事件（通过 Tauri emit 发到前端）
// ──────────────────────────────────────────────────────────────────────────────

use tauri::{AppHandle, Emitter};

use super::task::{AgentTask, TaskStep};

/// 向前端发送任务创建事件
pub fn emit_task_created(app: &AppHandle, task: &AgentTask) {
    let _ = app.emit("task_created", serde_json::json!({
        "task_id": task.id,
        "title":   task.title,
        "goal":    task.goal,
        "status":  task.status,
    }));
}

/// 向前端发送任务整体状态更新
pub fn emit_task_updated(app: &AppHandle, task: &AgentTask) {
    let _ = app.emit("task_updated", serde_json::json!({
        "task_id":       task.id,
        "title":         task.title,
        "status":        task.status,
        "status_label":  task.status.label(),
        "current_step":  task.current_step,
        "total_steps":   task.steps.len(),
        "done_steps":    task.completed_steps(),
        "progress":      task.progress(),
        "error_message": task.error_message,
        "result_summary": task.result_summary,
    }));
}

/// 某个步骤开始执行
pub fn emit_step_started(app: &AppHandle, task_id: &str, step: &TaskStep) {
    let _ = app.emit("task_step_started", serde_json::json!({
        "task_id":    task_id,
        "step_id":    step.id,
        "step_index": step.step_index,
        "title":      step.title,
    }));
}

/// 某个步骤执行完毕
pub fn emit_step_finished(app: &AppHandle, task_id: &str, step: &TaskStep) {
    let _ = app.emit("task_step_finished", serde_json::json!({
        "task_id":    task_id,
        "step_id":    step.id,
        "step_index": step.step_index,
        "title":      step.title,
        "status":     step.status,
        "result":     step.result,
        "error":      step.error,
    }));
}

/// 步骤等待用户确认
pub fn emit_waiting_confirm(app: &AppHandle, task_id: &str, step: &TaskStep) {
    let _ = app.emit("task_waiting_confirm", serde_json::json!({
        "task_id":     task_id,
        "step_id":     step.id,
        "step_index":  step.step_index,
        "title":       step.title,
        "description": step.description,
        "tool_name":   step.tool_name,
        "tool_args":   step.tool_args,
    }));
}

/// 任务全部完成
pub fn emit_task_completed(app: &AppHandle, task: &AgentTask) {
    let _ = app.emit("task_completed", serde_json::json!({
        "task_id":        task.id,
        "title":          task.title,
        "result_summary": task.result_summary,
    }));
}

/// 任务失败
pub fn emit_task_failed(app: &AppHandle, task: &AgentTask) {
    let _ = app.emit("task_failed", serde_json::json!({
        "task_id":       task.id,
        "title":         task.title,
        "error_message": task.error_message,
    }));
}

/// Chebo 正在规划步骤（用于 UI 显示规划进度）
pub fn emit_task_planning(app: &AppHandle, task_id: &str, title: &str) {
    let _ = app.emit("task_planning", serde_json::json!({
        "task_id": task_id,
        "title":   title,
    }));
}

/// 步骤正在进行 LLM 分析（中间进度，用于 UI 实时跳动）
pub fn emit_step_thinking(app: &AppHandle, task_id: &str, step_index: usize, hint: &str) {
    let _ = app.emit("task_step_thinking", serde_json::json!({
        "task_id":    task_id,
        "step_index": step_index,
        "hint":       hint,
    }));
}

// ─── 气泡提示（发给 ChatBubble）────────────────────────────────────────────────

/// 任务开始时 Chebo 发一条旁白
pub fn emit_task_status_comment(app: &AppHandle, content: &str) {
    let _ = app.emit("status_comment", serde_json::json!({
        "content": content,
        "emotion": "working",
    }));
}
