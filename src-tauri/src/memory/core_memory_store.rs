// ─── core_memory_store.rs ──────────────────────────────────────────────────────
// CoreMemoryStore：拥有 user_profile 和 persona_memory 两张表的所有 CRUD。
// 从 db.rs 迁移而来 —— Ticket 01, Commit 2。
// ─────────────────────────────────────────────────────────────────────────────

use anyhow::Result;
use sqlx::{Row, SqlitePool};

// ─── 数据结构 ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UserProfileEntry {
    pub key: String,
    pub value: String,
    /// 置信度 0.0–1.0（1.0=用户明确确认，0.5=LLM推断，0.3=临时性陈述）
    pub confidence: f64,
    /// 来源：'auto'=自动提取 | 'user'=用户手动设置 | 'inferred'=LLM推断
    pub source: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PersonaMemory {
    pub key: String,
    pub value: String,
    pub category: String,
    pub confidence: f64,
    pub updated_at: String,
}

// ─── user_profile CRUD ──────────────────────────────────────────────────────

pub async fn get_all_profile_entries(pool: &SqlitePool, limit: i64) -> Result<Vec<UserProfileEntry>> {
    get_user_profile_all(pool).await.map(|v| v.into_iter().take(limit as usize).collect())
}

pub async fn set_user_profile(pool: &SqlitePool, key: &str, value: &str) -> Result<()> {
    sqlx::query(
        "INSERT INTO user_profile (key, value)
         VALUES (?, ?)
         ON CONFLICT(key) DO UPDATE SET value=?, updated_at=datetime('now','localtime')",
    )
    .bind(key)
    .bind(value)
    .bind(value)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_user_profile_all(pool: &SqlitePool) -> Result<Vec<UserProfileEntry>> {
    let rows = sqlx::query(
        "SELECT key, value, confidence, source, updated_at
         FROM user_profile ORDER BY updated_at DESC",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|r| UserProfileEntry {
            key: r.get("key"),
            value: r.get("value"),
            confidence: r.get::<f64, _>("confidence"),
            source: r.get("source"),
            updated_at: r.get("updated_at"),
        })
        .collect())
}

pub async fn delete_user_profile_entry(pool: &SqlitePool, key: &str) -> Result<()> {
    sqlx::query("DELETE FROM user_profile WHERE key = ?")
        .bind(key)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_user_profile_entry(pool: &SqlitePool, key: &str, value: &str) -> Result<()> {
    sqlx::query(
        "INSERT INTO user_profile (key, value, confidence, source, updated_at)
         VALUES (?, ?, 1.0, 'user', datetime('now','localtime'))
         ON CONFLICT(key) DO UPDATE SET
           value = excluded.value,
           confidence = 1.0,
           source = 'user',
           updated_at = excluded.updated_at"
    )
    .bind(key)
    .bind(value)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn search_memories_by_keyword(
    pool: &SqlitePool,
    keyword: &str,
    limit: i64,
) -> Result<Vec<String>> {
    let pattern = format!("%{keyword}%");
    let rows = sqlx::query(
        "SELECT value FROM user_profile
         WHERE value LIKE ? OR key LIKE ?
         ORDER BY updated_at DESC LIMIT ?",
    )
    .bind(&pattern)
    .bind(&pattern)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows.iter().map(|r| r.get::<String, _>("value")).collect())
}

// ─── persona_memory CRUD ────────────────────────────────────────────────────

pub async fn upsert_persona_memory(
    pool: &SqlitePool,
    key: &str,
    value: &str,
    category: &str,
    confidence: f64,
) -> Result<()> {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    sqlx::query(
        "INSERT INTO persona_memory (key, value, category, confidence, updated_at)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(key) DO UPDATE SET
             value      = excluded.value,
             category   = excluded.category,
             confidence = excluded.confidence,
             updated_at = excluded.updated_at",
    )
    .bind(key)
    .bind(value)
    .bind(category)
    .bind(confidence)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_persona_memory_all(pool: &SqlitePool) -> Result<Vec<PersonaMemory>> {
    let rows = sqlx::query(
        "SELECT key, value, category, confidence, updated_at
         FROM persona_memory
         ORDER BY confidence DESC",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|r| PersonaMemory {
            key: r.get("key"),
            value: r.get("value"),
            category: r.get("category"),
            confidence: r.get("confidence"),
            updated_at: r.get("updated_at"),
        })
        .collect())
}

pub async fn get_persona_memory_by_category(
    pool: &SqlitePool,
    category: &str,
    min_conf: f64,
) -> Result<Vec<PersonaMemory>> {
    let rows = sqlx::query(
        "SELECT key, value, category, confidence, updated_at
         FROM persona_memory
         WHERE category = ? AND confidence >= ?
         ORDER BY confidence DESC",
    )
    .bind(category)
    .bind(min_conf)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|r| PersonaMemory {
            key: r.get("key"),
            value: r.get("value"),
            category: r.get("category"),
            confidence: r.get("confidence"),
            updated_at: r.get("updated_at"),
        })
        .collect())
}

pub async fn decay_persona_confidence(
    pool: &SqlitePool,
    key: &str,
    decay_rate: f64,
) -> Result<()> {
    sqlx::query(
        "UPDATE persona_memory
         SET confidence = MAX(0.0, confidence - ?),
             updated_at = datetime('now','localtime')
         WHERE key = ?",
    )
    .bind(decay_rate)
    .bind(key)
    .execute(pool)
    .await?;
    Ok(())
}
