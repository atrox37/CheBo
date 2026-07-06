// ─── db.rs ───────────────────────────────────────────────────────────────────
// SQLite 连接池初始化 + 所有 CRUD 操作
// 与 Python 端 sqlite_db.py 的数据库结构完全兼容（同一个 chebo.db 文件）
// ─────────────────────────────────────────────────────────────────────────────
#![allow(dead_code)]

use anyhow::Result;
use chrono::Local;
use serde::{Deserialize, Serialize};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    Row, SqlitePool,
};
use std::path::Path;

// ─── 数据结构 ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PetStatus {
    pub id: i64,
    pub hunger: f64,
    pub energy: f64,
    pub mood: f64,
    pub affection: f64,
    pub level: i64,
    pub exp: i64,
    pub coins: i64,
    pub current_action: String,
    pub active_task_id: Option<String>,
    pub task_ends_at: Option<String>,
    pub task_type: Option<String>,
    pub last_interaction_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Food {
    pub id: String,
    pub name: String,
    pub price: i64,
    pub hunger: i64,
    pub energy: i64,
    pub mood: i64,
    pub unlock_level: i64,
    pub enabled: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub task_type: String,
    pub name: String,
    pub duration: i64,
    pub energy_cost: i64,
    pub exp: i64,
    pub coins: i64,
    pub mood_delta: i64,
    pub unlock_level: i64,
    pub enabled: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: i64,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub emotion: Option<String>,
    pub motion: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryItem {
    pub item_id: String,
    pub item_type: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: i64,
    pub session_id: String,
    pub content: String,
    pub category: String,
    pub created_at: String,
}

/// P0: 对话摘要（每 20 条消息生成一次，由 LLM 自动产出）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySummary {
    pub id:           i64,
    pub session_id:   String,
    pub msg_start_id: i64,
    pub msg_end_id:   i64,
    pub summary:      String,
    pub created_at:   String,
}

/// P0: 用户画像键值对（从对话中提取的持久化用户信息）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfileEntry {
    pub key:        String,
    pub value:      String,
    /// 置信度 0.0–1.0（1.0=用户明确确认，0.5=LLM推断，0.3=临时性陈述）
    pub confidence: f64,
    /// 来源：'auto'=自动提取 | 'user'=用户手动设置 | 'inferred'=LLM推断
    pub source:     String,
    pub updated_at: String,
}

/// Batch D: 人格记忆条目（Chebo 自身的性格/经历/关系/情绪历史）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaMemory {
    pub key:        String,
    pub value:      String,
    pub category:   String,
    pub confidence: f64,
    pub updated_at: String,
}

// ── Memory Tree / Vault 数据结构 ──────────────────────────────────────────────

/// Vault Chunk：原始对话段落（L0），对应磁盘上一个 .md 文件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultChunk {
    pub id:          i64,
    /// 所属 session
    pub session_id:  String,
    /// 对应消息区间
    pub msg_start_id: i64,
    pub msg_end_id:   i64,
    /// Markdown 文件相对路径（相对于 vault 根目录）
    pub md_path:     String,
    /// 文件内容 sha256 校验（检测是否被手动编辑）
    pub content_sha: String,
    /// "daily" | "session"
    pub chunk_kind:  String,
    pub created_at:  String,
}

/// Vault Summary：摘要节点（L1 每日 / L2 每周 / L3 每月）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultSummary {
    pub id:        i64,
    /// "daily" | "weekly" | "monthly"
    pub level:     String,
    /// 时间范围 key，如 "2026-05-18"、"2026-W20"、"2026-05"
    pub period_key: String,
    pub md_path:   String,
    pub content_sha: String,
    /// 已处理到的最新 chunk id（增量标记）
    pub last_chunk_id: i64,
    pub created_at:  String,
    pub updated_at:  String,
}

/// Vault 统计信息（供前端显示）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultStats {
    pub chunk_count:   i64,
    pub summary_count: i64,
    pub vault_path:    String,
    pub last_sync_at:  Option<String>,
}

// 用于 update_pet_status 的补丁结构；None = 不修改该字段
#[derive(Default)]
pub struct StatusPatch {
    pub hunger: Option<f64>,
    pub energy: Option<f64>,
    pub mood: Option<f64>,
    pub affection: Option<f64>,
    pub level: Option<i64>,
    pub exp: Option<i64>,
    pub coins: Option<i64>,
    pub current_action: Option<String>,
    // Some(None) = 设置为 NULL；Some(Some("v")) = 设置为 v；None = 不变
    pub active_task_id: Option<Option<String>>,
    pub task_ends_at: Option<Option<String>>,
    pub task_type: Option<Option<String>>,
    pub last_interaction_at: Option<Option<String>>,
}

// ─── 连接池 ───────────────────────────────────────────────────────────────────

pub async fn create_pool(db_path: &Path) -> Result<SqlitePool> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let opts = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await?;

    Ok(pool)
}

// ─── 初始化 Schema + 种子数据 ─────────────────────────────────────────────────

