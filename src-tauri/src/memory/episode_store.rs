// ─── episode_store.rs ──────────────────────────────────────────────────────────
// EpisodeStore：拥有 messages 和 memory_summaries 两张表的所有 CRUD。
// 从 db.rs 迁移而来 —— Ticket 01, Commit 1。
// ─────────────────────────────────────────────────────────────────────────────

use anyhow::Result;
use sqlx::{Row, SqlitePool};

// ─── 数据结构 ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Message {
    pub id: i64,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub emotion: Option<String>,
    pub motion: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemorySummary {
    pub id: i64,
    pub session_id: String,
    pub msg_start_id: i64,
    pub msg_end_id: i64,
    pub summary: String,
    pub created_at: String,
}

// ─── messages 表 CRUD ────────────────────────────────────────────────────────

pub async fn save_message(
    pool: &SqlitePool,
    session_id: &str,
    role: &str,
    content: &str,
    emotion: Option<&str>,
    motion: Option<&str>,
) -> Result<i64> {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let result = sqlx::query(
        "INSERT INTO messages (session_id, role, content, emotion, motion, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
    )
    .bind(session_id)
    .bind(role)
    .bind(content)
    .bind(emotion)
    .bind(motion)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(result.last_insert_rowid())
}

pub async fn get_messages(
    pool: &SqlitePool,
    session_id: &str,
    limit: i64,
) -> Result<Vec<Message>> {
    let rows = sqlx::query_as::<_, (i64, String, String, String, Option<String>, Option<String>, String)>(
        "SELECT id, session_id, role, content, emotion, motion, created_at \
         FROM messages WHERE session_id = ?1 ORDER BY id DESC LIMIT ?2"
    )
    .bind(session_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .rev()
        .map(|(id, session_id, role, content, emotion, motion, created_at)| Message {
            id, session_id, role, content, emotion, motion, created_at,
        })
        .collect())
}

pub async fn get_all_messages(pool: &SqlitePool) -> Result<Vec<Message>> {
    let rows = sqlx::query_as::<_, (i64, String, String, String, Option<String>, Option<String>, String)>(
        "SELECT id, session_id, role, content, emotion, motion, created_at \
         FROM messages ORDER BY id"
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, session_id, role, content, emotion, motion, created_at)| Message {
            id, session_id, role, content, emotion, motion, created_at,
        })
        .collect())
}

pub async fn count_messages_after(
    pool: &SqlitePool,
    session_id: &str,
    after_id: i64,
) -> Result<i64> {
    let row = sqlx::query(
        "SELECT COUNT(*) as cnt FROM messages WHERE session_id = ?1 AND id > ?2"
    )
    .bind(session_id)
    .bind(after_id)
    .fetch_one(pool)
    .await?;
    Ok(row.get::<i64, _>("cnt"))
}

pub async fn get_messages_in_range(
    pool: &SqlitePool,
    session_id: &str,
    after_id: i64,
    end_id: i64,
) -> Result<Vec<Message>> {
    let rows = sqlx::query_as::<_, (i64, String, String, String, Option<String>, Option<String>, String)>(
        "SELECT id, session_id, role, content, emotion, motion, created_at \
         FROM messages WHERE session_id = ?1 AND id > ?2 AND id <= ?3 ORDER BY id"
    )
    .bind(session_id)
    .bind(after_id)
    .bind(end_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, session_id, role, content, emotion, motion, created_at)| Message {
            id, session_id, role, content, emotion, motion, created_at,
        })
        .collect())
}

pub async fn get_latest_message_id(pool: &SqlitePool, session_id: &str) -> Result<i64> {
    let row = sqlx::query(
        "SELECT COALESCE(MAX(id), 0) as max_id FROM messages WHERE session_id = ?1"
    )
    .bind(session_id)
    .fetch_one(pool)
    .await?;
    Ok(row.get::<i64, _>("max_id"))
}

pub async fn get_messages_after_chunk(
    pool: &SqlitePool,
    session_id: &str,
    last_processed_msg_id: i64,
    limit: i64,
) -> Result<Vec<Message>> {
    let rows = sqlx::query_as::<_, (i64, String, String, String, Option<String>, Option<String>, String)>(
        "SELECT id, session_id, role, content, emotion, motion, created_at \
         FROM messages WHERE session_id = ?1 AND id > ?2 ORDER BY id LIMIT ?3"
    )
    .bind(session_id)
    .bind(last_processed_msg_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, session_id, role, content, emotion, motion, created_at)| Message {
            id, session_id, role, content, emotion, motion, created_at,
        })
        .collect())
}

pub async fn get_all_session_ids(pool: &SqlitePool) -> Result<Vec<String>> {
    let rows = sqlx::query("SELECT DISTINCT session_id FROM messages ORDER BY session_id")
        .fetch_all(pool)
        .await?;
    Ok(rows.iter().map(|r| r.get::<String, _>("session_id")).collect())
}

// ─── memory_summaries 表 CRUD ────────────────────────────────────────────────

pub async fn save_summary(
    pool: &SqlitePool,
    session_id: &str,
    msg_start_id: i64,
    msg_end_id: i64,
    summary: &str,
) -> Result<i64> {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let result = sqlx::query(
        "INSERT INTO memory_summaries (session_id, msg_start_id, msg_end_id, summary, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5)"
    )
    .bind(session_id)
    .bind(msg_start_id)
    .bind(msg_end_id)
    .bind(summary)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(result.last_insert_rowid())
}

pub async fn get_summaries(pool: &SqlitePool, limit: i64) -> Result<Vec<MemorySummary>> {
    let rows = sqlx::query_as::<_, (i64, String, i64, i64, String, String)>(
        "SELECT id, session_id, msg_start_id, msg_end_id, summary, created_at \
         FROM memory_summaries ORDER BY id DESC LIMIT ?1"
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, session_id, msg_start_id, msg_end_id, summary, created_at)| {
            MemorySummary { id, session_id, msg_start_id, msg_end_id, summary, created_at }
        })
        .collect())
}

pub async fn get_last_summarized_msg_id(pool: &SqlitePool, session_id: &str) -> Result<i64> {
    let row = sqlx::query(
        "SELECT COALESCE(MAX(msg_end_id), 0) as last_id \
         FROM memory_summaries WHERE session_id = ?1"
    )
    .bind(session_id)
    .fetch_one(pool)
    .await?;
    Ok(row.get::<i64, _>("last_id"))
}
