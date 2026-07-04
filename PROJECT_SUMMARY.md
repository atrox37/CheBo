# Chebo AI 桌面宠物 — 项目技术总结

> **最后更新**：2026-05-20  
> **版本**：v2.1（Rust 全栈 + 双模式窗口 + Task System + 工具沙盒 + Task EventStream + 记忆管理 UI + 截图工具 + 角色状态融合）  
> **维护说明**：每次大功能改动后同步更新此文件

---

## 一、项目概述

**Chebo** 是一个运行在 Windows 桌面的 AI 伴侣宠物程序。它常驻屏幕角落，以透明悬浮窗形式存在，可以与用户对话、感知用户的电脑使用状态、调用工具完成任务、维护长期记忆，并具备完整的宠物养成系统。

### 技术选型

| 层次 | 技术 | 说明 |
|------|------|------|
| 桌面壳 | Tauri v2 | 透明窗口、无边框、置顶、托盘、全局快捷键 |
| 前端 | Vue 3 + Pinia + Vite | Composition API，`<script setup>` 语法糖 |
| 样式 | TailwindCSS 4 + 自定义 CSS | 响应式布局 |
| 图标 | lucide-vue-next | 统一图标库 |
| 后端 | Rust（Tauri 原生层） | 完全取代 Python，无需启动独立进程 |
| 数据库 | SQLite（via sqlx） | 宠物状态、聊天记录、记忆、任务全部本地持久化 |
| LLM | DeepSeek / OpenAI / Ollama | 可配置 Provider，支持流式输出 |

---

## 二、项目目录结构

```
erii-ai-desktop-pet/                 # 仓库目录（历史命名，产品品牌为 Chebo）
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
│   │   │   ├── LevelUpToast.vue     # 升级 Toast 动画
│   │   │   ├── ToolConfirmDialog.vue# L2/L3 工具调用确认弹窗
│   │   │   └── panels/
│   │   │       ├── StatusPanel.vue  # 宠物状态面板
│   │   │       ├── FeedPanel.vue    # 投喂面板
│   │   │       ├── ActionPanel.vue  # 动作/任务面板
│   │   │       ├── ShopPanel.vue    # 商店面板
│   │   │       ├── SettingsPanel.vue# 设置面板
│   │   │       └── AgentTaskPanel.vue# 长期任务面板（仅助手模式）
│   │   ├── composables/
│   │   │   ├── useAppMode.ts        # 双模式切换逻辑（pet ↔ assistant）
│   │   │   └── useTabNav.ts         # Tab 导航状态
│   │   ├── services/
│   │   │   └── tauriService.ts      # 所有 Tauri invoke/listen 封装
│   │   ├── utils/
│   │   │   └── storageKeys.ts       # localStorage 键名与旧版迁移
│   │   └── stores/
│   │       ├── chat.ts              # 聊天消息状态
│   │       ├── chebo.ts             # 角色展示状态（表情/动作/占位图）
│   │       └── pet.ts               # 宠物数值状态
│   └── src-tauri/                   # Rust 后端源码
│       ├── src/
│       │   ├── lib.rs               # 应用入口，AppState 初始化，模块注册
│       │   ├── lib_state.rs         # AppState / AppConfig 定义
│       │   ├── commands.rs          # 所有 #[tauri::command] 函数（37个）
│       │   ├── agent.rs             # Agent 状态机（10态）
│       │   ├── event_bus.rs         # 内部事件总线（tokio broadcast）
│       │   ├── llm.rs               # LLM 调用（流式 + 非流式）
│       │   ├── memory.rs            # 记忆管理（摘要、用户画像、上下文构建）
│       │   ├── memory_tree.rs       # 分层摘要树（L0-L3）
│       │   ├── vault.rs             # Markdown Vault 文件读写
│       │   ├── db.rs                # SQLite 建表 + CRUD
│       │   ├── character.rs         # 角色人设 Prompt 构建
│       │   ├── pet.rs               # 宠物数值衰减 / 升级逻辑
│       │   ├── perception.rs        # 感知系统（窗口/剪贴板/空闲检测）
│       │   ├── tool_trait.rs        # Tool trait + 权限枚举 + ToolSpec
│       │   ├── tool_registry.rs     # 工具注册表 + 意图路由集成
│       │   ├── tool_dispatcher.rs   # 工具调度器（解析/执行/确认等待）
│       │   ├── tools.rs             # 旧版工具实现（兼容层）
│       │   ├── sandbox.rs           # 工具沙盒（路径/命令/速率/审计）
│       │   ├── intent_router.rs     # 三级语义工具路由器
│       │   ├── tray.rs              # 系统托盘 + 关闭拦截
│       │   └── task/                # Task System（长期任务）
│       │       ├── mod.rs
│       │       ├── task.rs          # 数据结构（AgentTask, TaskStep）
│       │       ├── task_store.rs    # SQLite 持久化
│       │       ├── task_planner.rs  # LLM 分解任务为步骤
│       │       ├── task_executor.rs # 步骤执行循环
│       │       ├── task_manager.rs  # 高层 API
│       │       └── task_events.rs   # Tauri 事件定义
│       ├── capabilities/
│       │   └── default.json         # Tauri 权限声明（窗口API等）
│       └── tauri.conf.json          # 窗口初始配置
├── canvases/
│   └── chebo-architecture.canvas.tsx # 可视化架构图（Cursor Canvas）
├── PROJECT_SUMMARY.md               # 本文件
└── README.md                        # 快速开始指南
```