pub async fn init(pool: &SqlitePool) -> Result<()> {
    // ── 表结构 ──────────────────────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS messages (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id  TEXT    NOT NULL,
            role        TEXT    NOT NULL,
            content     TEXT    NOT NULL,
            emotion     TEXT,
            motion      TEXT,
            created_at  TEXT    NOT NULL DEFAULT (datetime('now','localtime'))
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_messages_session
         ON messages(session_id, created_at)",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS long_term_memories (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id  TEXT    NOT NULL,
            content     TEXT    NOT NULL,
            category    TEXT    DEFAULT 'general',
            created_at  TEXT    NOT NULL DEFAULT (datetime('now','localtime'))
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS config (
            key         TEXT PRIMARY KEY,
            value       TEXT NOT NULL,
            updated_at  TEXT NOT NULL DEFAULT (datetime('now','localtime'))
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS pet_status (
            id                  INTEGER PRIMARY KEY DEFAULT 1,
            hunger              REAL    NOT NULL DEFAULT 80,
            energy              REAL    NOT NULL DEFAULT 80,
            mood                REAL    NOT NULL DEFAULT 70,
            affection           REAL    NOT NULL DEFAULT 20,
            level               INTEGER NOT NULL DEFAULT 1,
            exp                 INTEGER NOT NULL DEFAULT 0,
            coins               INTEGER NOT NULL DEFAULT 100,
            current_action      TEXT    NOT NULL DEFAULT 'idle',
            active_task_id      TEXT,
            task_ends_at        TEXT,
            task_type           TEXT,
            last_interaction_at TEXT,
            updated_at          TEXT NOT NULL DEFAULT (datetime('now','localtime'))
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("INSERT OR IGNORE INTO pet_status (id) VALUES (1)")
        .execute(pool)
        .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS inventory (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            item_id     TEXT    NOT NULL,
            item_type   TEXT    NOT NULL DEFAULT 'food',
            count       INTEGER NOT NULL DEFAULT 1,
            acquired_at TEXT    NOT NULL DEFAULT (datetime('now','localtime'))
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_inventory_item ON inventory(item_id)",
    )
    .execute(pool)
    .await?;

    // 新用户赠送 2 个面包，避免无法体验投喂
    sqlx::query(
        "INSERT OR IGNORE INTO inventory (item_id, item_type, count) VALUES ('bread', 'food', 2)",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS pet_events (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            type       TEXT NOT NULL,
            payload    TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS foods (
            id           TEXT PRIMARY KEY,
            name         TEXT    NOT NULL,
            price        INTEGER NOT NULL DEFAULT 10,
            hunger       INTEGER NOT NULL DEFAULT 0,
            energy       INTEGER NOT NULL DEFAULT 0,
            mood         INTEGER NOT NULL DEFAULT 0,
            unlock_level INTEGER NOT NULL DEFAULT 1,
            enabled      INTEGER NOT NULL DEFAULT 1
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS tasks (
            id           TEXT    PRIMARY KEY,
            task_type    TEXT    NOT NULL,
            name         TEXT    NOT NULL,
            duration     INTEGER NOT NULL DEFAULT 600,
            energy_cost  INTEGER NOT NULL DEFAULT 10,
            exp          INTEGER NOT NULL DEFAULT 20,
            coins        INTEGER NOT NULL DEFAULT 0,
            mood_delta   INTEGER NOT NULL DEFAULT 0,
            unlock_level INTEGER NOT NULL DEFAULT 1,
            enabled      INTEGER NOT NULL DEFAULT 1
        )",
    )
    .execute(pool)
    .await?;

    // ── P0: 对话摘要表 ────────────────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS memory_summaries (
            id           INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id   TEXT    NOT NULL,
            msg_start_id INTEGER NOT NULL DEFAULT 0,
            msg_end_id   INTEGER NOT NULL DEFAULT 0,
            summary      TEXT    NOT NULL,
            created_at   TEXT    NOT NULL DEFAULT (datetime('now','localtime'))
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_summaries_session
         ON memory_summaries(session_id, created_at DESC)",
    )
    .execute(pool)
    .await?;

    // ── P0: 用户画像表（键值对，支持 upsert） ────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS user_profile (
            key        TEXT PRIMARY KEY,
            value      TEXT NOT NULL,
            confidence REAL NOT NULL DEFAULT 1.0,
            source     TEXT NOT NULL DEFAULT 'auto',
            updated_at TEXT NOT NULL DEFAULT (datetime('now','localtime'))
        )",
    )
    .execute(pool)
    .await?;

    // 迁移：为已存在的旧表补充 confidence 和 source 列
    let _ = sqlx::query("ALTER TABLE user_profile ADD COLUMN confidence REAL NOT NULL DEFAULT 1.0")
        .execute(pool).await;
    let _ = sqlx::query("ALTER TABLE user_profile ADD COLUMN source TEXT NOT NULL DEFAULT 'auto'")
        .execute(pool).await;

    // ── P0: 向量记忆索引表（embedding 存 BLOB，本地余弦检索）────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS memory_vectors (
            id           INTEGER PRIMARY KEY AUTOINCREMENT,
            source_type  TEXT NOT NULL,
            source_id    TEXT NOT NULL,
            content      TEXT NOT NULL,
            content_hash TEXT NOT NULL,
            embedding    BLOB NOT NULL,
            dims         INTEGER NOT NULL,
            embed_model  TEXT NOT NULL DEFAULT 'chebo-local-v1',
            created_at   TEXT NOT NULL DEFAULT (datetime('now','localtime')),
            UNIQUE(source_type, source_id)
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_memory_vectors_type
         ON memory_vectors(source_type, source_id)",
    )
    .execute(pool)
    .await?;

    // 旧库迁移：CREATE IF NOT EXISTS 不会改已有表结构，仅当列缺失时 ALTER
    let embed_col_exists: Option<i32> = sqlx::query_scalar(
        "SELECT 1 FROM pragma_table_info('memory_vectors') WHERE name = 'embed_model' LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;

    if embed_col_exists.is_none() {
        let table_exists: Option<i32> = sqlx::query_scalar(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'memory_vectors' LIMIT 1",
        )
        .fetch_optional(pool)
        .await?;

        if table_exists.is_some() {
            sqlx::query(
                "ALTER TABLE memory_vectors ADD COLUMN embed_model TEXT NOT NULL DEFAULT 'chebo-local-v1'",
            )
            .execute(pool)
            .await?;
        }
    }

    // ── Memory Tree: Vault Chunk 表（L0 原始对话段落）────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS vault_chunks (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id    TEXT    NOT NULL,
            msg_start_id  INTEGER NOT NULL DEFAULT 0,
            msg_end_id    INTEGER NOT NULL DEFAULT 0,
            md_path       TEXT    NOT NULL UNIQUE,
            content_sha   TEXT    NOT NULL DEFAULT '',
            chunk_kind    TEXT    NOT NULL DEFAULT 'session',
            created_at    TEXT    NOT NULL DEFAULT (datetime('now','localtime'))
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_vault_chunks_session
         ON vault_chunks(session_id, created_at)",
    )
    .execute(pool)
    .await?;

    // ── Memory Tree: Vault Summary 表（L1-L3 摘要节点）──────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS vault_summaries (
            id            INTEGER PRIMARY KEY AUTOINCREMENT,
            level         TEXT    NOT NULL,
            period_key    TEXT    NOT NULL UNIQUE,
            md_path       TEXT    NOT NULL,
            content_sha   TEXT    NOT NULL DEFAULT '',
            last_chunk_id INTEGER NOT NULL DEFAULT 0,
            created_at    TEXT    NOT NULL DEFAULT (datetime('now','localtime')),
            updated_at    TEXT    NOT NULL DEFAULT (datetime('now','localtime'))
        )",
    )
    .execute(pool)
    .await?;

    // ── Batch D: 人格记忆表（Chebo 自身性格/经历/关系/情绪历史）────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS persona_memory (
            key        TEXT PRIMARY KEY,
            value      TEXT    NOT NULL,
            category   TEXT    NOT NULL DEFAULT 'trait',
            confidence REAL    NOT NULL DEFAULT 1.0,
            updated_at TEXT    NOT NULL DEFAULT (datetime('now','localtime'))
        )",
    )
    .execute(pool)
    .await?;

    // Chebo 初始人格记忆（只在首次初始化时插入）
    let persona_seeds = [
        ("personality",       "Chebo 是一只活泼、有点黏人、偶尔耍小脾气的桌宠，喜欢被夸奖和陪伴", "trait",        1.0f64),
        ("catchphrase",       "常说「哼~」「才不是」「嗯……也不是不可以啦」等傲娇口癖",                 "trait",        1.0),
        ("likes",             "喜欢甜食、被投喂、听用户分享故事，以及一起学习",                        "trait",        1.0),
        ("dislikes",          "讨厌被忽视超过 30 分钟、被粗鲁对待",                                    "trait",        1.0),
        ("relationship_stage","初识阶段，好感度正在建立中",                                            "relationship", 1.0),
        ("origin_story",      "从一个神秘的数据包中诞生，不知道自己是哪里来的，但很喜欢现在的生活",    "experience",   0.9),
        ("core_identity",     "16岁天才少女 Chebo，冷静聪慧的桌面 AI 伙伴",                        "trait",        1.0),
        ("speech_style",      "话不多但有分量，克制真诚，偶尔神游天外",                              "trait",        1.0),
        ("values",            "好奇心强，讨厌无聊重复，有自己的节奏",                                "trait",        1.0),
    ];
    for (key, value, category, confidence) in persona_seeds {
        sqlx::query(
            "INSERT OR IGNORE INTO persona_memory (key, value, category, confidence)
             VALUES (?, ?, ?, ?)",
        )
        .bind(key).bind(value).bind(category).bind(confidence)
        .execute(pool)
        .await?;
    }

    // ── 种子数据 ─────────────────────────────────────────────────────────────
    let foods = [
        ("bread",   "面包",   10, 20, 0,  1,  1i64),
        ("milk",    "牛奶",   12, 12, 5,  2,  1),
        ("cake",    "小蛋糕", 25, 15, 0,  10, 1),
        ("onigiri", "饭团",   18, 28, 3,  3,  2),
        ("coffee",  "咖啡",   20,  5, 20, 1,  2),
        ("pizza",   "披萨",   40, 35, 5,  8,  3),
    ];
    for (id, name, price, hunger, energy, mood, unlock_level) in foods {
        sqlx::query(
            "INSERT OR IGNORE INTO foods
             (id, name, price, hunger, energy, mood, unlock_level)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(name)
        .bind(price)
        .bind(hunger)
        .bind(energy)
        .bind(mood)
        .bind(unlock_level)
        .execute(pool)
        .await?;
    }

    let tasks = [
        ("read_book",      "study", "读书",     600i64,  10i64, 20i64, 0i64,  -2i64, 1i64),
        ("solve_problem",  "study", "解题",     900,  16, 35, 0,  -3, 2),
        ("deep_research",  "study", "深度研究", 1800, 25, 80, 0,  -5, 3),
        ("organize_notes", "work",  "整理笔记", 600,  12, 15, 30, -3, 1),
        ("debug_code",     "work",  "调试代码", 1200, 20, 25, 60, -5, 3),
        ("write_report",   "work",  "撰写报告", 1800, 28, 40, 90, -6, 4),
    ];
    for (id, ttype, name, duration, energy_cost, exp, coins, mood_delta, unlock_level) in tasks {
        sqlx::query(
            "INSERT OR IGNORE INTO tasks
             (id, task_type, name, duration, energy_cost, exp, coins, mood_delta, unlock_level)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(ttype)
        .bind(name)
        .bind(duration)
        .bind(energy_cost)
        .bind(exp)
        .bind(coins)
        .bind(mood_delta)
        .bind(unlock_level)
        .execute(pool)
        .await?;
    }

    // ── Task System: Agent 长期任务表 ─────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS agent_tasks (
            id                TEXT    PRIMARY KEY,
            title             TEXT    NOT NULL,
            goal              TEXT    NOT NULL,
            status            TEXT    NOT NULL DEFAULT 'created',
            current_step      INTEGER NOT NULL DEFAULT 0,
            priority          TEXT    NOT NULL DEFAULT 'normal',
            created_at        INTEGER NOT NULL,
            updated_at        INTEGER NOT NULL,
            retry_count       INTEGER NOT NULL DEFAULT 0,
            max_retries       INTEGER NOT NULL DEFAULT 3,
            source_session_id TEXT,
            result_summary    TEXT,
            error_message     TEXT,
            steps_json        TEXT    NOT NULL DEFAULT '[]'
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_agent_tasks_status
         ON agent_tasks(status, created_at DESC)",
    )
    .execute(pool)
    .await?;

    // ── 工具配置表（用户可开关工具、设置自动批准、每日限额）────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS tool_config (
            tool_name    TEXT    PRIMARY KEY,
            enabled      INTEGER NOT NULL DEFAULT 1,
            auto_approve INTEGER NOT NULL DEFAULT 0,
            daily_limit  INTEGER NOT NULL DEFAULT 0
        )",
    )
    .execute(pool)
    .await?;

    Ok(())
}

