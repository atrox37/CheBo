// ─── sandbox.rs ───────────────────────────────────────────────────────────────
// Chebo 工具沙盒策略
//
// 三层防护：
//   1. Path Allowlist  — 文件读写必须在允许的目录前缀下
//   2. Command Denylist — Shell 命令不能含有危险关键词
//   3. Rate Limiter    — 每个工具每小时最多调用 N 次（滑动窗口）
//
// 附带：轻量级内存审计日志（最近 200 条，可通过 get_audit_log 命令查看）
// ─────────────────────────────────────────────────────────────────────────────
#![allow(dead_code)]

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};
use std::time::Instant;

use serde::{Deserialize, Serialize};

// ─── 审计记录 ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub tool:      String,
    pub action:    String,         // 调用的描述，如 "exec cmd: ls -la"
    pub allowed:   bool,
    pub reason:    Option<String>, // 拒绝时的原因
    pub timestamp: u64,            // unix timestamp (secs)
}

// ─── 策略主体 ─────────────────────────────────────────────────────────────────

pub struct SandboxPolicy {
    /// 总开关：false 时所有检查直接通过
    pub enabled: bool,

    /// 允许访问的路径前缀（RwLock 包裹，支持运行时热更新）
    pub allowed_paths: RwLock<Vec<PathBuf>>,

    /// Shell 命令禁止词（不区分大小写，子串匹配）
    pub denied_cmd_patterns: Vec<String>,

    /// 每个工具的速率上限（calls / rate_window_secs）
    pub rate_limits: HashMap<String, u32>,

    /// 速率窗口大小（默认 3600 秒 = 1 小时）
    pub rate_window_secs: u64,

    // ── 内部状态 ──────────────────────────────────────────────────────────────
    rate_tracker: Mutex<HashMap<String, VecDeque<Instant>>>,
    audit_log:    Mutex<VecDeque<AuditEntry>>,
}

impl Default for SandboxPolicy {
    fn default() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));

        let mut rate_limits = HashMap::new();
        rate_limits.insert("web_search".to_string(),    30);
        rate_limits.insert("safe_shell".to_string(),    10);
        rate_limits.insert("read_file".to_string(),     60);
        rate_limits.insert("list_dir".to_string(),      60);
        rate_limits.insert("clipboard_read".to_string(),30);
        rate_limits.insert("git_status".to_string(),    40);
        rate_limits.insert("memory_recall".to_string(), 40);

        Self {
            enabled: true,
            allowed_paths: RwLock::new(vec![
                home.join("Documents"),
                home.join("Desktop"),
                home.join("Downloads"),
                home.join("Projects"),
                home.join("dev"),
                home.join("code"),
            ]),
            denied_cmd_patterns: vec![
                "rm -rf".into(), "rmdir /s".into(), "rd /s".into(),
                "del /f".into(), "format ".into(), "mkfs".into(),
                "dd if=".into(),  ":(){:|:&};:".into(),
                "shutdown".into(), "reboot".into(), "halt".into(),
                "sudo rm".into(), "> /dev/".into(), "chmod 777".into(),
                "curl | sh".into(), "wget | sh".into(), "curl | bash".into(),
            ],
            rate_limits,
            rate_window_secs: 3600,
            rate_tracker: Mutex::new(HashMap::new()),
            audit_log:    Mutex::new(VecDeque::with_capacity(200)),
        }
    }
}

impl SandboxPolicy {
    // ── 路径检查 ─────────────────────────────────────────────────────────────

    /// 检查文件路径是否在允许范围内
    pub fn check_file_access(&self, tool: &str, path: &Path) -> Result<(), String> {
        let path_str = path.to_string_lossy().to_string();

        // 系统关键路径硬拒绝（无论策略是否开启）
        if is_system_path(path) {
            let reason = format!("系统保护路径 {path_str} 不可访问");
            self.record_audit(tool, &format!("read {path_str}"), false, Some(&reason));
            return Err(reason);
        }

        let paths = self.allowed_paths.read().unwrap();
        if !self.enabled || paths.is_empty() {
            self.record_audit(tool, &format!("read {path_str}"), true, None);
            return Ok(());
        }

        // 尝试规范化路径
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

        for allowed in paths.iter() {
            let canon_allowed = allowed.canonicalize().unwrap_or_else(|_| allowed.clone());
            if canonical.starts_with(&canon_allowed) {
                self.record_audit(tool, &format!("read {path_str}"), true, None);
                return Ok(());
            }
        }

        let allowed_list = paths.iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        drop(paths);
        let reason = format!(
            "路径 {path_str} 不在允许范围内。\n允许访问的目录：{allowed_list}"
        );
        self.record_audit(tool, &format!("read {path_str}"), false, Some(&reason));
        Err(reason)
    }

