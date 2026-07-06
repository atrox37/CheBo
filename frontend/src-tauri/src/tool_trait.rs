// ─── tool_trait.rs ────────────────────────────────────────────────────────────
// Tool 统一抽象层（参考 OpenHuman tool trait 设计）
#![allow(dead_code)]
//
// 所有工具实现同一个 Tool trait：
//   name()               → 工具名（模型调用时用）
//   description()        → 工具说明（注入 system prompt）
//   parameters_schema()  → JSON 参数 schema（告诉模型传什么参数）
//   permission_level()   → 权限等级 L0-L3
//   category()           → 工具分类（用于前端分组显示）
//   enabled_by_default() → 默认是否开启
//   execute()            → 实际执行
//
// 工具调用格式（XML-JSON 混合，兼容 DeepSeek/Ollama 等非 native 模型）：
//   <tool_call>
//   {"name":"web_search","arguments":{"query":"..."}}
//   </tool_call>
// ─────────────────────────────────────────────────────────────────────────────

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ─── 权限等级 ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ToolPermission {
    /// L0: 只读、无副作用（web_search、read_file 只读、list_dir）
    L0,
    /// L1: 查询类系统操作（git_status 等只读 git）
    L1,
    /// L2: 写入类操作（写文件、修改配置）需用户确认
    L2,
    /// L3: 高危操作（shell 命令、删除）需显式用户确认
    L3,
}

impl ToolPermission {
    pub fn label(&self) -> &str {
        match self {
            Self::L0 => "只读",
            Self::L1 => "查询",
            Self::L2 => "写入（需确认）",
            Self::L3 => "高危（需确认）",
        }
    }
    pub fn color_tag(&self) -> &str {
        match self {
            Self::L0 => "green",
            Self::L1 => "blue",
            Self::L2 => "orange",
            Self::L3 => "red",
        }
    }
    pub fn needs_confirmation(&self) -> bool {
        matches!(self, Self::L2 | Self::L3)
    }
}

// ─── 工具分类 ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolCategory {
    File,      // 文件操作：read/write/search
    Web,       // 网络：search/fetch/download
    System,    // 系统：shell/process/window
    Memory,    // 记忆：memory_recall/note
    Git,       // Git：status/commit
    Media,     // 媒体：screenshot/image/audio
    Code,      // 代码：grep/definition/run
    Clipboard, // 剪贴板
}

impl ToolCategory {
    pub fn label(&self) -> &str {
        match self {
            Self::File     => "文件",
            Self::Web      => "网络",
            Self::System   => "系统",
            Self::Memory   => "记忆",
            Self::Git      => "Git",
            Self::Media    => "媒体",
            Self::Code     => "代码",
            Self::Clipboard => "剪贴板",
        }
    }
}

// ─── 工具调用请求和结果 ───────────────────────────────────────────────────────

/// LLM 生成的一次工具调用请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRequest {
    /// 调用 id（供日志和前端跟踪）
    pub id:        String,
    /// 工具名
    pub name:      String,
    /// 参数（已解析为 JSON Value）
    pub arguments: Value,
}

/// 工具执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallResult {
    pub id:          String,
    pub name:        String,
    pub success:     bool,
    /// 输出内容（会被裁剪到 MAX_TOOL_OUTPUT 字符）
    pub output:      String,
    pub permission:  ToolPermission,
}

impl ToolCallResult {
    pub const MAX_OUTPUT: usize = 2000;

    pub fn ok(id: &str, name: &str, perm: ToolPermission, output: String) -> Self {
        let output = if output.len() > Self::MAX_OUTPUT {
            format!("{}…（输出已截断，原长 {} 字符）", &output[..Self::MAX_OUTPUT], output.len())
        } else {
            output
        };
        Self { id: id.to_string(), name: name.to_string(), success: true, output, permission: perm }
    }

    pub fn err(id: &str, name: &str, perm: ToolPermission, msg: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            success: false,
            output: format!("工具执行失败: {msg}"),
            permission: perm,
        }
    }

    /// 格式化成回写给 LLM 的文本
    pub fn to_context_text(&self) -> String {
        if self.success {
            format!("[工具 {} 结果]\n{}", self.name, self.output)
        } else {
            format!("[工具 {} 错误]\n{}", self.name, self.output)
        }
    }
}

// ─── 工具 Spec（用于注入 prompt 和 native function calling） ─────────────────

/// 工具描述（用于 system prompt 注入 / OpenAI tools array）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name:        String,
    pub description: String,
    /// 简化的参数说明（key: type - description）
    pub params:      Vec<ToolParam>,
    pub permission:  ToolPermission,
    pub category:    ToolCategory,
    /// 默认是否开启（用户可在设置中开关）
    pub enabled_by_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParam {
    pub name:        String,
    pub ty:          String,   // "string" | "number" | "boolean"
    pub description: String,
    pub required:    bool,
}

impl ToolSpec {
    /// 生成一行 prompt 描述
    pub fn one_line(&self) -> String {
        let params_desc: Vec<String> = self.params
            .iter()
            .map(|p| {
                if p.required {
                    format!("{}: {} (必填)", p.name, p.ty)
                } else {
                    format!("{}: {} (可选)", p.name, p.ty)
                }
            })
            .collect();
        format!(
            "- `{}` [{}] — {} | 参数: {}",
            self.name,
            self.permission.label(),
            self.description,
            if params_desc.is_empty() { "无".to_string() } else { params_desc.join(", ") }
        )
    }

    /// 生成 OpenAI native function calling 格式
    pub fn to_openai_function(&self) -> Value {
        let mut properties = serde_json::Map::new();
        let mut required_fields: Vec<Value> = Vec::new();

        for p in &self.params {
            properties.insert(p.name.clone(), serde_json::json!({
                "type": p.ty,
                "description": p.description,
            }));
            if p.required {
                required_fields.push(Value::String(p.name.clone()));
            }
        }

        serde_json::json!({
            "type": "function",
            "function": {
                "name": self.name,
                "description": self.description,
                "parameters": {
                    "type": "object",
                    "properties": properties,
                    "required": required_fields,
                }
            }
        })
    }
}

// ─── Tool trait ───────────────────────────────────────────────────────────────

/// 所有工具必须实现此 trait
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn params(&self) -> Vec<ToolParam>;
    fn permission_level(&self) -> ToolPermission;

    /// 工具分类（用于前端分组显示）
    fn category(&self) -> ToolCategory {
        ToolCategory::System
    }

    /// 默认是否开启（用户可在设置中关闭）
    fn enabled_by_default(&self) -> bool {
        true
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name:        self.name().to_string(),
            description: self.description().to_string(),
            params:      self.params(),
            permission:  self.permission_level(),
            category:    self.category(),
            enabled_by_default: self.enabled_by_default(),
        }
    }

    async fn execute(&self, id: &str, args: Value) -> ToolCallResult;
}