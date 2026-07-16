// ─── commands.rs ──────────────────────────────────────────────────────────────
// 所有 #[tauri::command] 函数
// P0 新增：AgentState 状态机守卫 + 富上下文记忆注入
// ─────────────────────────────────────────────────────────────────────────────

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use chrono::Local;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::agent::AgentState;
use crate::character::build_system_prompt;
use crate::chat_intent;
use crate::db::{self, Message, PetStatus, StatusPatch};
use crate::event_bus::AgentEvent;
use crate::lib_state::AppState;
use crate::llm::{self, AgentStreamOutcome, LlmConfig, LlmMessage};
use crate::context_builder;
use crate::memory;
use crate::memory_controller;
use crate::planner;
use crate::provider_registry;
use crate::working_memory;
use crate::task::{AgentTask, TaskSummaryDto};
use crate::tool_dispatcher::{self, ToolDispatcher};
use crate::tools::{self, PendingToolCall, ToolLevel, ToolResult};

type CmdResult<T> = Result<T, String>;
fn e(err: impl ToString) -> String { err.to_string() }

// ─── P0: AgentState 查询 ─────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_agent_state(state: State<'_, AppState>) -> CmdResult<AgentState> {
    Ok(state.agent.current())
}

// ─── 状态查询 ─────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_status(state: State<'_, AppState>) -> CmdResult<PetStatus> {
    db::get_pet_status(&state.pool).await.map_err(e)
}

// ─── 聊天 / 发消息（Tool Registry + Dispatcher 工具循环） ────────────────────

