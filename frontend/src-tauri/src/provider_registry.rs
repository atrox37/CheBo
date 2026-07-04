// ─── provider_registry.rs ─────────────────────────────────────────────────────
// 模型能力注册表：记录每个已知模型的能力（视觉/工具/上下文窗口/价格）
// 用于 Vision 路由：当用户发送图片时，决定是直接传给模型还是降级处理
// ─────────────────────────────────────────────────────────────────────────────

use serde::Serialize;
use std::collections::HashMap;

/// 单个模型的能力描述
#[derive(Debug, Clone, Serialize)]
pub struct ModelCapabilities {
    /// 模型 ID（与 API 请求中 model 字段一致）
    pub model_id: String,
    /// 用户友好的显示名称
    pub display_name: String,
    /// 提供商名称
    pub provider: String,
    /// 是否支持图片/视觉输入
    pub supports_vision: bool,
    /// 是否支持 Function Calling / Tool Use
    pub supports_tools: bool,
    /// 最大上下文窗口（token 数）
    pub context_window: u32,
    /// 输入价格（USD / 1K tokens，0 表示未知）
    pub cost_input_per_1k: f64,
    /// 输出价格（USD / 1K tokens，0 表示未知）
    pub cost_output_per_1k: f64,
    /// 备注
    pub notes: String,
}

impl ModelCapabilities {
    fn new(
        model_id: &str,
        display_name: &str,
        provider: &str,
        supports_vision: bool,
        supports_tools: bool,
        context_window: u32,
        cost_input_per_1k: f64,
        cost_output_per_1k: f64,
        notes: &str,
    ) -> Self {
        Self {
            model_id: model_id.to_string(),
            display_name: display_name.to_string(),
            provider: provider.to_string(),
            supports_vision,
            supports_tools,
            context_window,
            cost_input_per_1k,
            cost_output_per_1k,
            notes: notes.to_string(),
        }
    }
}

// ─── 静态注册表 ───────────────────────────────────────────────────────────────

lazy_static::lazy_static! {
    static ref REGISTRY: HashMap<String, ModelCapabilities> = build_registry();
}

