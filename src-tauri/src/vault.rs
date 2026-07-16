// ─── vault.rs ─────────────────────────────────────────────────────────────────
// Memory Tree Vault：双存储实现
//   - SQLite：存结构、索引、父子关系、路径、sha256
//   - Markdown：存人类可读内容，带 YAML front-matter
//
// Vault 目录结构：
//   {data_dir}/vault/
//   ├── Daily/   YYYY-MM-DD.md            L1 每日摘要
//   ├── Weekly/  YYYY-WW.md               L2 每周摘要
//   ├── Monthly/ YYYY-MM.md               L3 每月摘要
//   ├── Chunks/  YYYY-MM-DD-{id}.md       L0 原始对话段落
//   ├── Memories/ user-profile.md / persona.md
//   └── .obsidian/  graph.json / types.json
// ─────────────────────────────────────────────────────────────────────────────

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

use crate::db::Message;

// ─── Vault 根目录管理 ─────────────────────────────────────────────────────────

/// 初始化 Vault 目录结构（幂等，已存在不报错）
pub fn init_vault_dir(vault_root: &Path) -> Result<()> {
    for sub in &["Daily", "Weekly", "Monthly", "Chunks", "Memories", ".obsidian"] {
        std::fs::create_dir_all(vault_root.join(sub))
            .with_context(|| format!("创建 vault 目录失败: {sub}"))?;
    }
    write_obsidian_config(vault_root)?;
    write_readme(vault_root)?;
    Ok(())
}

/// 写入 Obsidian 配置（首次初始化；已存在则跳过）
fn write_obsidian_config(vault_root: &Path) -> Result<()> {
    let graph_path = vault_root.join(".obsidian/graph.json");
    if !graph_path.exists() {
        let graph_json = r#"{
  "collapse-filter": true,
  "search": "",
  "showTags": false,
  "showAttachments": false,
  "hideUnresolved": false,
  "showOrphans": true,
  "collapse-color-groups": false,
  "colorGroups": [
    { "query": "tag:#daily",   "color": { "a": 1, "rgb": 14530050 } },
    { "query": "tag:#summary", "color": { "a": 1, "rgb": 5614830  } },
    { "query": "tag:#persona", "color": { "a": 1, "rgb": 16744272 } }
  ],
  "collapse-display": false,
  "showArrow": true,
  "textFadeMultiplier": 0,
  "nodeSizeMultiplier": 1,
  "lineSizeMultiplier": 1,
  "collapse-forces": true,
  "centerStrength": 0.518713248970312,
  "repelStrength": 10,
  "linkStrength": 1,
  "linkDistance": 250,
  "scale": 1,
  "close": false
}"#;
        write_file(&graph_path, graph_json)?;
    }

    let types_path = vault_root.join(".obsidian/types.json");
    if !types_path.exists() {
        let types_json = r#"{
  "types": {
    "kind":             "text",
    "level":            "number",
    "session_id":       "text",
    "period_key":       "text",
    "source_kind":      "text",
    "time_range_start": "datetime",
    "time_range_end":   "datetime",
    "sealed_at":        "datetime",
    "tags":             "multitext",
    "children":         "multitext"
  }
}"#;
        write_file(&types_path, types_json)?;
    }
    Ok(())
}

fn write_readme(vault_root: &Path) -> Result<()> {
    let readme = vault_root.join("README.md");
    if !readme.exists() {
        let content = concat!(
            "# Chebo Memory Vault\n\n",
            "这是 Chebo 的外部大脑，记录了对话历史、摘要和人格记忆。\n\n",
            "## 目录结构\n\n",
            "- **Chunks/** — 原始对话片段（L0）\n",
            "- **Daily/** — 每日对话摘要（L1）\n",
            "- **Weekly/** — 每周综合摘要（L2）\n",
            "- **Monthly/** — 每月长期摘要（L3）\n",
            "- **Memories/** — 用户画像 + Chebo 人格记忆\n\n",
            "## 使用说明\n\n",
            "- 可以在 [Obsidian](https://obsidian.md) 中打开此目录作为 Vault\n",
            "- 摘要文件之间通过 `[[wikilink]]` 相互连接，在 Obsidian 图谱中可视化\n",
            "- 你可以手动编辑 `Memories/` 中的文件来调整 Chebo 的记忆\n",
            "- 文件会由 Chebo 自动更新，请勿删除 `.obsidian/` 目录\n"
        );
        write_file(&readme, content)?;
    }
    Ok(())
}

// ─── L0 Chunk 写入 ───────────────────────────────────────────────────────────