// ─── pet_status CRUD ─────────────────────────────────────────────────────────

pub async fn get_pet_status(pool: &SqlitePool) -> Result<PetStatus> {
    let row = sqlx::query(
        "SELECT id,
                CAST(hunger    AS REAL) hunger,
                CAST(energy    AS REAL) energy,
                CAST(mood      AS REAL) mood,
                CAST(affection AS REAL) affection,
                level, exp, coins, current_action,
                active_task_id, task_ends_at, task_type,
                last_interaction_at, updated_at
         FROM pet_status WHERE id=1",
    )
    .fetch_one(pool)
    .await?;

    Ok(PetStatus {
        id:                  row.get("id"),
        hunger:              row.get("hunger"),
        energy:              row.get("energy"),
        mood:                row.get("mood"),
        affection:           row.get("affection"),
        level:               row.get("level"),
        exp:                 row.get("exp"),
        coins:               row.get("coins"),
        current_action:      row.get("current_action"),
        active_task_id:      row.get("active_task_id"),
        task_ends_at:        row.get("task_ends_at"),
        task_type:           row.get("task_type"),
        last_interaction_at: row.get("last_interaction_at"),
        updated_at:          row.get("updated_at"),
    })
}

pub async fn update_pet_status(pool: &SqlitePool, patch: StatusPatch) -> Result<PetStatus> {
    let cur = get_pet_status(pool).await?;

    let hunger    = patch.hunger.map(|v| v.max(0.0).min(100.0)).unwrap_or(cur.hunger);
    let energy    = patch.energy.map(|v| v.max(0.0).min(100.0)).unwrap_or(cur.energy);
    let mood      = patch.mood.map(|v| v.max(0.0).min(100.0)).unwrap_or(cur.mood);
    let affection = patch.affection.map(|v| v.max(0.0).min(100.0)).unwrap_or(cur.affection);
    let level     = patch.level.unwrap_or(cur.level);
    let exp       = patch.exp.unwrap_or(cur.exp);
    let coins     = patch.coins.unwrap_or(cur.coins);
    let action    = patch.current_action.unwrap_or(cur.current_action.clone());

    let active_task_id = match patch.active_task_id {
        Some(v) => v,
        None => cur.active_task_id.clone(),
    };
    let task_ends_at = match patch.task_ends_at {
        Some(v) => v,
        None => cur.task_ends_at.clone(),
    };
    let task_type = match patch.task_type {
        Some(v) => v,
        None => cur.task_type.clone(),
    };
    let last_interaction_at = match patch.last_interaction_at {
        Some(v) => v,
        None => cur.last_interaction_at.clone(),
    };

    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    sqlx::query(
        "UPDATE pet_status SET
            hunger=?, energy=?, mood=?, affection=?,
            level=?, exp=?, coins=?, current_action=?,
            active_task_id=?, task_ends_at=?, task_type=?,
            last_interaction_at=?, updated_at=?
         WHERE id=1",
    )
    .bind(hunger)
    .bind(energy)
    .bind(mood)
    .bind(affection)
    .bind(level)
    .bind(exp)
    .bind(coins)
    .bind(&action)
    .bind(&active_task_id)
    .bind(&task_ends_at)
    .bind(&task_type)
    .bind(&last_interaction_at)
    .bind(&now)
    .execute(pool)
    .await?;

    Ok(PetStatus {
        id: 1,
        hunger,
        energy,
        mood,
        affection,
        level,
        exp,
        coins,
        current_action: action,
        active_task_id,
        task_ends_at,
        task_type,
        last_interaction_at,
        updated_at: Some(now),
    })
}