fn build_registry() -> HashMap<String, ModelCapabilities> {
    let mut m = HashMap::new();

    // ── DeepSeek ──────────────────────────────────────────────────────────────
    // 注：DeepSeek 官方 API 目前不支持视觉输入（VL2 仅支持本地部署）
    for entry in [
        ModelCapabilities::new(
            "deepseek-chat", "DeepSeek Chat (旧名)", "DeepSeek",
            false, true, 64_000, 0.00014, 0.00028,
            "2026/07 后将弃用，等同于 deepseek-v4-flash",
        ),
        ModelCapabilities::new(
            "deepseek-v4-flash", "DeepSeek V4 Flash", "DeepSeek",
            false, true, 64_000, 0.00014, 0.00028,
            "快速响应，适合日常对话",
        ),
        ModelCapabilities::new(
            "deepseek-v4-pro", "DeepSeek V4 Pro", "DeepSeek",
            false, true, 64_000, 0.00027, 0.00110,
            "强推理，适合复杂任务",
        ),
        ModelCapabilities::new(
            "deepseek-reasoner", "DeepSeek Reasoner", "DeepSeek",
            false, true, 64_000, 0.00055, 0.00219,
            "含思维链，2026/07 后弃用",
        ),
    ] {
        m.insert(entry.model_id.clone(), entry);
    }

    // ── OpenAI ────────────────────────────────────────────────────────────────
    for entry in [
        ModelCapabilities::new(
            "gpt-4o", "GPT-4o", "OpenAI",
            true, true, 128_000, 0.0025, 0.010,
            "旗舰视觉+文字，推荐用作视觉回退模型",
        ),
        ModelCapabilities::new(
            "gpt-4o-mini", "GPT-4o Mini", "OpenAI",
            true, true, 128_000, 0.00015, 0.0006,
            "轻量视觉模型，价格极低，适合图片描述",
        ),
        ModelCapabilities::new(
            "gpt-4.1", "GPT-4.1", "OpenAI",
            true, true, 1_000_000, 0.002, 0.008,
            "超长上下文，支持视觉",
        ),
        ModelCapabilities::new(
            "o4-mini", "o4-mini", "OpenAI",
            true, true, 200_000, 0.0011, 0.0044,
            "轻量推理+视觉",
        ),
    ] {
        m.insert(entry.model_id.clone(), entry);
    }

    // ── Anthropic Claude ──────────────────────────────────────────────────────
    for entry in [
        ModelCapabilities::new(
            "claude-opus-4-5", "Claude Opus 4.5", "Anthropic",
            true, true, 200_000, 0.015, 0.075,
            "旗舰模型，强视觉理解",
        ),
        ModelCapabilities::new(
            "claude-sonnet-4-5", "Claude Sonnet 4.5", "Anthropic",
            true, true, 200_000, 0.003, 0.015,
            "性价比高，推荐视觉回退",
        ),
        ModelCapabilities::new(
            "claude-haiku-3-5", "Claude Haiku 3.5", "Anthropic",
            true, true, 200_000, 0.0008, 0.004,
            "最快最便宜，适合轻量视觉任务",
        ),
    ] {
        m.insert(entry.model_id.clone(), entry);
    }

    // ── Google Gemini ─────────────────────────────────────────────────────────
    for entry in [
        ModelCapabilities::new(
            "gemini-2.5-pro", "Gemini 2.5 Pro", "Google",
            true, true, 1_048_576, 0.00125, 0.010,
            "超长上下文，原生多模态",
        ),
        ModelCapabilities::new(
            "gemini-2.5-flash", "Gemini 2.5 Flash", "Google",
            true, true, 1_048_576, 0.000075, 0.0003,
            "极速极便宜的视觉模型",
        ),
    ] {
        m.insert(entry.model_id.clone(), entry);
    }

    // ── OpenRouter（一个 Key 访问所有模型）────────────────────────────────────
    // OpenRouter 使用标准 OpenAI 格式，model 字段为 "provider/model-name"
    for entry in [
        ModelCapabilities::new(
            "openai/gpt-4o", "GPT-4o (via OpenRouter)", "OpenRouter",
            true, true, 128_000, 0.0025, 0.010,
            "通过 OpenRouter 访问，一个 Key 搞定",
        ),
        ModelCapabilities::new(
            "anthropic/claude-sonnet-4-5", "Claude Sonnet 4.5 (via OpenRouter)", "OpenRouter",
            true, true, 200_000, 0.003, 0.015,
            "通过 OpenRouter 访问",
        ),
        ModelCapabilities::new(
            "deepseek/deepseek-chat-v3-0324", "DeepSeek Chat V3 (via OpenRouter)", "OpenRouter",
            false, true, 64_000, 0.00014, 0.00028,
            "通过 OpenRouter 访问 DeepSeek",
        ),
        ModelCapabilities::new(
            "meta-llama/llama-4-scout", "Llama 4 Scout (via OpenRouter)", "OpenRouter",
            true, true, 512_000, 0.00011, 0.00011,
            "Meta 免费视觉模型",
        ),
    ] {
        m.insert(entry.model_id.clone(), entry);
    }

    // ── Ollama 本地模型（按模型名推断能力）────────────────────────────────────
    for entry in [
        ModelCapabilities::new(
            "llava", "LLaVA (本地)", "Ollama",
            true, false, 4_096, 0.0, 0.0,
            "本地视觉模型，免费",
        ),
        ModelCapabilities::new(
            "llava:13b", "LLaVA 13B (本地)", "Ollama",
            true, false, 4_096, 0.0, 0.0,
            "本地视觉模型，免费",
        ),
        ModelCapabilities::new(
            "llama3", "Llama 3 (本地)", "Ollama",
            false, true, 8_192, 0.0, 0.0,
            "本地模型，免费",
        ),
        ModelCapabilities::new(
            "mistral", "Mistral (本地)", "Ollama",
            false, true, 8_192, 0.0, 0.0,
            "本地模型，免费",
        ),
    ] {
        m.insert(entry.model_id.clone(), entry);
    }

    m
}

// ─── 公共 API ─────────────────────────────────────────────────────────────────

/// 查询模型能力（未知模型返回保守的默认值）
pub fn lookup(model_id: &str) -> ModelCapabilities {
    // 精确匹配
    if let Some(c) = REGISTRY.get(model_id) {
        return c.clone();
    }

    // 前缀模糊匹配（如 "gpt-4o-2024-11-20" → GPT-4o 能力）
    for (key, cap) in REGISTRY.iter() {
        if model_id.starts_with(key.as_str()) || key.starts_with(model_id) {
            return cap.clone();
        }
    }

    // 通过模型名推断部分能力（Ollama 自定义模型等）
    let supports_vision = model_id.contains("vision")
        || model_id.contains("llava")
        || model_id.contains("vl")
        || model_id.contains("4o")
        || model_id.contains("gemini")
        || model_id.contains("claude");

    ModelCapabilities::new(
        model_id, model_id, "Unknown",
        supports_vision, true, 4_096, 0.0, 0.0,
        "未知模型，能力为推断值",
    )
}

/// 返回所有已知模型列表（用于前端下拉选择）
pub fn all_known() -> Vec<ModelCapabilities> {
    let mut list: Vec<ModelCapabilities> = REGISTRY.values().cloned().collect();
    list.sort_by(|a, b| a.provider.cmp(&b.provider).then(a.model_id.cmp(&b.model_id)));
    list
}
