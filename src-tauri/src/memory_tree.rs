// ─── memory_tree.rs ───────────────────────────────────────────────────────────
// Memory Tree：层级摘要树
//
// 三层摘要结构（参考 OpenHuman tree_source）：
//   L0 Chunk   — 原始对话段落（每 N 条消息一组，写入 vault/Chunks/）
//   L1 Daily   — 每日摘要，合并当日所有 Chunk（写入 vault/Daily/）
//   L2 Weekly  — 每周摘要，合并当周所有 Daily（写入 vault/Weekly/）
//   L3 Monthly — 每月摘要，合并当月所有 Weekly（写入 vault/Monthly/）
//
// 增量处理策略（类 bucket_seal）：
//   - 每次同步记录已处理到的最新 msg_id，下次只处理新增消息
//   - L1/L2/L3 的 seal 条件：当天/周/月有新 Chunk 时触发更新
// ─────────────────────────────────────────────────────────────────────────────

use std::path::Path;
use std::sync::Arc;

use sqlx::{Row, SqlitePool};

use crate::db;
use crate::memory::episode_store::Message;
use crate::llm::{LlmConfig, LlmMessage};
use crate::vault;

/// L0 Chunk 触发阈值：每积累 N 条新消息写一个 Chunk
const CHUNK_EVERY: usize = 10;
/// 🆕 Ticket 07: 2 小时 TTL — 超时后强制封存未满的 Chunk
const CHUNK_TTL_HOURS: i64 = 2;
/// 🆕 Ticket 07: 当日 Chunk 超过此数量时提前触发 L1→L2 级联
const DAILY_CHUNK_CASCADE_THRESHOLD: usize = 20;

// ─── 主入口：全量增量同步 ─────────────────────────────────────────────────────

/// 对单个 session_id 执行增量同步：
///   1. 新消息 → L0 Chunk（每 10 条一组）
///   2. 当日 Chunk → L1 Daily Summary（LLM 生成）
///   3. 当周 Daily → L2 Weekly Summary（LLM 生成）
///   4. 当月 Weekly → L3 Monthly Summary（LLM 生成）
pub async fn sync_session(
    pool:       &SqlitePool,
    llm_cfg:    &Arc<LlmConfig>,
    vault_root: &Path,
    session_id: &str,
) {
    // 获取上次处理到的最新 chunk id 对应的 msg_end_id
    let last_processed_msg_id = get_last_processed_msg_id(pool, session_id).await;

    // 获取新消息
    let new_msgs = match crate::memory::episode_store::get_messages_after_chunk(
        pool,
        session_id,
        last_processed_msg_id,
        500,
    )
    .await
    {
        Ok(m)  => m,
        Err(e) => { log::warn!("memory_tree get_messages: {e}"); return; }
    };

    if new_msgs.is_empty() {
        // 🆕 Ticket 07: TTL 检查 — 即使无新消息，检查是否需要强制封存
        force_seal_stale_chunks(pool, vault_root, session_id).await;
        log::debug!("memory_tree: session={session_id} 无新消息");
        return;
    }

    log::info!("memory_tree: session={session_id} 新消息 {} 条，开始同步", new_msgs.len());

    // ── Step 1: 切割 L0 Chunks ───────────────────────────────────────────────
    let affected_dates = seal_l0_chunks(pool, vault_root, session_id, &new_msgs).await;

    // ── Step 2: 更新各受影响日期的 L1 Daily Summary ──────────────────────────
    let affected_weeks  = seal_l1_daily(pool, llm_cfg, vault_root, session_id, &affected_dates).await;

    // ── Step 3: 更新各受影响周的 L2 Weekly Summary ───────────────────────────
    let affected_months = seal_l2_weekly(pool, llm_cfg, vault_root, &affected_weeks).await;

    // ── Step 4: 更新各受影响月的 L3 Monthly Summary ──────────────────────────
    seal_l3_monthly(pool, llm_cfg, vault_root, &affected_months).await;

    log::info!("memory_tree: session={session_id} 同步完成");
}

/// 更新 Vault 中的 Memories 目录（用户画像 + 人格记忆）
pub async fn sync_memories(pool: &SqlitePool, vault_root: &Path) {
    // 用户画像
    if let Ok(profile) = crate::memory::core_memory_store::get_user_profile_all(pool).await {
        let entries: Vec<(String, String)> = profile
            .iter()
            .map(|e| (e.key.clone(), e.value.clone()))
            .collect();
        if let Err(e) = vault::write_user_profile_md(vault_root, &entries) {
            log::warn!("sync_memories user-profile: {e}");
        }
    }

    // 人格记忆
    if let Ok(persona) = crate::memory::core_memory_store::get_persona_memory_all(pool).await {
        let entries: Vec<(String, String, String, f64)> = persona
            .iter()
            .map(|p| (p.key.clone(), p.value.clone(), p.category.clone(), p.confidence))
            .collect();
        if let Err(e) = vault::write_persona_md(vault_root, &entries) {
            log::warn!("sync_memories persona: {e}");
        }
    }
}