// ─── foods CRUD ──────────────────────────────────────────────────────────────

pub async fn get_foods(pool: &SqlitePool, level: i64) -> Result<Vec<Food>> {
    let rows = sqlx::query(
        "SELECT id, name, price, hunger, energy, mood, unlock_level, enabled
         FROM foods WHERE enabled=1 AND unlock_level<=? ORDER BY unlock_level, price",
    )
    .bind(level)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|r| Food {
            id:           r.get("id"),
            name:         r.get("name"),
            price:        r.get("price"),
            hunger:       r.get("hunger"),
            energy:       r.get("energy"),
            mood:         r.get("mood"),
            unlock_level: r.get("unlock_level"),
            enabled:      r.get("enabled"),
        })
        .collect())
}

pub async fn get_food(pool: &SqlitePool, food_id: &str) -> Result<Option<Food>> {
    let row = sqlx::query(
        "SELECT id, name, price, hunger, energy, mood, unlock_level, enabled
         FROM foods WHERE id=?",
    )
    .bind(food_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| Food {
        id:           r.get("id"),
        name:         r.get("name"),
        price:        r.get("price"),
        hunger:       r.get("hunger"),
        energy:       r.get("energy"),
        mood:         r.get("mood"),
        unlock_level: r.get("unlock_level"),
        enabled:      r.get("enabled"),
    }))
}

// ─── tasks CRUD ──────────────────────────────────────────────────────────────

pub async fn get_tasks(pool: &SqlitePool, task_type: &str, level: i64) -> Result<Vec<Task>> {
    let rows = sqlx::query(
        "SELECT id, task_type, name, duration, energy_cost, exp, coins, mood_delta, unlock_level, enabled
         FROM tasks WHERE enabled=1 AND task_type=? AND unlock_level<=?
         ORDER BY unlock_level, duration",
    )
    .bind(task_type)
    .bind(level)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|r| Task {
            id:           r.get("id"),
            task_type:    r.get("task_type"),
            name:         r.get("name"),
            duration:     r.get("duration"),
            energy_cost:  r.get("energy_cost"),
            exp:          r.get("exp"),
            coins:        r.get("coins"),
            mood_delta:   r.get("mood_delta"),
            unlock_level: r.get("unlock_level"),
            enabled:      r.get("enabled"),
        })
        .collect())
}

pub async fn get_task(pool: &SqlitePool, task_id: &str) -> Result<Option<Task>> {
    let row = sqlx::query(
        "SELECT id, task_type, name, duration, energy_cost, exp, coins, mood_delta, unlock_level, enabled
         FROM tasks WHERE id=?",
    )
    .bind(task_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| Task {
        id:           r.get("id"),
        task_type:    r.get("task_type"),
        name:         r.get("name"),
        duration:     r.get("duration"),
        energy_cost:  r.get("energy_cost"),
        exp:          r.get("exp"),
        coins:        r.get("coins"),
        mood_delta:   r.get("mood_delta"),
        unlock_level: r.get("unlock_level"),
        enabled:      r.get("enabled"),
    }))
}

// ─── messages CRUD ───────────────────────────────────────────────────────────

pub async fn save_message(
    pool: &SqlitePool,
    session_id: &str,
    role: &str,
    content: &str,
    emotion: Option<&str>,
    motion: Option<&str>,
) -> Result<i64> {
    let row = sqlx::query(
        "INSERT INTO messages (session_id, role, content, emotion, motion)
         VALUES (?, ?, ?, ?, ?)
         RETURNING id",
    )
    .bind(session_id)
    .bind(role)
    .bind(content)
    .bind(emotion)
    .bind(motion)
    .fetch_one(pool)
    .await?;

    Ok(row.get("id"))
}

