# Chebo AI 桌面宠物 — 项目技术总结

> **最后更新**：2026-07-07  
> **版本**：v3.0（ChatIntent + ContextBuilder + WorkingMemory + MemoryController）

---

## 一、项目概述

**Chebo** 是一个运行在 Windows 桌面的 AI 伴侣宠物（Ambient Agent）。它常驻屏幕角落，以透明悬浮窗形式存在，支持双模式切换：桌宠模式（轻量聊天）与助手模式（完整工作台）。

### 技术选型

| 层次 | 技术 | 说明 |
|------|------|------|
| 桌面壳 | Tauri v2 | 透明窗口、无边框、置顶、托盘、全局快捷键 |
| 前端 | Vue 3 + Pinia + Vite | Composition API，`<script setup>` 语法糖 |
| 样式 | TailwindCSS 4 + 自定义 CSS | 响应式布局 |
| 图标 | lucide-vue-next | 统一图标库 |
| 后端 | Rust（Tauri 原生层） | 完全取代 Python，无需启动独立进程 |
| 数据库 | SQLite（via sqlx） | 宠物状态、聊天记录、记忆、任务全部本地持久化 |
| LLM | DeepSeek / OpenAI / Ollama / Anthropic / Google / OpenRouter | 可配置 Provider，支持流式输出 |

---

## 二、项目目录结构

```
erii-ai-desktop-pet/
├── frontend/                        # Tauri 项目根
│   ├── src/                         # Vue 前端源码
│   │   ├── App.vue                  # 根组件（模式分支：桌宠 / 助手）
│   │   ├── main.ts                  # Vue 入口
│   │   ├── style.css                # 全局样式
│   │   ├── components/
│   │   │   ├── AssistantLayout.vue  # 助手模式双栏布局
│   │   │   ├── CharacterDisplay.vue # 宠物图片展示 + 点击
│   │   │   ├── ChatBubble.vue       # 对话气泡（常驻）
│   │   │   ├── ChatInput.vue        # 聊天输入框（双击唤出）
│   │   │   ├── ToolConfirmDialog.vue# L2/L3 工具调用确认弹窗
│   │   │   └── panels/
│   │   │       ├── SettingsPanel.vue# 设置面板（含工具管理 UI）
│   │   │       ├── StatusPanel.vue  # 宠物状态面板
│   │   │       ├── FeedPanel.vue    # 投喂面板
│   │   │       ├── ActionPanel.vue  # 动作/任务面板
│   │   │       ├── ShopPanel.vue    # 商店面板
│   │   │       └── AgentTaskPanel.vue# 长期任务面板（仅助手模式）
│   │   ├── composables/
│   │   │   ├── useAppMode.ts        # 双模式切换（桌宠/助手，含 skipTaskbar）
│   │   │   └── useTabNav.ts         # Tab 导航状态
│   │   ├── services/
│   │   │   └── tauriService.ts      # 所有 Tauri invoke/listen 封装
│   │   ├── stores/
│   │   │   ├── chat.ts              # 聊天消息状态
│   │   │   ├── chebo.ts             # 角色展示状态
│   │   │   └── pet.ts               # 宠物数值状态
│   │   └── utils/
│   │       ├── storageKeys.ts       # localStorage 键名
│   │       └── speechTiming.ts      # TTS 语音时序
│   └── src-tauri/                   # Rust 后端源码
│       ├── src/
│       │   ├── lib.rs               # 应用入口，AppState 初始化（30+ 模块）
│       │   ├── lib_state.rs         # AppState / AppConfig 定义
│       │   ├── commands.rs          # 所有 #[tauri::command] 函数
│       │   │
│       │   ├── chat_intent.rs       # P1：三层意图路由（硬信号→AI→规则兜底）
│       │   ├── context_builder.rs   # P2：按 IntentDecision 构建 ContextPack
│       │   ├── working_memory.rs    # P3：当前进行中状态管理
│       │   ├── memory_controller.rs # P4：统一记忆写入（分类/评分/冲突）
│       │   │
│       │   ├── agent.rs             # Agent 状态机（10态）
│       │   ├── character.rs         # 角色人设 Prompt 构建 + 记忆召回指令
│       │   ├── memory.rs            # 四层记忆系统 + 摘要（每10条200-300字）
│       │   ├── memory_vector.rs     # 向量记忆（语义检索 + 自动 Top-K 召回）
│       │   ├── memory_tree.rs       # 分层摘要树
│       │   ├── vault.rs             # Markdown Vault 读写
│       │   ├── db.rs                # SQLite 建表 + CRUD（15+ 表）
│       │   ├── llm.rs               # LLM 调用（流式 + 非流式 + 重试）
│       │   ├── local_embed.rs       # 内置零配置向量模型
│       │   ├── tool_trait.rs        # Tool trait + ToolCategory + 权限枚举
│       │   ├── tool_registry.rs     # 工具注册表（17 个工具）
│       │   ├── tool_dispatcher.rs   # 工具调度器
│       │   ├── sandbox.rs           # 工具沙盒（路径/命令/速率/审计）
│       │   ├── intent_router.rs     # 三级语义工具路由
│       │   ├── perception.rs        # 感知系统（窗口/剪贴板/空闲）
│       │   ├── pet.rs               # 宠物数值
│       │   ├── provider_registry.rs # 20+ 模型能力注册表
│       │   ├── tray.rs              # 系统托盘
│       │   └── task/                # Task System（7 文件）
│       ├── capabilities/
│       │   └── default.json         # Tauri 权限声明
│       └── tauri.conf.json          # 窗口配置（320×285 桌宠默认）
├── docs/                            # 产品文档
├── PROJECT_SUMMARY.md               # 本文件
├── README.md                        # 快速开始
└── QUICKSTART.md                    # 详细启动指南
```

