// ─── tool_registry.rs ─────────────────────────────────────────────────────────
// Tool Registry：统一注册表
#![allow(dead_code)]
//
// 工具注册 → 模型可见 → 执行（参考 OpenHuman all_tools_with_runtime）
//
// 工具列表（按类别）：
//   系统工具:  read_file, list_dir, safe_shell, git_status
//   网络工具:  web_search
//   内存工具:  memory_recall（从 Vault 和 SQLite 检索记忆）
//
// 每个工具都实现 Tool trait，Tool Registry 统一持有。
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;
use sqlx::SqlitePool;

use crate::intent_router;
use crate::llm::LlmConfig;
use crate::memory_vector;
use crate::sandbox::SandboxPolicy;
use crate::tool_trait::{Tool, ToolCallResult, ToolParam, ToolPermission};

// ─── Registry 主体 ────────────────────────────────────────────────────────────

pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: HashMap::new() }
    }

    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn get(&self, name: &str) -> Option<&Arc<dyn Tool>> {
        self.tools.get(name)
    }

    pub fn names(&self) -> Vec<&str> {
        self.tools.keys().map(|s| s.as_str()).collect()
    }

    /// 生成注入 system prompt 的工具说明文本（全量）
    pub fn tools_prompt_block(&self) -> String {
        self.build_prompt_block(self.tools.values().map(Arc::clone).collect())
    }

    /// 根据用户消息语义，智能选择需要注入的工具（三级意图路由）
    ///
    /// - 如果路由器有明确建议 → 只注入相关工具（减少 token 消耗）
    /// - 如果没有明确建议（Fallback）→ 注入全量工具，让 LLM 自决
    pub fn tools_prompt_block_for(&self, message: &str, recent_tools: &[String]) -> String {
        let hint = intent_router::route(message, recent_tools);

        if hint.use_all_tools() {
            // Fallback：全量工具
            return self.tools_prompt_block();
        }

        // 过滤出命中的工具（合并 hint + 始终可见的基础工具）
        let always_visible = ["memory_recall"]; // 记忆工具总是可见
        let relevant: Vec<Arc<dyn Tool>> = self.tools
            .iter()
            .filter(|(name, _)| {
                hint.tools.iter().any(|h| h == name.as_str())
                    || always_visible.contains(&name.as_str())
            })
            .map(|(_, t)| Arc::clone(t))
            .collect();

        if relevant.is_empty() {
            return String::new(); // 纯聊天，不注入任何工具
        }

        self.build_prompt_block(relevant)
    }

    fn build_prompt_block(&self, tools: Vec<Arc<dyn Tool + 'static>>) -> String {
        let mut lines: Vec<String> = tools
            .iter()
            .map(|t| t.spec().one_line())
            .collect();
        lines.sort();

        format!(
            "## 可用工具\n\
             当你需要查询信息或执行操作时，可以调用以下工具。\n\
             调用格式：\n\
             <tool_call>\n\
             {{\"name\":\"工具名\",\"arguments\":{{\"参数名\":\"参数值\"}}}}\n\
             </tool_call>\n\
             工具执行完成后结果会出现在对话中，你可以基于结果继续回答。\n\n\
             {}\n",
            lines.join("\n")
        )
    }

    /// 生成 OpenAI native function calling 的 tools 数组
    pub fn to_openai_tools(&self) -> Vec<Value> {
        self.tools.values().map(|t| t.spec().to_openai_function()).collect()
    }
}

// ─── 内置工具初始化入口 ───────────────────────────────────────────────────────

/// 注册所有内置工具（在 AppState 初始化时调用）
pub fn build_registry(pool: SqlitePool, data_dir: PathBuf, sandbox: Arc<SandboxPolicy>, llm_cfg: Arc<LlmConfig>) -> ToolRegistry {
    let mut reg = ToolRegistry::new();

    // 系统工具（传入 sandbox）
    reg.register(Arc::new(ReadFileTool { sandbox: sandbox.clone() }));
    reg.register(Arc::new(ListDirTool));
    reg.register(Arc::new(SafeShellTool { sandbox: sandbox.clone() }));
    reg.register(Arc::new(GitStatusTool));

    // 网络工具
    reg.register(Arc::new(WebSearchTool));

    // 记忆工具（语义向量检索 + 关键词回退）
    reg.register(Arc::new(MemoryRecallTool { pool, llm_cfg }));

    // 剪贴板工具
    reg.register(Arc::new(ClipboardReadTool));

    // 截图工具（L1 权限，需用户同意）
    reg.register(Arc::new(ScreenshotTool));

    let _ = data_dir;
    reg
}

