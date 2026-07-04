// ─── tools.rs ────────────────────────────────────────────────────────────────
// Tool System（P1 基础 + 架构强化：L0–L3 权限分级）
#![allow(dead_code)]
//
// 工具清单：
//   1. read_file   — 读取本地文本文件（L0：只读信息，直接执行）
//   2. list_dir    — 列出目录内容（L0：只读信息，直接执行）
//   3. git_status  — Git 状态/最近提交（L0：只读信息，直接执行）
//   4. web_search  — DuckDuckGo Instant Answer API（L1：轻量查询，可配置自动执行）
//   5. safe_shell  — 只读 shell 命令（L2：系统操作，需前端确认）
//                  — 写入/修改命令（L3：高危操作，永远需确认 + 风险说明）
//
// 权限分级执行策略：
//   L0 → 直接执行，无需确认
//   L1 → 可配置是否自动执行（默认直接执行）
//   L2 → 必须等待前端用户确认后执行
//   L3 → 永远需要前端确认 + 显示高危风险说明
// ─────────────────────────────────────────────────────────────────────────────

use serde::{Deserialize, Serialize};

// ─── 权限分级 ─────────────────────────────────────────────────────────────────

/// 工具调用权限等级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolLevel {
    /// L0：只读信息（read_file / list_dir / git_status） — 直接执行
    L0,
    /// L1：轻量查询（web_search） — 可配置自动执行
    L1,
    /// L2：系统操作（只读 shell 命令） — 需前端确认
    L2,
    /// L3：高危操作（写入/修改/删除命令） — 永远需确认 + 风险说明
    L3,
}

impl ToolLevel {
    /// 是否需要前端确认才能执行
    pub fn requires_confirm(&self) -> bool {
        matches!(self, ToolLevel::L2 | ToolLevel::L3)
    }

    /// 权限等级的数字表示（用于 Event Bus 传递）
    pub fn as_u8(&self) -> u8 {
        match self {
            ToolLevel::L0 => 0,
            ToolLevel::L1 => 1,
            ToolLevel::L2 => 2,
            ToolLevel::L3 => 3,
        }
    }
}

/// 根据工具名称和命令参数判断权限等级
pub fn tool_level(tool: &str, args: &str) -> ToolLevel {
    match tool {
        "read_file" | "list_dir" | "git_status" => ToolLevel::L0,
        "web_search" => ToolLevel::L1,
        "safe_shell" => {
            // 分析 shell 命令是否涉及写入/高危操作
            if is_write_or_dangerous_cmd(args) {
                ToolLevel::L3
            } else {
                ToolLevel::L2
            }
        }
        _ => ToolLevel::L2, // 未知工具默认 L2 需确认
    }
}

/// 判断 shell 命令是否属于写入/高危操作（L3）
fn is_write_or_dangerous_cmd(cmd: &str) -> bool {
    let first = cmd.trim().split_whitespace().next().unwrap_or("").to_lowercase();
    // 这些命令涉及写入或危险操作
    let write_cmds = ["rm", "del", "mv", "cp", "mkdir", "rmdir",
                      "chmod", "chown", "touch", "write", "install",
                      "npm", "pip", "cargo"];
    write_cmds.contains(&first.as_str())
}

/// 返回权限等级的中文描述（用于前端确认对话框）
pub fn level_description(level: ToolLevel) -> &'static str {
    match level {
        ToolLevel::L0 => "只读操作，安全可信，直接执行",
        ToolLevel::L1 => "轻量查询操作，风险极低",
        ToolLevel::L2 => "系统级操作，建议确认后执行",
        ToolLevel::L3 => "高危操作：可能修改系统文件或执行不可逆命令，请仔细确认",
    }
}

/// 返回权限等级对应的风险颜色标识（供前端使用）
pub fn level_color_tag(level: ToolLevel) -> &'static str {
    match level {
        ToolLevel::L0 => "green",
        ToolLevel::L1 => "blue",
        ToolLevel::L2 => "yellow",
        ToolLevel::L3 => "red",
    }
}

// ─── 工具执行结果 ─────────────────────────────────────────────────────────────

/// 工具执行结果（扩展：包含权限信息）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool:    String,
    pub success: bool,
    pub content: String,
    /// 权限等级（0-3，供前端展示）
    pub level:   u8,
}

impl ToolResult {
    pub fn ok(tool: &str, content: impl Into<String>) -> Self {
        Self { tool: tool.into(), success: true, content: content.into(), level: 0 }
    }
    pub fn ok_with_level(tool: &str, content: impl Into<String>, level: ToolLevel) -> Self {
        Self { tool: tool.into(), success: true, content: content.into(), level: level.as_u8() }
    }
    pub fn err(tool: &str, reason: impl Into<String>) -> Self {
        Self { tool: tool.into(), success: false, content: reason.into(), level: 0 }
    }

