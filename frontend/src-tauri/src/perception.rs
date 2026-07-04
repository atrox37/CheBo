// ─── perception.rs ───────────────────────────────────────────────────────────
// P1: 环境感知系统
//   1. 活跃窗口监听（Windows 原生 API）— 感知用户正在做什么
//   2. 剪贴板监听（arboard）— 检测代码/错误等有价值内容
//   3. 空闲检测（基于 AgentRuntime.idle_secs()）— 超时切入 Sleeping
// ─────────────────────────────────────────────────────────────────────────────

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::agent::{AgentRuntime, AgentState};
use crate::event_bus::{AgentEvent, EventBus};

// ─── Windows 活跃窗口标题 ────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn get_active_window_title() -> Option<String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW,
    };
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd == 0 {
            return None;
        }
        let len = GetWindowTextLengthW(hwnd);
        if len == 0 {
            return None;
        }
        let mut buf = vec![0u16; (len + 2) as usize];
        let written = GetWindowTextW(hwnd, buf.as_mut_ptr(), len + 1);
        if written == 0 {
            return None;
        }
        String::from_utf16(&buf[..written as usize]).ok()
    }
}

#[cfg(not(target_os = "windows"))]
fn get_active_window_title() -> Option<String> {
    None
}

// ─── 应用类别分类 ─────────────────────────────────────────────────────────────

fn classify_app(title: &str) -> &'static str {
    let t = title.to_lowercase();
    if t.contains("code")
        || t.contains("cursor")
        || t.contains("intellij")
        || t.contains("vim")
        || t.contains("nvim")
        || t.contains("terminal")
        || t.contains("powershell")
        || t.contains("cmd")
        || t.contains("git bash")
        || t.contains("rust")
        || t.contains("python")
    {
        "coding"
    } else if t.contains("chrome")
        || t.contains("firefox")
        || t.contains("edge")
        || t.contains("safari")
        || t.contains("browser")
    {
        "browsing"
    } else if t.contains("word")
        || t.contains("excel")
        || t.contains("powerpoint")
        || t.contains("notion")
        || t.contains("obsidian")
        || t.contains("typora")
        || t.contains("docs")
    {
        "office"
    } else if t.contains("steam") || t.contains("game") {
        "gaming"
    } else {
        "other"
    }
}

// ─── 判断剪贴板内容是否值得关注 ──────────────────────────────────────────────

fn is_interesting_clipboard(text: &str) -> bool {
    let t = text.trim();
    if t.len() < 20 {
        return false;
    }
    // 代码 / 错误 / URL 特征
    t.contains("error") || t.contains("Error")
        || t.contains("Exception") || t.contains("panic")
        || t.contains("fn ") || t.contains("def ")
        || t.contains("class ") || t.contains("import ")
        || t.contains("const ") || t.contains("async ")
        || t.starts_with("http") || t.contains("TODO")
        || t.contains("FIXME") || t.contains("BUG")
}

// ─── 感知事件的前端 payload ───────────────────────────────────────────────────

#[derive(Serialize, Clone)]
pub struct PerceptionPayload {
    pub kind:     String,
    pub data:     String,
    pub category: Option<String>,
}

// ─── Batch E: 实时感知状态（供 ProactiveGuard 访问）─────────────────────────

/// 当前用户环境感知快照（供主动发言节流使用）
#[derive(Debug, Clone, Default)]
pub struct PerceptionState {
    /// 当前活跃应用分类：coding / browsing / meeting / gaming / office / other
    pub app_type:      String,
    /// 是否检测到全屏（gaming / fullscreen 类应用）
    pub is_fullscreen: bool,
    /// 是否在会议类应用中（zoom / teams / meet）
    pub is_meeting:    bool,
    /// 用户最近一次感知到的空闲秒数
    pub idle_secs:     u64,
}

/// 检测是否全屏应用（gaming / 全屏视频等）
fn is_likely_fullscreen(app_type: &str, title: &str) -> bool {
    let t = title.to_lowercase();
    app_type == "gaming"
        || t.contains("fullscreen")
        || t.contains("全屏")
        || t.contains("presentation")
}

/// 检测是否会议类应用
fn is_meeting_app(title: &str) -> bool {
    let t = title.to_lowercase();
    t.contains("zoom") || t.contains("teams") || t.contains("meet")
        || t.contains("会议") || t.contains("webex") || t.contains("腾讯会议")
}