// ─── 具体工具实现 ─────────────────────────────────────────────────────────────

// ── ReadFileTool ──────────────────────────────────────────────────────────────

pub struct ReadFileTool {
    pub sandbox: Arc<SandboxPolicy>,
}

#[async_trait::async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str { "read_file" }
    fn description(&self) -> &str { "读取本地文件内容（前 200 行），适合查看代码、配置文件等" }
    fn permission_level(&self) -> ToolPermission { ToolPermission::L0 }
    fn params(&self) -> Vec<ToolParam> {
        vec![ToolParam {
            name: "path".into(),
            ty: "string".into(),
            description: "文件绝对路径或相对路径".into(),
            required: true,
        }]
    }

    async fn execute(&self, id: &str, args: Value) -> ToolCallResult {
        let path_str = match args.get("path").and_then(|v| v.as_str()) {
            Some(p) => p.to_string(),
            None => return ToolCallResult::err(id, self.name(), self.permission_level(), "缺少 path 参数"),
        };

        let path = PathBuf::from(&path_str);

        // 速率检查
        if let Err(msg) = self.sandbox.check_rate_limit(self.name()) {
            return ToolCallResult::err(id, self.name(), self.permission_level(), &msg);
        }
        // 路径沙盒检查
        if let Err(msg) = self.sandbox.check_file_access(self.name(), &path) {
            return ToolCallResult::err(id, self.name(), self.permission_level(), &msg);
        }

        match std::fs::read_to_string(&path) {
            Ok(content) => {
                let lines: Vec<&str> = content.lines().take(200).collect();
                let output = lines.join("\n");
                let suffix = if content.lines().count() > 200 {
                    format!("\n…（仅显示前 200 行，共 {} 行）", content.lines().count())
                } else {
                    String::new()
                };
                ToolCallResult::ok(id, self.name(), self.permission_level(), format!("{output}{suffix}"))
            }
            Err(e) => ToolCallResult::err(id, self.name(), self.permission_level(), &e.to_string()),
        }
    }
}

// ── ListDirTool ───────────────────────────────────────────────────────────────

pub struct ListDirTool;

#[async_trait::async_trait]
impl Tool for ListDirTool {
    fn name(&self) -> &str { "list_dir" }
    fn description(&self) -> &str { "列出目录内容，查看文件结构" }
    fn permission_level(&self) -> ToolPermission { ToolPermission::L0 }
    fn params(&self) -> Vec<ToolParam> {
        vec![ToolParam {
            name: "dir".into(),
            ty: "string".into(),
            description: "目录路径（\".\" 表示当前目录）".into(),
            required: true,
        }]
    }

    async fn execute(&self, id: &str, args: Value) -> ToolCallResult {
        let dir = match args.get("dir").and_then(|v| v.as_str()) {
            Some(d) => d.to_string(),
            None => ".".to_string(),
        };

        match std::fs::read_dir(&dir) {
            Ok(entries) => {
                let mut lines: Vec<String> = entries
                    .filter_map(|e| e.ok())
                    .map(|e| {
                        let name = e.file_name().to_string_lossy().to_string();
                        if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                            format!("📁 {}/", name)
                        } else {
                            format!("📄 {}", name)
                        }
                    })
                    .collect();
                lines.sort();
                ToolCallResult::ok(id, self.name(), self.permission_level(), lines.join("\n"))
            }
            Err(e) => ToolCallResult::err(id, self.name(), self.permission_level(), &e.to_string()),
        }
    }
}

// ── SafeShellTool ─────────────────────────────────────────────────────────────

pub struct SafeShellTool {
    pub sandbox: Arc<SandboxPolicy>,
}

#[async_trait::async_trait]
impl Tool for SafeShellTool {
    fn name(&self) -> &str { "safe_shell" }
    fn description(&self) -> &str {
        "执行安全的 shell 命令（仅允许 git/ls/pwd/cat/echo/date/cargo/pnpm 等只读命令）"
    }
    fn permission_level(&self) -> ToolPermission { ToolPermission::L3 }
    fn params(&self) -> Vec<ToolParam> {
        vec![ToolParam {
            name: "cmd".into(),
            ty: "string".into(),
            description: "要执行的命令".into(),
            required: true,
        }]
    }

