// ─── lib.rs ───────────────────────────────────────────────────────────────────
// Tauri 应用入口
// P0: AgentRuntime 状态机 + EventBus
// P1: Perception 感知 + Tool System
// P2: 系统托盘 + 全局快捷键
// ─────────────────────────────────────────────────────────────────────────────

mod agent;
mod character;
mod chat_router;
mod commands;
mod db;
mod event_bus;
mod intent_router;
mod lib_state;
mod llm;
mod local_embed;
mod memory;
mod memory_tree;
mod memory_vector;
mod perception;
mod pet;
mod provider_registry;
mod sandbox;
mod task;
mod tool_dispatcher;
mod tool_registry;
mod tool_trait;
mod tools;
mod tray;
mod vault;
mod voice;

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use sqlx::SqlitePool;
use tauri::Manager;

use crate::agent::AgentRuntime;
use crate::commands::*;
use crate::event_bus::EventBus;
use crate::lib_state::{AppConfig, AppState};
use crate::llm::LlmConfig;

// ─── 平台数据目录 ─────────────────────────────────────────────────────────────

fn chebo_data_dir() -> std::path::PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("Chebo")
}

// ─── 从 SQLite config 表加载运行时配置 ────────────────────────────────────────

async fn load_runtime_config(pool: &SqlitePool, base: &mut LlmConfig) {
    if let Ok(Some(k)) = db::get_config(pool, "llm_api_key").await {
        if !k.is_empty() { base.api_key = k; }
    }
    if let Ok(Some(u)) = db::get_config(pool, "llm_base_url").await {
        if !u.is_empty() { base.base_url = u; }
    }
    if let Ok(Some(m)) = db::get_config(pool, "llm_model").await {
        if !m.is_empty() { base.model = m; }
    }
}