    /// 生成供 LLM 上下文使用的摘要（不超过 200 字）
    pub fn to_summary(&self) -> String {
        let prefix = if self.success { "[成功]" } else { "[失败]" };
        let body = if self.content.len() > 200 {
            format!("{}...", &self.content[..200])
        } else {
            self.content.clone()
        };
        format!("{prefix} {}: {body}", self.tool)
    }
}

// ─── 待确认工具调用（L2/L3 的挂起状态）────────────────────────────────────────

/// 挂起等待确认的工具调用，由 commands.rs 存储在 AppState 中
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingToolCall {
    /// 前端确认令牌（UUID）
    pub token:   String,
    pub tool:    String,
    pub args:    String,
    pub level:   ToolLevel,
    /// 风险描述（L3 时必填）
    pub risk_desc: String,
}

// ─── 1. 文件读取（L0）────────────────────────────────────────────────────────

const MAX_FILE_BYTES: u64 = 50 * 1024; // 50 KB

/// 允许读取的文本文件扩展名白名单
const ALLOWED_EXTS: &[&str] = &[
    "txt", "md", "rs", "py", "js", "ts", "vue", "jsx", "tsx",
    "json", "toml", "yaml", "yml", "html", "css", "scss",
    "sh", "bash", "go", "java", "c", "cpp", "h", "cs", "rb",
    "php", "swift", "kt", "sql", "env", "gitignore", "conf",
    "cfg", "ini", "log", "xml", "svg",
];

pub async fn read_file(path_str: &str) -> ToolResult {
    use std::path::Path;

    let path = Path::new(path_str);

    // 安全：解析绝对路径，防止路径穿越
    let abs = match path.canonicalize() {
        Ok(p)  => p,
        Err(e) => return ToolResult::err("read_file", format!("路径无效: {e}")),
    };

    // 扩展名白名单
    let ext = abs.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if !ALLOWED_EXTS.contains(&ext.as_str()) {
        return ToolResult::err("read_file", format!("不支持读取 .{ext} 文件（仅支持文本类型）"));
    }

    // 文件大小限制
    let size = std::fs::metadata(&abs).map(|m| m.len()).unwrap_or(0);
    if size > MAX_FILE_BYTES {
        return ToolResult::err(
            "read_file",
            format!("文件太大（{} KB），最大支持 50 KB", size / 1024),
        );
    }

    match std::fs::read_to_string(&abs) {
        Ok(content) => ToolResult::ok_with_level(
            "read_file",
            format!("**{}**\n\n```\n{}\n```", abs.display(), content),
            ToolLevel::L0,
        ),
        Err(e) => ToolResult::err("read_file", format!("读取失败: {e}")),
    }
}

/// 从消息文本中提取可能的本地文件路径
pub fn extract_file_paths(text: &str) -> Vec<String> {
    use std::path::Path;

    let mut found = Vec::new();

    for word in text.split_whitespace() {
        let cleaned = word.trim_matches(|c| matches!(c, '\'' | '"' | '`' | '，' | '。' | '、'));

        let is_path = (cleaned.len() > 4
            && cleaned.chars().nth(1) == Some(':')
            && cleaned.chars().nth(2) == Some('\\'))
            || cleaned.starts_with('/')
            || cleaned.starts_with("~/")
            || (cleaned.contains('/') || cleaned.contains('\\')
                && cleaned.len() > 5);

        if is_path && Path::new(cleaned).exists() {
            let meta = std::fs::metadata(cleaned);
            if matches!(meta, Ok(ref m) if m.is_file()) {
                found.push(cleaned.to_string());
            }
        }
    }

    found
}

// ─── 2. Web 搜索（L1）────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct DdgResponse {
    #[serde(rename = "Abstract")]
    abstract_text: String,
    #[serde(rename = "AbstractSource")]
    source: String,
    #[serde(rename = "Answer")]
    answer: String,
    #[serde(rename = "RelatedTopics")]
    related: Vec<DdgTopic>,
}

#[derive(Deserialize)]
struct DdgTopic {
    #[serde(rename = "Text")]
    text: Option<String>,
    #[serde(rename = "FirstURL")]
    url:  Option<String>,
}

