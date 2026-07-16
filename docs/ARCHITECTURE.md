# Chebo — 详细架构说明文档

> 版本：2026-05  
> 技术栈：Tauri 2 + Vue 3 + Rust + SQLite  
> 本文描述各子系统的设计原理、执行流程与模块边界。

---

## 目录

1. [整体架构概览](#1-整体架构概览)
2. [Tauri IPC 通信层](#2-tauri-ipc-通信层)
3. [Agent Runtime（智能体状态机）](#3-agent-runtime-智能体状态机)
4. [Tool System（工具系统）](#4-tool-system-工具系统)
5. [Task System（任务系统）](#5-task-system-任务系统)
6. [Memory System（记忆系统）](#6-memory-system-记忆系统)
7. [Perception System（感知系统）](#7-perception-system-感知系统)
8. [AI Layer（模型调度层）](#8-ai-layer-模型调度层)
9. [Pet System（桌宠状态系统）](#9-pet-system-桌宠状态系统)
10. [Sandbox（安全沙盒）](#10-sandbox-安全沙盒)
11. [Frontend（前端 Vue 层）](#11-frontend-前端-vue-层)
12. [数据库结构](#12-数据库结构)
13. [完整数据流：用户发消息的全链路](#13-完整数据流用户发消息的全链路)
14. [与同类产品对比](#14-与同类产品对比)

---

## 1. 整体架构概览

```
┌─────────────────────────────────────────────────────────────────┐
│                      Frontend (Vue 3 + Vite)                    │
│  ┌──────────┐  ┌───────────────┐  ┌──────────────────────────┐  │
│  │ PetMode  │  │ AssistantMode │  │     Pinia Stores          │  │
│  │  (桌宠)  │  │   (助手模式)  │  │  chatStore / petStore...  │  │
│  └────┬─────┘  └───────┬───────┘  └──────────────────────────┘  │
│       └────────────────┴──────── invoke / listen ───────────────┤
├─────────────────────────────────────────────────────────────────┤
│                   Tauri IPC Bridge (commands.rs)                │
│   send_message / get_status / execute_tool / task_create ...    │
├─────────────────────────────────────────────────────────────────┤
│                      Rust Backend                               │
│                                                                 │
│  ┌──────────────┐  ┌────────────┐  ┌──────────────────────┐    │
│  │ Agent Runtime│  │ Tool System│  │    Task System        │    │
│  │  (状态机)    │  │ (工具注册) │  │  (长期任务调度)       │    │
│  └──────┬───────┘  └─────┬──────┘  └──────────┬───────────┘    │
│         │                │                     │                │
│  ┌──────┴───────────────────────────────────┐  │                │
│  │           Event Bus (broadcast)          │  │                │
│  └──────────────────────────────────────────┘  │                │
│                                                │                │
│  ┌──────────────┐  ┌────────────┐  ┌──────────┴─────────────┐  │
│  │ Memory System│  │ Perception │  │   AI Layer / LLM       │  │
│  │ (SQLite+Vault│  │  (感知)    │  │  Provider Registry     │  │
│  └──────┬───────┘  └─────┬──────┘  └────────────────────────┘  │
│         │                │                                      │
│  ┌──────┴────────────────┴──────────────────────────────────┐   │
│  │                 SQLite (chebo.db)                        │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

**进程模型：**  
- 只有 1 个进程（Tauri WebView + Rust 在同进程内）  
- Rust 后端运行在 `tokio` 异步运行时上  
- 前端是 WebView（Chromium/WebKit），通过 IPC 与 Rust 通信  
- 所有持久化数据存在 `%AppData%\Chebo\chebo.db`

---

## 2. Tauri IPC 通信层

### 2.1 前端 → 后端：`invoke`

```
Frontend                         Rust (commands.rs)
  │                                      │
  │──── invoke('send_message', {...}) ───►│
  │                                      │  #[tauri::command]
  │                                      │  pub async fn send_message(...)
  │◄─── return / Result ─────────────────│
```

**所有 Tauri 命令（`src-tauri/src/commands.rs`）：**

| 命令 | 说明 |
|------|------|
| `send_message` | 发送聊天消息，触发 LLM 流式响应 |
| `get_status` | 获取桌宠当前状态（饥饿/精力/心情等） |
| `feed` | 投喂食物，更新状态 |
| `buy_item` / `get_foods` | 商店操作 |
| `execute_tool` | 直接执行 L0/L1 工具 |
| `confirm_tool_call` | 用户批准 L2/L3 工具 |
| `get_app_config` | 读取 LLM 配置（脱敏） |
| `update_app_config` | 热更新 LLM / Vision 配置 |
| `get_sandbox_paths` | 读取沙盒允许路径 |
| `set_sandbox_paths` | 更新沙盒允许路径（立即生效） |
| `get_model_capabilities` | 查询模型能力（视觉/工具支持） |
| `task_create` / `task_list` ... | Task System 操作 |
| `get_vault_stats` / `trigger_vault_sync` | Memory Vault 操作 |

### 2.2 后端 → 前端：`app.emit`

Rust 后端通过事件推送实时状态：

| 事件名 | 触发场景 | 数据 |
|--------|----------|------|
| `chat_token` | LLM 流式输出每个字 | `{token: "你好"}` |
| `chat_done` | LLM 回复完成 | `{role, content, emotion}` |
| `backend_error` | 后端报错 | `{message: "..."}` |
| `pet_status_update` | 桌宠状态变化 | `PetStatus` |
| `proactive_speech` | Chebo 主动发言 | `{text, emotion}` |
| `agent_state_changed` | Agent 状态机跳转 | `{state: "Thinking"}` |
| `tool_permission_request` | L2/L3 工具申请确认 | `{token, tool, args}` |
| `task_step_thinking` | 任务执行中间状态 | `{task_id, step, thought}` |
| `task_completed` | 任务完成 | `{task_id, result}` |
| `vault_sync_done` | Vault 同步完成 | `{}` |

---

## 3. Agent Runtime（智能体状态机）

**文件：** `src-tauri/src/agent.rs`

### 3.1 状态定义

```
Idle ──[收到消息]──► Thinking ──[有工具调用]──► UsingTool
  ▲                     │                          │
  │           [回复完成]  │           [工具完成]     │
  │◄──────────────────── │◄────────────────────────┘
  │
  ├─[主动发言]──► Talking ──[说完]──► Idle
  ├─[执行任务]──► Working
  ├─[等待确认]──► WaitingConfirm
  └─[出错]──────► Error ──[恢复]──► Idle
```

完整状态（`AgentState` enum）：
- `Idle` — 待机
- `Thinking` — 正在思考/等待 LLM 响应
- `Talking` — 主动发言中（proactive speech）
- `UsingTool` — 工具执行中
- `Working` — 长期任务运行中
- `WaitingConfirm` — 等待用户批准高权限工具
- `Sleeping` — 低活跃度节能模式
- `Happy` / `Sad` / `Surprised` — 情绪状态（临时）
- `Error` — 异常状态

### 3.2 状态迁移规则

```rust
// 进入 Thinking（要求当前是 Idle 才允许）
pub fn try_start_thinking(&self, app: &AppHandle) -> bool

// 设置任意状态（内部使用）
pub fn set_state(&self, new_state: AgentState, app: &AppHandle)

// 重置回 Idle
pub fn reset_to_idle(&self, app: &AppHandle)
```

每次状态变化都会触发 `agent_state_changed` 事件推送给前端，前端据此更新图标/动画。

---

## 4. Tool System（工具系统）

**文件：** `src-tauri/src/tools.rs`, `tool_registry.rs`, `tool_dispatcher.rs`

### 4.1 工具权限等级

| 等级 | 名称 | 是否需要确认 | 示例 |
|------|------|------------|------|
| L0 | 只读安全 | 否，立即执行 | `read_file`, `list_dir`, `clipboard_read` |
| L1 | 只读网络 | 否，立即执行 | `web_search` |
| L2 | 写操作 | 需要用户点击确认 | `write_file`, `git_commit` |
| L3 | 系统控制 | 需要用户点击确认 | `safe_shell`, `desktop_control` |

### 4.2 工具注册流程

```rust
// tool_registry.rs
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn permission_level(&self) -> ToolLevel;
    fn description(&self) -> &str;
    async fn execute(&self, args: &serde_json::Value) -> ToolResult;
}
```

内置工具：

| 工具名 | 权限 | 功能 |
|--------|------|------|
| `read_file` | L0 | 读取文件内容（受沙盒路径限制） |
| `list_dir` | L0 | 列出目录 |
| `clipboard_read` | L0 | 读取剪贴板 |
| `web_search` | L1 | 网络搜索（当前占位实现） |
| `write_file` | L2 | 写入文件 |
| `git_status` / `git_diff` | L1 | Git 查询 |
| `git_commit` | L2 | Git 提交 |
| `safe_shell` | L3 | 执行 Shell 命令（黑名单过滤） |
| `memory_recall` | L0 | 查询记忆 |
| `screenshot` | L1 | 截图分析 |

### 4.3 语义工具路由（Semantic Router）

LLM 的回复不需要手动指定工具，由 **3 级语义路由** 自动决策：

```
用户输入
    │
    ▼
[1] 关键词匹配（快速路径）
    "帮我看看" → read_file
    "截图" → screenshot
    "搜索" → web_search
    │
    ▼（无匹配）
[2] LLM 意图启发式（Heuristic）
    分析动词+名词组合推断工具类别
    │
    ▼（置信度 < 0.6）
[3] 直接文本回复（不调用工具）
```

### 4.4 工具调用格式（XML）

LLM 在回复中嵌入 XML 格式调用工具：

```xml
<tool_call>
  <name>read_file</name>
  <args>
    <path>C:\Users\me\Documents\note.txt</path>
  </args>
</tool_call>
```

Tool Dispatcher 解析 XML → 检查权限 → 执行或请求确认 → 返回结果。

### 4.5 Tool Loop（Agent 循环）

```
用户消息
    │
    ▼
LLM 调用（stream_chat）
    │
    ├── 无工具调用 → 直接回复用户
    │
    └── 检测到 <tool_call>
            │
            ├── L0/L1：立即执行 → 结果注入上下文 → 继续 LLM 循环（最多 5 轮）
            │
            └── L2/L3：emit tool_permission_request → 等待用户确认（30s 超时）
                         ├── 用户批准 → 执行 → 继续循环
                         └── 用户拒绝 → 告知 LLM → 结束本轮
```

---

## 5. Task System（任务系统）

**文件：** `src-tauri/src/task/`（`task_manager.rs`, `task_planner.rs`, `task_executor.rs`, `task_store.rs`, `task_events.rs`）

### 5.1 任务生命周期

```
创建（task_create）
    │
    ▼
规划（task_planner.rs）
    LLM 把目标分解为步骤列表 Steps[]
    │
    ▼
执行（task_executor.rs）
    逐步执行，每步：
      LLM 选择工具 → 调用 ToolDispatcher → 获取结果 → 决定下一步
    │
    ├── 步骤需要用户确认 → 暂停（Paused）→ 等待批准
    ├── 步骤失败且可重试 → 自动重试（最多 3 次）
    └── 全部步骤完成 → 完成（Completed）
```

### 5.2 任务状态

| 状态 | 说明 |
|------|------|
| `Pending` | 已创建，等待规划 |
| `Planning` | LLM 规划步骤中 |
| `Running` | 执行中 |
| `Paused` | 等待用户批准或手动暂停 |
| `Completed` | 成功完成 |
| `Failed` | 最终失败 |
| `Cancelled` | 用户取消 |

### 5.3 应用重启恢复

应用启动后 3 秒，`TaskManager::resume_interrupted_on_startup()` 自动将 `Running` 状态的任务恢复执行（防止崩溃导致任务丢失）。

### 5.4 前端实时显示

每执行一步都会 emit `task_step_thinking` 事件，前端 `AgentTaskPanel.vue` 实时显示思考过程，类似 "思维链" 可视化。

---

## 6. Memory System（记忆系统）

**文件：** `src-tauri/src/memory.rs`, `memory_tree.rs`, `vault.rs`

> 设计目标：让 Chebo 记住"你是谁、你喜欢什么、我们聊过什么"，即使重启应用也不会遗忘。

---

### 6.1 四层记忆结构（从短到长）

```
┌─────────────────────────────────────────────────────────────────┐
│  层 0 — 当前对话上下文（In-Memory，会话结束即丢弃）             │
│         消息列表保存在 Arc<Mutex<Vec<LlmMessage>>>              │
│         Token 超出上限时从头部裁剪（滑动窗口）                  │
├─────────────────────────────────────────────────────────────────┤
│  层 1 — 会话历史（SQLite: messages 表）                         │
│         每条消息永久保存，包含 role/content/emotion/timestamp   │
│         每次对话注入最近 10 条（max_history_messages 配置）     │
├─────────────────────────────────────────────────────────────────┤
│  层 2 — 用户档案（SQLite: user_profile 表）                     │
│         LLM 从对话中自动提取用户偏好/习惯/事实                  │
│         每条带 confidence（0~1）和 source（来源会话 ID）        │
├─────────────────────────────────────────────────────────────────┤
│  层 3 — 长期摘要（SQLite: memory_summaries + Vault .md 文件）   │
│         每个会话结束后，LLM 生成 3 级摘要树，永久保存           │
│         同步到本地 Markdown 文件，可用 Obsidian 打开            │
└─────────────────────────────────────────────────────────────────┘
```

---

### 6.2 各层详细说明

#### 层 0：当前会话上下文

```rust
// commands.rs::send_message
let history = db::get_recent_messages(&pool, &session_id, max_hist).await?;
// history 是最近 max_hist 条消息，作为 LLM messages 数组直接传入
```

- 存在 SQLite 里，每次对话时从数据库读出最近 N 条
- `max_history_messages` 默认 10 条（约 2000~4000 tokens）
- 防止 Token 超限：`max_hist` 可在 `app_config` 表中调整

#### 层 1：会话历史（持久化）

```sql
-- messages 表结构
CREATE TABLE messages (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT    NOT NULL,  -- 会话标识（UUID）
    role       TEXT    NOT NULL,  -- "user" | "assistant" | "system"
    content    TEXT    NOT NULL,  -- 消息正文
    emotion    TEXT,              -- 情绪标签（happy/sad/neutral...）
    motion     TEXT,              -- 动作标签
    created_at TEXT    NOT NULL   -- ISO 8601 时间戳
);
```

- 每次发消息/收到回复都写入
- 支持按 `session_id`、`created_at` 查询
- 前端的"历史记录"弹窗从此表按日期过滤显示

#### 层 2：用户档案（语义提取）

```sql
-- user_profile 表结构
CREATE TABLE user_profile (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    key        TEXT    NOT NULL UNIQUE,  -- 属性名，如 "user_name", "prefers_language"
    value      TEXT    NOT NULL,         -- 属性值，如 "小明", "中文"
    confidence REAL    DEFAULT 0.8,      -- 可信度（0~1）
    source     TEXT,                     -- 来源会话 ID
    updated_at TEXT    NOT NULL
);
```

**生成过程：**

```
每 N 条对话后（或 memory_tree 同步时）
    │
    ▼
LLM 分析本次对话，提取 JSON：
    {
      "user_name": "小明",
      "occupation": "程序员",
      "prefers_language": "中文",
      "hobby": "打游戏",
      "dislikes": "被催促"
    }
    │
    ▼
对每条信息：
    - 如果 key 已存在且新 confidence > 旧 confidence → 更新
    - 如果是新 key → 插入
    - confidence < 0.5 的条目会被标记为"待验证"
```

**注入到 Prompt 的格式：**

```
【关于用户的记忆】
- 名字：小明（置信度：0.95）
- 职业：程序员（置信度：0.88）
- 语言偏好：中文（置信度：1.0）
- 兴趣：打游戏（置信度：0.72）
```

#### 层 3：长期摘要 + Memory Tree

每个会话结束（或触发 Vault 同步）时，`memory_tree::sync_session()` 执行：

```
会话消息
    │
    ▼
[L0 摘要] LLM 生成 200 字以内的本次对话摘要
    │
    ▼
[L1 摘要] 将多个 L0 摘要合并为更精简的摘要（~500 字，跨多个会话）
    │
    ▼
[L2 摘要] 进一步压缩，提取长期规律和用户画像要点（~200 字）
    │
    ▼
[L3 摘要] 最终压缩为 Chebo 对你的整体认知（~100 字，极度精简）
```

**存储位置：**

| 存储 | 路径 | 用途 |
|------|------|------|
| SQLite | `memory_summaries` 表 | 程序快速读取 |
| Vault 文件 | `%AppData%\Chebo\vault\sessions\{date}-{id}.md` | 人工查阅/Obsidian |
| Vault 摘要 | `%AppData%\Chebo\vault\summaries\L0/L1/L2/L3/` | 层级摘要树 |

---

### 6.3 记忆检索（Recall）

每次用户发消息时，`memory::recall_relevant()` 从记忆中检索相关内容：

```rust
// memory.rs
pub async fn recall_relevant(pool: &SqlitePool, query: &str) -> Result<String> {
    // 1. 从 user_profile 取所有档案（高 confidence 优先）
    // 2. 从 memory_summaries 取最近 3 条 L2/L3 级摘要
    // 3. 格式化为 Prompt 文本并返回
}
```

**检索策略（当前版本）：**

| 数据源 | 检索方式 | 说明 |
|--------|----------|------|
| `user_profile` | 全量加载（数量不多） | 所有已知用户偏好都注入 |
| `memory_summaries` | 最近 L2~L3 级摘要 | 按时间取最近 3 条 |
| 当前会话历史 | 最近 10 条 messages | 直接从数据库读取 |

> **未来升级方向：** 引入向量嵌入（Embedding）进行语义相似度检索，对话量大后可以只注入真正"相关"的记忆，而不是简单取最近几条。参考：[MemGPT/Letta](https://github.com/letta-ai/letta)

---

### 6.4 完整 Prompt 注入流程

```rust
// commands.rs::send_message 内部
let memories = memory::recall_relevant(&pool, &content).await?;
let pet_status = db::get_pet_status(&pool).await?;
let system_prompt = character::build_system_prompt(&pet_status, &memories);

// 最终 System Prompt 结构：
// ┌─────────────────────────────────────────┐
// │ [角色定义] 你是 Chebo...                │
// │ [状态] 饥饿:80/精力:65/心情:70          │
// │ [动态人格修饰符] 你现在状态不错...       │
// │ [用户档案] 名字:小明/职业:程序员...      │
// │ [相关记忆] 上次我们聊了...               │
// │ [会话历史] 最近10条消息                  │
// └─────────────────────────────────────────┘
```

---

### 6.5 Vault 同步机制

```
Vault 同步循环（每 20 分钟自动 / 手动触发）
    │
    ├── 遍历所有 session_id
    │       ├── 如果会话有新消息 → 生成/更新 L0 摘要
    │       └── 写入 vault/sessions/{date}-{id}.md
    │
    ├── 合并多个 L0 → L1 摘要
    ├── 合并多个 L1 → L2 摘要
    └── 合并多个 L2 → L3 摘要（整体认知快照）
```

**手动同步：** 助手模式 → 设置 → "立即同步 Vault" 按钮  
**查看 Vault：** 用 Obsidian 打开 `%AppData%\Chebo\vault\` 目录

---

### 6.6 记忆管理（前端 UI）

助手模式 → "记忆" Tab 提供：

| 功能 | 说明 |
|------|------|
| 查看用户档案 | 显示所有 `user_profile` 条目及其可信度 |
| 编辑/删除档案 | 可手动修正错误的记忆 |
| 查看摘要树 | 展示 L0~L3 级摘要内容 |
| 历史记录查询 | 按日期过滤完整对话历史 |
| Vault 统计 | 显示已同步的会话数和文件数 |

---

### 6.7 隐私说明

- **所有记忆数据存在本地**，位于 `%AppData%\Chebo\`，不上传到任何服务器
- LLM API 调用中会包含记忆摘要（发送给 DeepSeek/OpenAI 等），这是 AI 理解上下文的必要前提
- 可随时在"记忆" Tab 删除任意条目
- 删除 `chebo.db` 文件可彻底清空所有记忆

---

## 7. Perception System（感知系统）

**文件：** `src-tauri/src/perception.rs`

### 7.1 感知循环（每 5 秒执行）

```
perception_loop
    │
    ├── 获取当前活跃窗口标题（Windows API）
    │       变化时 → emit "context_window_changed" 事件
    │
    ├── 读取剪贴板内容
    │       变化时 → emit "clipboard_changed" 事件
    │
    └── 计算用户空闲时长（鼠标无移动）
            超过阈值时 → Agent 可触发主动发言
```

### 7.2 感知状态（PerceptionState）

```rust
pub struct PerceptionState {
    pub active_window:   String,
    pub clipboard_text:  String,
    pub idle_secs:       u64,
    pub last_active_at:  Instant,
}
```

感知结果注入 `ProactiveGuard` 和 Agent 的上下文决策。

---

## 8. AI Layer（模型调度层）

**文件：** `src-tauri/src/llm.rs`, `provider_registry.rs`, `character.rs`

### 8.1 Provider 能力注册表

`provider_registry.rs` 记录所有已知模型的能力：

```rust
pub struct ModelCapabilities {
    pub model_id:          String,
    pub supports_vision:   bool,    // 是否支持图片输入
    pub supports_tools:    bool,    // 是否支持 Function Calling
    pub context_window:    u32,     // 上下文窗口大小（tokens）
    pub cost_input_per_1k: f64,     // 每千 token 输入成本（USD）
    ...
}
```

已注册模型（部分）：

| 模型 | 提供商 | 视觉 | 工具 | 上下文 |
|------|--------|------|------|--------|
| deepseek-v4-flash | DeepSeek | ✗ | ✓ | 64K |
| deepseek-r1 | DeepSeek | ✗ | ✗ | 64K |
| gpt-4o | OpenAI | ✓ | ✓ | 128K |
| gpt-4o-mini | OpenAI | ✓ | ✓ | 128K |
| claude-opus-4-5 | Anthropic | ✓ | ✓ | 200K |
| gemini-2.5-pro | Google | ✓ | ✓ | 1M |
| openai/gpt-4o | OpenRouter | ✓ | ✓ | 128K |
| llava | Ollama（本地） | ✓ | ✗ | 4K |

### 8.2 Vision 路由逻辑

用户发送含图片的消息时：

```
用户发送图片
      │
      ▼
检查主模型能力 (provider_registry::lookup)
      │
      ├── supports_vision = true
      │       → 图片以 base64 data URL 直接传入 LLM
      │
      ├── supports_vision = false，且配置了 Vision 回退模型
      │       → 调用回退模型（如 gpt-4o-mini）描述图片
      │       → 将描述文字注入主模型上下文
      │
      └── 无视觉支持，无回退
              → 提示用户"当前模型不支持图片"
```

### 8.3 流式响应实现

```rust
// llm.rs
pub async fn stream_chat(
    app: &AppHandle,
    messages: Vec<LlmMessage>,
    images: &[String],
    cfg: &LlmConfig,
) {
    // 1. 构建 OpenAI-compatible 请求
    // 2. reqwest POST，stream = true
    // 3. 逐行读取 SSE 数据
    // 4. 每个 token 通过 app.emit("chat_token", ...) 推送给前端
    // 5. 完成时 emit "chat_done"
}
```

### 8.4 角色人格注入（character.rs）

每次调用 LLM 前，构建包含以下内容的 System Prompt：

```
你是 Chebo，一个可爱的 AI 桌面伙伴...

【当前状态】
- 饥饿度：{hunger}/100
- 精力：{energy}/100
- 心情：{mood}/100
- 当前行为：{current_action}

【动态人格修饰符】
{根据状态生成：如"你现在有点饿，回复时略显慵懒"}

【相关记忆】
{从 memory 系统检索到的相关内容}

【用户档案】
{user_profile 摘要}
```

---

## 9. Pet System（桌宠状态系统）

**文件：** `src-tauri/src/pet.rs`

### 9.1 核心状态

| 属性 | 范围 | 说明 |
|------|------|------|
| `hunger` | 0~100 | 饥饿度（越低越饿） |
| `energy` | 0~100 | 精力 |
| `mood` | 0~100 | 心情 |
| `affection` | 0~100 | 好感度 |
| `level` | 1~∞ | 等级 |
| `exp` | 0~max | 经验值 |
| `coins` | 0~∞ | 金币 |

### 9.2 后台衰减机制（每 60 秒）

```
background_tick（每分钟）
    │
    ├── hunger -= 0.5（每分钟减少）
    ├── energy -= 0.3
    ├── mood -= 0.2（受 hunger/energy 影响）
    │
    ├── 检查 keepPerfect 模式
    │       true → 自动补满所有状态（用于演示）
    │
    └── 更新数据库 → emit pet_status_update
```

### 9.3 主动发言策略（ProactiveGuard）

```
ai_comment_loop（每 5~15 分钟随机触发）
    │
    ├── 检查关怀模式（care_mode = false → 跳过）
    ├── 检查 Agent 状态（非 Idle → 跳过）
    ├── 检查上次发言时间（冷却期内 → 跳过）
    │
    ├── 构建 Prompt：当前状态 + 感知信息
    ├── LLM 生成主动发言文本
    └── emit proactive_speech → 前端显示气泡
```

---

## 10. Sandbox（安全沙盒）

**文件：** `src-tauri/src/sandbox.rs`

### 10.1 三层防护

#### 层 1：路径白名单
```
工具尝试读取文件
    │
    ▼
检查是否为系统保护路径（硬拒绝）：
    C:\Windows\, C:\Program Files\, /etc/, /sys/ ...
    │
    ▼
检查是否在 allowed_paths 白名单内
    允许 → 继续执行
    拒绝 → 返回错误 + 记录审计日志
```

默认允许路径：`~/Documents`, `~/Desktop`, `~/Downloads`, `~/Projects`, `~/dev`, `~/code`

**用户可通过设置页面（助手模式 → 设置）添加/删除路径，立即生效。**

#### 层 2：Shell 命令黑名单
拒绝包含以下关键词的 Shell 命令：
`rm -rf`, `rmdir /s`, `format`, `shutdown`, `sudo rm`, `curl | sh` 等

#### 层 3：速率限制（每小时）

| 工具 | 限制次数 |
|------|---------|
| `web_search` | 30 次 |
| `safe_shell` | 10 次 |
| `read_file` | 60 次 |
| `clipboard_read` | 30 次 |

### 10.2 审计日志

每次工具调用（无论成功/拒绝）都记录到内存中的循环缓冲区（最近 200 条），可通过前端查看。

---

## 11. Frontend（前端 Vue 层）

**框架：** Vue 3 + Pinia + TailwindCSS 4 + Vite

### 11.1 双模式窗口

| 模式 | 窗口大小 | 说明 |
|------|---------|------|
| 桌宠模式（Pet Mode） | ~200×400px | 透明悬浮，始终置顶，可拖动 |
| 助手模式（Assistant Mode） | ~900×600px | 完整聊天 + 设置 + 任务面板 |

切换方式：双击桌宠图像 / 系统托盘菜单

### 11.2 核心组件

| 组件 | 功能 |
|------|------|
| `PetModeLayout.vue` | 桌宠模式主界面，显示 Chebo 图像 + 气泡 |
| `AssistantLayout.vue` | 助手模式主界面，含聊天/任务/设置三 Tab |
| `ChatInput.vue` | 聊天输入框（支持拖拽/粘贴图片/文件） |
| `ToolConfirmDialog.vue` | L2/L3 工具权限确认弹窗 |
| `AgentTaskPanel.vue` | 长期任务列表 + 思考过程展示 |
| `SettingsPanel.vue` | 桌宠数值设置（投喂/商店等） |

### 11.3 Pinia Stores

| Store | 状态 |
|-------|------|
| `chatStore` | 当前会话消息列表、打字状态 |
| `petStore` | 桌宠状态数据 |
| `uiStore` | 当前激活 Tab、模式切换标志 |

### 11.4 聊天输入功能

- 文字输入 + Enter 发送
- 拖拽图片/文件到输入框 → 显示预览 chip → 随消息一起发送
- Ctrl+V 粘贴截图 → 自动识别图片
- Vision Router 自动决定是否传给 LLM 还是文字描述

---

## 12. 数据库结构

**路径：** `%AppData%\Chebo\chebo.db`（SQLite）

### 主要表

| 表名 | 用途 |
|------|------|
| `messages` | 聊天消息（session_id, role, content, emotion, timestamp） |
| `pet_status` | 桌宠状态（hunger, energy, mood, affection, level, exp, coins） |
| `foods` | 食物配置（price, hunger_restore, energy_restore...） |
| `tasks_pet` | 桌宠任务（工作/学习/休息，收益配置） |
| `inventory` | 用户背包（food_id, quantity） |
| `memory_summaries` | 对话摘要（session_id, summary, level） |
| `user_profile` | 用户档案（key, value, confidence, source） |
| `persona_memory` | Chebo 人格记忆 |
| `agent_tasks` | 长期任务记录（goal, steps, status） |
| `agent_task_steps` | 任务步骤详情 |
| `app_config` | 键值对配置（API key, model, sandbox_paths 等） |

### app_config 关键键

| Key | 说明 |
|-----|------|
| `llm_api_key` | 主 LLM 的 API Key |
| `llm_base_url` | 主 LLM 的 Base URL |
| `llm_model` | 主 LLM 的模型名 |
| `vision_api_key` | Vision 回退模型的 API Key |
| `vision_model` | Vision 回退模型名 |
| `sandbox_allowed_paths` | 沙盒允许路径（`||` 分隔） |
| `keep_perfect` | 是否开启满状态模式 |

---

## 13. 完整数据流：用户发消息的全链路

以"帮我读一下桌面的 note.txt 内容"为例：

```
1. 用户在 ChatInput.vue 输入文字，按 Enter

2. ChatInput.vue
   → chatStore.sendMessage("帮我读一下桌面的 note.txt")

3. tauriService.sendMessage(content, images=[])
   → invoke('send_message', { sessionId, content, images })

4. Rust: commands::send_message()
   → state.agent.try_start_thinking()  // 状态机: Idle → Thinking
   → 加载最近 10 条对话历史
   → memory::recall_relevant() 检索相关记忆
   → character::build_system_prompt() 构建含状态+记忆的 System Prompt

5. llm::stream_chat() 调用 DeepSeek API
   → 每个 token emit "chat_token" → 前端打字机效果

6. LLM 回复包含工具调用：
   <tool_call>
     <name>read_file</name>
     <args><path>C:\Users\me\Desktop\note.txt</path></args>
   </tool_call>

7. tool_dispatcher 解析 XML
   → read_file 是 L0，无需确认
   → sandbox.check_file_access("read_file", path)
       Desktop 在白名单内 → 通过
   → tools::read_file::execute({path: "..."}) → 读取文件

8. 文件内容注入上下文，继续第 2 轮 LLM 调用
   → LLM 根据文件内容生成自然语言回复

9. emit "chat_done" → 前端显示完整回复
   → db::save_message() 保存到 SQLite
   → state.agent.reset_to_idle()  // 状态机: Thinking → Idle

10. memory_tree: 后台每 20 分钟将会话摘要写入 Vault
```

---

## 14. 与同类产品对比

| 特性 | Chebo | OpenHuman | Character.ai | Replika | GitHub Copilot |
|------|-------|-----------|--------------|---------|----------------|
| 运行方式 | 本地桌面 | Web/本地 | 云端 | 移动端 | IDE 插件 |
| 工具调用 | ✓（分权限） | ✓（无权限分级） | ✗ | ✗ | ✓（代码工具） |
| 本地记忆 | ✓（SQLite + Vault） | ✓（Obsidian） | ✗（服务端） | 部分 | ✗ |
| 任务调度 | ✓（多步骤任务） | ✗ | ✗ | ✗ | 部分（Copilot Workspace） |
| 多模型支持 | ✓（DeepSeek/GPT/Claude/Ollama） | 部分 | ✗ | ✗ | ✗ |
| 安全沙盒 | ✓（路径/命令/速率） | ✗ | N/A | N/A | ✗ |
| 视觉输入 | ✓（Vision Router） | 部分 | ✓ | ✗ | ✓ |
| 双模式切换 | ✓（桌宠/助手） | ✗ | ✗ | ✗ | ✗ |
| 完全本地可用 | ✓（配合 Ollama） | 部分 | ✗ | ✗ | ✗ |
| 开源 | ✓（规划中） | ✓ | ✗ | ✗ | ✗ |

### Chebo 的核心差异化

1. **桌宠人格化 + Agent 能力合一**：不只是聊天机器人，也不只是工具调用助手
2. **本地优先**：全量数据存本地，可完全离线（切 Ollama）
3. **分权限工具体系**：L0~L3 让用户对 AI 能做什么保持控制
4. **状态影响人格**：饥饿/心情等状态实时影响 AI 的说话风格
5. **长期任务闭环**：Task System 让 Chebo 能完成跨多步骤的复杂工作

---

*最后更新：2026-05 | 维护者：Chebo 开发团队*
