// ─── tray.rs ─────────────────────────────────────────────────────────────────
// P2: 系统托盘 + 全局快捷键
//   - 系统托盘图标（左键切换显示/隐藏，右键菜单）
//   - 关闭按钮 → 最小化到托盘而非退出
//   - 全局快捷键 Ctrl+Shift+Space 唤醒 Chebo
// ─────────────────────────────────────────────────────────────────────────────

use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, WebviewWindow,
};

// ─── 帮助函数 ──────────────────────────────────────────────────────────────────

/// 切换主窗口显示/隐藏
pub fn toggle_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let visible = win.is_visible().unwrap_or(false);
        if visible {
            let _ = win.hide();
        } else {
            let _ = win.show();
            let _ = win.set_focus();
        }
    }
}

/// 初始化系统托盘
pub fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    // ── 菜单项 ────────────────────────────────────────────────────────────────
    let show_item      = MenuItem::with_id(app, "show",      "显示 Chebo",     true, None::<&str>)?;
    let hide_item      = MenuItem::with_id(app, "hide",      "隐藏 Chebo",     true, None::<&str>)?;
    let assistant_item = MenuItem::with_id(app, "assistant", "切换到助手模式", true, None::<&str>)?;
    let sep1           = PredefinedMenuItem::separator(app)?;
    let reset_item     = MenuItem::with_id(app, "reset",     "重置状态",       true, None::<&str>)?;
    let sep2           = PredefinedMenuItem::separator(app)?;
    let about_item     = MenuItem::with_id(app, "about",     "关于 Chebo",     true, None::<&str>)?;
    let quit_item      = MenuItem::with_id(app, "quit",      "退出",           true, None::<&str>)?;

    let menu = Menu::with_items(app, &[
        &show_item, &hide_item, &assistant_item,
        &sep1, &reset_item,
        &sep2, &about_item, &quit_item,
    ])?;

    // ── 托盘图标 ──────────────────────────────────────────────────────────────
    TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("Chebo — 人格化 AI 桌宠")
        .menu(&menu)
        .show_menu_on_left_click(false) // 左键切换窗口，右键才显示菜单
        // ── 托盘图标点击事件 ──────────────────────────────────────────────────
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_window(tray.app_handle());
            }
        })
        // ── 菜单点击事件 ──────────────────────────────────────────────────────
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => {
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }
            "hide" => {
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.hide();
                }
            }
            "assistant" => {
                // 通知前端切换到助手模式
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.show();
                    let _ = win.set_focus();
                }
                let _ = app.emit("open_assistant", ());
            }
            "reset" => {
                // 发送重置信号给前端
                let _ = app.emit("tray_reset", ());
            }
            "about" => {
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;

    Ok(())
}

/// 为主窗口绑定"关闭 → 智能处理"行为
///
/// 规则：
///   - 窗口有装饰（助手模式）→ 发 switch_to_pet 事件，由前端切回桌宠模式
///   - 窗口无装饰（桌宠模式）→ 直接 hide 到托盘
pub fn setup_close_to_tray(window: &WebviewWindow) {
    let win_clone = window.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            // 判断当前是否有装饰（有装饰 = 助手模式）
            let has_decorations = win_clone.is_decorated().unwrap_or(false);
            if has_decorations {
                // 助手模式关闭 → 通知前端切回桌宠模式
                let _ = win_clone.show();
                let _ = win_clone.unminimize();
                let _ = win_clone.app_handle().emit("switch_to_pet", ());
            } else {
                // 桌宠模式关闭 → 最小化到托盘
                let _ = win_clone.hide();
            }
        }
    });
}