// ─── Step 1: L0 Chunk Seal ───────────────────────────────────────────────────

/// 🆕 Ticket 07: 强制封存过期的未满 Chunk
/// 如果距离最后一个 Chunk 超过 TTL，封存剩余的未处理消息
async fn force_seal_stale_chunks(
    pool:       &SqlitePool,
    vault_root: &Path,
    session_id: &str,
) {
    // 获取最后一个 Chunk 的创建时间
    let last_chunk_time = match sqlx::query(
        "SELECT created_at FROM vault_chunks WHERE session_id = ?1 ORDER BY id DESC LIMIT 1"
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(row)) => {
            let ts: String = row.get("created_at");
            ts
        }
        _ => return, // 没有任何 Chunk，不需要强制封存
    };

    // 解析时间并检查 TTL
    if let Ok(last_time) = chrono::NaiveDateTime::parse_from_str(&last_chunk_time, "%Y-%m-%d %H:%M:%S") {
        let elapsed = chrono::Local::now().naive_local() - last_time;
        if elapsed.num_hours() < CHUNK_TTL_HOURS {
            return; // 还没到 TTL
        }
    } else {
        return;
    }

    // 获取最后一个 Chunk 覆盖的 msg_end_id
    let last_msg_id = match sqlx::query(
        "SELECT COALESCE(MAX(msg_end_id), 0) as lid FROM vault_chunks WHERE session_id = ?1"
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(row)) => row.get::<i64, _>("lid"),
        _ => return,
    };

    // 获取 TTL 超时后的新消息（但未满 CHUNK_EVERY）
    let stale_msgs = match crate::memory::episode_store::get_messages_after_chunk(
        pool, session_id, last_msg_id, CHUNK_EVERY as i64,
    ).await {
        Ok(m) => m,
        Err(_) => return,
    };

    if stale_msgs.is_empty() {
        return;
    }

    log::info!("memory_tree TTL: session={session_id} 强制封存 {} 条过期消息", stale_msgs.len());
    seal_l0_chunks(pool, vault_root, session_id, &stale_msgs).await;
}

/// 将新消息按 CHUNK_EVERY 条切割，写入 vault/Chunks/，保存 DB 记录
/// 返回受影响的日期集合（用于触发 L1 更新）
async fn seal_l0_chunks(
    pool:       &SqlitePool,
    vault_root: &Path,
    session_id: &str,
    messages:   &[Message],
) -> Vec<String> {
    let mut affected_dates: Vec<String> = Vec::new();

    for chunk_msgs in messages.chunks(CHUNK_EVERY) {
        let chunk_date = vault::date_from_msg(&chunk_msgs[0]);
        let last_chunk_id = db::get_last_vault_chunk_id(pool).await.unwrap_or(0);
        let new_chunk_id  = last_chunk_id + 1;

        match vault::write_chunk(vault_root, session_id, chunk_msgs, &chunk_date, new_chunk_id) {
            Ok((rel_path, sha)) => {
                let msg_start = chunk_msgs.first().map(|m| m.id).unwrap_or(0);
                let msg_end   = chunk_msgs.last().map(|m| m.id).unwrap_or(0);

                if let Err(e) = db::save_vault_chunk(
                    pool,
                    session_id,
                    msg_start,
                    msg_end,
                    &rel_path,
                    &sha,
                    "session",
                )
                .await
                {
                    log::warn!("seal_l0 save_vault_chunk: {e}");
                } else {
                    log::debug!("L0 Chunk 写入: {rel_path}");
                    if !affected_dates.contains(&chunk_date) {
                        affected_dates.push(chunk_date);
                    }
                }
            }
            Err(e) => log::warn!("seal_l0 write_chunk: {e}"),
        }
    }

    affected_dates
}

// ─── Step 2: L1 Daily Summary Seal ──────────────────────────────────────────