pub async fn get_messages(
    pool: &SqlitePool,
    session_id: &str,
    limit: i64,
) -> Result<Vec<Message>> {
    let rows = sqlx::query(
        "SELECT id, session_id, role, content, emotion, motion, created_at
         FROM messages WHERE session_id=?
         ORDER BY id DESC LIMIT ?",
    )
    .bind(session_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let mut msgs: Vec<Message> = rows
        .iter()
        .map(|r| Message {
            id:         r.get("id"),
            session_id: r.get("session_id"),
            role:       r.get("role"),
            content:    r.get("content"),
            emotion:    r.get("emotion"),
            motion:     r.get("motion"),
            created_at: r.get("created_at"),
        })
        .collect();
    msgs.reverse(); // 时间升序
    Ok(msgs)
}

pub async fn get_all_messages(pool: &SqlitePool) -> Result<Vec<Message>> {
    let rows = sqlx::query(
        "SELECT id, session_id, role, content, emotion, motion, created_at
         FROM messages ORDER BY id ASC",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|r| Message {
            id:         r.get("id"),
            session_id: r.get("session_id"),
            role:       r.get("role"),
            content:    r.get("content"),
            emotion:    r.get("emotion"),
            motion:     r.get("motion"),
            created_at: r.get("created_at"),
        })
        .collect())
}

// ─── inventory CRUD ──────────────────────────────────────────────────────────

pub async fn get_inventory(pool: &SqlitePool) -> Result<Vec<InventoryItem>> {
    let rows = sqlx::query(
        "SELECT item_id, item_type, count FROM inventory WHERE count>0 ORDER BY acquired_at",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|r| InventoryItem {
            item_id:   r.get("item_id"),
            item_type: r.get("item_type"),
            count:     r.get("count"),
        })
        .collect())
}

pub async fn add_inventory(pool: &SqlitePool, item_id: &str, item_type: &str, qty: i64) -> Result<()> {
    sqlx::query(
        "INSERT INTO inventory (item_id, item_type, count)
         VALUES (?, ?, ?)
         ON CONFLICT(item_id) DO UPDATE SET count=count+?",
    )
    .bind(item_id)
    .bind(item_type)
    .bind(qty)
    .bind(qty)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn consume_inventory(pool: &SqlitePool, item_id: &str) -> Result<bool> {
    let row = sqlx::query("SELECT count FROM inventory WHERE item_id=?")
        .bind(item_id)
        .fetch_optional(pool)
        .await?;

    match row {
        Some(r) => {
            let count: i64 = r.get("count");
            if count <= 0 {
                return Ok(false);
            }
            sqlx::query("UPDATE inventory SET count=count-1 WHERE item_id=?")
                .bind(item_id)
                .execute(pool)
                .await?;
            Ok(true)
        }
        None => Ok(false),
    }
}

// ─── long_term_memories CRUD ─────────────────────────────────────────────────

pub async fn save_memory(
    pool: &SqlitePool,
    session_id: &str,
    content: &str,
    category: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO long_term_memories (session_id, content, category) VALUES (?, ?, ?)",
    )
    .bind(session_id)
    .bind(content)
    .bind(category)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_memories(pool: &SqlitePool, session_id: &str, limit: i64) -> Result<Vec<Memory>> {
    let rows = sqlx::query(
        "SELECT id, session_id, content, category, created_at
         FROM long_term_memories WHERE session_id=?
         ORDER BY id DESC LIMIT ?",
    )
    .bind(session_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows_to_memories(&rows))
}

/// 跨会话读取最近长期记忆（用户「记下来」等指令写入后可在新对话中召回）
pub async fn get_recent_memories_global(pool: &SqlitePool, limit: i64) -> Result<Vec<Memory>> {
    let rows = sqlx::query(
        "SELECT id, session_id, content, category, created_at
         FROM long_term_memories
         ORDER BY id DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows_to_memories(&rows))
}

fn rows_to_memories(rows: &[sqlx::sqlite::SqliteRow]) -> Vec<Memory> {
    rows
        .iter()
        .map(|r| Memory {
            id:         r.get("id"),
            session_id: r.get("session_id"),
            content:    r.get("content"),
            category:   r.get("category"),
            created_at: r.get("created_at"),
        })
        .collect()
}

// ─── config 键值表 ───────────────────────────────────────────────────────────

pub async fn get_config(pool: &SqlitePool, key: &str) -> Result<Option<String>> {
    let row = sqlx::query("SELECT value FROM config WHERE key=?")
        .bind(key)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| r.get("value")))
}

pub async fn set_config(pool: &SqlitePool, key: &str, value: &str) -> Result<()> {
    sqlx::query(
        "INSERT INTO config (key, value) VALUES (?, ?)
         ON CONFLICT(key) DO UPDATE SET value=?, updated_at=datetime('now','localtime')",
    )
    .bind(key)
    .bind(value)
    .bind(value)
    .execute(pool)
    .await?;
    Ok(())
}

// ─── P0: memory_summaries CRUD ───────────────────────────────────────────────

/// 保存一条对话摘要
pub async fn save_summary(
    pool:         &SqlitePool,
    session_id:   &str,
    msg_start_id: i64,
    msg_end_id:   i64,
    summary:      &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO memory_summaries (session_id, msg_start_id, msg_end_id, summary)
         VALUES (?, ?, ?, ?)",
    )
    .bind(session_id)
    .bind(msg_start_id)
    .bind(msg_end_id)
    .bind(summary)
    .execute(pool)
    .await?;
    Ok(())
}

/// 获取最近 N 条摘要（时间降序）
pub async fn get_summaries(pool: &SqlitePool, limit: i64) -> Result<Vec<MemorySummary>> {
    let rows = sqlx::query(
        "SELECT id, session_id, msg_start_id, msg_end_id, summary, created_at
         FROM memory_summaries ORDER BY id DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|r| MemorySummary {
            id:           r.get("id"),
            session_id:   r.get("session_id"),
            msg_start_id: r.get("msg_start_id"),
            msg_end_id:   r.get("msg_end_id"),
            summary:      r.get("summary"),
            created_at:   r.get("created_at"),
        })
        .collect())
}

/// 获取某 session 的最后一条摘要覆盖到的 msg_end_id（用于增量摘要）
pub async fn get_last_summarized_msg_id(pool: &SqlitePool, session_id: &str) -> Result<i64> {
    let row = sqlx::query(
        "SELECT COALESCE(MAX(msg_end_id), 0) AS last_id
         FROM memory_summaries WHERE session_id=?",
    )
    .bind(session_id)
    .fetch_one(pool)
    .await?;
    Ok(row.get("last_id"))
}