---

## 三、核心功能模块详解

### 3.1 双模式窗口（useAppMode.ts）

Chebo 支持两种窗口模式，通过 Tauri 窗口 API 动态切换。

**桌宠模式（Pet Mode）**
- 窗口尺寸：320 × 285 px
- 样式：透明背景、无边框、始终置顶、不可调整大小
- 触发：默认启动状态，或从助手模式关闭后返回
- UI：宠物图片 + 悬停显示圆形操作按钮列 + 侧滑面板
- 交互：单击宠物、双击唤出聊天输入框

**助手模式（Assistant Mode）**
- 窗口尺寸：1000 × 680 px
- 样式：标准带边框窗口、不置顶、可调整大小、居中
- 触发：点击悬停按钮栏中的展开图标（Maximize2）
- UI：左侧导航栏（聊天/长期任务/记忆/设置）+ 右侧内容区
- 关闭按钮：拦截后切回桌宠模式而不退出程序

**切换逻辑**（`useAppMode.ts`）：
```
switchToAssistant():
  1. mode.value = 'assistant'  ← 先切 Vue UI（立即响应）
  2. setDecorations(true)      ← 加边框
  3. setAlwaysOnTop(false)
  4. setResizable(true)
  5. setSize(1000, 680)
  6. center()

switchToPet():
  1. mode.value = 'pet'
  2. setDecorations(false)
  3. setAlwaysOnTop(true)
  4. setResizable(false)
  5. setSize(320, 285)
  6. setPosition(savedPos)     ← 恢复之前位置
```

权限要求（`capabilities/default.json`）：`allow-set-decorations`、`allow-set-resizable`、`allow-center`、`allow-outer-position`、`allow-set-position`

---

### 3.2 Agent 状态机（agent.rs）

定义 10 种状态，防止并发消息冲突和无效操作：

```
Idle ──→ Thinking ──→ Talking ──→ Idle
  │                                  ↑
  ├──→ Working ────────────────────→ Idle
  ├──→ Sleeping ───(用户活动)───────→ Idle
  ├──→ Observing ──────────────────→ Idle
  ├──→ WaitingConfirm ─→ ExecutingTool ──→ Idle
  └──→ ErrorRecover ────────────────→ Idle
任意态 ──→ Interrupted ──(500ms)──→ Idle
```

`can_receive_message()` 在 `send_message` 命令入口检查状态，拒绝重复发送。

---