    async fn execute(&self, id: &str, args: Value) -> ToolCallResult {
        let cmd = match args.get("cmd").and_then(|v| v.as_str()) {
            Some(c) => c.to_string(),
            None => return ToolCallResult::err(id, self.name(), self.permission_level(), "缺少 cmd 参数"),
        };

        // 速率检查
        if let Err(msg) = self.sandbox.check_rate_limit(self.name()) {
            return ToolCallResult::err(id, self.name(), self.permission_level(), &msg);
        }
        // 命令安全检查（sandbox 含审计日志）
        if let Err(msg) = self.sandbox.check_command(&cmd) {
            return ToolCallResult::err(id, self.name(), self.permission_level(), &msg);
        }

        let parts: Vec<&str> = cmd.split_whitespace().collect();
        if parts.is_empty() {
            return ToolCallResult::err(id, self.name(), self.permission_level(), "命令为空");
        }

        match std::process::Command::new(parts[0]).args(&parts[1..]).output() {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                let combined = if stderr.is_empty() {
                    stdout
                } else {
                    format!("{stdout}\n[stderr]\n{stderr}")
                };
                ToolCallResult::ok(id, self.name(), self.permission_level(), combined)
            }
            Err(e) => ToolCallResult::err(id, self.name(), self.permission_level(), &e.to_string()),
        }
    }
}

// ── GitStatusTool ─────────────────────────────────────────────────────────────

pub struct GitStatusTool;

#[async_trait::async_trait]
impl Tool for GitStatusTool {
    fn name(&self) -> &str { "git_status" }
    fn description(&self) -> &str { "查看 Git 仓库状态（当前分支、变更文件、最近提交）" }
    fn permission_level(&self) -> ToolPermission { ToolPermission::L1 }
    fn params(&self) -> Vec<ToolParam> {
        vec![ToolParam {
            name: "dir".into(),
            ty: "string".into(),
            description: "仓库路径（\".\" 表示当前目录）".into(),
            required: false,
        }]
    }

    async fn execute(&self, id: &str, args: Value) -> ToolCallResult {
        let dir = args.get("dir").and_then(|v| v.as_str()).unwrap_or(".").to_string();

        let mut parts = Vec::new();

        // git status
        if let Ok(out) = std::process::Command::new("git")
            .args(["-C", &dir, "status", "--short"])
            .output()
        {
            parts.push(format!("=== 变更文件 ===\n{}", String::from_utf8_lossy(&out.stdout)));
        }

        // git branch
        if let Ok(out) = std::process::Command::new("git")
            .args(["-C", &dir, "branch", "--show-current"])
            .output()
        {
            parts.push(format!("=== 当前分支 ===\n{}", String::from_utf8_lossy(&out.stdout).trim()));
        }

        // git log --oneline -5
        if let Ok(out) = std::process::Command::new("git")
            .args(["-C", &dir, "log", "--oneline", "-5"])
            .output()
        {
            parts.push(format!("=== 最近提交 ===\n{}", String::from_utf8_lossy(&out.stdout)));
        }

        ToolCallResult::ok(id, self.name(), self.permission_level(), parts.join("\n"))
    }
}

// ── WebSearchTool ─────────────────────────────────────────────────────────────

pub struct WebSearchTool;

#[async_trait::async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str { "web_search" }
    fn description(&self) -> &str { "通过 DuckDuckGo 搜索网络信息" }
    fn permission_level(&self) -> ToolPermission { ToolPermission::L0 }
    fn params(&self) -> Vec<ToolParam> {
        vec![ToolParam {
            name: "query".into(),
            ty: "string".into(),
            description: "搜索关键词".into(),
            required: true,
        }]
    }

    async fn execute(&self, id: &str, args: Value) -> ToolCallResult {
        let query = match args.get("query").and_then(|v| v.as_str()) {
            Some(q) => q.to_string(),
            None => return ToolCallResult::err(id, self.name(), self.permission_level(), "缺少 query 参数"),
        };

        let encoded = urlencoding::encode(&query);
        let url = format!("https://duckduckgo.com/html/?q={encoded}");

        match reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (compatible; Chebo/1.0)")
            .timeout(std::time::Duration::from_secs(10))
            .build()
        {
            Ok(client) => match client.get(&url).send().await {
                Ok(resp) => match resp.text().await {
                    Ok(html) => {
                        let snippets = extract_ddg_snippets(&html, 5);
                        if snippets.is_empty() {
                            ToolCallResult::ok(
                                id, self.name(), self.permission_level(),
                                format!("未找到 \"{query}\" 的搜索结果"),
                            )
                        } else {
                            ToolCallResult::ok(
                                id, self.name(), self.permission_level(),
                                format!("搜索 \"{}\" 的结果：\n\n{}", query, snippets.join("\n\n")),
                            )
                        }
                    }
                    Err(e) => ToolCallResult::err(id, self.name(), self.permission_level(), &e.to_string()),
                },
                Err(e) => ToolCallResult::err(id, self.name(), self.permission_level(), &e.to_string()),
            },
            Err(e) => ToolCallResult::err(id, self.name(), self.permission_level(), &e.to_string()),
        }
    }
}