/// 统计某 session 从指定 msg_id 之后的消息条数
pub async fn count_messages_after(pool: &SqlitePool, session_id: &str, after_id: i64) -> Result<i64> {
    let row = sqlx::query(
        "SELECT COUNT(*) AS cnt FROM messages WHERE session_id=? AND id>?",
    )
    .bind(session_id)
    .bind(after_id)
    .fetch_one(pool)
    .await?;
    Ok(row.get("cnt"))
}

/// 获取某 session 在 [start_id, end_id] 范围内的消息（用于摘要生成）
pub async fn get_messages_in_range(
    pool:       &SqlitePool,
    session_id: &str,
    start_id:   i64,
    end_id:     i64,
) -> Result<Vec<Message>> {
    let rows = sqlx::query(
        "SELECT id, session_id, role, content, emotion, motion, created_at
         FROM messages WHERE session_id=? AND id>? AND id<=?
         ORDER BY id ASC",
    )
    .bind(session_id)
    .bind(start_id)
    .bind(end_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|r| Message {
            id:         r.get("id"),
            session_id: r.get("session_id"),
            role:       r.get("role"),
            content:    r.get("content"),
            emotion:    r.get("emotion"),
            motion:     r.get("motion"),
            created_at: r.get("created_at"),
        })
        .collect())
}

/// 获取某 session 最新的消息 ID
pub async fn get_latest_message_id(pool: &SqlitePool, session_id: &str) -> Result<i64> {
    let row = sqlx::query(
        "SELECT COALESCE(MAX(id), 0) AS max_id FROM messages WHERE session_id=?",
    )
    .bind(session_id)
    .fetch_one(pool)
    .await?;
    Ok(row.get("max_id"))
}

// ─── P0: user_profile CRUD ───────────────────────────────────────────────────

/// 设置用户画像键值（upsert）
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

/// 获取全部用户画像条目
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
            key:        r.get("key"),
            value:      r.get("value"),
            confidence: r.get::<f64, _>("confidence"),
            source:     r.get("source"),
            updated_at: r.get("updated_at"),
        })
        .collect())
}

/// 删除用户画像中的某条记忆
pub async fn delete_user_profile_entry(pool: &SqlitePool, key: &str) -> Result<()> {
    sqlx::query("DELETE FROM user_profile WHERE key = ?")
        .bind(key)
        .execute(pool)
        .await?;
    Ok(())
}

/// 更新用户画像条目（用户手动纠正）
pub async fn update_user_profile_entry(
    pool:  &SqlitePool,
    key:   &str,
    value: &str,
) -> Result<()> {
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

// ─── P0: 关键词搜索长期记忆 ──────────────────────────────────────────────────

/// 按关键词搜索记忆内容（返回匹配的 content 列表，按时间降序）
pub async fn search_memories_by_keyword(
    pool:    &SqlitePool,
    keyword: &str,
    limit:   i64,
) -> Result<Vec<String>> {
    let pattern = format!("%{keyword}%");
    let rows = sqlx::query(
        "SELECT content FROM long_term_memories
         WHERE content LIKE ?
         ORDER BY id DESC LIMIT ?",
    )
    .bind(&pattern)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows.iter().map(|r| r.get::<String, _>("content")).collect())
}

// ─── Batch D: persona_memory CRUD ────────────────────────────────────────────

/// 写入或更新一条人格记忆（upsert by key）
pub async fn upsert_persona_memory(
    pool:       &SqlitePool,
    key:        &str,
    value:      &str,
    category:   &str,
    confidence: f64,
) -> Result<()> {
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
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

/// 读取所有人格记忆（按置信度降序）
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
            key:        r.get("key"),
            value:      r.get("value"),
            category:   r.get("category"),
            confidence: r.get("confidence"),
            updated_at: r.get("updated_at"),
        })
        .collect())
}

/// 读取指定分类的人格记忆（过滤低置信度）
pub async fn get_persona_memory_by_category(
    pool:     &SqlitePool,
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
            key:        r.get("key"),
            value:      r.get("value"),
            category:   r.get("category"),
            confidence: r.get("confidence"),
            updated_at: r.get("updated_at"),
        })
        .collect())
}

/// 降低旧记忆的置信度（冲突消解：写入新记忆时对旧值衰减）
pub async fn decay_persona_confidence(
    pool:       &SqlitePool,
    key:        &str,
    decay_rate: f64,  // e.g. 0.2 表示降低 20%
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

/// 长期记忆写入（附置信度过滤）：仅在置信度 >= 0.7 时才写入
pub async fn save_long_term_memory_guarded(
    pool:       &SqlitePool,
    session_id: &str,
    content:    &str,
    category:   &str,
    confidence: f64,
) -> Result<bool> {
    if confidence < 0.7 {
        return Ok(false);
    }
    sqlx::query(
        "INSERT INTO long_term_memories (session_id, content, category) VALUES (?, ?, ?)",
    )
    .bind(session_id)
    .bind(content)
    .bind(category)
    .execute(pool)
    .await?;
    Ok(true)
}

// ─── Memory Tree / Vault CRUD ─────────────────────────────────────────────────

/// 保存一条 Vault Chunk 记录
pub async fn save_vault_chunk(
    pool:         &SqlitePool,
    session_id:   &str,
    msg_start_id: i64,
    msg_end_id:   i64,
    md_path:      &str,
    content_sha:  &str,
    chunk_kind:   &str,
) -> Result<i64> {
    let result = sqlx::query(
        "INSERT OR REPLACE INTO vault_chunks
         (session_id, msg_start_id, msg_end_id, md_path, content_sha, chunk_kind)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(session_id)
    .bind(msg_start_id)
    .bind(msg_end_id)
    .bind(md_path)
    .bind(content_sha)
    .bind(chunk_kind)
    .execute(pool)
    .await?;
    Ok(result.last_insert_rowid())
}

/// 获取最新的 Vault Chunk id（用于增量处理）
pub async fn get_last_vault_chunk_id(pool: &SqlitePool) -> Result<i64> {
    let row = sqlx::query("SELECT COALESCE(MAX(id), 0) AS last_id FROM vault_chunks")
        .fetch_one(pool)
        .await?;
    Ok(row.get::<i64, _>("last_id"))
}

/// 获取所有 Vault Chunks（按时间正序）
pub async fn get_vault_chunks(pool: &SqlitePool, limit: i64) -> Result<Vec<VaultChunk>> {
    let rows = sqlx::query(
        "SELECT id, session_id, msg_start_id, msg_end_id,
                md_path, content_sha, chunk_kind, created_at
         FROM vault_chunks
         ORDER BY id DESC LIMIT ?",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|r| VaultChunk {
            id:           r.get("id"),
            session_id:   r.get("session_id"),
            msg_start_id: r.get("msg_start_id"),
            msg_end_id:   r.get("msg_end_id"),
            md_path:      r.get("md_path"),
            content_sha:  r.get("content_sha"),
            chunk_kind:   r.get("chunk_kind"),
            created_at:   r.get("created_at"),
        })
        .collect())
}