#[tauri::command]
pub async fn send_message(
    app:        AppHandle,
    state:      State<'_, AppState>,
    content:    String,
    session_id: String,
    images:     Option<Vec<String>>,
    deep_think: Option<bool>,
    assistant_mode: Option<bool>,
) -> CmdResult<()> {
    // ── P0: 状态守卫 ─────────────────────────────────────────────────────────
    if !state.agent.can_receive_message() {
        return Err("Chebo 正在思考中，请稍等一下~".to_string());
    }

    state.agent.mark_activity();
    state.agent.try_start_thinking(&app);

    let images = images.unwrap_or_default();
    let pool          = state.pool.clone();
    // 使用热更新配置（用户在设置中修改 Key 后立即生效，无需重启）
    let llm_cfg       = Arc::new(state.llm_cfg_hot.lock().unwrap().clone());
    let vision_cfg    = state.vision_cfg.clone();
    let agent         = state.agent.clone();
    let event_bus     = state.event_bus.clone();
    let tool_registry = state.tool_registry.clone();
    let agent_pending = state.agent_pending.clone();
    let max_hist      = state.config.max_history_messages;
    let chat_cancel   = state.chat_cancel.clone();
    let deep_think    = deep_think.unwrap_or(false);
    let assistant_mode_flag = assistant_mode.unwrap_or(false);

    // 新一轮生成：清除取消标志
    chat_cancel.store(false, Ordering::SeqCst);

    // 保存用户消息
    db::save_message(&pool, &session_id, "user", &content, None, None)
        .await
        .map_err(e)?;

    let deep_think_flag = deep_think;
    let images_empty = images.is_empty();

    // ── ChatIntent 分类 ────────────────────────────────────────────────────────
    let intent_input = chat_intent::IntentInput {
        content: content.clone(),
        deep_think: deep_think_flag,
        assistant_mode: assistant_mode_flag,
        has_images: !images_empty,
    };
    let intent_context = chat_intent::IntentContext {
        recent_messages_brief: vec![], // P2: 接 context_builder
        working_memory_brief: None,    // P3: 接 working_memory
    };
    let intent_decision = chat_intent::decide(intent_input, intent_context, &llm_cfg).await;

    log::info!(
        "send_message: intent={:?}, confidence={}, recall={:?}, memory={:?}, response={:?}, task={}",
        intent_decision.intent,
        intent_decision.confidence,
        intent_decision.recall_strategy,
        intent_decision.memory_action,
        intent_decision.response_mode,
        intent_decision.should_start_task,
    );

    // 多步任务：在聊天中自动识别，后台执行
    if images_empty && chat_intent::should_run_agent_task(&content, deep_think_flag) {
        let tm    = state.task_manager.clone();
        let goal  = content.clone();
        let sid   = Some(session_id.clone());
        let app2  = app.clone();
        let agent = state.agent.clone();
        tauri::async_runtime::spawn(async move {
            agent.set_idle(&app2);
            if let Err(e) = tm.create_task(goal, sid).await {
                let _ = app2.emit("backend_error", format!("任务启动失败：{e}"));
            }
        });
        return Ok(());
    }

    event_bus.publish(AgentEvent::UserMessage {
        content:    content.clone(),
        session_id: session_id.clone(),
    });

    // 异步提取用户画像（不阻塞主流程）
    let pool2 = pool.clone();
    let cont2 = content.clone();
    let sid2  = session_id.clone();
    tauri::async_runtime::spawn(async move {
        memory::extract_user_profile(&pool2, &cont2).await;
        let _ = memory::maybe_extract_memory(&pool2, &sid2, &cont2).await;
    });

    // ── 后台工具循环 ─────────────────────────────────────────────────────────
    tauri::async_runtime::spawn(async move {
        let cancel = chat_cancel;
        let assistant_mode = assistant_mode_flag;
        let mut llm_cfg_use = (*llm_cfg).clone();
        if assistant_mode {
            llm_cfg_use.max_tokens = llm_cfg_use.max_tokens.max(8192);
        }
        let llm_cfg_ref = llm_cfg_use;

        /// 用户取消或中断：持久化 partial、通知前端、恢复 Idle
        async fn finish_cancelled(
            app:        &AppHandle,
            pool:       &sqlx::SqlitePool,
            session_id: &str,
            agent:      &crate::agent::AgentRuntime,
            partial:    &str,
        ) {
            if !partial.trim().is_empty() {
                let _ = db::save_message(pool, session_id, "assistant", partial, None, None).await;
            }
            let _ = app.emit(
                "generation_cancelled",
                serde_json::json!({ "partial": partial }),
            );
            agent.set_idle(app);
        }

        fn is_cancelled(cancel: &AtomicBool) -> bool {
            cancel.load(Ordering::Relaxed)
        }
        // ── 0. 视觉路由：处理图片附件 ────────────────────────────────────────
        //   a. 主模型支持视觉 → 直接传图
        //   b. 主模型不支持视觉 + 已配置视觉回退模型 → 先调视觉模型提取描述，
        //      描述文字注入上下文，不向主模型发图
        //   c. 均不支持 → 消息中附加 [图片: N 张，当前模型不支持视觉] 说明
        let caps = provider_registry::lookup(&llm_cfg.model);
        let (effective_images, vision_note) = if images.is_empty() {
            (vec![], String::new())
        } else if caps.supports_vision {
            // 主模型支持视觉，直接传
            let _ = app.emit("vision_route", serde_json::json!({
                "mode": "direct",
                "model": llm_cfg.model,
                "image_count": images.len(),
            }));
            (images.clone(), String::new())
        } else {
            // 主模型不支持视觉
            let vision_model = vision_cfg.lock().unwrap().clone();
            if let Some(vcfg) = vision_model {
                // 调视觉回退模型提取图片描述
                let _ = app.emit("vision_route", serde_json::json!({
                    "mode": "fallback",
                    "main_model":   llm_cfg.model,
                    "vision_model": vcfg.model,
                    "image_count":  images.len(),
                }));
                match llm::describe_images(&images, &content, &vcfg).await {
                    Ok(desc) => {
                        let note = format!(
                            "\n\n[📷 图片分析结果（由 {} 处理）]\n{}",
                            vcfg.model, desc
                        );
                        (vec![], note)   // 图片不传给主模型，只传描述
                    }
                    Err(err) => {
                        log::warn!("视觉回退模型调用失败: {err}");
                        let note = format!(
                            "\n\n[图片附件: {} 张，视觉分析失败：{}]",
                            images.len(), err
                        );
                        (vec![], note)
                    }
                }
            } else {
                // 没有视觉回退模型，只附加说明
                let note = format!(
                    "\n\n[图片附件: {} 张。当前模型 {} 不支持视觉输入。\
                    如需分析图片，请在设置中配置视觉回退模型（如 gpt-4o）。]",
                    images.len(), llm_cfg.model
                );
                (vec![], note)
            }
        };

        // ── 1. 构建初始 messages ──────────────────────────────────────────────
        let history = memory::load_history_for_context(&pool, &session_id, max_hist as i64)
            .await
            .unwrap_or_default();

        let status = db::get_pet_status(&pool).await.unwrap_or_else(|_| PetStatus {
            id: 1, hunger: 80.0, energy: 80.0, mood: 70.0, affection: 20.0,
            level: 1, exp: 0, coins: 100,
            current_action: "idle".to_string(),
            active_task_id: None, task_ends_at: None, task_type: None,
            last_interaction_at: None, updated_at: None,
        });

        // 使用 ContextPack 替代原来的 build_rich_context_string
        let context_pack = context_builder::build_context_pack(
            &pool,
            &session_id,
            &content,
            &intent_decision,
            &llm_cfg_ref,
        ).await;
        let mem_ctx = context_pack.to_prompt_section();

        let tool_block = tool_registry.tools_prompt_block_for(&content, &[]);

        let system = if deep_think_flag {
            format!(
                "{}\n\n{}\n\n{}\n\n{}",
                build_system_prompt(&status, !assistant_mode),
                mem_ctx,
                chat_intent::DEEP_THINK_SYSTEM_ADDITION,
                tool_block
            )
        } else {
            format!(
                "{}\n\n{}\n\n{}",
                build_system_prompt(&status, !assistant_mode),
                mem_ctx,
                tool_block
            )
        };

        let max_tool_turns = if deep_think_flag {
            chat_intent::DEEP_THINK_MAX_TOOL_TURNS
        } else {
            tool_dispatcher::MAX_TOOL_TURNS
        };

        // 用户消息：文字 + 可能的视觉分析注释
        let user_text = if vision_note.is_empty() {
            content.clone()
        } else {
            format!("{}{}", content, vision_note)
        };

        let mut messages = vec![LlmMessage::system(&system)];
        messages.extend(history);
        messages.push(LlmMessage::user(&user_text));

        // ── Phase B: 复杂请求预规划 ─────────────────────────────────────
        if planner::should_plan(intent_decision.intent) {
            if let Some(plan) = planner::quick_plan(&content, intent_decision.intent, &llm_cfg).await {
                // 在 system prompt 与历史消息之间插入计划
                // 让模型在第 1 轮工具循环前就知道执行路径
                let plan_msg = format!(
                    "【执行计划（请按此顺序执行）】\n\
                     以下是为当前请求制定的执行计划，\n\
                     请按步骤顺序执行，每完成一步后继续下一步。\n\
                     如果某一步结果不符合预期，重新调整后再继续。\n\n\
                     {plan}"
                );
                // 插入到 history 之后、user 消息之前
                let last_user = messages.pop();
                messages.push(LlmMessage::system(&plan_msg));
                if let Some(msg) = last_user {
                    messages.push(msg);
                }
                log::info!("计划已注入: intent={:?}", intent_decision.intent);
            }
        }

        // ── 2. 工具调用循环（最多 MAX_TOOL_TURNS 轮，真流式） ───────────────
        let mut final_text        = String::new();
        let mut final_emotion     = "normal".to_string();
        let mut used_tools        = false;
        let mut streamed_done     = false;
        let mut reflection_done   = false;

        'tool_loop: for turn in 0..max_tool_turns {
            if is_cancelled(&cancel) {
                finish_cancelled(&app, &pool, &session_id, &agent, "").await;
                return;
            }

            let turn_images = if turn == 0 {
                effective_images.as_slice()
            } else {
                &[]
            };

            match llm::stream_agent_turn(
                &app,
                messages.clone(),
                turn_images,
                &llm_cfg_ref,
                cancel.clone(),
                &agent,
            )
            .await
            {
                AgentStreamOutcome::Cancelled { partial } => {
                    finish_cancelled(&app, &pool, &session_id, &agent, &partial).await;
                    return;
                }
                AgentStreamOutcome::Complete { raw, streamed, .. } => {
                    let tool_calls = ToolDispatcher::parse_xml(&raw);

                    if tool_calls.is_empty() {
                        // ── Reflection: 最终输出前自检 ─────────────────────
                        if used_tools && !reflection_done {
                            reflection_done = true;
                            messages.push(LlmMessage::system(
                                "【最终审查】请检查你上面的回答是否完整、准确。\n\
                                 如果发现遗漏或错误，请修正后重新输出最终答案。\n\
                                 如果确认无误，直接输出最终答案。"
                            ));
                            continue 'tool_loop;
                        }
                        let (clean, emo) = crate::character::detect_emotion(&raw);
                        final_text = clean;
                        final_emotion = emo;
                        streamed_done = streamed;
                        break 'tool_loop;
                    }

                    used_tools = true;

                    let visible = ToolDispatcher::strip_tool_calls(&raw);
                    if !visible.is_empty() {
                        let _ = app.emit("assistant_thinking", serde_json::json!({
                            "content": visible,
                            "turn":    turn,
                        }));
                    }

                    messages.push(LlmMessage::assistant(&raw));
                    agent.set_executing_tool(&app);

                    let results = ToolDispatcher::execute_all(
                        &tool_registry,
                        tool_calls,
                        &agent_pending,
                        Some(&app),
                    )
                    .await;

                    if is_cancelled(&cancel) {
                        finish_cancelled(&app, &pool, &session_id, &agent, "").await;
                        return;
                    }

                    for r in &results {
                        let _ = app.emit("tool_result", serde_json::json!({
                            "tool":    r.name,
                            "success": r.success,
                            "content": r.output,
                            "level":   r.permission as u8,
                        }));
                    }

                    let tool_result_text = ToolDispatcher::format_results_as_message(&results);
                    messages.push(LlmMessage::user(&tool_result_text));

                    // ── Reflection: 分析工具结果后再决策 ──────────────────
                    messages.push(LlmMessage::system(
                        "【结果分析】请检查以上工具返回的结果。\n\
                         - 如果信息足够 → 直接给出最终答案，无需再调工具\n\
                         - 如果信息不足或需要进一步查证 → 继续调用工具\n\
                         - 如果结果包含错误或异常 → 请指出并考虑替代方案"
                    ));

                    for r in &results {
                        event_bus.publish(AgentEvent::ToolCallFinished {
                            tool:    r.name.clone(),
                            success: r.success,
                            summary: r.output.chars().take(80).collect(),
                        });
                    }

                    agent.resume_thinking_after_tools(&app);

                    if turn == max_tool_turns - 1 {
                        final_text = "（已完成工具调用）".to_string();
                        final_emotion = "normal".to_string();
                    }
                }
            }
        }

        if is_cancelled(&cancel) {
            finish_cancelled(&app, &pool, &session_id, &agent, "").await;
            return;
        }

        let _ = used_tools;

        // 未走流式完成事件时（纯工具轮或空回复）补发 assistant_done
        if !streamed_done && !final_text.is_empty() {
            agent.set_talking(&app);
            let _ = app.emit("assistant_chunk", serde_json::json!({ "content": &final_text }));
            let _ = app.emit("assistant_done", serde_json::json!({
                "full_content": final_text.clone(),
                "emotion":      final_emotion.clone(),
            }));
        } else if !streamed_done && final_text.is_empty() {
            let _ = app.emit("assistant_done", serde_json::json!({
                "full_content": "",
                "emotion":      final_emotion.clone(),
            }));
        }

        // ── 4. 持久化 + 后处理 ───────────────────────────────────────────────
        let _ = db::save_message(&pool, &session_id, "assistant", &final_text, Some(&final_emotion), None).await;

        if let Ok(s) = db::get_pet_status(&pool).await {
            let meaningful = content.trim().chars().count() >= 2 && !final_text.trim().is_empty();
            let affection_patch = if meaningful {
                Some((s.affection + 0.2).min(100.0))
            } else {
                None
            };
            let updated = db::update_pet_status(&pool, StatusPatch {
                affection:           affection_patch,
                last_interaction_at: Some(Some(
                    Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                )),
                ..Default::default()
            }).await;
            if let Ok(status) = updated {
                let _ = app.emit("status_update", &status);
            }
        }

        event_bus.publish(AgentEvent::AssistantDone {
            text:       final_text.clone(),
            emotion:    final_emotion.clone(),
            session_id: session_id.clone(),
        });

        // 异步后处理：摘要 + 工作记忆 + 记忆控制器
        let pool3 = pool.clone();
        let cfg3  = llm_cfg.clone();
        let sid3  = session_id.clone();
        let pool_wm = pool.clone();
        let cfg_wm  = llm_cfg.clone();
        let sid_wm  = session_id.clone();
        let decision_wm = intent_decision.clone();
        let pool_mc = pool.clone();
        let cfg_mc  = llm_cfg.clone();
        let content_mc = content.clone();
        let sid_mc  = session_id.clone();
        let decision_mc = intent_decision.clone();
        tauri::async_runtime::spawn(async move {
            // 摘要（每10条）
            memory::maybe_summarize(&pool3, &cfg3, &sid3).await;

            // Working Memory 更新（仅在必要场景触发）
            if working_memory::should_update(&decision_wm, &content) {
                let recent = db::get_messages(&pool_wm, &sid_wm, 6).await.unwrap_or_default();
                if let Err(e) = working_memory::update_from_conversation(
                    &pool_wm, &cfg_wm,
                    &working_memory::default_scope(),
                    &recent,
                    &decision_wm,
                ).await {
                    log::warn!("working_memory update failed: {e}");
                }
            }

            // Memory Controller：提取候选记忆、评分、冲突检测、持久化
            memory_controller::process_event(
                &pool_mc,
                &cfg_mc,
                memory_controller::MemoryEvent::UserMessage {
                    content: content_mc,
                    session_id: sid_mc,
                    decision: decision_mc,
                },
            ).await;
        });

        agent.set_idle(&app);
    });

    Ok(())
}