### 3.3 工具系统（tool_trait / tool_registry / tool_dispatcher）

#### 权限分级

| 级别 | 名称 | 行为 | 工具 |
|------|------|------|------|
| L0 | 只读 | 自动执行 | read_file, list_dir, web_search, memory_recall |
| L1 | 查询 | 自动执行 | clipboard_read, git_status |
| L2 | 写入 | 需用户确认 | write_file（扩展） |
| L3 | 高危 | 需用户确认 | safe_shell |

#### 工具调用格式（XML-JSON 混合）

```xml
<tool_call>
{"name":"web_search","arguments":{"query":"Rust async 教程"}}
</tool_call>
```

#### 工具循环（send_message 内）

```
LLM 返回文本
  ↓
parse_xml() 提取 tool_call
  ↓ 有工具调用
L0/L1 → 直接执行
L2/L3 → emit tool_call_pending → 前端弹确认框
  ↓ 等待确认（最多 30 秒）
执行工具 → 结果追加到 messages
  ↓
再次调用 LLM（最多 8 轮）
  ↓ 无工具调用
输出最终回复
```

---

### 3.4 语义工具路由（intent_router.rs）

解决"用户不说'用搜索工具'，LLM 也能自动选择工具"的问题。

**三级路由**：

| 级别 | 方式 | 覆盖率 | 示例 |
|------|------|--------|------|
| Tier 1 | 关键词精确匹配 | ~60% | "查一下" → web_search |
| Tier 2 | 启发式语义评分 | ~25% | 含`.rs`文件名 → read_file |
| Tier 3 | Fallback | ~15% | 不确定 → 全量工具 |

**Tier 2 语义特征（不需要关键词也能命中）**：
- 含问号 + 时间词（最近/今天）→ web_search 得分 0.8
- 含文件扩展名（.rs/.py/.ts 等）→ read_file 得分 0.8
- 含路径分隔符（/ 或 \）→ read_file + list_dir
- 消息 < 12 字且无问号 → 纯聊天，不注入工具（节省 token）

**效果**：原来每次对话注入全部 7 个工具描述（~400 token），现在纯聊天 0 个工具，针对性查询只注入 1-2 个，减少 60-90% 的工具 prompt token 消耗。

---

### 3.5 工具沙盒（sandbox.rs）

三层防护体系：

**层1：硬性拒绝**（无论策略开关）
- 系统路径：`/etc/`, `/sys/`, `/proc/`, `C:\Windows\System32\`
- 极端命令：`rm -rf /`, `format c:`, `del /f /s /q c:\`

**层2：可配置策略**（`SandboxPolicy`）
```
路径白名单（默认）：
  ~/Documents, ~/Desktop, ~/Downloads, ~/Projects, ~/dev, ~/code

命令黑名单（17个模式）：
  rm -rf, rmdir /s, del /f, format, mkfs, dd if=,
  shutdown, reboot, halt, sudo rm, > /dev/, ...

速率限制（滑动窗口，1小时）：
  web_search  ≤ 30次
  safe_shell  ≤ 10次
  read_file   ≤ 60次
```

**层3：审计日志**
- 每次工具访问写入内存日志（最近 200 条）
- 记录：工具名、操作内容、是否通过、拒绝原因、时间戳
- 可通过 `get_audit_log` 命令查询

---

### 3.6 Task System（task/）

将对话拆解为可持久化、可续跑的长期任务。

**核心数据结构**：
```rust
AgentTask {
  id, title, description,
  status: Pending → Planning → Running → WaitingApproval
          → Paused → Completed / Failed,
  steps: Vec<TaskStep>,
  created_at, updated_at
}

TaskStep {
  id, task_id, order_index,
  description, tool_hint,
  status: Pending → Running → Completed / Failed / Skipped,
  result
}
```

**执行流程**：
```
task_create(描述)
  → TaskPlanner: LLM 分解为 N 个步骤
  → TaskExecutor: 逐步执行
      每步：调用 LLM → 可能触发工具调用 → 记录结果
      遇到 L2/L3 工具 → 状态变 WaitingApproval → 等待前端确认
  → 所有步骤完成 → 状态变 Completed
  → 程序重启 → 自动恢复 Running/WaitingApproval 的任务（3秒后）