---

## 三、核心功能模块详解

### 3.1 双模式窗口（useAppMode.ts）

| 模式 | 尺寸 | 特点 |
|------|------|------|
| 桌宠模式 | 320×285px | 透明背景、无边框、置顶、**隐藏任务栏图标**（skipTaskbar=true） |
| 助手模式 | 1000×680px | 标准窗口、有边框、不置顶、**显示任务栏图标**（skipTaskbar=false） |

### 3.2 Agent 状态机（10 态）

```
Idle ──→ Thinking ──→ Talking ──→ Idle
  ├──→ Working ──────→ Idle
  ├──→ Sleeping ─────→ Idle
  ├──→ Observing ────→ Idle
  ├──→ WaitingConfirm → ExecutingTool ──→ Idle
  └──→ ErrorRecover ──→ Idle
任意态 ──→ Interrupted ──(500ms)──→ Idle
```

### 3.3 P1: ChatIntent 三层路由（chat_intent.rs）

用户消息进入后，先经过意图路由：

```
hard_signal_classify()   → 覆盖确定场景（deep_think/空输入/带图/记住）
  └─ 未命中 → ai_classify() → validate_decision()
       └─ 失败 → rule_based_fallback()
  └─ 短句无强信号 → 规则兜底（跳过 AI）
```

输出 `IntentDecision`，包含：

| 字段 | 说明 |
|------|------|
| `intent` | CasualChat / TechnicalQa / ContinueTask / ProjectReview / RememberRequest / ToolOperation / EmotionalSupport / DeepThink |
| `confidence` | 置信度 0-1 |
| `recall_strategy` | None / RecentOnly / WorkingMemory / VectorTopK / ProjectContext / FullHybrid |
| `memory_action` | None / Candidate / WriteExplicit / UpdateWorkingMemory |
| `response_mode` | PetShort / AssistantDetailed / TaskMode |
| `tool_policy` | None / ReadOnly / LightTools / FullTools |
| `suggested_tools` | 建议工具列表 |
| `should_start_task` | 是否应创建长期任务 |

### 3.4 P2: ContextBuilder（context_builder.rs）

按 `IntentDecision` 构建结构化上下文包 `ContextPack`：

| 意图 | 画像 | 人格 | 摘要 | 向量 | 工作记忆 |
|------|:---:|:----:|:----:|:----:|:-------:|
| CasualChat | 2条 | 4条 | ❌ | ❌ | ❌ |
| TechnicalQa | 4条 | 2条 | 3条 | Top5 | 可选 |
| ContinueTask | 5条 | 3条 | 5条 | Top8 | ✅**
| ProjectReview | 5条 | 2条 | 6条 | Top8 | ✅** |
| RememberRequest | 3条 | 2条 | ❌ | ❌ | ❌ |
| ToolOperation | 2条 | ❌ | ❌ | Top5 | ❌ |
| EmotionalSupport | 3条 | 5条 | ❌ | ❌ | ❌ |
| DeepThink | 8条 | 4条 | 10条 | Top10 | ✅** |