    // ── Shell 命令检查 ────────────────────────────────────────────────────────

    /// 检查 Shell 命令是否包含危险模式
    pub fn check_command(&self, cmd: &str) -> Result<(), String> {
        // 硬拒绝：无论策略是否开启
        let lower = cmd.to_lowercase();
        let hard_deny = ["rm -rf /", "del /f /s /q c:\\", "format c:"];
        for d in &hard_deny {
            if lower.contains(d) {
                let reason = format!("命令包含极度危险的操作 \"{d}\"，已硬性拒绝");
                self.record_audit("safe_shell", cmd, false, Some(&reason));
                return Err(reason);
            }
        }

        if !self.enabled {
            self.record_audit("safe_shell", cmd, true, None);
            return Ok(());
        }

        for pattern in &self.denied_cmd_patterns {
            if lower.contains(&pattern.to_lowercase()) {
                let reason = format!("命令包含禁止的操作模式: \"{pattern}\"");
                self.record_audit("safe_shell", cmd, false, Some(&reason));
                return Err(reason);
            }
        }

        self.record_audit("safe_shell", cmd, true, None);
        Ok(())
    }

    // ── 速率限制 ─────────────────────────────────────────────────────────────

    /// 检查并记录工具调用速率（令牌桶 / 滑动窗口）
    pub fn check_rate_limit(&self, tool: &str) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }

        let limit = match self.rate_limits.get(tool) {
            Some(&l) => l,
            None => return Ok(()), // 没有配置限制 = 不限
        };

        let window = std::time::Duration::from_secs(self.rate_window_secs);
        let now = Instant::now();

        let mut tracker = self.rate_tracker.lock().unwrap();
        let calls = tracker.entry(tool.to_string()).or_insert_with(VecDeque::new);

        // 清理过期记录
        while let Some(&front) = calls.front() {
            if now.duration_since(front) > window {
                calls.pop_front();
            } else {
                break;
            }
        }

        if calls.len() >= limit as usize {
            let remaining = calls.front()
                .map(|&t| window.checked_sub(now.duration_since(t)).unwrap_or_default())
                .unwrap_or_default();
            let reason = format!(
                "工具 {tool} 调用次数已达上限（{limit} 次/{} 分钟），\
                 请 {:.0} 秒后重试",
                self.rate_window_secs / 60,
                remaining.as_secs_f32()
            );
            return Err(reason);
        }

        calls.push_back(now);
        Ok(())
    }

    // ── 审计日志 ─────────────────────────────────────────────────────────────

    fn record_audit(&self, tool: &str, action: &str, allowed: bool, reason: Option<&str>) {
        let entry = AuditEntry {
            tool:    tool.to_string(),
            action:  action.to_string(),
            allowed,
            reason:  reason.map(|s| s.to_string()),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };

        let mut log = self.audit_log.lock().unwrap();
        if log.len() >= 200 {
            log.pop_front();
        }
        log.push_back(entry);
    }

    /// 获取最近 N 条审计记录
    pub fn recent_audit(&self, n: usize) -> Vec<AuditEntry> {
        let log = self.audit_log.lock().unwrap();
        log.iter().rev().take(n).cloned().collect()
    }

    /// 获取被拒绝的操作（用于 UI 安全面板）
    pub fn denied_audit(&self, n: usize) -> Vec<AuditEntry> {
        let log = self.audit_log.lock().unwrap();
        log.iter().rev()
            .filter(|e| !e.allowed)
            .take(n)
            .cloned()
            .collect()
    }

    /// 获取当前允许路径列表（String 形式，供前端展示）
    pub fn get_allowed_paths(&self) -> Vec<String> {
        self.allowed_paths.read().unwrap()
            .iter()
            .map(|p| p.display().to_string())
            .collect()
    }

    /// 替换允许路径列表（运行时热更新，立即生效）
    pub fn set_allowed_paths(&self, paths: Vec<String>) {
        let mut lock = self.allowed_paths.write().unwrap();
        *lock = paths.into_iter().map(PathBuf::from).collect();
    }
}

// ─── 辅助函数 ─────────────────────────────────────────────────────────────────

fn is_system_path(path: &Path) -> bool {
    let p = path.to_string_lossy().to_lowercase();
    let system_prefixes = [
        "/etc/", "/sys/", "/proc/", "/dev/", "/boot/",
        "c:\\windows\\", "c:\\program files\\",
        "/private/etc/", "/private/var/",
    ];
    system_prefixes.iter().any(|prefix| p.starts_with(prefix))
}