```

**UI 入口**：仅在助手模式（Assistant Mode）的 `AgentTaskPanel` 中显示，桌宠模式不展示。

---

### 3.7 记忆系统（memory / memory_tree / vault）

四层记忆架构：

| 层级 | 名称 | 存储 | 内容 |
|------|------|------|------|
| L0 | Working Memory | 内存 | 当前会话最近 20 条 |
| L1 | Episode Memory | SQLite `chat_messages` | 完整对话历史 |
| L2 | Summary Memory | SQLite `memory_summaries` | 自动摘要（每 20 条一次） |
| L3 | Core Memory | SQLite `user_profile` + `persona_memory` | 用户画像 + 关系记忆 |

**Obsidian Vault 双存储**：
- 每条摘要同步写入 `{data_dir}/vault/` 下的 Markdown 文件
- 文件名基于时间戳，内容包含元数据头（日期、会话 ID、摘要级别）
- SHA2 内容哈希去重，避免重复写入
- 支持手动在 Obsidian 中打开查看

**上下文构建**（`build_rich_context_string`）：
每次 LLM 调用前，自动拼接：用户画像摘要 + L2 摘要（最近3条）+ L3 关键记忆

---

### 3.8 感知系统（perception.rs）

后台静默感知用户的电脑使用状态，通过 EventBus 触发主动行为：

| 感知器 | 技术 | 触发事件 |
|--------|------|---------|
| ActiveWindow | `windows-sys` Win32 API | 检测前台应用切换 |
| ClipboardMonitor | `arboard` | 剪贴板内容变化 |
| IdleDetector | 计时器 | 用户离开/回来 |

感知事件经由 `EventBus`（tokio broadcast channel）广播，`ProactiveGuard` 执行 7 维节流（时间间隔/状态/关怀模式开关等）后决定是否触发主动发言。

---

### 3.9 宠物养成系统（pet.rs / db.rs）

**核心数值**（存储在 SQLite `pet_status` 表）：

| 字段 | 范围 | 说明 |
|------|------|------|
| hunger | 0-100 | 每小时 -5，投喂恢复 |
| energy | 0-100 | 每小时 -3，睡觉恢复 |
| mood | 0-100 | 受交互频率影响 |
| affection | 0-100 | 好感度，影响 AI 对话风格 |
| level | 1+ | 升级需要经验值 |
| exp | 0+ | 完成任务/对话获得 |
| coins | 0+ | 可在商店消费 |

**时间衰减**：每 30 秒后台 tick 执行一次衰减，并通过 `keepPerfect` 机制同步前后端状态。

**投喂/商店道具**：配置存储在 SQLite `foods` 和 `shop_items` 表，不再硬编码。

---

### 3.10 LLM 集成（llm.rs）

支持三种 Provider：

```
.env 配置：
  LLM_PROVIDER=deepseek | openai | ollama
  DEEPSEEK_API_KEY=sk-xxx
  DEEPSEEK_MODEL=deepseek-chat

调用类型：
  call_stream()   → 流式输出（用于聊天，推送 chat_chunk 事件到前端）
  call_silent()   → 非流式（用于工具循环、任务规划等后台调用）

情绪解析：
  LLM 回复中包含 [emotion:happy] 等标签
  前端解析后控制宠物图片状态