**向量召回保护**：相似度阈值按意图设定（0.60-0.72），去重、截断（220字/条）。

### 3.5 P3: WorkingMemory（working_memory.rs）

维护当前进行中的状态（scope 支持多项目）：

```json
{
  "current_project": "Chebo AI 桌面宠物",
  "current_topic": "记忆系统优化",
  "user_goal": "让 Chebo 自动理解上下文",
  "confirmed_decisions": ["Chebo 定位为 Ambient Agent"],
  "open_questions": ["如何设计自动召回策略"],
  "next_actions": ["完善 ContextBuilder"]
}
```

- 更新采用 **patch merge**（LLM 输出补丁，不整行覆盖）
- 仅在 `MemoryAction::UpdateWorkingMemory` 或 ContinueTask/ProjectReview/DeepThink 时触发
- `ContextBuilder` 注入 `to_brief()` 格式（非完整 JSON）

### 3.6 P4: MemoryController（memory_controller.rs）

统一记忆写入管理：

```
MemoryEvent::UserMessage
↓
extract_candidates()   — 规则提取
↓
write_score()          — 加权评分（confidence/importance/stability/explicitness/future_usefulness）
↓
score < 0.45           → memory_candidates 表（待确认）
score ≥ 0.45           → ConflictResolver
↓
冲突规则：explicit > recent > inferred
↓
按 MemoryType 分发：
  Fact/Preference         → user_profile
  Project/Decision/Task   → memory_items
  Episode                 → long_term_memories
  Procedure/Relationship  → persona_memory
  TemporaryState          → candidate
```

### 3.7 工具系统（17 个工具，L0-L3 权限分级）

| 工具 | 权限 | 默认 |
|------|------|------|
| read_file, list_dir, search_files | L0 | ✅ |
| write_file, replace_in_file | L2 | ❌ |
| safe_shell | L3 | ✅ |
| open_file, get_system_info, process_list | L0/L1 | ✅ |
| set_reminder | L1 | ❌ |
| web_search, web_fetch | L0 | ✅ |
| memory_recall, note_take | L0/L1 | ✅ |
| clipboard_read, take_screenshot | L1 | ✅ |
| git_status | L1 | ✅ |

用户可在设置面板 → **工具管理** 中对每个工具：
- 启用/关闭
- L2/L3 工具设置「免确认」模式

### 3.8 完整对话链路

```
用户输入
├─ chat_intent::decide()          → IntentDecision
├─ context_builder::build()        → ContextPack（自动向量召回）
├─ 构建 system prompt
├─ LLM 回复 + 工具循环
└─ 异步后处理
   ├─ maybe_summarize()            每10条摘要（200-300字）
   ├─ working_memory::update()     patch merge
   └─ memory_controller::process() 分类+评分+冲突+存储
```

### 3.9 系统托盘 & 全局快捷键

- `Ctrl + Shift + Space`：切换窗口显示/隐藏
- 桌宠模式隐藏任务栏图标，助手模式显示

---

## 四、数据库表结构（15+ 表）

| 表名 | 说明 |
|------|------|
| `messages` | 完整对话历史 |
| `memory_summaries` | LLM 摘要（每10条触发，200-300字） |
| `memory_vectors` | 向量索引（BLOB + 余弦检索） |
| `memory_candidates` | 候选记忆（低分待确认） |
| `user_profile` | 用户画像（KV + 置信度 + 来源） |
| `persona_memory` | Chebo 人格记忆 |
| `long_term_memories` | 长期记忆片段 |
| `working_memory` | 当前进行中的状态（scope + patch） |
| `tool_config` | 工具配置（开关/自动批准/限额） |
| `pet_status` | 宠物核心数值 |
| `foods`, `tasks` | 食物/任务配置 |
| `inventory` | 用户背包 |
| `config` | KV 配置 |
| `agent_tasks`, `task_steps` | 长期任务 |

---

## 五、开发环境

```bash
cd frontend
pnpm install
pnpm tauri dev      # 开发模式
pnpm tauri build     # 构建发布包
```

数据目录：`%APPDATA%\Chebo\`

---

## 六、已知限制

- 向量记忆未接入每次聊天的自动上下文（需 LLM 主动调用 memory_recall）
- 用户画像仍为关键词硬匹配（未接入 LLM 深度提取）
- 无定时任务系统（仅简化版 set_reminder）
- 无 Live2D 动画（静态 PNG 立绘）