pub fn render_chunk_md(
    session_id:  &str,
    messages:    &[Message],
    chunk_date:  &str,
    chunk_id:    i64,
) -> String {
    let msg_start = messages.first().map(|m| m.id).unwrap_or(0);
    let msg_end   = messages.last().map(|m| m.id).unwrap_or(0);
    let first_ts  = messages.first().map(|m| m.created_at.as_str()).unwrap_or(chunk_date);
    let last_ts   = messages.last().map(|m| m.created_at.as_str()).unwrap_or(chunk_date);

    let mut body = String::new();
    for msg in messages {
        let role_label = if msg.role == "user" { "**用户**" } else { "**Chebo**" };
        body.push_str(&format!("{}: {}\n\n", role_label, msg.content));
    }

    // 使用普通字符串拼接，避免 r# 原始字符串与 YAML tags 中 "#daily" 冲突
    let front = format!(
        "---\nkind: chunk\nsource_kind: chat\nsession_id: \"{}\"\nchunk_id: {}\n\
         msg_start_id: {}\nmsg_end_id: {}\ntime_range_start: \"{}\"\n\
         time_range_end: \"{}\"\ntags:\n  - \"#daily\"\n  - \"#chunk\"\n---\n\n",
        session_id, chunk_id, msg_start, msg_end, first_ts, last_ts
    );
    format!("{}{}", front, body.trim())
}

pub fn write_chunk(
    vault_root:  &Path,
    session_id:  &str,
    messages:    &[Message],
    chunk_date:  &str,
    chunk_id:    i64,
) -> Result<(String, String)> {
    let filename  = format!("{chunk_date}-{chunk_id}.md");
    let rel_path  = format!("Chunks/{filename}");
    let full_path = vault_root.join(&rel_path);
    let content   = render_chunk_md(session_id, messages, chunk_date, chunk_id);
    let sha       = sha256_str(&content);
    write_file(&full_path, &content)?;
    Ok((rel_path, sha))
}

// ─── L1 Daily Summary 写入 ───────────────────────────────────────────────────

pub fn render_daily_md(
    date:         &str,
    summary_text: &str,
    chunk_refs:   &[String],
    msg_count:    usize,
) -> String {
    let children_block = if chunk_refs.is_empty() {
        String::new()
    } else {
        let links: Vec<String> = chunk_refs.iter()
            .map(|r| format!("  - \"[[{}]]\"", r))
            .collect();
        format!("children:\n{}\n", links.join("\n"))
    };

    format!(
        "---\nkind: summary\nlevel: daily\nperiod_key: \"{date}\"\n\
         tree_kind: global/daily\ntime_range_start: \"{date} 00:00:00\"\n\
         time_range_end: \"{date} 23:59:59\"\nmessage_count: {msg_count}\n\
         sealed_at: \"{date}\"\n{children}tags:\n  - \"#summary\"\n  - \"#daily\"\n---\n\n\
         # {date} 对话摘要\n\n{summary_text}\n",
        date     = date,
        msg_count = msg_count,
        children  = children_block,
        summary_text = summary_text,
    )
}

pub fn write_daily_summary(
    vault_root:   &Path,
    date:         &str,
    summary_text: &str,
    chunk_refs:   &[String],
    msg_count:    usize,
) -> Result<(String, String)> {
    let rel_path  = format!("Daily/{date}.md");
    let full_path = vault_root.join(&rel_path);
    let content   = render_daily_md(date, summary_text, chunk_refs, msg_count);
    let sha       = sha256_str(&content);
    write_file(&full_path, &content)?;
    Ok((rel_path, sha))
}

// ─── L2 Weekly Summary 写入 ──────────────────────────────────────────────────

pub fn render_weekly_md(
    week_key:     &str,
    date_start:   &str,
    date_end:     &str,
    summary_text: &str,
    daily_refs:   &[String],
) -> String {
    let children_block = if daily_refs.is_empty() {
        String::new()
    } else {
        let links: Vec<String> = daily_refs.iter()
            .map(|r| format!("  - \"[[{}]]\"", r))
            .collect();
        format!("children:\n{}\n", links.join("\n"))
    };

    format!(
        "---\nkind: summary\nlevel: weekly\nperiod_key: \"{week_key}\"\n\
         tree_kind: global/weekly\ntime_range_start: \"{date_start} 00:00:00\"\n\
         time_range_end: \"{date_end} 23:59:59\"\nsealed_at: \"{date_end}\"\n\
         {children}tags:\n  - \"#summary\"\n  - \"#weekly\"\n---\n\n\
         # {week_key} 周总结\n\n{summary_text}\n",
        week_key   = week_key,
        date_start = date_start,
        date_end   = date_end,
        children   = children_block,
        summary_text = summary_text,
    )
}