/// P1: 中断当前聊天生成（流式 LLM / 工具循环）
#[tauri::command]
pub async fn cancel_chat_generation(
    app:   AppHandle,
    state: State<'_, AppState>,
) -> CmdResult<()> {
    state.chat_cancel.store(true, Ordering::SeqCst);
    state.agent.cancel_generation(&app);
    Ok(())
}

// ─── Agent 工具确认（前端批准/拒绝 agent 循环中的 L2/L3 工具） ───────────────

#[tauri::command]
pub async fn approve_agent_tool(
    state:    State<'_, AppState>,
    id:       String,
    approved: bool,
) -> CmdResult<()> {
    let mut map = state.agent_pending.lock().await;
    if let Some(call) = map.get_mut(&id) {
        call.approved = Some(approved);
        log::info!("approve_agent_tool: {} → {}", id, approved);
        Ok(())
    } else {
        Err(format!("工具调用 id 不存在或已过期: {id}"))
    }
}

// ─── 聊天历史 ─────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_chat_history(state: State<'_, AppState>) -> CmdResult<Vec<Message>> {
    memory::get_all_messages(&state.pool).await.map_err(e)
}

// ─── 获取/更新 App 配置 ───────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
pub struct AppConfigDto {
    pub llm_provider:     String,
    pub llm_base_url:     String,
    pub llm_model:        String,
    pub has_api_key:      bool,
    /// 视觉回退模型（空字符串表示未配置）
    pub vision_model:     String,
    pub vision_base_url:  String,
    pub has_vision_key:   bool,
}

