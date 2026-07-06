# Chebo AI 桌面宠物 — 项目技术总结

> **最后更新**：2026-07-06  
> **版本**：v2.2（17 工具 + 记忆增强 + 双模式窗口 + 工具配置管理）

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
│       │   ├── lib.rs               # 应用入口，AppState 初始化
│       │   ├── lib_state.rs         # AppState / AppConfig 定义
│       │   ├── commands.rs          # 所有 #[tauri::command] 函数（40+个）
│       │   ├── agent.rs             # Agent 状态机（10态）
│       │   ├── character.rs         # 角色人设 Prompt 构建 + 记忆召回指令
│       │   ├── memory.rs            # 四层记忆系统 + 摘要 + 画像提取
│       │   ├── memory_vector.rs     # 向量记忆（语义检索）
│       │   ├── memory_tree.rs       # 分层摘要树
│       │   ├── vault.rs             # Markdown Vault 读写
│       │   ├── db.rs                # SQLite 建表 + CRUD + tool_config
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
│       │   ├── chat_router.rs       # 聊天路由（多步任务识别）
│       │   ├── tray.rs              # 系统托盘
│       │   └── task/                # Task System（7 文件）
│       ├── capabilities/
│       │   └── default.json         # Tauri 权限声明（含窗口权限）
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
| 助手模式 | 1000×680px | 标准窗口、有边框、不置顶、**显示任务栏图标**（skipTaskbar=false）、可调整大小 |

### 3.2 Agent 状态机（10 态）

```
Idle ──→ Thinking ──→ Talking ──→ Idle
  ├──→ Working ──────→ Idle
  ├──→ Sleeping ─────→ Idle（用户活动）
  ├──→ Observing ────→ Idle
  ├──→ WaitingConfirm → ExecutingTool ──→ Idle
  └──→ ErrorRecover ──→ Idle
任意态 ──→ Interrupted ──(500ms)──→ Idle
```

### 3.3 工具系统（17 个工具，L0-L3 权限分级）

| 工具 | 权限 | 分类 | 默认开启 |
|------|------|------|---------|
| read_file | L0 | 文件 | ✅ |
| write_file | L2 | 文件 | ❌ |
| replace_in_file | L2 | 文件 | ❌ |
| list_dir | L0 | 文件 | ✅ |
| search_files | L0 | 文件 | ✅ |
| safe_shell | L3 | 系统 | ✅ |
| git_status | L1 | Git | ✅ |
| open_file | L1 | 系统 | ✅ |
| get_system_info | L0 | 系统 | ✅ |
| process_list | L0 | 系统 | ✅ |
| set_reminder | L1 | 系统 | ❌ |
| web_search | L0 | 网络 | ✅ |
| web_fetch | L0 | 网络 | ✅ |
| memory_recall | L0 | 记忆 | ✅ |
| note_take | L1 | 记忆 | ✅ |
| clipboard_read | L1 | 剪贴板 | ✅ |
| take_screenshot | L1 | 媒体 | ✅ |

用户可在设置面板 → **工具管理** 中对每个工具进行：
- 启用/关闭
- L2/L3 工具设置「免确认」模式

### 3.4 记忆系统（四层架构 + 向量检索）

| 层级 | 名称 | 存储 | 触发 |
|------|------|------|------|
| L0 | Working Memory | 内存（最近 20 条） | 每次对话自动加载 |
| L1 | Episode Memory | SQLite `messages` | 每次对话自动保存 |
| L2 | Summary Memory | SQLite `memory_summaries` | **每 10 条消息**触发 LLM 摘要（200-300 字，含项目/技术/个人信息） |
| L3 | Core Memory | SQLite `user_profile` + `persona_memory` + `long_term_memories` | 关键词提取 / LLM 摘要 / 用户指令 |

**每次对话注入的内容**：
- 【Chebo 人格记忆】最多 6 条（置信度 ≥ 0.7）
- 【用户画像】最多 8 条（关键词提取）
- 【历史摘要】最近 **10** 条 LLM 摘要（每 10 条消息生成一次，200-300 字详细格式）
- 【记忆片段】跨会话最近 8 条长期记忆

**system prompt 引导**：规则 6/7 要求 LLM 在记忆模糊时**主动调用 memory_recall** 工具检索向量记忆。

### 3.5 系统托盘 & 全局快捷键

- `Ctrl + Shift + Space`：切换窗口显示/隐藏
- 关闭按钮：桌宠模式 → 最小化到托盘；助手模式 → 切回桌宠模式

---

## 四、工具配置管理

`tool_config` 表存储用户对每个工具的个性化设置：

| 字段 | 说明 |
|------|------|
| `tool_name` | 工具名（PK） |
| `enabled` | 1=开启, 0=关闭 |
| `auto_approve` | 1=跳过确认弹窗（仅 L2/L3） |
| `daily_limit` | 每日调用上限（0=无限制） |

前端设置页有「工具管理」弹窗，按分类分组展示所有 17 个工具，支持实时开关。

---

## 五、Tauri 命令列表（40+ 个）

| 分类 | 命令 |
|------|------|
| Agent 状态 | `get_agent_state` |
| 聊天 | `send_message`, `cancel_chat_generation`, `get_chat_history` |
| 配置 | `get_app_config`, `update_app_config`, `get_model_capabilities`, `list_known_models`, `get_sandbox_paths`, `set_sandbox_paths` |
| 工具配置 | `get_tool_configs`, `update_tool_config` |
| 工具 | `execute_tool`, `confirm_tool_call` |
| 窗口 | `start_drag` |
| 托盘 | `toggle_window` |
| Agent 工具确认 | `approve_agent_tool` |
| Task System | `task_create/list/detail/pause/resume/cancel_agent/approve_step/retry` |
| 记忆管理 | `get_user_profile`, `get_chebo_profile`, `update_chebo_profile_entry`, `delete_chebo_profile_entry`, `get_memory_summaries`, `get_long_term_memories`, `delete_memory_entry`, `update_memory_entry` |
| Vault | `get_vault_stats`, `open_vault_folder`, `trigger_vault_sync` |
| 语音 | `voice_get_config`, `voice_update_config`, `voice_synthesize`, `voice_transcribe` |

---

## 六、数据库表结构

| 表名 | 说明 |
|------|------|
| `pet_status` | 宠物核心数值 |
| `chat_messages` | 聊天历史 |
| `memory_summaries` | 对话摘要（L2层，每10条生成一次） |
| `memory_vectors` | 向量索引（BLOB） |
| `user_profile` | 用户画像（KV+置信度） |
| `persona_memory` | Chebo 人格记忆 |
| `long_term_memories` | 长期记忆片段 |
| `tool_config` | 工具配置（开关/自动批准/限额） |
| `foods`, `shop_items` | 食物/商店配置 |
| `inventory` | 用户背包 |
| `config` | KV 配置 |
| `agent_tasks`, `task_steps` | 长期任务 |

---

## 七、开发环境

```bash
cd frontend
pnpm install
pnpm tauri dev    # 开发模式
pnpm tauri build   # 构建发布包
```

数据目录：`%APPDATA%\Chebo\`

---

## 八、已知限制

- 向量记忆未接入每次聊天的自动上下文（需 LLM 主动调用 memory_recall）
- 用户画像仍为关键词硬匹配（未接入 LLM 深度提取）
- 无定时任务系统（仅简化版 set_reminder）
- 无 Live2D 动画（静态 PNG 立绘）