// ─── 应用入口 ─────────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // P2: 全局快捷键插件
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            // ── 1. 数据目录 ───────────────────────────────────────────────────
            let data_dir = chebo_data_dir();

            // ── 2. 加载配置 ───────────────────────────────────────────────────
            let config  = AppConfig::load_from(&data_dir);
            let mut llm_cfg = LlmConfig {
                api_key:     config.llm_api_key.clone(),
                base_url:    config.llm_base_url.clone(),
                model:       config.llm_model.clone(),
                temperature: config.temperature,
                max_tokens:  config.max_tokens,
            };

            // ── 3. 初始化 SQLite ──────────────────────────────────────────────
            let db_path = data_dir.join("chebo.db");
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| e.to_string())?;

            let (pool, vision_cfg_init, sandbox_paths_init) = rt.block_on(async {
                let pool = db::create_pool(&db_path).await?;
                db::init(&pool).await?;
                load_runtime_config(&pool, &mut llm_cfg).await;

                // 同步加载 vision 配置
                let vkey   = db::get_config(&pool, "vision_api_key").await.ok().flatten().unwrap_or_default();
                let vurl   = db::get_config(&pool, "vision_base_url").await.ok().flatten()
                    .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
                let vmodel = db::get_config(&pool, "vision_model").await.ok().flatten().unwrap_or_default();
                let vcfg = if vkey.is_empty() || vmodel.is_empty() {
                    None
                } else {
                    Some(crate::llm::LlmConfig {
                        api_key:     vkey,
                        base_url:    vurl,
                        model:       vmodel,
                        temperature: 0.3,
                        max_tokens:  800,
                    })
                };

                // 同步加载沙盒路径配置
                let sandbox_paths = db::get_config(&pool, "sandbox_allowed_paths").await.ok().flatten()
                    .filter(|s| !s.is_empty())
                    .map(|s| s.split("||").filter(|p| !p.is_empty()).map(|p| p.to_string()).collect::<Vec<_>>());

                Ok::<_, anyhow::Error>((pool, vcfg, sandbox_paths))
            })
            .map_err(|e: anyhow::Error| e.to_string())?;

            drop(rt);

            let llm_cfg_hot  = Arc::new(Mutex::new(llm_cfg.clone()));
            let llm_cfg_arc  = Arc::new(llm_cfg);
            let agent        = Arc::new(AgentRuntime::new());
            let event_bus    = Arc::new(EventBus::new());
            let last_window  = Arc::new(Mutex::new(String::new()));
            let last_clip    = Arc::new(Mutex::new(String::new()));
            // Batch C: 挂起工具调用表
            let pending_tools = Arc::new(Mutex::new(HashMap::new()));
            // Batch E: 实时感知状态（供 ProactiveGuard 读取）
            let perception_state = Arc::new(Mutex::new(
                crate::perception::PerceptionState::default()
            ));
            let chat_cancel = Arc::new(AtomicBool::new(false));

            // Sandbox Policy: 使用默认策略（启动时已从 SQLite 加载自定义路径）
            let sandbox = Arc::new(sandbox::SandboxPolicy::default());
            if let Some(saved_paths) = &sandbox_paths_init {
                sandbox.set_allowed_paths(saved_paths.clone());
            }

            // Tool Registry: 统一工具注册表
            let tool_registry = Arc::new(
                tool_registry::build_registry(pool.clone(), data_dir.clone(), sandbox.clone(), llm_cfg_arc.clone())
            );
            // Agent 驱动的工具调用挂起表（tokio Mutex，供 async 等待确认）
            let agent_pending: tool_dispatcher::PendingMap =
                Arc::new(tokio::sync::Mutex::new(HashMap::new()));

            // Task System: 长期任务管理器（在 AppState 注册后初始化，需要 AppHandle）
            // 注意：此处用一个临时 handle，正式 handle 在 setup 完成后获取
            let task_store   = Arc::new(task::TaskStore::new(pool.clone()));
            let task_manager = Arc::new(task::TaskManager::new(
                task_store.clone(),
                tool_registry.clone(),
                agent_pending.clone(),
                llm_cfg_arc.clone(),
                app.handle().clone(),
            ));

            // Memory Tree: 初始化 Vault 目录
            let vault_root_path = vault::vault_root(&data_dir);
            if let Err(e) = vault::init_vault_dir(&vault_root_path) {
                log::warn!("Vault 初始化失败: {e}");
            }

            // ── 5. 注册 AppState ──────────────────────────────────────────────
            app.manage(AppState {
                pool:          pool.clone(),
                config:        config.clone(),
                llm_cfg:       llm_cfg_arc.clone(),
                llm_cfg_hot:   llm_cfg_hot.clone(),
                vision_cfg:    Arc::new(Mutex::new(vision_cfg_init)),
                agent:         agent.clone(),
                event_bus:     event_bus.clone(),
                pending_tools: pending_tools.clone(),
                tool_registry: tool_registry.clone(),
                agent_pending: agent_pending.clone(),
                task_manager:  task_manager.clone(),
                vault_root:    vault_root_path.clone(),
                sandbox:       sandbox.clone(),
                chat_cancel:   chat_cancel.clone(),
            });

            // ── 6. P2: 系统托盘 ───────────────────────────────────────────────
            tray::setup_tray(app)?;

            // ── 7. P2: 关闭 → 最小化到托盘 ───────────────────────────────────
            if let Some(window) = app.get_webview_window("main") {
                tray::setup_close_to_tray(&window);
            }

            // ── 8. P2: 全局快捷键（注册失败时仅 warn，不崩溃）───────────────
            {
                use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, ShortcutState};

                // 候选组合：依次尝试直到有一个注册成功
                let candidates: &[(Modifiers, Code)] = &[
                    (Modifiers::CONTROL | Modifiers::SHIFT, Code::Space),
                    (Modifiers::CONTROL | Modifiers::SHIFT, Code::KeyE),
                    (Modifiers::CONTROL | Modifiers::ALT,   Code::KeyC),
                ];

                let mut registered_key: Option<String> = None;

                for &(mods, code) in candidates {
                    let shortcut = tauri_plugin_global_shortcut::Shortcut::new(Some(mods), code);
                    let hk_handle = app.handle().clone();

                    // 注册快捷键（失败则尝试下一个）
                    let reg = app.handle().global_shortcut().register(shortcut.clone());
                    match reg {
                        Ok(_) => {
                            // 注册成功后再绑定回调
                            let _ = app.handle().global_shortcut().on_shortcut(
                                shortcut,
                                move |_app, _s, event| {
                                    if event.state() == ShortcutState::Pressed {
                                        tray::toggle_window(&hk_handle);
                                    }
                                },
                            );
                            registered_key = Some(format!("{mods:?}+{code:?}"));
                            break;
                        }
                        Err(e) => {
                            log::warn!("快捷键 {mods:?}+{code:?} 注册失败（可能被占用）: {e}");
                        }
                    }
                }

                match registered_key {
                    Some(k) => log::info!("全局快捷键已注册: {k}"),
                    None    => log::warn!("所有候选快捷键均注册失败，可通过托盘图标访问 Chebo。"),
                }
            }

            // ── 9. 后台任务（Ambient：无养成 tick / 无定时主动聊天）────────────
            // 传统 pet::start_background_tasks 已移除，见 docs/PET_AMBIENT_AGENT.md

            // P1 + Batch E: 感知循环（仅采集，不主动发言）
            {
                let h  = app.handle().clone();
                let eb = event_bus.clone();
                let ag = agent.clone();
                let lw = last_window.clone();
                let lc = last_clip.clone();
                let ps = perception_state.clone();
                tauri::async_runtime::spawn(async move {
                    perception::start_perception_loop(h, eb, ag, lw, lc, ps).await;
                });
            }

            // Memory Tree: Vault 自动同步循环（每 20 分钟）
            {
                let pool_v  = pool.clone();
                let cfg_v   = llm_cfg_arc.clone();
                let vault_p = vault_root_path.clone();
                tauri::async_runtime::spawn(async move {
                    start_vault_sync_loop(pool_v, cfg_v, vault_p).await;
                });
            }

            // Task System: 恢复应用重启前被中断的任务
            {
                let tm = task_manager.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                    tm.resume_interrupted_on_startup().await;
                });
            }

            // P0: 向量记忆后台索引
            {
                let pool_mv = pool.clone();
                let cfg_mv  = llm_cfg_arc.clone();
                tauri::async_runtime::spawn(async move {
                    memory_vector::start_index_loop(pool_mv, cfg_mv).await;
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // P0: AgentState
            get_agent_state,
            // 状态
            get_status,
            // 聊天
            send_message,
            cancel_chat_generation,
            get_chat_history,
            // 设置
            get_app_config,
            update_app_config,
            // Provider 能力注册表
            get_model_capabilities,
            list_known_models,
            // 沙盒路径配置
            get_sandbox_paths,
            set_sandbox_paths,
            // 窗口
            start_drag,
            // Batch C: 工具（权限分级）
            execute_tool,
            confirm_tool_call,
            // 关怀模式

            // Memory Tree: Vault 命令
            get_vault_stats,
            open_vault_folder,
            trigger_vault_sync,
            // P2: 托盘
            toggle_window,
            // Tool Registry: Agent 工具确认
            approve_agent_tool,
            // Task System: 长期任务
            task_create,
            task_list,
            task_detail,
            task_pause,
            task_resume,
            task_cancel_agent,
            task_approve_step,
            task_retry,
            // 记忆管理
            get_user_profile,
            get_chebo_profile,
            update_chebo_profile_entry,
            delete_chebo_profile_entry,
            get_memory_summaries,
            get_long_term_memories,
            delete_memory_entry,
            update_memory_entry,
            // Voice: TTS / STT
            voice::voice_get_config,
            voice::voice_update_config,
            voice::voice_synthesize,
            voice::voice_transcribe,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// ─── Vault 后台同步循环（每 20 分钟） ─────────────────────────────────────────

async fn start_vault_sync_loop(
    pool:       SqlitePool,
    llm_cfg:    Arc<LlmConfig>,
    vault_root: std::path::PathBuf,
) {
    use tokio::time::{interval, Duration};

    // 启动后先等待 60 秒（让应用完全初始化）
    tokio::time::sleep(Duration::from_secs(60)).await;

    let mut ticker = interval(Duration::from_secs(20 * 60));
    log::info!("Vault 同步循环已启动（每 20 分钟）");

    loop {
        ticker.tick().await;

        log::info!("Vault 开始增量同步...");

        // 获取所有 session_id
        let session_ids = match db::get_all_session_ids(&pool).await {
            Ok(ids) => ids,
            Err(e) => {
                log::warn!("vault_sync: 获取 session ids 失败: {e}");
                continue;
            }
        };

        for session_id in &session_ids {
            memory_tree::sync_session(&pool, &llm_cfg, &vault_root, session_id).await;
        }

        // 更新 Memories 目录
        memory_tree::sync_memories(&pool, &vault_root).await;

        log::info!("Vault 同步完成，共处理 {} 个会话", session_ids.len());
    }
}