#[tauri::command]
pub async fn get_app_config(state: State<'_, AppState>) -> CmdResult<AppConfigDto> {
    let vision = state.vision_cfg.lock().unwrap().clone();
    let hot    = state.llm_cfg_hot.lock().unwrap().clone();
    Ok(AppConfigDto {
        llm_provider:    state.config.llm_provider.clone(),
        llm_base_url:    hot.base_url.clone(),
        llm_model:       hot.model.clone(),
        has_api_key:     !hot.api_key.is_empty(),
        vision_model:    vision.as_ref().map(|v| v.model.clone()).unwrap_or_default(),
        vision_base_url: vision.as_ref().map(|v| v.base_url.clone()).unwrap_or_default(),
        has_vision_key:  vision.as_ref().map(|v| !v.api_key.is_empty()).unwrap_or(false),
    })
}

#[derive(Deserialize)]
pub struct UpdateConfigPayload {
    pub api_key:       Option<String>,
    pub base_url:      Option<String>,
    pub model:         Option<String>,
    pub llm_provider:  Option<String>,
    /// 视觉回退模型配置（留空 = 清除配置）
    pub vision_api_key:  Option<String>,
    pub vision_base_url: Option<String>,
    pub vision_model:    Option<String>,
}

#[tauri::command]
pub async fn update_app_config(
    state:   State<'_, AppState>,
    payload: UpdateConfigPayload,
) -> CmdResult<()> {
    // ── 持久化到 SQLite ───────────────────────────────────────────────────────
    if let Some(key) = &payload.api_key {
        db::set_config(&state.pool, "llm_api_key", key).await.map_err(e)?;
    }
    if let Some(url) = &payload.base_url {
        db::set_config(&state.pool, "llm_base_url", url).await.map_err(e)?;
    }
    if let Some(model) = &payload.model {
        db::set_config(&state.pool, "llm_model", model).await.map_err(e)?;
    }
    if let Some(provider) = &payload.llm_provider {
        db::set_config(&state.pool, "llm_provider", provider).await.map_err(e)?;
    }

    // ── 热更新运行时 llm_cfg_hot（立即生效，无需重启）────────────────────────
    {
        let mut hot = state.llm_cfg_hot.lock().unwrap();
        if let Some(key) = &payload.api_key {
            hot.api_key = key.clone();
        }
        if let Some(url) = &payload.base_url {
            if !url.is_empty() { hot.base_url = url.clone(); }
        }
        if let Some(model) = &payload.model {
            if !model.is_empty() { hot.model = model.clone(); }
        }
    }

    // 视觉模型配置持久化 + 热更新 vision_cfg
    let vision_key   = payload.vision_api_key.as_deref().unwrap_or("");
    let vision_url   = payload.vision_base_url.as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("https://api.openai.com/v1");
    let vision_model = payload.vision_model.as_deref().unwrap_or("");

    if payload.vision_api_key.is_some()
        || payload.vision_base_url.is_some()
        || payload.vision_model.is_some()
    {
        db::set_config(&state.pool, "vision_api_key",  vision_key).await.map_err(e)?;
        db::set_config(&state.pool, "vision_base_url", vision_url).await.map_err(e)?;
        db::set_config(&state.pool, "vision_model",    vision_model).await.map_err(e)?;

        // 热更新运行时配置
        let new_vcfg = if vision_key.is_empty() || vision_model.is_empty() {
            None
        } else {
            Some(LlmConfig {
                api_key:     vision_key.to_string(),
                base_url:    vision_url.to_string(),
                model:       vision_model.to_string(),
                temperature: 0.3,
                max_tokens:  800,
            })
        };
        *state.vision_cfg.lock().unwrap() = new_vcfg;
    }

    Ok(())
}

