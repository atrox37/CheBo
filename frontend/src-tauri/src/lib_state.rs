// ─── lib_state.rs ─────────────────────────────────────────────────────────────
// AppState 与 AppConfig 定义，供 lib.rs 和 commands.rs 共同引用
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use sqlx::SqlitePool;

use crate::agent::AgentRuntime;
use crate::event_bus::EventBus;
use crate::llm::LlmConfig;
use crate::sandbox::SandboxPolicy;
use crate::task::TaskManager;
use crate::tool_dispatcher::PendingMap;
use crate::tool_registry::ToolRegistry;
use crate::tools::PendingToolCall;

// ─── 全局运行时状态（Tauri 托管） ────────────────────────────────────────────

pub struct AppState {
    pub pool:         SqlitePool,
    pub config:       AppConfig,
    /// 启动时加载的静态配置（后台任务使用）
    pub llm_cfg:      Arc<LlmConfig>,
    /// 用户可在设置页热更新的运行时配置（send_message 使用此字段）
    pub llm_cfg_hot:  Arc<Mutex<LlmConfig>>,
    /// Vision 回退模型配置（None = 未配置，图片只保留文字描述）
    pub vision_cfg:   Arc<Mutex<Option<LlmConfig>>>,
    /// P0: Agent 状态机
    pub agent:        Arc<AgentRuntime>,
    /// P0: 内部事件总线
    pub event_bus:    Arc<EventBus>,
    /// Batch C: 挂起等待确认的 L2/L3 工具调用（token → PendingToolCall）—— UI 发起
    pub pending_tools: Arc<Mutex<HashMap<String, PendingToolCall>>>,
    /// Tool Registry: 统一工具注册表（供 Agent 工具循环使用）
    pub tool_registry: Arc<ToolRegistry>,
    /// Agent 工具调用挂起表（Agent 循环发起的 L2/L3 工具等待用户确认）
    pub agent_pending: PendingMap,
    /// Sandbox Policy: 工具执行安全策略（路径/命令/速率）
    #[allow(dead_code)]
    pub sandbox: Arc<SandboxPolicy>,
    /// Task System: 长期任务管理器
    pub task_manager:  Arc<TaskManager>,
    /// Memory Tree: Vault 根路径（{data_dir}/vault）
    pub vault_root:   std::path::PathBuf,
    /// P1: 当前聊天生成取消标志（send_message 后台任务轮询）
    pub chat_cancel:  Arc<AtomicBool>,
}

// AppState 的字段全部 Send + Sync，Tauri manage() 要求此 trait
unsafe impl Send for AppState {}
unsafe impl Sync for AppState {}

// ─── 应用配置（从 .env 读取） ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub llm_provider:         String,
    pub llm_api_key:          String,
    pub llm_base_url:         String,
    pub llm_model:            String,
    pub temperature:          f64,
    pub max_tokens:           u32,
    pub max_history_messages: usize,
}

impl AppConfig {
    /// 加载配置：先尝试 `data_dir/.env`，再尝试当前目录 `.env`
    pub fn load_from(data_dir: &Path) -> Self {
        let env_in_data = data_dir.join(".env");
        if env_in_data.exists() {
            let _ = dotenvy::from_path(&env_in_data);
        } else {
            let _ = dotenvy::dotenv();
        }
        Self::from_env()
    }

    fn from_env() -> Self {
        let provider = std::env::var("LLM_PROVIDER")
            .unwrap_or_else(|_| "deepseek".to_string());

        let (api_key, base_url, model) = match provider.as_str() {
            "openai" => (
                std::env::var("OPENAI_API_KEY").unwrap_or_default(),
                std::env::var("OPENAI_BASE_URL")
                    .unwrap_or_else(|_| "https://api.openai.com/v1".to_string()),
                std::env::var("OPENAI_MODEL")
                    .unwrap_or_else(|_| "gpt-4o-mini".to_string()),
            ),
            "ollama" => (
                String::new(),
                std::env::var("OLLAMA_BASE_URL")
                    .unwrap_or_else(|_| "http://localhost:11434/v1".to_string()),
                std::env::var("OLLAMA_MODEL")
                    .unwrap_or_else(|_| "llama3".to_string()),
            ),
            _ => (
                // deepseek（默认）
                std::env::var("DEEPSEEK_API_KEY")
                    .or_else(|_| std::env::var("LLM_API_KEY"))
                    .unwrap_or_default(),
                std::env::var("DEEPSEEK_BASE_URL")
                    .or_else(|_| std::env::var("LLM_BASE_URL"))
                    .unwrap_or_else(|_| "https://api.deepseek.com/v1".to_string()),
                std::env::var("DEEPSEEK_MODEL")
                    .or_else(|_| std::env::var("LLM_MODEL"))
                    .unwrap_or_else(|_| "deepseek-chat".to_string()),
            ),
        };

        AppConfig {
            llm_provider:         provider,
            llm_api_key:          api_key,
            llm_base_url:         base_url,
            llm_model:            model,
            temperature:          0.8,
            max_tokens:           1000,
            max_history_messages: 10,
        }
    }
}
