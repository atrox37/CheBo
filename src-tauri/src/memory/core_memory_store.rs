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
    pub confidence: f64,
    pub source: String,
    pub updated_at: String,
    pub source_session_id: Option<String>,
    pub source_msg_id: Option<i64>,
    pub extracted_at: Option<String>,
    pub extraction_method: Option<String>,
    pub preference_type: Option<String>,  // 🆕 Ticket 08: 'explicit' | 'situational' | None(legacy)
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PersonaMemory {
    pub key: String,
    pub value: String,
    pub category: String,
    pub confidence: f64,
    pub updated_at: String,
    // 🆕 Ticket 05: provenance
    pub source_session_id: Option<String>,
    pub source_msg_id: Option<i64>,
    pub extracted_at: Option<String>,
    pub extraction_method: Option<String>,
}

// ─── user_profile CRUD ──────────────────────────────────────────────────────

pub async fn get_all_profile_entries(pool: &SqlitePool, limit: i64) -> Result<Vec<UserProfileEntry>> {
    get_user_profile_all(pool).await.map(|v| v.into_iter().take(limit as usize).collect())
}

pub async fn set_user_profile(pool: &SqlitePool, key: &str, value: &str) -> Result<()> {
    set_user_profile_with_source(pool, key, value, None, None).await
}

/// 🆕 Ticket 05: 带 provenance 的写入
pub async fn set_user_profile_with_source(
    pool: &SqlitePool,
    key: &str,
    value: &str,
    session_id: Option<&str>,
    msg_id: Option<i64>,
) -> Result<()> {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    sqlx::query(
        "INSERT INTO user_profile (key, value, source_session_id, source_msg_id, extracted_at, extraction_method)
         VALUES (?1, ?2, ?3, ?4, ?5, 'llm')
         ON CONFLICT(key) DO UPDATE SET value=?2, updated_at=?5,
             source_session_id=COALESCE(?3, source_session_id),
             source_msg_id=COALESCE(?4, source_msg_id),
             extracted_at=?5,
             extraction_method='llm'",
    )
    .bind(key)
    .bind(value)
    .bind(session_id)
    .bind(msg_id)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_user_profile_all(pool: &SqlitePool) -> Result<Vec<UserProfileEntry>> {
    let rows = sqlx::query(
        "SELECT key, value, confidence, source, updated_at,
                source_session_id, source_msg_id, extracted_at, extraction_method, preference_type
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
            source_session_id: r.get("source_session_id"),
            source_msg_id: r.get("source_msg_id"),
            extracted_at: r.get("extracted_at"),
            extraction_method: r.get("extraction_method"),
            preference_type: r.get("preference_type"),
        })
        .collect())
}

/// 🆕 Ticket 08: 获取显式偏好（注入 System Prompt）
pub async fn get_explicit_preferences(pool: &SqlitePool) -> Result<Vec<UserProfileEntry>> {
    let rows = sqlx::query(
        "SELECT key, value, confidence, source, updated_at,
                source_session_id, source_msg_id, extracted_at, extraction_method, preference_type
         FROM user_profile WHERE preference_type = 'explicit'
         ORDER BY updated_at DESC LIMIT 20",
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
            source_session_id: r.get("source_session_id"),
            source_msg_id: r.get("source_msg_id"),
            extracted_at: r.get("extracted_at"),
            extraction_method: r.get("extraction_method"),
            preference_type: Some("explicit".to_string()),
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

/// 🆕 Ticket 08: 标记偏好为显式（手动设置，置信度 1.0）
pub async fn set_explicit_preference(pool: &SqlitePool, key: &str, value: &str) -> Result<()> {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    sqlx::query(
        "INSERT INTO user_profile (key, value, confidence, source, preference_type, updated_at)
         VALUES (?1, ?2, 1.0, 'user', 'explicit', ?3)
         ON CONFLICT(key) DO UPDATE SET
             value = excluded.value,
             confidence = 1.0,
             source = 'user',
             preference_type = 'explicit',
             updated_at = excluded.updated_at"
    )
    .bind(key)
    .bind(value)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(())
}

// ─── persona_memory CRUD ────────────────────────────────────────────────────

pub async fn upsert_persona_memory(
    pool: &SqlitePool,
    key: &str,
    value: &str,
    category: &str,
    confidence: f64,
) -> Result<()> {
    upsert_persona_memory_with_source(pool, key, value, category, confidence, None, None).await
}

/// 🆕 Ticket 05: 带 provenance 的写入
pub async fn upsert_persona_memory_with_source(
    pool: &SqlitePool,
    key: &str,
    value: &str,
    category: &str,
    confidence: f64,
    session_id: Option<&str>,
    msg_id: Option<i64>,
) -> Result<()> {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    sqlx::query(
        "INSERT INTO persona_memory (key, value, category, confidence, updated_at,
                                     source_session_id, source_msg_id, extracted_at, extraction_method)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'llm')
         ON CONFLICT(key) DO UPDATE SET
             value      = excluded.value,
             category   = excluded.category,
             confidence = excluded.confidence,
             updated_at = excluded.updated_at,
             source_session_id = COALESCE(excluded.source_session_id, source_session_id),
             source_msg_id     = COALESCE(excluded.source_msg_id, source_msg_id),
             extracted_at      = excluded.extracted_at,
             extraction_method = 'llm'",
    )
    .bind(key)
    .bind(value)
    .bind(category)
    .bind(confidence)
    .bind(&now)
    .bind(session_id)
    .bind(msg_id)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_persona_memory_all(pool: &SqlitePool) -> Result<Vec<PersonaMemory>> {
    let rows = sqlx::query(
        "SELECT key, value, category, confidence, updated_at,
                source_session_id, source_msg_id, extracted_at, extraction_method
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
            source_session_id: r.get("source_session_id"),
            source_msg_id: r.get("source_msg_id"),
            extracted_at: r.get("extracted_at"),
            extraction_method: r.get("extraction_method"),
        })
        .collect())
}

pub async fn get_persona_memory_by_category(
    pool: &SqlitePool,
    category: &str,
    min_conf: f64,
) -> Result<Vec<PersonaMemory>> {
    let rows = sqlx::query(
        "SELECT key, value, category, confidence, updated_at,
                source_session_id, source_msg_id, extracted_at, extraction_method
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
            source_session_id: r.get("source_session_id"),
            source_msg_id: r.get("source_msg_id"),
            extracted_at: r.get("extracted_at"),
            extraction_method: r.get("extraction_method"),
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