// ─── 沙盒路径配置 ─────────────────────────────────────────────────────────────

/// 获取当前沙盒允许的路径列表
#[tauri::command]
pub fn get_sandbox_paths(state: State<'_, AppState>) -> Vec<String> {
    state.sandbox.get_allowed_paths()
}

/// 更新沙盒允许的路径列表（立即热更新，无需重启）
/// paths: 绝对路径字符串数组，如 ["C:\\Users\\me\\Documents", "D:\\Projects"]
#[tauri::command]
pub async fn set_sandbox_paths(
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> CmdResult<()> {
    // 持久化到 SQLite（逗号分隔）
    let serialized = paths.join("||");
    db::set_config(&state.pool, "sandbox_allowed_paths", &serialized).await.map_err(e)?;
    // 热更新运行时策略
    state.sandbox.set_allowed_paths(paths);
    Ok(())
}

// ─── Provider 能力注册表查询 ──────────────────────────────────────────────────

/// 查询单个模型的能力（未知模型返回推断值）
#[tauri::command]
pub fn get_model_capabilities(model: String) -> provider_registry::ModelCapabilities {
    provider_registry::lookup(&model)
}

/// 获取所有已知模型列表（用于设置页面下拉）
#[tauri::command]
pub fn list_known_models() -> Vec<provider_registry::ModelCapabilities> {
    provider_registry::all_known()
}

// ─── 工具配置管理 ──────────────────────────────────────────────────────────────

/// 获取所有工具的 spec 和配置状态（供设置页显示）
#[derive(Serialize)]
pub struct ToolConfigStatusDto {
    pub name:           String,
    pub description:    String,
    pub permission:     u8,
    pub permission_label: String,
    pub category:       String,
    pub enabled_by_default: bool,
    pub enabled:        i64,
    pub auto_approve:   i64,
    pub daily_limit:    i64,
}

#[tauri::command]
pub async fn get_tool_configs(state: State<'_, AppState>) -> CmdResult<Vec<ToolConfigStatusDto>> {
    let specs = state.tool_registry.all_specs();
    let configs = db::get_all_tool_configs(&state.pool).await.map_err(e)?;

    Ok(specs.iter().map(|spec| {
        let cfg = configs.iter().find(|c| c.tool_name == spec.name);
        ToolConfigStatusDto {
            name:              spec.name.clone(),
            description:       spec.description.clone(),
            permission:        spec.permission as u8 + 1, // L0=0→1, L1=1→2, etc.
            permission_label:  spec.permission.label().to_string(),
            category:          spec.category.label().to_string(),
            enabled_by_default: spec.enabled_by_default,
            enabled:           cfg.map(|c| c.enabled).unwrap_or(if spec.enabled_by_default { 1 } else { 0 }),
            auto_approve:      cfg.map(|c| c.auto_approve).unwrap_or(0),
            daily_limit:       cfg.map(|c| c.daily_limit).unwrap_or(0),
        }
    }).collect())
}

#[derive(Deserialize)]
pub struct UpdateToolConfigPayload {
    pub tool_name:    String,
    pub enabled:      Option<i64>,
    pub auto_approve: Option<i64>,
    pub daily_limit:  Option<i64>,
}

/// 更新单个工具配置
#[tauri::command]
pub async fn update_tool_config(
    state:   State<'_, AppState>,
    payload: UpdateToolConfigPayload,
) -> CmdResult<()> {
    // 读取当前配置
    let current = db::get_tool_config(&state.pool, &payload.tool_name)
        .await
        .map_err(e)?
        .unwrap_or(db::ToolConfigEntry {
            tool_name:    payload.tool_name.clone(),
            enabled:      1,
            auto_approve: 0,
            daily_limit:  0,
        });

    db::upsert_tool_config(
        &state.pool,
        &payload.tool_name,
        payload.enabled.unwrap_or(current.enabled),
        payload.auto_approve.unwrap_or(current.auto_approve),
        payload.daily_limit.unwrap_or(current.daily_limit),
    )
    .await
    .map_err(e)?;

    Ok(())
}

// ─── 拖拽（原生无边框窗口） ───────────────────────────────────────────────────

#[tauri::command]
pub fn start_drag(window: tauri::Window) {
    let _ = window.start_dragging();
}

// ─── Batch C: 工具系统命令（L0–L3 权限分级）─────────────────────────────────

/// 工具执行统一入口（带权限分级）
/// L0/L1 直接执行；L2/L3 挂起等待前端确认后调用 confirm_tool_call
#[tauri::command]
pub async fn execute_tool(
    app:   AppHandle,
    state: State<'_, AppState>,
    tool:  String,
    args:  serde_json::Value,
) -> CmdResult<ToolResult> {
    let args_str = args.to_string();
    let level = tools::tool_level(&tool, &args_str);

    // L2 / L3：挂起，等待用户确认
    if level.requires_confirm() {
        let token = uuid::Uuid::new_v4().to_string();
        let risk_desc = if level == ToolLevel::L3 {
            format!(
                "高危操作：{} {}\n{}",
                tool,
                args_str,
                tools::level_description(ToolLevel::L3)
            )
        } else {
            tools::level_description(ToolLevel::L2).to_string()
        };

        let pending = PendingToolCall {
            token:     token.clone(),
            tool:      tool.clone(),
            args:      args_str.clone(),
            level,
            risk_desc: risk_desc.clone(),
        };

        state.pending_tools.lock()
            .map_err(|e| e.to_string())?
            .insert(token.clone(), pending);

        // 进入等待确认状态
        state.agent.set_waiting_confirm(&app);

        // 通知前端显示确认对话框
        let _ = app.emit("tool_confirm_required", serde_json::json!({
            "token":     token,
            "tool":      tool,
            "args":      args_str,
            "level":     level.as_u8(),
            "levelTag":  tools::level_color_tag(level),
            "riskDesc":  risk_desc,
        }));

        // 发布工具调用请求事件
        state.event_bus.publish(AgentEvent::ToolCallRequested {
            tool: tool.clone(),
            args: args_str,
            level: level.as_u8(),
            token: "pending_confirm".to_string(),
        });

        return Ok(ToolResult {
            tool:    tool,
            success: false,
            content: "等待用户确认...".to_string(),
            level:   level.as_u8(),
        });
    }

    // L0 / L1：直接执行
    let result = run_tool_directly(&tool, &args).await?;

    // 发布完成事件
    state.event_bus.publish(AgentEvent::ToolCallFinished {
        tool:    result.tool.clone(),
        success: result.success,
        summary: result.to_summary(),
    });

    let _ = app.emit("tool_result", serde_json::json!({
        "tool":    result.tool,
        "success": result.success,
        "content": result.content,
        "level":   result.level,
    }));

    Ok(result)
}

/// 用户确认 L2/L3 工具调用后的执行命令
#[tauri::command]
pub async fn confirm_tool_call(
    app:     AppHandle,
    state:   State<'_, AppState>,
    token:   String,
    confirm: bool,
) -> CmdResult<ToolResult> {
    let pending = state.pending_tools.lock()
        .map_err(|e| e.to_string())?
        .remove(&token);

    let pending = match pending {
        Some(p) => p,
        None    => return Err(format!("确认令牌已过期或不存在: {token}")),
    };

    if !confirm {
        // 用户拒绝 → 回到 Idle
        state.agent.set_idle(&app);
        state.event_bus.publish(AgentEvent::ToolCallFinished {
            tool:    pending.tool.clone(),
            success: false,
            summary: "用户拒绝执行".to_string(),
        });
        return Ok(ToolResult {
            tool:    pending.tool,
            success: false,
            content: "已取消".to_string(),
            level:   pending.level.as_u8(),
        });
    }

    // 用户确认 → 执行工具
    state.agent.set_executing_tool(&app);

    let args: serde_json::Value = serde_json::from_str(&pending.args)
        .unwrap_or(serde_json::Value::Object(Default::default()));

    let result = run_tool_directly(&pending.tool, &args).await?;

    // 执行完毕 → 回到 Idle
    state.agent.set_idle(&app);

    state.event_bus.publish(AgentEvent::ToolCallFinished {
        tool:    result.tool.clone(),
        success: result.success,
        summary: result.to_summary(),
    });

    let _ = app.emit("tool_result", serde_json::json!({
        "tool":    result.tool,
        "success": result.success,
        "content": result.content,
        "level":   result.level,
    }));

    Ok(result)
}

/// 内部辅助：根据工具名和参数直接执行（不做权限检查）
async fn run_tool_directly(tool: &str, args: &serde_json::Value) -> CmdResult<ToolResult> {
    let client = reqwest::Client::new();
    match tool {
        "read_file" => {
            let path = args["path"].as_str().unwrap_or("").to_string();
            if path.is_empty() { return Err("缺少参数 path".to_string()); }
            Ok(tools::read_file(&path).await)
        }
        "web_search" => {
            let query = args["query"].as_str().unwrap_or("").to_string();
            if query.is_empty() { return Err("缺少参数 query".to_string()); }
            Ok(tools::web_search(&query, &client).await)
        }
        "git_status" => {
            let dir = args["dir"].as_str().unwrap_or(".").to_string();
            Ok(tools::git_status(&dir).await)
        }
        "safe_shell" => {
            let cmd = args["cmd"].as_str().unwrap_or("").to_string();
            if cmd.is_empty() { return Err("缺少参数 cmd".to_string()); }
            Ok(tools::safe_shell(&cmd).await)
        }
        "list_dir" => {
            let dir = args["dir"].as_str().unwrap_or(".").to_string();
            Ok(tools::list_dir(&dir).await)
        }
        other => Err(format!("未知工具: {other}。可用：read_file / web_search / git_status / safe_shell / list_dir")),
    }
}

// ─── P2: 托盘相关命令 ─────────────────────────────────────────────────────────

/// 切换主窗口显示/隐藏（供前端调用）
#[tauri::command]
pub fn toggle_window(app: AppHandle) {
    crate::tray::toggle_window(&app);
}

// ─── Memory Tree: Vault 命令 ──────────────────────────────────────────────────

/// 获取 Vault 统计信息（供设置页显示）
#[tauri::command]
pub async fn get_vault_stats(state: State<'_, AppState>) -> CmdResult<crate::db::VaultStats> {
    let vault_path = state.vault_root.to_string_lossy().to_string();
    crate::db::get_vault_stats(&state.pool, &vault_path)
        .await
        .map_err(e)
}

/// 打开 Vault 目录（系统文件管理器）
#[tauri::command]
pub fn open_vault_folder(state: State<'_, AppState>) {
    let path = state.vault_root.to_string_lossy().to_string();
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer")
            .arg(&path)
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(&path).spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open").arg(&path).spawn();
    }
}