```

---

## 四、Tauri 命令列表（37个）

### 聊天 / Agent
| 命令 | 说明 |
|------|------|
| `send_message` | 发送消息，触发工具循环 + 流式回复 |
| `get_agent_state` | 获取当前 Agent 状态 |
| `get_chat_history` | 获取对话历史 |
| `clear_chat_history` | 清空历史 |

### 宠物系统
| 命令 | 说明 |
|------|------|
| `get_status` / `get_pet_status` | 获取宠物数值 |
| `feed_pet` | 投喂食物 |
| `do_task` | 发起任务（工作/学习） |
| `cancel_task` | 取消任务 |
| `buy_item` | 购买商店道具 |
| `use_item` | 使用道具 |
| `keep_perfect` | 强制同步状态 |

### 记忆系统
| 命令 | 说明 |
|------|------|
| `get_memory_summaries` | 获取摘要列表 |
| `get_user_profile` | 获取用户画像 |
| `search_memories` | 关键词搜索记忆 |

### Task System
| 命令 | 说明 |
|------|------|
| `task_create` | 创建长期任务 |
| `task_list` | 列出所有任务 |
| `task_detail` | 获取任务详情 |
| `task_pause` / `task_resume` | 暂停/恢复 |
| `task_cancel_agent` | 取消任务 |
| `task_approve_step` | 批准等待确认的步骤 |
| `task_retry` | 重试失败的步骤 |

### 工具系统
| 命令 | 说明 |
|------|------|
| `tool_approve` / `tool_reject` | 审批 L2/L3 工具调用 |
| `get_audit_log` | 查询沙盒审计日志 |

### 设置
| 命令 | 说明 |
|------|------|
| `get_settings` / `save_settings` | 读写用户设置 |
| `toggle_care_mode` | 切换关怀/静默模式 |
| `get_config` / `set_config` | 读写 KV 配置（API Key 等） |

---

## 五、前端事件监听（tauriService.ts）

| 事件名 | 触发方 | 说明 |
|--------|--------|------|
| `chat_chunk` | Rust | 流式回复分片 |
| `chat_done` | Rust | 回复完成 |
| `pet_status_update` | Rust | 宠物数值变化 |
| `level_up` | Rust | 升级通知 |
| `tool_call_pending` | Rust | L2/L3 工具等待确认 |
| `tool_result` | Rust | 工具执行结果 |
| `assistant_thinking` | Rust | 工具循环中间思考过程 |
| `task_created` | Rust | 任务已创建 |
| `task_updated` | Rust | 任务状态变化 |
| `task_completed` | Rust | 任务完成 |
| `task_failed` | Rust | 任务失败 |
| `backend_error` | Rust | 后台错误 |
| `open_assistant` | Rust | 托盘菜单触发切换到助手模式 |
| `switch_to_pet` | Rust | 关闭助手模式窗口时切回桌宠 |

---

## 六、数据库表结构（SQLite）

| 表名 | 说明 |
|------|------|
| `pet_status` | 宠物核心数值（单行） |
| `chat_messages` | 聊天历史（role/content/session） |
| `memory_summaries` | 对话摘要（L2层记忆） |
| `user_profile` | 用户画像（KV） |
| `persona_memory` | 人格关系记忆 |
| `long_term_memories` | 长期记忆片段 |
| `foods` | 食物配置（名称/效果/价格） |
| `shop_items` | 商店道具配置 |
| `inventory` | 用户背包 |
| `tasks` | 宠物任务（工作/学习） |
| `config` | KV 配置（API Key、设置等） |
| `agent_tasks` | Agent 长期任务 |
| `task_steps` | 任务步骤 |

数据目录：`{APPDATA}/Chebo/`（Windows 下通常为 `C:\Users\{用户名}\AppData\Roaming\Chebo\`）

---

## 七、系统托盘 & 全局快捷键

**系统托盘**（`tray.rs`）：
- 图标常驻通知区
- 菜单：显示/隐藏 Chebo、切换助手模式、退出
- 关闭按钮行为：桌宠模式 → 最小化到托盘；助手模式 → 切回桌宠模式

**全局快捷键**：
- `Ctrl + Shift + Space`：切换窗口显示/隐藏

---

## 八、开发环境配置

### 环境要求
- Node.js 18+
- Rust 1.75+（`rustup update stable`）
- pnpm（`npm i -g pnpm`）
- Visual Studio C++ Build Tools（Windows）

### 启动开发模式
```bash
# 1. 配置 API Key
# 在 {APPDATA}/Chebo/.env 或项目根 .env 中添加：
DEEPSEEK_API_KEY=sk-xxxxxxxx
# 或
OPENAI_API_KEY=sk-xxxxxxxx
LLM_PROVIDER=openai