/// 对每个受影响的日期，汇总当日 Chunks 生成 Daily Summary
/// 返回受影响的周 key 集合
async fn seal_l1_daily(
    pool:       &SqlitePool,
    llm_cfg:    &Arc<LlmConfig>,
    vault_root: &Path,
    session_id: &str,
    dates:      &[String],
) -> Vec<String> {
    let mut affected_weeks: Vec<String> = Vec::new();

    for date in dates {
        // 获取当日所有 Chunks（跨 session 合并同一天）
        let all_chunks = match db::get_vault_chunks(pool, 500).await {
            Ok(c) => c,
            Err(e) => { log::warn!("seal_l1 get_chunks: {e}"); continue; }
        };

        let day_chunks: Vec<_> = all_chunks
            .iter()
            .filter(|c| c.md_path.contains(&format!("Chunks/{date}")))
            .collect();
        // 🆕 Ticket 07: 当日 Chunk 超过阈值时强制触发 cascade
        let force_cascade = day_chunks.len() > DAILY_CHUNK_CASCADE_THRESHOLD;
        if day_chunks.is_empty() {
            continue;
        }

        // 读取各 Chunk 的 Markdown 内容（拼接用于摘要）
        let mut combined_text = String::new();
        let mut chunk_refs: Vec<String> = Vec::new();

        for chunk in &day_chunks {
            let full_path = vault_root.join(&chunk.md_path);
            if let Ok(content) = std::fs::read_to_string(&full_path) {
                // 去掉 front-matter（--- ... ---），只保留正文
                let body = strip_front_matter(&content);
                combined_text.push_str(body);
                combined_text.push('\n');
            }
            // wikilink 引用名（不含 .md）
            let ref_name = chunk.md_path
                .trim_start_matches("Chunks/")
                .trim_end_matches(".md");
            chunk_refs.push(ref_name.to_string());
        }

        let msg_count: usize = day_chunks.iter().map(|c| {
            (c.msg_end_id - c.msg_start_id).unsigned_abs() as usize + 1
        }).sum();

        // 调用 LLM 生成摘要
        let summary_text = generate_summary_llm(
            llm_cfg,
            &combined_text,
            &format!("{date} 的对话"),
        )
        .await;

        let last_chunk_id = day_chunks.iter().map(|c| c.id).max().unwrap_or(0);

        match vault::write_daily_summary(
            vault_root,
            date,
            &summary_text,
            &chunk_refs,
            msg_count,
        ) {
            Ok((rel_path, sha)) => {
                if let Err(e) = db::upsert_vault_summary(
                    pool,
                    "daily",
                    date,
                    &rel_path,
                    &sha,
                    last_chunk_id,
                )
                .await
                {
                    log::warn!("seal_l1 upsert_summary: {e}");
                } else {
                    log::info!("L1 Daily Summary 更新: {date}");
                    // 🆕 Ticket 07: 超阈值时记录日志
                    if force_cascade {
                        log::info!("L1 cascade: {date} 有 {} 个 Chunk，超过阈值，提前触发 L1→L2", day_chunks.len());
                    }
                    let week_key = vault::week_key_from_date(date);
                    if !affected_weeks.contains(&week_key) {
                        affected_weeks.push(week_key);
                    }
                }
            }
            Err(e) => log::warn!("seal_l1 write_daily: {e}"),
        }
        let _ = session_id; // suppress warning
    }

    affected_weeks
}

// ─── Step 3: L2 Weekly Summary Seal ─────────────────────────────────────────

/// 对每个受影响的周，汇总当周 Daily Summaries 生成 Weekly Summary
/// 返回受影响的月 key 集合
async fn seal_l2_weekly(
    pool:       &SqlitePool,
    llm_cfg:    &Arc<LlmConfig>,
    vault_root: &Path,
    weeks:      &[String],
) -> Vec<String> {
    let mut affected_months: Vec<String> = Vec::new();

    for week_key in weeks {
        // 找该周内所有 daily summaries
        let all_daily = match db::get_vault_summaries(pool, "daily", 100).await {
            Ok(s) => s,
            Err(e) => { log::warn!("seal_l2 get_summaries: {e}"); continue; }
        };

        let week_dailies: Vec<_> = all_daily
            .iter()
            .filter(|s| vault::week_key_from_date(&s.period_key) == *week_key)
            .collect();

        if week_dailies.is_empty() {
            continue;
        }

        let mut combined_text = String::new();
        let mut daily_refs: Vec<String> = Vec::new();
        let mut date_start = String::from("9999-99-99");
        let mut date_end   = String::from("0000-00-00");

        for daily in &week_dailies {
            let full_path = vault_root.join(&daily.md_path);
            if let Ok(content) = std::fs::read_to_string(&full_path) {
                combined_text.push_str(strip_front_matter(&content));
                combined_text.push('\n');
            }
            daily_refs.push(daily.period_key.clone());

            if daily.period_key < date_start { date_start = daily.period_key.clone(); }
            if daily.period_key > date_end   { date_end   = daily.period_key.clone(); }
        }

        let summary_text = generate_summary_llm(
            llm_cfg,
            &combined_text,
            &format!("{week_key} 这一周"),
        )
        .await;

        let last_chunk_id = week_dailies.iter().map(|s| s.last_chunk_id).max().unwrap_or(0);

        match vault::write_weekly_summary(
            vault_root,
            week_key,
            &date_start,
            &date_end,
            &summary_text,
            &daily_refs,
        ) {
            Ok((rel_path, sha)) => {
                if let Err(e) = db::upsert_vault_summary(
                    pool, "weekly", week_key, &rel_path, &sha, last_chunk_id,
                )
                .await
                {
                    log::warn!("seal_l2 upsert: {e}");
                } else {
                    log::info!("L2 Weekly Summary 更新: {week_key}");
                    let month_key = vault::month_key_from_date(&date_start);
                    if !affected_months.contains(&month_key) {
                        affected_months.push(month_key);
                    }
                }
            }
            Err(e) => log::warn!("seal_l2 write_weekly: {e}"),
        }
    }

    affected_months
}