/// 立即触发一次 Vault 同步（不必等 20 分钟）
#[tauri::command]
pub async fn trigger_vault_sync(
    app:   AppHandle,
    state: State<'_, AppState>,
) -> CmdResult<String> {
    let pool     = state.pool.clone();
    let cfg      = state.llm_cfg.clone();
    let vault_p  = state.vault_root.clone();

    let session_ids = crate::db::get_all_session_ids(&pool).await.map_err(e)?;
    let count = session_ids.len();

    // 在后台异步执行，避免阻塞前端
    tauri::async_runtime::spawn(async move {
        for session_id in &session_ids {
            crate::memory_tree::sync_session(&pool, &cfg, &vault_p, session_id).await;
        }
        crate::memory_tree::sync_memories(&pool, &vault_p).await;
        log::info!("手动 Vault 同步完成：{} 个会话", session_ids.len());
        let _ = app.emit("vault_sync_done", ());
    });

    Ok(format!("已启动 Vault 同步，共 {count} 个会话"))
}

// ─── Task System 命令 ──────────────────────────────────────────────────────────

/// 创建任务：LLM 规划步骤 + 立即开始执行
#[tauri::command]
pub async fn task_create(
    state:      State<'_, AppState>,
    goal:       String,
    session_id: Option<String>,
) -> CmdResult<AgentTask> {
    state.task_manager.create_task(goal, session_id)
        .await
        .map_err(e)
}