fn extract_ddg_snippets(html: &str, max: usize) -> Vec<String> {
    let mut results = Vec::new();
    let mut pos = 0;
    while results.len() < max {
        if let Some(start) = html[pos..].find("class=\"result__snippet\"") {
            let abs = pos + start;
            if let Some(tag_end) = html[abs..].find('>') {
                let content_start = abs + tag_end + 1;
                if let Some(close) = html[content_start..].find("</a>") {
                    let raw = &html[content_start..content_start + close];
                    let cleaned = raw
                        .replace("<b>", "")
                        .replace("</b>", "")
                        .replace("&amp;", "&")
                        .replace("&lt;", "<")
                        .replace("&gt;", ">")
                        .replace("&quot;", "\"")
                        .replace("&#x27;", "'");
                    let cleaned = cleaned.trim().to_string();
                    if !cleaned.is_empty() {
                        results.push(cleaned);
                    }
                    pos = content_start + close + 4;
                    continue;
                }
            }
        }
        break;
    }
    results
}

// ── MemoryRecallTool ──────────────────────────────────────────────────────────

pub struct MemoryRecallTool {
    pub pool:     SqlitePool,
    pub llm_cfg:  Arc<LlmConfig>,
}

#[async_trait::async_trait]
impl Tool for MemoryRecallTool {
    fn name(&self) -> &str { "memory_recall" }
    fn description(&self) -> &str {
        "从 Chebo 的长期记忆中检索信息（对话摘要、用户习惯、关系记忆等）"
    }
    fn permission_level(&self) -> ToolPermission { ToolPermission::L0 }
    fn params(&self) -> Vec<ToolParam> {
        vec![ToolParam {
            name: "query".into(),
            ty: "string".into(),
            description: "要检索的关键词或问题".into(),
            required: true,
        }]
    }

    async fn execute(&self, id: &str, args: Value) -> ToolCallResult {
        let query = match args.get("query").and_then(|v| v.as_str()) {
            Some(q) => q.to_string(),
            None => return ToolCallResult::err(id, self.name(), self.permission_level(), "缺少 query 参数"),
        };

        let mut results: Vec<String> = Vec::new();
        let mut used_semantic = false;

        // P0: 优先语义向量检索
        match memory_vector::recall_semantic(&self.pool, &self.llm_cfg, &query, 8).await {
            Ok(hits) if !hits.is_empty() => {
                used_semantic = true;
                results = hits
                    .iter()
                    .map(|h| format!("{} (相关度 {:.0}%)", h.content, h.score * 100.0))
                    .collect();
            }
            Ok(_) => {}
            Err(e) => {
                log::debug!("memory_recall semantic fallback: {e}");
            }
        }

        // 向量不可用或无命中时回退关键词检索
        if results.is_empty() {
            results = memory_vector::recall_keyword(&self.pool, &query).await;
        }

        if results.is_empty() {
            ToolCallResult::ok(
                id, self.name(), self.permission_level(),
                format!("没有找到关于 \"{}\" 的记忆", query),
            )
        } else {
            let header = if used_semantic {
                format!("关于 \"{}\" 的语义相关记忆：", query)
            } else {
                format!("关于 \"{}\" 的记忆：", query)
            };
            ToolCallResult::ok(
                id, self.name(), self.permission_level(),
                format!("{header}\n\n{}", results.join("\n")),
            )
        }
    }
}

// ── ClipboardReadTool ─────────────────────────────────────────────────────────

pub struct ClipboardReadTool;