// ─── 主感知循环 ───────────────────────────────────────────────────────────────

pub async fn start_perception_loop(
    app:             AppHandle,
    event_bus:       Arc<EventBus>,
    agent:           Arc<AgentRuntime>,
    last_window:     Arc<Mutex<String>>,
    last_clipboard:  Arc<Mutex<String>>,
    // Batch E: 实时感知状态，供 ProactiveGuard 读取
    perception_state: Arc<Mutex<PerceptionState>>,
) {
    // 启动延迟：等待应用完全初始化
    tokio::time::sleep(Duration::from_secs(5)).await;

    let mut interval   = tokio::time::interval(Duration::from_secs(3));
    let mut tick_count: u32 = 0;

    loop {
        interval.tick().await;
        tick_count = tick_count.wrapping_add(1);

        // ── 1. 活跃窗口检测（每 3 秒） ──────────────────────────────────────
        if let Some(title) = get_active_window_title() {
            let tl = title.to_lowercase();
            if !tl.contains("chebo") && !tl.contains("erii") && !tl.is_empty() {
                let mut prev = last_window.lock().unwrap();
                if *prev != title {
                    let new_cat = classify_app(&title);
                    let old_cat = classify_app(&prev);
                    let old_is_empty = prev.is_empty();
                    *prev = title.clone();
                    drop(prev);

                    // 更新感知状态快照（Batch E）
                    {
                        let mut ps = perception_state.lock().unwrap();
                        ps.app_type      = new_cat.to_string();
                        ps.is_fullscreen = is_likely_fullscreen(new_cat, &title);
                        ps.is_meeting    = is_meeting_app(&title);
                        ps.idle_secs     = agent.idle_secs();
                    }

                    // 只在分类发生切换时通知（减少噪音）
                    if old_is_empty || old_cat != new_cat {
                        let snippet = &title[..title.len().min(80)];
                        let payload = PerceptionPayload {
                            kind:     "window_switch".to_string(),
                            data:     snippet.to_string(),
                            category: Some(new_cat.to_string()),
                        };
                        let _ = app.emit("perception_event", &payload);
                        // 发布 PerceptionChanged 事件（Batch B）
                        event_bus.publish(AgentEvent::PerceptionChanged {
                            app_type:     new_cat.to_string(),
                            idle_secs:    agent.idle_secs(),
                            is_fullscreen: is_likely_fullscreen(new_cat, &title),
                        });
                        event_bus.publish(AgentEvent::Perception {
                            kind: "window_switch".to_string(),
                            data: format!("{}:{}", new_cat, snippet),
                        });
                        log::debug!("Perception: 窗口切换 [{new_cat}] {snippet}");
                    }
                }
            }
        }

        // ── 2. 剪贴板监听（每 5 tick = 每 15 秒）────────────────────────────
        if tick_count % 5 == 0 {
            if let Ok(mut board) = arboard::Clipboard::new() {
                if let Ok(text) = board.get_text() {
                    let mut prev = last_clipboard.lock().unwrap();
                    if *prev != text && is_interesting_clipboard(&text) {
                        *prev = text.clone();
                        drop(prev);

                        let snippet = text[..text.len().min(120)].to_string();
                        let payload = PerceptionPayload {
                            kind:     "clipboard".to_string(),
                            data:     snippet.clone(),
                            category: None,
                        };
                        let _ = app.emit("perception_event", &payload);
                        event_bus.publish(AgentEvent::Perception {
                            kind: "clipboard".to_string(),
                            data: snippet,
                        });
                        log::debug!("Perception: 剪贴板变化，长度={}", text.len());
                    }
                }
            }
        }

        // ── 3. 空闲 / 睡眠检测（每 20 tick = 每 60 秒）─────────────────────
        if tick_count % 20 == 0 {
            let idle = agent.idle_secs();
            match agent.current() {
                AgentState::Idle if idle > 600 => {
                    agent.set_sleeping(&app);
                    log::info!("AgentState → Sleeping（idle {}s）", idle);
                }
                AgentState::Sleeping if idle < 30 => {
                    // 检测到最近活动 → 唤醒
                    agent.set_idle(&app);
                    log::info!("AgentState → Idle（wake from sleep）");
                }
                _ => {}
            }
        }
    }
}