/// 获取所有任务摘要列表
#[tauri::command]
pub async fn task_list(state: State<'_, AppState>) -> CmdResult<Vec<TaskSummaryDto>> {
    let tasks = state.task_manager.store.list_all().await.map_err(e)?;
    Ok(tasks.iter().map(TaskSummaryDto::from).collect())
}

/// 获取单个任务完整详情（含步骤）
#[tauri::command]
pub async fn task_detail(
    state:   State<'_, AppState>,
    task_id: String,
) -> CmdResult<AgentTask> {
    state.task_manager.store.get(&task_id)
        .await
        .map_err(e)?
        .ok_or_else(|| format!("任务不存在: {task_id}"))
}

/// 暂停任务
#[tauri::command]
pub async fn task_pause(
    state:   State<'_, AppState>,
    task_id: String,
) -> CmdResult<()> {
    state.task_manager.pause_task(&task_id).await.map_err(e)
}

/// 恢复暂停/中断的任务
#[tauri::command]
pub async fn task_resume(
    state:   State<'_, AppState>,
    task_id: String,
) -> CmdResult<()> {
    state.task_manager.resume_task(&task_id).await.map_err(e)
}

/// 取消任务（不可恢复）
#[tauri::command]
pub async fn task_cancel_agent(
    state:   State<'_, AppState>,
    task_id: String,
) -> CmdResult<()> {
    state.task_manager.cancel_task(&task_id).await.map_err(e)
}