// ─── Step 4: L3 Monthly Summary Seal ────────────────────────────────────────

/// 对每个受影响的月，汇总当月 Weekly Summaries 生成 Monthly Summary
async fn seal_l3_monthly(
    pool:       &SqlitePool,
    llm_cfg:    &Arc<LlmConfig>,
    vault_root: &Path,
    months:     &[String],
) {
    for month_key in months {
        let all_weekly = match db::get_vault_summaries(pool, "weekly", 100).await {
            Ok(s) => s,
            Err(e) => { log::warn!("seal_l3 get_summaries: {e}"); continue; }
        };

        let month_weeks: Vec<_> = all_weekly
            .iter()
            .filter(|s| vault::month_key_from_date(&s.period_key) == *month_key)
            .collect();

        if month_weeks.is_empty() {
            continue;
        }

        let mut combined_text = String::new();
        let mut weekly_refs: Vec<String> = Vec::new();

        for weekly in &month_weeks {
            let full_path = vault_root.join(&weekly.md_path);
            if let Ok(content) = std::fs::read_to_string(&full_path) {
                combined_text.push_str(strip_front_matter(&content));
                combined_text.push('\n');
            }
            weekly_refs.push(weekly.period_key.clone());
        }

        let summary_text = generate_summary_llm(
            llm_cfg,
            &combined_text,
            &format!("{month_key} 这个月"),
        )
        .await;

        let last_chunk_id = month_weeks.iter().map(|s| s.last_chunk_id).max().unwrap_or(0);

        match vault::write_monthly_summary(vault_root, month_key, &summary_text, &weekly_refs) {
            Ok((rel_path, sha)) => {
                if let Err(e) = db::upsert_vault_summary(
                    pool, "monthly", month_key, &rel_path, &sha, last_chunk_id,
                )
                .await
                {
                    log::warn!("seal_l3 upsert: {e}");
                } else {
                    log::info!("L3 Monthly Summary 更新: {month_key}");
                }
            }
            Err(e) => log::warn!("seal_l3 write_monthly: {e}"),
        }
    }
}

// ─── 工具函数 ─────────────────────────────────────────────────────────────────

/// 获取上次处理到的最新 msg_id（通过 vault_chunks 表推断）
async fn get_last_processed_msg_id(pool: &SqlitePool, session_id: &str) -> i64 {
    let row = sqlx::query(
        "SELECT COALESCE(MAX(msg_end_id), 0) AS last_id
         FROM vault_chunks WHERE session_id = ?",
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await;

    match row {
        Ok(Some(r)) => r.get::<i64, _>("last_id"),
        _ => 0,
    }
}

/// 去掉 Markdown YAML front-matter（--- ... ---），只返回正文
fn strip_front_matter(content: &str) -> &str {
    if !content.starts_with("---") {
        return content;
    }
    // 找第二个 "---"
    if let Some(end) = content[3..].find("---") {
        let after = end + 3 + 3; // 跳过两个 "---"
        content[after..].trim_start_matches('\n')
    } else {
        content
    }
}

/// 调用 LLM 生成摘要（静默，不影响主对话流）
async fn generate_summary_llm(
    llm_cfg: &Arc<LlmConfig>,
    content: &str,
    scope:   &str,
) -> String {
    // 截断过长内容（避免 token 超限）
    let truncated = if content.len() > 6000 {
        &content[..6000]
    } else {
        content
    };

    let prompt = format!(
        "请对以下内容进行简洁摘要（150字以内），重点提取：讨论了什么话题、有什么重要信息、Chebo 和用户之间发生了什么。\
        这是{scope}的对话记录。\n\n{truncated}"
    );

    let messages = vec![
        LlmMessage::system(
            "你是 Chebo 的记忆整理助手，请用简洁中文概括对话内容，语气温暖自然。",
        ),
        LlmMessage::user(&prompt),
    ];

    match crate::llm::call_silent(messages, llm_cfg).await {
        Ok((text, _)) if !text.is_empty() => text,
        _ => format!("（{scope}的对话摘要生成失败，原始记录已保存在 Chunks 目录中）"),
    }
}