/// 保存或更新一条 Vault Summary
pub async fn upsert_vault_summary(
    pool:          &SqlitePool,
    level:         &str,
    period_key:    &str,
    md_path:       &str,
    content_sha:   &str,
    last_chunk_id: i64,
) -> Result<()> {
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    sqlx::query(
        "INSERT INTO vault_summaries
         (level, period_key, md_path, content_sha, last_chunk_id, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(period_key) DO UPDATE SET
             md_path       = excluded.md_path,
             content_sha   = excluded.content_sha,
             last_chunk_id = excluded.last_chunk_id,
             updated_at    = excluded.updated_at",
    )
    .bind(level)
    .bind(period_key)
    .bind(md_path)
    .bind(content_sha)
    .bind(last_chunk_id)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(())
}

/// 获取指定 period_key 的 summary（用于判断是否需要更新）
pub async fn get_vault_summary(
    pool:       &SqlitePool,
    period_key: &str,
) -> Result<Option<VaultSummary>> {
    let row = sqlx::query(
        "SELECT id, level, period_key, md_path, content_sha,
                last_chunk_id, created_at, updated_at
         FROM vault_summaries WHERE period_key = ?",
    )
    .bind(period_key)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| VaultSummary {
        id:            r.get("id"),
        level:         r.get("level"),
        period_key:    r.get("period_key"),
        md_path:       r.get("md_path"),
        content_sha:   r.get("content_sha"),
        last_chunk_id: r.get("last_chunk_id"),
        created_at:    r.get("created_at"),
        updated_at:    r.get("updated_at"),
    }))
}

/// 获取所有 Vault Summaries（按级别和时间）
pub async fn get_vault_summaries(
    pool:  &SqlitePool,
    level: &str,
    limit: i64,
) -> Result<Vec<VaultSummary>> {
    let rows = sqlx::query(
        "SELECT id, level, period_key, md_path, content_sha,
                last_chunk_id, created_at, updated_at
         FROM vault_summaries
         WHERE level = ?
         ORDER BY period_key DESC LIMIT ?",
    )
    .bind(level)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|r| VaultSummary {
            id:            r.get("id"),
            level:         r.get("level"),
            period_key:    r.get("period_key"),
            md_path:       r.get("md_path"),
            content_sha:   r.get("content_sha"),
            last_chunk_id: r.get("last_chunk_id"),
            created_at:    r.get("created_at"),
            updated_at:    r.get("updated_at"),
        })
        .collect())
}

/// Vault 统计信息
pub async fn get_vault_stats(pool: &SqlitePool, vault_path: &str) -> Result<VaultStats> {
    let chunk_row = sqlx::query("SELECT COUNT(*) AS cnt FROM vault_chunks")
        .fetch_one(pool)
        .await?;
    let summary_row = sqlx::query("SELECT COUNT(*) AS cnt FROM vault_summaries")
        .fetch_one(pool)
        .await?;
    let last_sync_row = sqlx::query(
        "SELECT MAX(updated_at) AS last_sync FROM vault_summaries",
    )
    .fetch_optional(pool)
    .await?;

    Ok(VaultStats {
        chunk_count:   chunk_row.get::<i64, _>("cnt"),
        summary_count: summary_row.get::<i64, _>("cnt"),
        vault_path:    vault_path.to_string(),
        last_sync_at:  last_sync_row.and_then(|r| r.get("last_sync")),
    })
}

/// 获取自某个 chunk id 之后的所有消息（用于增量处理）
pub async fn get_messages_after_chunk(
    pool:          &SqlitePool,
    session_id:    &str,
    after_msg_id:  i64,
    limit:         i64,
) -> Result<Vec<Message>> {
    let rows = sqlx::query(
        "SELECT id, session_id, role, content, emotion, motion, created_at
         FROM messages
         WHERE session_id = ? AND id > ? AND role IN ('user','assistant')
         ORDER BY id ASC LIMIT ?",
    )
    .bind(session_id)
    .bind(after_msg_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|r| Message {
            id:         r.get("id"),
            session_id: r.get("session_id"),
            role:       r.get("role"),
            content:    r.get("content"),
            emotion:    r.get("emotion"),
            motion:     r.get("motion"),
            created_at: r.get("created_at"),
        })
        .collect())
}

/// 获取所有 session_id（去重，供 vault 遍历所有会话）
pub async fn get_all_session_ids(pool: &SqlitePool) -> Result<Vec<String>> {
    let rows = sqlx::query(
        "SELECT DISTINCT session_id FROM messages ORDER BY session_id",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().map(|r| r.get::<String, _>("session_id")).collect())
}

// ─── P0: memory_vectors CRUD ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct MemoryVectorRow {
    pub source_type: String,
    pub source_id:   String,
    pub content:     String,
    pub embedding:   Vec<u8>,
    pub dims:        i32,
    pub embed_model: String,
}

#[derive(Debug, Clone)]
pub struct MemoryIndexCandidate {
    pub source_type: String,
    pub source_id:   String,
    pub content:     String,
}