pub async fn web_search(query: &str, client: &reqwest::Client) -> ToolResult {
    let encoded = urlencoding::encode(query);
    let url = format!(
        "https://api.duckduckgo.com/?q={encoded}&format=json&no_html=1&skip_disambig=1"
    );

    let resp = match client
        .get(&url)
        .header("User-Agent", "Chebo-Desktop-Pet/1.0 (educational use)")
        .timeout(std::time::Duration::from_secs(8))
        .send()
        .await
    {
        Ok(r)  => r,
        Err(e) => return ToolResult::err("web_search", format!("网络请求失败: {e}")),
    };

    let data: DdgResponse = match resp.json().await {
        Ok(d)  => d,
        Err(e) => return ToolResult::err("web_search", format!("解析结果失败: {e}")),
    };

    let mut parts: Vec<String> = Vec::new();

    if !data.answer.is_empty() {
        parts.push(format!("**直接答案**: {}", data.answer));
    }
    if !data.abstract_text.is_empty() {
        parts.push(format!(
            "**摘要** (来源: {}):\n{}",
            data.source, data.abstract_text
        ));
    }

    let topics: Vec<String> = data
        .related
        .iter()
        .filter_map(|t| {
            t.text.as_ref().map(|txt| match &t.url {
                Some(u) => format!("• {} — {}", txt, u),
                None    => format!("• {}", txt),
            })
        })
        .take(4)
        .collect();
    if !topics.is_empty() {
        parts.push(format!("**相关内容**:\n{}", topics.join("\n")));
    }

    if parts.is_empty() {
        ToolResult::ok_with_level(
            "web_search",
            format!("未找到关于「{query}」的直接答案，建议在浏览器中搜索。"),
            ToolLevel::L1,
        )
    } else {
        ToolResult::ok_with_level("web_search", parts.join("\n\n"), ToolLevel::L1)
    }
}

// ─── 3. Git 状态（L0）────────────────────────────────────────────────────────

pub async fn git_status(dir: &str) -> ToolResult {
    use std::process::Command;
    use std::path::Path;

    let path = Path::new(dir);
    if !path.exists() {
        return ToolResult::err("git_status", format!("目录不存在: {dir}"));
    }

    let mut parts: Vec<String> = Vec::new();

    if let Ok(out) = Command::new("git")
        .args(["status", "--short", "--branch"])
        .current_dir(path)
        .output()
    {
        let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !text.is_empty() {
            parts.push(format!("**Status**:\n```\n{}\n```", text));
        }
    }

    if let Ok(out) = Command::new("git")
        .args(["log", "--oneline", "-5"])
        .current_dir(path)
        .output()
    {
        let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !text.is_empty() {
            parts.push(format!("**最近提交**:\n```\n{}\n```", text));
        }
    }

    if let Ok(out) = Command::new("git")
        .args(["diff", "--stat", "HEAD"])
        .current_dir(path)
        .output()
    {
        let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !text.is_empty() {
            parts.push(format!("**变更统计**:\n```\n{}\n```", text));
        }
    }

    if parts.is_empty() {
        ToolResult::err("git_status", "不是 Git 仓库，或没有提交记录。")
    } else {
        ToolResult::ok_with_level("git_status", parts.join("\n\n"), ToolLevel::L0)
    }
}

// ─── 4. 安全 Shell（L2 / L3）────────────────────────────────────────────────

/// 允许执行的命令白名单（只读/查询类）
const SAFE_CMDS: &[&str] = &[
    "git", "ls", "dir", "pwd", "echo", "cat", "type",
    "where", "which", "cargo", "node", "python", "rustc",
    "grep", "find", "head", "tail", "wc",
];

pub fn is_safe_command(cmd: &str) -> bool {
    let first = cmd.trim().split_whitespace().next().unwrap_or("");
    SAFE_CMDS.contains(&first.to_lowercase().as_str())
}

pub async fn safe_shell(cmd: &str) -> ToolResult {
    use std::process::Command;

    if !is_safe_command(cmd) {
        return ToolResult::err(
            "shell",
            format!(
                "命令「{}」不在安全白名单中。\n允许的命令：{}",
                cmd.split_whitespace().next().unwrap_or(""),
                SAFE_CMDS.join(", ")
            ),
        );
    }

    let parts: Vec<&str> = cmd.trim().split_whitespace().collect();
    if parts.is_empty() {
        return ToolResult::err("shell", "命令为空");
    }

    let level = tool_level("safe_shell", cmd);
    let result = Command::new(parts[0]).args(&parts[1..]).output();

    match result {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            let content = if stderr.is_empty() {
                format!("```\n{}\n```", stdout.trim())
            } else {
                format!("```\n{}\n```\nStderr: {}", stdout.trim(), stderr.trim())
            };
            ToolResult::ok_with_level("shell", content, level)
        }
        Err(e) => ToolResult::err("shell", format!("执行失败: {e}")),
    }
}

// ─── 5. 列出目录（L0）────────────────────────────────────────────────────────

pub async fn list_dir(dir: &str) -> ToolResult {
    use std::path::Path;

    let path = Path::new(dir);
    if !path.is_dir() {
        return ToolResult::err("list_dir", format!("不是有效目录: {dir}"));
    }

    let entries = match std::fs::read_dir(path) {
        Ok(e)  => e,
        Err(e) => return ToolResult::err("list_dir", format!("无法读取目录: {e}")),
    };

    let mut lines: Vec<String> = Vec::new();
    for entry in entries.flatten().take(50) {
        let name = entry.file_name().to_string_lossy().to_string();
        let is_dir = entry.path().is_dir();
        lines.push(if is_dir { format!("{}/", name) } else { name });
    }

    lines.sort();
    ToolResult::ok_with_level(
        "list_dir",
        format!("**{}**\n{}", path.display(), lines.join("\n")),
        ToolLevel::L0,
    )
}