#[async_trait::async_trait]
impl Tool for ClipboardReadTool {
    fn name(&self) -> &str { "clipboard_read" }
    fn description(&self) -> &str { "读取用户当前剪贴板内容" }
    fn permission_level(&self) -> ToolPermission { ToolPermission::L1 }
    fn params(&self) -> Vec<ToolParam> { vec![] }

    async fn execute(&self, id: &str, _args: Value) -> ToolCallResult {
        match arboard::Clipboard::new().and_then(|mut c| c.get_text()) {
            Ok(text) if !text.is_empty() => {
                ToolCallResult::ok(id, self.name(), self.permission_level(), text)
            }
            Ok(_) => ToolCallResult::ok(
                id, self.name(), self.permission_level(), "（剪贴板为空）".to_string(),
            ),
            Err(e) => ToolCallResult::err(id, self.name(), self.permission_level(), &e.to_string()),
        }
    }
}

// ─── 工具函数 ─────────────────────────────────────────────────────────────────

fn is_forbidden_path(path: &PathBuf) -> bool {
    let p = path.to_string_lossy().to_lowercase();
    let forbidden = ["/etc/", "/sys/", "/proc/", "c:\\windows\\system32\\"];
    forbidden.iter().any(|f| p.starts_with(f))
}

fn is_dangerous_cmd(cmd: &str) -> bool {
    let lower = cmd.to_lowercase();
    let dangerous = ["rm ", "del ", "rmdir", "rd ", "sudo", "format", "mkfs",
                     ":(){", "dd if", "> /dev", "shutdown", "reboot", "halt"];
    dangerous.iter().any(|d| lower.contains(d))
}

// ── ScreenshotTool ────────────────────────────────────────────────────────────

pub struct ScreenshotTool;

#[async_trait::async_trait]
impl Tool for ScreenshotTool {
    fn name(&self) -> &str { "take_screenshot" }
    fn description(&self) -> &str {
        "截取当前屏幕截图，以 base64 PNG 格式返回。适合需要查看用户当前屏幕内容时使用。\
         如果 LLM 支持视觉能力，可以分析图片内容；否则返回截图的保存路径。"
    }
    fn permission_level(&self) -> ToolPermission { ToolPermission::L1 }
    fn params(&self) -> Vec<ToolParam> {
        vec![ToolParam {
            name: "monitor".into(),
            ty: "number".into(),
            description: "显示器索引（从 0 开始），默认 0（主屏幕）".into(),
            required: false,
        }]
    }

    async fn execute(&self, id: &str, args: Value) -> ToolCallResult {
        use base64::Engine as _;

        let monitor_idx = args.get("monitor")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        // 在阻塞线程中执行截图（避免阻塞 tokio 运行时）
        let result = tokio::task::spawn_blocking(move || {
            let screens = screenshots::Screen::all()
                .map_err(|e| format!("获取显示器列表失败: {e}"))?;

            let screen = screens.get(monitor_idx)
                .or_else(|| screens.first())
                .ok_or_else(|| "未找到任何显示器".to_string())?;

            let image = screen.capture()
                .map_err(|e| format!("截图失败: {e}"))?;

            // 保存到临时目录
            let tmp_path = std::env::temp_dir().join(
                format!("chebo_screenshot_{}.png",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs())
            );

            // 转为 DynamicImage 后 save（自动推断 PNG 格式）
            let dyn_img = screenshots::image::DynamicImage::ImageRgba8(image);
            dyn_img.save(&tmp_path)
                .map_err(|e| format!("保存截图失败: {e}"))?;

            let png_bytes = std::fs::read(&tmp_path)
                .map_err(|e| format!("读取截图文件失败: {e}"))?;

            let b64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);
            let size_kb = png_bytes.len() / 1024;

            Ok::<String, String>(format!(
                "截图已完成（{}×{} px，{} KB）\n保存路径：{}\ndata:image/png;base64,{}",
                screen.display_info.width,
                screen.display_info.height,
                size_kb,
                tmp_path.display(),
                &b64[..b64.len().min(200)],  // 只展示前200字符，避免超出 MAX_OUTPUT
            ))
        }).await;

        match result {
            Ok(Ok(output)) => ToolCallResult::ok(id, self.name(), self.permission_level(), output),
            Ok(Err(msg))   => ToolCallResult::err(id, self.name(), self.permission_level(), &msg),
            Err(e)         => ToolCallResult::err(id, self.name(), self.permission_level(), &e.to_string()),
        }
    }
}