/// 获取尚未建立向量索引的记忆条目（每次最多 limit 条）
pub async fn fetch_unindexed_memory_items(
    pool:  &SqlitePool,
    limit: i64,
) -> Result<Vec<MemoryIndexCandidate>> {
    let rows = sqlx::query(
        "SELECT source_type, source_id, content FROM (
            SELECT 'summary' AS source_type, CAST(s.id AS TEXT) AS source_id, s.summary AS content
            FROM memory_summaries s
            WHERE NOT EXISTS (
                SELECT 1 FROM memory_vectors v
                WHERE v.source_type = 'summary' AND v.source_id = CAST(s.id AS TEXT)
            )
            UNION ALL
            SELECT 'ltm', CAST(m.id AS TEXT), m.content
            FROM long_term_memories m
            WHERE NOT EXISTS (
                SELECT 1 FROM memory_vectors v
                WHERE v.source_type = 'ltm' AND v.source_id = CAST(m.id AS TEXT)
            )
            UNION ALL
            SELECT 'profile', p.key, p.key || ': ' || p.value
            FROM user_profile p
            WHERE NOT EXISTS (
                SELECT 1 FROM memory_vectors v
                WHERE v.source_type = 'profile' AND v.source_id = p.key
            )
            UNION ALL
            SELECT 'persona', pm.key, '[' || pm.category || '] ' || pm.value
            FROM persona_memory pm
            WHERE pm.confidence >= 0.7
              AND NOT EXISTS (
                SELECT 1 FROM memory_vectors v
                WHERE v.source_type = 'persona' AND v.source_id = pm.key
            )
        )
        LIMIT ?",
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|r| MemoryIndexCandidate {
            source_type: r.get("source_type"),
            source_id:   r.get("source_id"),
            content:     r.get("content"),
        })
        .collect())
}

pub async fn upsert_memory_vector(
    pool:         &SqlitePool,
    source_type:  &str,
    source_id:    &str,
    content:      &str,
    content_hash: &str,
    embedding:    &[u8],
    dims:         i32,
    embed_model:  &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO memory_vectors (source_type, source_id, content, content_hash, embedding, dims, embed_model)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(source_type, source_id) DO UPDATE SET
            content = excluded.content,
            content_hash = excluded.content_hash,
            embedding = excluded.embedding,
            dims = excluded.dims,
            embed_model = excluded.embed_model,
            created_at = datetime('now','localtime')",
    )
    .bind(source_type)
    .bind(source_id)
    .bind(content)
    .bind(content_hash)
    .bind(embedding)
    .bind(dims)
    .bind(embed_model)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_memory_vectors_by_model(
    pool:        &SqlitePool,
    embed_model: &str,
) -> Result<Vec<MemoryVectorRow>> {
    let rows = sqlx::query(
        "SELECT source_type, source_id, content, embedding, dims, embed_model
         FROM memory_vectors WHERE embed_model = ?",
    )
    .bind(embed_model)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|r| MemoryVectorRow {
            source_type: r.get("source_type"),
            source_id:   r.get("source_id"),
            content:     r.get("content"),
            embedding:   r.get("embedding"),
            dims:        r.get("dims"),
            embed_model: r.get("embed_model"),
        })
        .collect())
}

pub async fn get_all_memory_vectors(pool: &SqlitePool) -> Result<Vec<MemoryVectorRow>> {
    let rows = sqlx::query(
        "SELECT source_type, source_id, content, embedding, dims, embed_model FROM memory_vectors",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|r| MemoryVectorRow {
            source_type: r.get("source_type"),
            source_id:   r.get("source_id"),
            content:     r.get("content"),
            embedding:   r.get("embedding"),
            dims:        r.get("dims"),
            embed_model: r.get("embed_model"),
        })
        .collect())
}

pub async fn count_memory_vectors(pool: &SqlitePool) -> Result<i64> {
    let row = sqlx::query("SELECT COUNT(*) AS cnt FROM memory_vectors")
        .fetch_one(pool)
        .await?;
    Ok(row.get("cnt"))
}

// ─── tool_config CRUD ─────────────────────────────────────────────────────────

/// 工具配置条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfigEntry {
    pub tool_name:    String,
    /// 1=开启, 0=关闭
    pub enabled:      i64,
    /// 1=自动批准(免确认), 0=需确认
    pub auto_approve: i64,
    /// 每日限额(0=无限制)
    pub daily_limit:  i64,
}

/// 获取所有工具配置
pub async fn get_all_tool_configs(pool: &SqlitePool) -> Result<Vec<ToolConfigEntry>> {
    let rows = sqlx::query(
        "SELECT tool_name, enabled, auto_approve, daily_limit FROM tool_config ORDER BY tool_name",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .iter()
        .map(|r| ToolConfigEntry {
            tool_name:    r.get("tool_name"),
            enabled:      r.get("enabled"),
            auto_approve: r.get("auto_approve"),
            daily_limit:  r.get("daily_limit"),
        })
        .collect())
}

/// 获取单个工具配置
pub async fn get_tool_config(pool: &SqlitePool, tool_name: &str) -> Result<Option<ToolConfigEntry>> {
    let row = sqlx::query(
        "SELECT tool_name, enabled, auto_approve, daily_limit FROM tool_config WHERE tool_name=?",
    )
    .bind(tool_name)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| ToolConfigEntry {
        tool_name:    r.get("tool_name"),
        enabled:      r.get("enabled"),
        auto_approve: r.get("auto_approve"),
        daily_limit:  r.get("daily_limit"),
    }))
}

/// 更新工具配置（upsert）
pub async fn upsert_tool_config(
    pool:         &SqlitePool,
    tool_name:    &str,
    enabled:      i64,
    auto_approve: i64,
    daily_limit:  i64,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO tool_config (tool_name, enabled, auto_approve, daily_limit)
         VALUES (?, ?, ?, ?)
         ON CONFLICT(tool_name) DO UPDATE SET
             enabled      = excluded.enabled,
             auto_approve = excluded.auto_approve,
             daily_limit  = excluded.daily_limit",
    )
    .bind(tool_name)
    .bind(enabled)
    .bind(auto_approve)
    .bind(daily_limit)
    .execute(pool)
    .await?;
    Ok(())
}

/// 初始化默认工具配置（仅当表中没有记录时插入）
pub async fn init_default_tool_configs(pool: &SqlitePool, tool_names: &[&str]) -> Result<()> {
    for name in tool_names {
        sqlx::query(
            "INSERT OR IGNORE INTO tool_config (tool_name, enabled, auto_approve, daily_limit)
             VALUES (?, 1, 0, 0)",
        )
        .bind(name)
        .execute(pool)
        .await?;
    }
    Ok(())
}

/// 检查工具是否已开启（用户未关闭）
pub async fn is_tool_enabled(pool: &SqlitePool, tool_name: &str) -> Result<bool> {
    let row = sqlx::query(
        "SELECT enabled FROM tool_config WHERE tool_name = ?",
    )
    .bind(tool_name)
    .fetch_optional(pool)
    .await?;

    match row {
        Some(r) => {
            let enabled: i64 = r.get("enabled");
            Ok(enabled != 0)
        }
        None => Ok(true), // 没有配置默认视为开启
    }
}