/// 确认/跳过一个 WaitingConfirm 步骤
#[tauri::command]
pub async fn task_approve_step(
    state:    State<'_, AppState>,
    task_id:  String,
    step_id:  String,
    approved: bool,
) -> CmdResult<()> {
    state.task_manager.approve_step(&task_id, &step_id, approved)
        .await
        .map_err(e)
}

/// 重试失败的任务
#[tauri::command]
pub async fn task_retry(
    state:   State<'_, AppState>,
    task_id: String,
) -> CmdResult<()> {
    state.task_manager.retry_task(&task_id).await.map_err(e)
}

// ─── 记忆管理命令 ─────────────────────────────────────────────────────────────

/// 获取用户画像（含置信度）
#[tauri::command]
pub async fn get_user_profile(state: State<'_, AppState>) -> CmdResult<Vec<db::UserProfileEntry>> {
    db::get_user_profile_all(&state.pool).await.map_err(e)
}

/// Chebo 自身画像（persona_memory，供用户了解 Chebo）
#[tauri::command]
pub async fn get_chebo_profile(state: State<'_, AppState>) -> CmdResult<Vec<db::PersonaMemory>> {
    db::get_persona_memory_all(&state.pool).await.map_err(e)
}

#[tauri::command]
pub async fn update_chebo_profile_entry(
    state: State<'_, AppState>,
    key:   String,
    value: String,
) -> CmdResult<()> {
    db::upsert_persona_memory(&state.pool, &key, &value, "trait", 1.0)
        .await
        .map_err(e)
}

#[tauri::command]
pub async fn delete_chebo_profile_entry(
    state: State<'_, AppState>,
    key:   String,
) -> CmdResult<()> {
    sqlx::query("DELETE FROM persona_memory WHERE key = ?")
        .bind(&key)
        .execute(&state.pool)
        .await
        .map_err(e)?;
    Ok(())
}

/// 获取对话摘要列表（最近 20 条）
#[tauri::command]
pub async fn get_memory_summaries(
    state: State<'_, AppState>,
) -> CmdResult<Vec<db::MemorySummary>> {
    let rows = sqlx::query(
        "SELECT id, session_id, msg_start_id, msg_end_id, summary, created_at
         FROM memory_summaries ORDER BY id DESC LIMIT 20"
    )
    .fetch_all(&state.pool)
    .await
    .map_err(e)?;

    use sqlx::Row as _;
    Ok(rows.iter().map(|r| db::MemorySummary {
        id:           r.get("id"),
        session_id:   r.get("session_id"),
        msg_start_id: r.get("msg_start_id"),
        msg_end_id:   r.get("msg_end_id"),
        summary:      r.get("summary"),
        created_at:   r.get("created_at"),
    }).collect())
}

/// 获取长期记忆片段（最近 30 条）
#[tauri::command]
pub async fn get_long_term_memories(
    state: State<'_, AppState>,
) -> CmdResult<Vec<serde_json::Value>> {
    use sqlx::Row as _;
    let rows = sqlx::query(
        "SELECT id, content, created_at FROM long_term_memories ORDER BY id DESC LIMIT 30"
    )
    .fetch_all(&state.pool)
    .await
    .map_err(e)?;

    Ok(rows.iter().map(|r| {
        let id: i64         = r.get("id");
        let content: String = r.get("content");
        let ts: String      = r.get("created_at");
        serde_json::json!({ "id": id, "content": content, "created_at": ts })
    }).collect())
}

/// 删除用户画像中的某条记忆（用户手动纠错）
#[tauri::command]
pub async fn delete_memory_entry(
    state: State<'_, AppState>,
    key:   String,
) -> CmdResult<()> {
    db::delete_user_profile_entry(&state.pool, &key).await.map_err(e)
}

/// 用户手动更新某条记忆（纠正 LLM 的错误理解，置信度自动设为 1.0）
#[tauri::command]
pub async fn update_memory_entry(
    state: State<'_, AppState>,
    key:   String,
    value: String,
) -> CmdResult<()> {
    db::update_user_profile_entry(&state.pool, &key, &value).await.map_err(e)
}