pub fn write_weekly_summary(
    vault_root:   &Path,
    week_key:     &str,
    date_start:   &str,
    date_end:     &str,
    summary_text: &str,
    daily_refs:   &[String],
) -> Result<(String, String)> {
    let rel_path  = format!("Weekly/{week_key}.md");
    let full_path = vault_root.join(&rel_path);
    let content   = render_weekly_md(week_key, date_start, date_end, summary_text, daily_refs);
    let sha       = sha256_str(&content);
    write_file(&full_path, &content)?;
    Ok((rel_path, sha))
}

// ─── L3 Monthly Summary 写入 ─────────────────────────────────────────────────

pub fn render_monthly_md(
    month_key:    &str,
    summary_text: &str,
    weekly_refs:  &[String],
) -> String {
    let children_block = if weekly_refs.is_empty() {
        String::new()
    } else {
        let links: Vec<String> = weekly_refs.iter()
            .map(|r| format!("  - \"[[{}]]\"", r))
            .collect();
        format!("children:\n{}\n", links.join("\n"))
    };

    format!(
        "---\nkind: summary\nlevel: monthly\nperiod_key: \"{month_key}\"\n\
         tree_kind: global/monthly\n{children}tags:\n  - \"#summary\"\n  - \"#monthly\"\n---\n\n\
         # {month_key} 月度回顾\n\n{summary_text}\n",
        month_key  = month_key,
        children   = children_block,
        summary_text = summary_text,
    )
}

pub fn write_monthly_summary(
    vault_root:   &Path,
    month_key:    &str,
    summary_text: &str,
    weekly_refs:  &[String],
) -> Result<(String, String)> {
    let rel_path  = format!("Monthly/{month_key}.md");
    let full_path = vault_root.join(&rel_path);
    let content   = render_monthly_md(month_key, summary_text, weekly_refs);
    let sha       = sha256_str(&content);
    write_file(&full_path, &content)?;
    Ok((rel_path, sha))
}

// ─── Memories 目录 ────────────────────────────────────────────────────────────

pub fn write_user_profile_md(
    vault_root: &Path,
    entries:    &[(String, String)],
) -> Result<()> {
    let lines: Vec<String> = entries.iter()
        .map(|(k, v)| format!("- **{}**: {}", k, v))
        .collect();

    let content = format!(
        "---\nkind: user-profile\ntags:\n  - \"#memory\"\n  - \"#user\"\n---\n\n\
         # 用户画像\n\nChebo 从对话中提取的用户信息。\n\n{}\n",
        lines.join("\n")
    );
    write_file(&vault_root.join("Memories/user-profile.md"), &content)
}

pub fn write_persona_md(
    vault_root: &Path,
    entries:    &[(String, String, String, f64)],
) -> Result<()> {
    let mut by_cat: std::collections::HashMap<String, Vec<String>> = Default::default();
    for (key, value, category, confidence) in entries {
        if *confidence >= 0.7 {
            by_cat
                .entry(category.clone())
                .or_default()
                .push(format!("- **{}**: {} *(置信度: {:.0}%)*", key, value, confidence * 100.0));
        }
    }

    let mut sections = String::new();
    for (cat, lines) in &by_cat {
        sections.push_str(&format!("\n## {}\n\n{}\n", cat, lines.join("\n")));
    }

    let content = format!(
        "---\nkind: persona\ntags:\n  - \"#memory\"\n  - \"#persona\"\n  - \"#chebo\"\n---\n\n\
         # Chebo 人格记忆\n\nChebo 的性格特征、成长经历与关系记忆。\n\n{sections}",
        sections = sections,
    );
    write_file(&vault_root.join("Memories/persona.md"), &content)
}

// ─── 工具函数 ─────────────────────────────────────────────────────────────────

pub fn write_file(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("创建目录失败: {}", parent.display()))?;
    }
    let mut f = std::fs::File::create(path)
        .with_context(|| format!("创建文件失败: {}", path.display()))?;
    f.write_all(content.as_bytes())
        .with_context(|| format!("写入文件失败: {}", path.display()))?;
    Ok(())
}

pub fn sha256_str(s: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn date_from_msg(msg: &Message) -> String {
    msg.created_at.get(..10).unwrap_or("1970-01-01").to_string()
}

pub fn week_key_from_date(date: &str) -> String {
    use chrono::Datelike;
    if let Ok(d) = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d") {
        let iso = d.iso_week();
        format!("{}-W{:02}", iso.year(), iso.week())
    } else {
        date.get(..7).unwrap_or(date).to_string()
    }
}

pub fn month_key_from_date(date: &str) -> String {
    date.get(..7).unwrap_or(date).to_string()
}

pub fn vault_root(data_dir: &Path) -> PathBuf {
    data_dir.join("vault")
}
