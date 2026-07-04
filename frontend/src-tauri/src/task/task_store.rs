// ─── task/task_store.rs ────────────────────────────────────────────────────────
// Task 持久化层（SQLite CRUD）
#![allow(dead_code)]
//
// 使用单表 agent_tasks，steps 序列化为 JSON 列（MVP 版本）
// 这样查询简单，支持重启后恢复
// ──────────────────────────────────────────────────────────────────────────────

use anyhow::{Context, Result};
use sqlx::SqlitePool;

use super::task::{AgentTask, TaskStatus};

pub struct TaskStore {
    pub pool: SqlitePool,
}

impl TaskStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // ── 保存（upsert） ──────────────────────────────────────────────────────

    pub async fn save(&self, task: &AgentTask) -> Result<()> {
        let status = serde_json::to_string(&task.status)?;
        let priority = serde_json::to_string(&task.priority)?;
        let steps_json = serde_json::to_string(&task.steps)?;

        sqlx::query(r#"
            INSERT INTO agent_tasks
              (id, title, goal, status, current_step, priority,
               created_at, updated_at, retry_count, max_retries,
               source_session_id, result_summary, error_message, steps_json)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
              title             = excluded.title,
              goal              = excluded.goal,
              status            = excluded.status,
              current_step      = excluded.current_step,
              priority          = excluded.priority,
              updated_at        = excluded.updated_at,
              retry_count       = excluded.retry_count,
              max_retries       = excluded.max_retries,
              source_session_id = excluded.source_session_id,
              result_summary    = excluded.result_summary,
              error_message     = excluded.error_message,
              steps_json        = excluded.steps_json
        "#)
        .bind(&task.id)
        .bind(&task.title)
        .bind(&task.goal)
        .bind(status.trim_matches('"'))
        .bind(task.current_step as i64)
        .bind(priority.trim_matches('"'))
        .bind(task.created_at)
        .bind(task.updated_at)
        .bind(task.retry_count as i64)
        .bind(task.max_retries as i64)
        .bind(&task.source_session_id)
        .bind(&task.result_summary)
        .bind(&task.error_message)
        .bind(steps_json)
        .execute(&self.pool)
        .await
        .context("save agent_task")?;

        Ok(())
    }

    // ── 查询单个 ───────────────────────────────────────────────────────────

    pub async fn get(&self, id: &str) -> Result<Option<AgentTask>> {
        let row = sqlx::query_as::<_, TaskRow>(
            "SELECT * FROM agent_tasks WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .context("get agent_task")?;

        Ok(row.map(|r| r.into_task()))
    }

    // ── 列出所有任务 ───────────────────────────────────────────────────────

    pub async fn list_all(&self) -> Result<Vec<AgentTask>> {
        let rows = sqlx::query_as::<_, TaskRow>(
            "SELECT * FROM agent_tasks ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await
        .context("list agent_tasks")?;

        Ok(rows.into_iter().map(|r| r.into_task()).collect())
    }

    // ── 列出活跃任务（未完成/未取消）─────────────────────────────────────

    pub async fn list_active(&self) -> Result<Vec<AgentTask>> {
        let rows = sqlx::query_as::<_, TaskRow>(
            "SELECT * FROM agent_tasks
             WHERE status NOT IN ('completed','cancelled','failed')
             ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await
        .context("list active agent_tasks")?;

        Ok(rows.into_iter().map(|r| r.into_task()).collect())
    }

    // ── 列出被中断的任务（应用重启后恢复） ─────────────────────────────────

    pub async fn list_interrupted(&self) -> Result<Vec<AgentTask>> {
        let rows = sqlx::query_as::<_, TaskRow>(
            "SELECT * FROM agent_tasks WHERE status = 'interrupted' ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await
        .context("list interrupted agent_tasks")?;

        Ok(rows.into_iter().map(|r| r.into_task()).collect())
    }

    // ── 更新状态（便捷函数）──────────────────────────────────────────────

    pub async fn update_status(&self, id: &str, status: &TaskStatus) -> Result<()> {
        let status_str = serde_json::to_string(status)?;
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            "UPDATE agent_tasks SET status = ?, updated_at = ? WHERE id = ?"
        )
        .bind(status_str.trim_matches('"'))
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await
        .context("update agent_task status")?;
        Ok(())
    }
}

// ─── SQLite 行映射 ─────────────────────────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct TaskRow {
    id:                String,
    title:             String,
    goal:              String,
    status:            String,
    current_step:      i64,
    priority:          String,
    created_at:        i64,
    updated_at:        i64,
    retry_count:       i64,
    max_retries:       i64,
    source_session_id: Option<String>,
    result_summary:    Option<String>,
    error_message:     Option<String>,
    steps_json:        String,
}

impl TaskRow {
    fn into_task(self) -> AgentTask {
        use super::task::{TaskPriority, TaskStatus};

        let status: TaskStatus = serde_json::from_str(&format!("\"{}\"", self.status))
            .unwrap_or(TaskStatus::Created);
        let priority: TaskPriority = serde_json::from_str(&format!("\"{}\"", self.priority))
            .unwrap_or(TaskPriority::Normal);
        let steps = serde_json::from_str(&self.steps_json).unwrap_or_default();

        AgentTask {
            id:                self.id,
            title:             self.title,
            goal:              self.goal,
            status,
            steps,
            current_step:      self.current_step as usize,
            created_at:        self.created_at,
            updated_at:        self.updated_at,
            retry_count:       self.retry_count as u32,
            max_retries:       self.max_retries as u32,
            priority,
            source_session_id: self.source_session_id,
            result_summary:    self.result_summary,
            error_message:     self.error_message,
        }
    }
}