# 2. 安装前端依赖
cd frontend
pnpm install

# 3. 启动开发服务（编译 Rust + 启动 Vite + 打开窗口）
pnpm tauri dev
```

### 构建发布包
```bash
cd frontend
# Windows 可能需要增加栈大小避免编译器崩溃：
$env:RUST_MIN_STACK="67108864"
pnpm tauri build
# 生成的安装包在 frontend/src-tauri/target/release/bundle/
```

---

## 九、近期重大改动记录

| 日期 | 改动 | 影响文件 |
|------|------|---------|
| 2026-05-20 | **P4 角色状态融合**：pet数值动态影响 system prompt 语气（精力/饥饿/心情/好感/等级六维调节） | `character.rs` |
| 2026-05-20 | **P1 记忆可信度 + 管理 UI**：user_profile 加 confidence(0-1.0)/source 字段，助手模式可删除/纠正画像 | `db.rs`, `commands.rs`, `AssistantLayout.vue` |
| 2026-05-20 | **P2 截图工具**：`ScreenshotTool` L1权限，抓取屏幕 PNG、base64编码，保存临时文件 | `tool_registry.rs`, `Cargo.toml` |
| 2026-05-20 | **P0 Task EventStream**：`task_step_thinking` 新事件，AgentTaskPanel 加实时活动流面板 | `task_events.rs`, `task_executor.rs`, `AgentTaskPanel.vue` |
| 2026-05-20 | 双模式窗口（Pet / Assistant）| `useAppMode.ts`, `AssistantLayout.vue`, `App.vue`, `tray.rs` |
| 2026-05-20 | capabilities 补齐窗口权限 | `capabilities/default.json` |
| 2026-05-20 | 工具沙盒（路径/命令/速率/审计）| `sandbox.rs` |
| 2026-05-20 | 三级语义工具路由 | `intent_router.rs`, `tool_registry.rs` |
| 2026-05-20 | Task System 仅在助手模式显示 | `App.vue`, `useTabNav.ts` |
| 2026-05 | Task System 长期任务调度层 | `task/` 目录全部文件 |
| 2026-05 | Memory Tree + Obsidian Vault | `memory_tree.rs`, `vault.rs` |
| 2026-05 | Python → Rust 全量迁移 | 整个 `src-tauri/src/` |
| 2026-05 | Agent Runtime 状态机 | `agent.rs`, `event_bus.rs` |
| 2026-05 | Tool System（L0-L3权限分级）| `tool_trait.rs`, `tool_registry.rs`, `tool_dispatcher.rs` |
| 2026-05 | Perception System | `perception.rs` |

---

## 十、已知限制 & 待实现功能

| 功能 | 状态 | 说明 |
|------|------|------|
| Live2D 动画 | 未实现 | 目前用静态 PNG；需引入 Live2D SDK |
| 语音合成（TTS）| 未实现 | 需接入 TTS API |
| 语音识别（STT）| 未实现 | 需接入 Whisper/本地模型 |
| 沙盒路径 UI 配置 | 未实现 | 当前只能修改代码；可加入设置面板 |
| 向量数据库记忆 | 未实现 | 目前用关键词搜索；可引入 sqlite-vss |
| 多窗口支持 | 未实现 | 目前单窗口双模式 |
| 云端同步 | 未实现 | 数据完全本地 |
| 截图发给视觉 LLM | 已截图、未传图 | 需配置支持 vision 的模型（如 GPT-4V），并修改 `llm.rs` 传 base64 |
| P3 Tool EventStream | 未实现 | 工具执行过程实时推流给前端（start/progress/complete 三态） |
