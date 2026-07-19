# Chebo 终极版产品设计说明书

> **版本**：v1.0 Final  
> **日期**：2026-07-19  
> **性质**：终极版产品需求与系统设计，所有后续开发以此为唯一基准  
> **原则**：本文档描述 **目标态**，当前实现与目标态的差距在各模块"当前→目标"中标注

---

## 目录

1. [产品定义](#一产品定义)
2. [用户体验模型](#二用户体验模型)
3. [系统架构总览](#三系统架构总览)
4. [会话管线（最核心）](#四会话管线)
5. [记忆系统](#五记忆系统)
6. [Agent 与工具系统](#六agent-与工具系统)
7. [任务系统](#七任务系统)
8. [感知系统](#八感知系统)
9. [角色表现系统](#九角色表现系统)
10. [语音系统](#十语音系统)
11. [数据与存储](#十一数据与存储)
12. [安全沙盒](#十二安全沙盒)
13. [演进路线](#十三演进路线)

---

## 一、产品定义

### 1.1 一句话定位

> **Chebo 是一个有桌宠外壳的 Windows 本地 AI 桌面智能体——桌宠形态是入口，Agent 能力是内核，长期记忆是资产，本地优先是信任根基。**

### 1.2 核心价值主张

| 层次 | 用户感知 | 系统能力 |
|------|---------|---------|
| 情感层 | "桌面上有个一直陪着我的小家伙" | 常驻透明悬浮窗、情绪化立绘、主动感知式发言 |
| 关系层 | "它记得我喜欢什么、在做什么项目" | 四层记忆 + 跨会话全文检索 + Provenance 溯源 |
| 能力层 | "它能帮我读文件、搜信息、写代码" | 17 工具 + Agent 循环 + 长期任务规划执行 |
| 控制层 | "我知道它不会乱改我的文件" | L0-L3 权限 + 路径白名单 + 人工确认 + 审计日志 |
| 隐私层 | "我的数据都在自己电脑上" | SQLite 本地存储 + 可选本地模型 + 云端模型仅传必要上下文 |

### 1.3 与竞品的坐标

| 产品 | 陪伴感 | Agent 工具 | 本地记忆 | 桌宠形态 | 长期任务 |
|------|--------|-----------|---------|---------|---------|
| ChatGPT Desktop | ★★ | ★★★ | ★ | ✗ | ✗ |
| Character.ai | ★★★★★ | ★ | ★★(云端) | ✗ | ✗ |
| OpenHuman | ★ | ★★★★★ | ★★★★★ | ✗ | ★★★★ |
| Obsidian Copilot | ★ | ★★★ | ★★★★★ | ✗ | ✗ |
| **Chebo（目标态）** | **★★★★** | **★★★★** | **★★★★★** | **★★★★★** | **★★★★** |

### 1.4 明确不做

- ❌ 重养成玩法（喂食/赚金币/升级/商店）—— 已从 Phase B 正式移除
- ❌ 云端同步 —— 本地优先，不引入账号系统和云存储
- ❌ 多设备协同 —— 单机产品
- ❌ 定时自言自语 —— 仅事件驱动主动发言
- ❌ 插件市场 / MCP 集成 —— 保持工具可控
- ❌ Web/移动端 —— Windows 专有

---

## 二、用户体验模型

### 2.1 双模式架构

```
┌──────────────────────────────────────────────────────┐
│                    Chebo 双模式                        │
│                                                      │
│  桌宠模式 (320×285)              助手模式 (1000×680)   │
│  ┌─────────────────┐           ┌──────────────────┐  │
│  │   🎭 角色立绘    │           │  📁 聊天 | 任务   │  │
│  │   💬 对话气泡    │  ════▶   │  🧠 记忆 | 设置   │  │
│  │   ⌨ 双击输入    │  点击    │  🔧 工具 | 伙伴   │  │
│  │   🔽 工作台入口  │  "工作台" │  🏠 返回桌宠      │  │
│  └─────────────────┘           └──────────────────┘  │
│                                                      │
│  轻量陪伴 · 快速问答            深度工作 · 配置管理     │
└──────────────────────────────────────────────────────┘
```

**设计原则**：

- 桌宠模式 = 零摩擦入口，用户不需要"打开一个 App"
- 助手模式 = 完整工作台，所有复杂操作在此完成
- 切换通过底部按钮一键完成，不中断当前会话
- 复杂事项 Agent 可通过 `open_assistant` 主动引导用户切换

### 2.2 桌宠模式交互规范

| 交互 | 行为 |
|------|------|
| 看到立绘 | 常驻桌面，透明无边框，隐藏任务栏图标 |
| 拖拽 | 可拖拽移动位置（`data-tauri-drag-region`） |
| 单击立绘 | 无操作（避免误触） |
| 双击立绘 | 弹出输入框 + 立绘播放惊讶表情 + 弹跳动画 |
| 对话气泡 | 自动显示最新回复，普通消息 3.5s 后淡出 |
| 确认气泡 | L2/L3 工具确认时气泡保持，直到用户操作 |
| 底部按钮 | "工作台" 切换到助手模式 |

### 2.3 助手模式布局

```
┌─────────────────────────────────────────────────┐
│  [左侧导航]          │  [右侧内容区]              │
│                      │                          │
│  🏠 返回桌宠         │  （根据选中的 Tab 展示）    │
│  💬 聊天             │                          │
│  📋 任务             │  聊天：MessageList +       │
│  🧠 记忆             │       ChatInput           │
│  🔧 工具管理         │                          │
│  ⚙ 设置             │  任务：AgentTaskPanel      │
│  💝 伙伴             │                          │
│                      │  记忆：记忆浏览/搜索       │
│                      │                          │
│                      │  设置：LLM/Voice/Sandbox  │
│                      │  伙伴：默契度/人格记忆     │
└─────────────────────────────────────────────────┘
```

### 2.4 主动发言策略

**唯一原则：有理由才开口，不由定时器触发。**

| 触发事件 | 发言行为 |
|---------|---------|
| 用户发消息 → LLM 回复 | ✅ 显示气泡 |
| Agent 任务步骤完成/失败 | ✅ 显示气泡 |
| Agent 检测到需用户确认 | ✅ 显示确认气泡 |
| 用户设置的一次性提醒到期 | ✅ 显示气泡 |
| 感知到用户长时间未活动（>2h） | ✅ 轻声问候（可选，默认关闭） |
| 定时器触发 | ❌ 永久禁止 |
| 窗口失焦/睡眠/托盘 | ❌ 不发言 |

---

## 三、系统架构总览

### 3.1 分层架构图

```
┌──────────────────────────────────────────────────────────────────┐
│                     Vue 3 前端（WebView）                         │
│                                                                  │
│  App.vue ──┬── PetMode (CheboAvatar + ChatBubble + ChatInput)    │
│            ├── AssistantMode (AssistantLayout)                    │
│            │    ├── MessageList + ChatInput                       │
│            │    ├── AgentTaskPanel                                │
│            │    ├── CompanionPanel                                │
│            │    └── SettingsPanel                                 │
│            └── ToolConfirmDialog (L2/L3 全局弹窗)                 │
│                                                                  │
│  Stores:  chatStore / petStore / cheboStore                       │
│  Services: tauriService (invoke + listen 统一封装)                │
├──────────────────────────────────────────────────────────────────┤
│                    Tauri IPC Bridge                               │
│                                                                  │
│  invoke:  send_message / execute_tool / confirm_tool_call         │
│           task_create / task_list / get_app_config / ...          │
│  listen:  chat_token / chat_done / agent_state_changed            │
│           tool_permission_request / task_step_thinking / ...      │
├──────────────────────────────────────────────────────────────────┤
│                    Rust Core（单进程 Tokio 异步）                  │
│                                                                  │
│  ┌──────────────────────────────────────────────────────┐       │
│  │              P1-P4 会话管线（核心创新）                │       │
│  │                                                      │       │
│  │  P1: ChatIntent    →  三层意图路由                    │       │
│  │  P2: ContextBuilder →  按意图按需召回记忆              │       │
│  │  P3: WorkingMemory →  当前进行中状态维护              │       │
│  │  P4: MemoryController → 统一记忆写入/评分/冲突        │       │
│  └──────────────────────────────────────────────────────┘       │
│                                                                  │
│  ┌──────────┐  ┌──────────┐  ┌──────────────┐                  │
│  │ Agent    │  │ Tool     │  │ Task         │                  │
│  │ Runtime  │  │ System   │  │ System       │                  │
│  │ (10态)   │  │ (17工具) │  │ (规划+执行)   │                  │
│  └────┬─────┘  └────┬─────┘  └──────┬───────┘                  │
│       └──────────────┴───────────────┘                          │
│                      │                                           │
│  ┌───────────────────┴───────────────────────────┐              │
│  │           Event Bus（状态广播）                 │              │
│  └───────────────────────────────────────────────┘              │
│                                                                  │
│  ┌──────────┐  ┌──────────┐  ┌──────────────────┐              │
│  │ Memory   │  │Perception│  │ LLM / Provider   │              │
│  │ System   │  │ System   │  │ Registry         │              │
│  │ 4层+向量 │  │ 窗口/空闲│  │ 多Provider流式    │              │
│  │ +Tree    │  │ /剪贴板  │  │                  │              │
│  └────┬─────┘  └────┬─────┘  └────────┬─────────┘              │
│       └──────────────┴───────────────┘                          │
│                      │                                           │
│  ┌───────────────────┴───────────────────────────┐              │
│  │     SQLite (chebo.db) + Vault (Markdown)       │              │
│  └───────────────────────────────────────────────┘              │
└──────────────────────────────────────────────────────────────────┘
```

### 3.2 核心设计原则

| 原则 | 说明 |
|------|------|
| **单进程架构** | Rust 后端运行在 Tauri WebView 同进程内，零外部依赖启动 |
| **管线化处理** | P1→P2→P3→P4 顺序执行，每步精确控制 token 开销 |
| **按需召回** | 不同意图召回不同记忆量，CasualChat 零额外 token 开销 |
| **System Prompt 冻结** | 会话首次生成后冻结，动态内容放 User Message（KV Cache 友好） |
| **事件驱动解耦** | Agent 状态变更通过 EventBus 广播，前端被动响应 |
| **本地优先** | 所有数据存 SQLite，向量检索本地完成，云端仅传必要上下文 |
| **可追溯记忆** | 每条记忆记录来源会话、消息、时间和提取方式 |

---

## 四、会话管线

> 这是 Chebo 最核心的创新——不是简单地 `buildPrompt() → callLLM()`，而是根据意图精确控制每一步的 token 预算。

### 4.1 完整链路

```
用户发送消息
    │
    ▼
┌─────────────────────────────────────────────────────┐
│ P1: ChatIntent 意图分类                              │
│                                                      │
│ 硬信号层 (hard_signal_classify)                       │
│   ├─ "记住xxx" → RememberRequest, 0ms               │
│   ├─ "帮我查/搜/读/写" → ToolOperation, 0ms          │
│   └─ 未命中 ↓                                       │
│                                                      │
│ AI 分类层 (ai_classify)                               │
│   └─ 轻量 LLM 调用 → 7 类意图之一, ~300ms            │
│                                                      │
│ 规则兜底层 (rule_based_fallback)                      │
│   └─ AI 失败 → CasualChat                            │
│                                                      │
│ 输出: IntentDecision { intent, recall_strategy,       │
│        memory_action, tool_policy, response_mode }    │
└──────────────────────┬──────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────┐
│ P2: ContextBuilder 按需构建上下文                      │
│                                                      │
│ 根据 IntentDecision 决定召回量：                       │
│                                                      │
│  CasualChat:    画像2条+人格4条, ≤1500字              │
│  TechnicalQa:   画像4条+摘要3条+向量5条, ≤3500字      │
│  ContinueTask:  画像5条+摘要5条+向量8条+WM, ≤5500字   │
│  ProjectReview: 画像5条+摘要6条+向量8条+WM, ≤6000字   │
│  ToolOperation: 画像2条+向量5条, ≤3000字              │
│  EmotionalSupport: 画像3条+人格5条, ≤2200字           │
│  DeepThink:     画像3条+摘要3条+向量8条+WM, ≤8000字   │
│                                                      │
│  输出: ContextPack → to_prompt_section()             │
└──────────────────────┬──────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────┐
│ 会话策略（关键！）                                    │
│                                                      │
│ ┌─ System Prompt（会话首次构建，之后冻结）────────┐  │
│ │ · 角色人设 + 行为规范                            │  │
│ │ · 工具说明（按 tool_policy 注入）                │  │
│ │ · 用户显式偏好（长期稳定的）                      │  │
│ │ · 高置信人格记忆                                  │  │
│ │ · 当前模式（桌宠短回复 / 助手详细回复）           │  │
│ └────────────────────────────────────────────────┘  │
│                                                      │
│ ┌─ User Message（每轮动态拼接）───────────────────┐  │
│ │ · [当前工作记忆] WorkingMemory.brief              │  │
│ │ · [相关记忆] ContextPack 的动态召回内容           │  │
│ │ · [用户消息] 用户原始输入                         │  │
│ └────────────────────────────────────────────────┘  │
│                                                      │
│ 原则: 稳定内容放 System Prompt（冻结 → KV Cache 命中）│
│       动态内容放 User Message（每轮变化）             │
└──────────────────────┬──────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────┐
│ LLM 调用 + Agent 工具循环（最多 8 轮）                │
│                                                      │
│  流式输出 → 解析 <tool_call> →                         │
│    ├─ 无工具 → 直接回复用户                           │
│    ├─ L0/L1 → 立即执行 → 结果注入上下文 → 继续循环    │
│    └─ L2/L3 → emit 确认请求 → 等待用户 → 继续/终止   │
│                                                      │
│  上下文压缩（接近窗口上限时）：                         │
│    1. 旧工具结果正文 → 替换为 [已执行: xxx, 成功]     │
│    2. 最旧的消息对 → 移除（保留 System Message）      │
│    3. 绝不断开 tool_call / tool_result 配对           │
└──────────────────────┬──────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────┐
│ P3: WorkingMemory 状态更新                            │
│                                                      │
│  LLM 输出 patch → 合并到 WorkingMemory               │
│    · current_project / current_topic / user_goal     │
│    · confirmed_decisions / open_questions            │
│    · next_actions                                    │
└──────────────────────┬──────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────┐
│ P4: MemoryController 记忆写入                          │
│                                                      │
│  提取候选记忆 → 分类 → 评分 → 冲突检测 → 写入        │
│    · write_score = confidence×0.25 + importance×0.20 │
│                   + stability×0.20 + explicitness×0.20│
│                   + future_usefulness×0.15           │
│    · 低于阈值(0.4) → 丢弃                            │
│    · 同 key 冲突 → 新值写入 + 旧值置信度衰减          │
│    · 每条记忆记录 provenance（来源会话/消息/时间）    │
└──────────────────────┬──────────────────────────────┘
                       │
                       ▼
                  回复用户 + 后端异步：
                    · 触发摘要检查 (maybe_summarize)
                    · 触发 Memory Tree 同步
                    · 触发跨会话 FTS 索引更新
```

### 4.2 System Prompt 冻结策略（目标态）

> **引用自 OpenHuman 分析的核心洞察**

| 内容类别 | 放置位置 | 更新时机 |
|---------|---------|---------|
| 角色人设、行为规范 | System Prompt | 会话首次，永不更新 |
| 工具说明 | System Prompt | 会话首次，永不更新 |
| 用户显式偏好 | System Prompt | 会话首次（用户可手动刷新） |
| 高置信人格记忆 | System Prompt | 会话首次 |
| 工作记忆 brief | User Message 前缀 | 每轮更新 |
| 本次召回的记忆 | User Message 前缀 | 每轮更新 |
| 跨会话 FTS 结果 | User Message 前缀 | 每轮更新（仅非 CasualChat） |

**为什么这样设计**：当 System Prompt 前缀不变时，LLM 提供商的 KV Cache 可以复用，避免每次重新 prefill 整个 system prompt。对于长 system prompt（工具说明 + 角色人设 ≈ 2000-3000 token），每次重建的延迟和成本不可忽视。

### 4.3 会话恢复策略（目标态）

当前加载历史消息时，需要处理非完整工具调用链：

```
恢复时的修复逻辑：
  ┌─ 加载 messages 表最近 N 条
  ├─ 检测 tool_call / tool_result 配对完整性
  ├─ 删除开头没有对应 tool_call 的孤立 tool_result
  ├─ 删除结尾没有 tool_result 的未完成 tool_call
  ├─ 确保 system message 始终在首位
  └─ 超过 max_history_messages 时优先保留最近消息
```

### 4.4 上下文压缩分阶段策略（目标态）

当上下文接近模型窗口上限时，按以下优先级压缩：

```
优先级 1: Microcompact — 旧工具结果正文替换为摘要
   "[工具 read_file 已执行: 读取了 C:\project\main.rs (156行)]"

优先级 2: 旧消息对移除 — 保留最近 6 轮对话，更早的移除
   保证 tool_call/tool_result 成对移除

优先级 3: 硬截断 — 最后手段，从最早消息开始截断
   System Message 永远保留
```

---

## 五、记忆系统

### 5.1 五层记忆模型

```
┌─────────────────────────────────────────────────────────────┐
│                     记忆金字塔                               │
│                                                             │
│                        ┌─────┐                              │
│                        │Core │  ← 用户画像 + 人格记忆        │
│                        │     │     置信≥0.7，持久化          │
│                       ┌┴─────┴┐                             │
│                       │Summary│  ← 每10条消息 LLM 摘要       │
│                       │       │     200-300字，含项目/技术    │
│                      ┌┴───────┴┐                            │
│                      │Memory   │ ← 长期记忆片段              │
│                      │ Tree    │    L0 Chunk→L1 Daily       │
│                      │         │    →L2 Weekly→L3 Monthly   │
│                     ┌┴─────────┴┐                           │
│                     │  Vector   │ ← 语义向量检索             │
│                     │  Index    │    余弦相似度 Top-K        │
│                    ┌┴───────────┴┐                          │
│                    │   Episode   │ ← 完整对话记录             │
│                    │             │    messages 表 + FTS5    │
│                   ┌┴─────────────┴┐                         │
│                   │   Working     │ ← 当前会话窗口            │
│                   │               │    最近 20 条消息         │
│                   └───────────────┘                         │
│                                                             │
│  注入策略:                                                  │
│    Working → 每次对话自动加载                                │
│    Episode → FTS5 全文跨会话检索（新：快速通道）             │
│    Vector → memory_recall 工具按需召回                       │
│    Tree   → 会话摘要自动注入 + Vault 浏览                    │
│    Core   → System Prompt 首次注入 + 每轮动态画像注入        │
└─────────────────────────────────────────────────────────────┘
```

### 5.2 各层详细规格

#### Working Memory（当前会话窗口）

| 属性 | 值 |
|------|-----|
| 存储 | `messages` 表，按 `session_id` 过滤 |
| 窗口大小 | 最近 20 条 |
| 注入时机 | 每次对话自动加载 |
| 注入位置 | User Message 前缀的对话历史 |
| 加载成本 | 零 LLM 开销，纯 SQL 查询 |

#### Episode（完整对话记录 + 跨会话快速检索）

| 属性 | 值 |
|------|-----|
| 存储 | `messages` 表，每行一条消息 |
| 索引 | FTS5 全文索引（`content` 列） |
| 跨会话检索 | `memory_recall` 工具 → FTS5 搜索，排除当前 session |
| 结果截断 | 每条结果 ≤ 200 字，最多 5 条 |
| 标记 | 结果标注 "【跨会话历史记录，能力可能已变化】" |
| 目标态新增 | 独立的 `message_fts` 虚拟表，支持中文分词 |

#### Memory Tree（分层摘要树）

```
L0 Chunk（原始对话段落）
  ├─ 触发：每 10 条消息封存一个 Chunk
  ├─ TTL：2 小时无新消息强制封存当前 Chunk
  ├─ 存储：vault/Chunks/{date}_{seq}.md
  └─ 内容：原始消息文本，保留 speaker 标记

L1 Daily（每日摘要）
  ├─ 触发：当日有新 Chunk → LLM 生成/更新 Daily
  ├─ 存储：vault/Daily/{date}.md
  └─ 内容：当日对话要点、决策、情绪趋势

L2 Weekly（每周摘要）
  ├─ 触发：当周有新 Daily → LLM 生成/更新 Weekly
  ├─ 存储：vault/Weekly/{year}_W{week}.md
  └─ 内容：当周重要事项、项目进展、关系变化

L3 Monthly（每月摘要）
  ├─ 触发：当月有新 Weekly → LLM 生成/更新 Monthly
  ├─ 存储：vault/Monthly/{year}_{month}.md
  └─ 内容：月度回顾、长期趋势、关键里程碑
```

#### Vector Index（语义向量检索）

| 属性 | 值 |
|------|-----|
| 存储 | `memory_vectors` 表（SQLite BLOB） |
| 向量来源 | 摘要文本、长期记忆、用户画像、人格记忆 |
| 默认模型 | `chebo-local-v1`（内置零配置） |
| 替代模型 | Ollama `nomic-embed-text` / OpenAI `text-embedding-3-small` |
| 检索方式 | 余弦相似度 Top-K（K=8） |
| 失败回退 | 关键词匹配（SQL LIKE） |
| 触发方式 | Agent 调用 `memory_recall` 工具 |

#### Core Memory（画像与人格）

| 子类 | 存储表 | 注入方式 | 数量上限 |
|------|--------|---------|---------|
| 用户画像 | `user_profile` | System Prompt 首次 + 动态召回 | 首次 8 条，动态 2-5 条 |
| 人格记忆 | `persona_memory` | System Prompt 首次 + 动态召回 | 首次 6 条，动态 2-4 条 |
| 长期记忆 | `long_term_memories` | 向量检索召回 | 每轮最多 8 条 |

### 5.3 记忆的 Provenance 追踪（目标态新增）

> **引用自 OpenHuman 分析**：每一条记忆都应该能回答"Chebo 为什么会记住这个？"

所有记忆表（`user_profile`、`persona_memory`、`long_term_memories`、`memory_summaries`）统一增加以下字段：

| 字段 | 类型 | 说明 |
|------|------|------|
| `source_session_id` | TEXT | 来源会话 ID |
| `source_msg_id` | INTEGER | 来源消息 ID（可空） |
| `extracted_at` | TEXT | 提取时间 |
| `extraction_method` | TEXT | `llm` / `heuristic` / `user_explicit` |

**用户可追问**："你为什么觉得我喜欢 Rust？"
**Chebo 可回答**："在 7月15日 的对话中，你说'Rust 是我用过最舒服的语言'，我当时记录了下来。"

### 5.4 记忆冲突解决策略

当新记忆与已有记忆的 key 相同时：

```
1. 比较 confidence（置信度）
   ├─ 新 > 旧 × 1.2 → 替换
   ├─ 新 ≈ 旧       → 保留两者，标记为"可能存在矛盾"
   └─ 新 < 旧 × 0.8 → 丢弃新记忆

2. 旧值置信度衰减
   ├─ 每次被新值挑战 → confidence *= 0.85
   └─ confidence < 0.3 → 标记为 archived，不再主动注入
```

---

## 六、Agent 与工具系统

### 6.1 Agent 状态机（10 态）

```
                     ┌─────────────┐
          ┌─────────→│    Idle     │←──────────┐
          │          └──────┬──────┘           │
          │                 │                   │
    用户活动         收到消息/主动发言      工具完成/回复完成
          │                 │                   │
          │                 ▼                   │
   ┌──────┴──────┐   ┌──────────┐    ┌─────────┴────────┐
   │  Sleeping   │   │ Thinking │───→│    ExecutingTool  │
   │  (空闲>10min)│  └────┬─────┘    └────────┬─────────┘
   └─────────────┘       │                    │
                  第一个token到达         L2/L3工具需确认
                          │                    │
                          ▼                    ▼
                    ┌──────────┐    ┌─────────────────┐
                    │ Talking  │    │ WaitingConfirm   │
                    └────┬─────┘    └────────┬────────┘
                         │                   │
                    流式输出完成        用户确认/拒绝
                         │                   │
                         ▼                   ▼
                    ┌──────────┐    ┌─────────────────┐
                    │   Idle   │    │ ExecutingTool /  │
                    └──────────┘    │ Idle             │
                                    └─────────────────┘

   特殊状态:
   ┌──────────┐     ┌─────────────┐     ┌──────────────┐
   │ Working  │     │  Observing  │     │ ErrorRecover │
   │ (任务中) │     │  (环境扫描) │     │  (错误恢复)  │
   └──────────┘     └─────────────┘     └──────────────┘

   过渡态:
   ┌──────────────┐
   │ Interrupted  │──→500ms──→Idle
   │ (被打断)     │
   └──────────────┘
```

### 6.2 工具系统

#### 工具权限等级

| 等级 | 名称 | 确认策略 | 典型工具 |
|------|------|---------|---------|
| L0 | 只读安全 | 静默自动执行 | `read_file`, `list_dir`, `search_files`, `web_search`, `web_fetch`, `memory_recall`, `get_system_info`, `process_list` |
| L1 | 只读/轻量 | 自动执行 + 显示气泡通知 | `clipboard_read`, `take_screenshot`, `git_status`, `open_file`, `set_reminder`, `note_take` |
| L2 | 写操作 | 弹窗确认（支持"免确认"白名单） | `write_file`, `replace_in_file` |
| L3 | 系统控制 | 弹窗确认（必须每次确认） | `safe_shell` |

#### 工具列表（17 个）

| 工具 | 权限 | 分类 | 默认启用 | 说明 |
|------|------|------|---------|------|
| `read_file` | L0 | 文件 | ✅ | 读取文件内容（受沙盒限制） |
| `write_file` | L2 | 文件 | ❌ | 创建/覆盖文件 |
| `replace_in_file` | L2 | 文件 | ❌ | 精确查找替换 |
| `list_dir` | L0 | 文件 | ✅ | 列出目录 |
| `search_files` | L0 | 文件 | ✅ | 文件名/内容搜索 |
| `safe_shell` | L3 | 系统 | ✅ | 受限 Shell 命令 |
| `open_file` | L1 | 系统 | ✅ | 用默认程序打开文件 |
| `get_system_info` | L0 | 系统 | ✅ | 系统信息查询 |
| `process_list` | L0 | 系统 | ✅ | 进程列表 |
| `set_reminder` | L1 | 系统 | ❌ | 定时提醒 |
| `web_search` | L0 | 网络 | ✅ | 网页搜索 |
| `web_fetch` | L0 | 网络 | ✅ | 抓取网页内容 |
| `memory_recall` | L0 | 记忆 | ✅ | 语义/关键词记忆检索 |
| `note_take` | L1 | 记忆 | ✅ | 笔记 CRUD |
| `clipboard_read` | L1 | 剪贴板 | ✅ | 读取剪贴板 |
| `take_screenshot` | L1 | 媒体 | ✅ | 屏幕截图 |
| `git_status` | L1 | Git | ✅ | Git 状态查询 |

#### 工具调用格式

LLM 在回复中嵌入 XML 格式调用工具（兼容非原生 function calling 的模型）：

```xml
<tool_call>
{"name":"read_file","arguments":{"path":"C:\\Users\\me\\note.txt"}}
</tool_call>
```

同时支持 OpenAI 原生 `function calling`（`tool_registry.to_openai_tools()`）。

#### 语义工具路由（三级）

```
用户输入
    │
    ▼
[1] 硬信号路由 (hard_signal_classify) — 0ms
    "帮我看看 xxx" → read_file
    "截个图" → take_screenshot
    "搜索一下" → web_search
    "记住 xxx" → memory_recall / note_take
    "运行/执行" → safe_shell
    │
    ▼（未命中）
[2] AI 语义路由 (ai_classify) — ~300ms
    轻量 LLM 调用分析意图 → 推荐工具子集
    │
    ▼（AI 失败/低置信度）
[3] 规则兜底 (rule_based_fallback)
    注入全量工具，让主 LLM 自决
```

#### Agent 工具循环

```
用户消息
    │
    ▼
LLM 调用（流式）
    │
    ├── 无 <tool_call> → 直接回复用户，本轮结束
    │
    └── 检测到 <tool_call>
            │
            ├── L0/L1 → 立即执行 → 结果注入上下文 → 继续 LLM（最多 8 轮）
            │
            └── L2/L3 → emit tool_permission_request
                        → 等待用户确认（无超时，用户主动操作）
                            ├── 批准 → 执行 → 结果注入 → 继续 LLM
                            └── 拒绝 → 告知 LLM "用户拒绝了此操作" → 继续/结束
```

### 6.3 用户可配置项

| 配置项 | 默认值 | 说明 |
|--------|--------|------|
| 每个工具开关 | 见上表 | 在设置 → 工具管理中独立开关 |
| L2 免确认白名单 | 空 | 用户可信任特定 L2 工具 |
| L3 免确认 | 不支持 | L3 始终需要确认 |
| 最大工具循环轮数 | 8 | 防止无限循环 |

---

## 七、任务系统

### 7.1 任务生命周期

```
task_create (用户发起或 Agent 建议)
    │
    ▼
Pending ──→ Planning ──→ Running ──→ Completed
               │            │
               │            ├── 步骤需确认 → Paused ──→ Running
               │            ├── 步骤失败 → 自动重试(≤3次)
               │            └── 用户取消 → Cancelled
               │
               └── 规划失败 → Failed
```

### 7.2 任务数据结构

```rust
struct AgentTask {
    id: String,
    title: String,
    description: String,
    status: TaskStatus,       // Pending | Planning | Running | Paused | Completed | Failed | Cancelled
    priority: TaskPriority,   // Low | Normal | High | Urgent
    steps: Vec<TaskStep>,     // 规划后的步骤列表
    created_at: String,
    updated_at: String,
    completed_at: Option<String>,
}

struct TaskStep {
    id: String,
    description: String,
    status: StepStatus,       // Pending | Running | Completed | Failed | Skipped
    tool_to_use: Option<String>,
    result: Option<String>,
    retry_count: u32,
}
```

### 7.3 任务执行引擎

```
TaskExecutor:
  for each step in task.steps:
    ├─ 构建 step 上下文（任务目标 + 已完成步骤结果）
    ├─ LLM 决策：使用哪个工具、什么参数
    ├─ 调用 ToolDispatcher 执行
    ├─ 评估结果：
    │   ├─ 成功 → 记录结果，进入下一步
    │   ├─ 需要确认 → 暂停，emit task_step_waiting_confirm
    │   └─ 失败 → 重试(≤3次) or 标记失败
    └─ emit task_step_completed
```

### 7.4 桌宠模式下的任务反馈

```
Agent 任务进行中:
  · Agent 状态: working
  · 立绘姿态: 工作姿态（低头思考、偶尔抬头）
  · 步骤完成时: 气泡通知 "已完成: 读取配置文件 ✓"
  · 需要确认时: 确认气泡 + 立绘等待姿态

桌宠模式不展示完整任务面板：
  · 用户点击"工作台" → 助手模式 → 任务 Tab 查看详情
```

---

## 八、感知系统

### 8.1 感知能力

| 感知项 | 检测方式 | 频率 | 用途 |
|--------|---------|------|------|
| 活动窗口标题 | Windows API | 每 30s | 了解用户当前上下文 |
| 剪贴板变化 | 剪贴板监听 | 变化时 | `clipboard_read` 工具的数据源 |
| 用户空闲时间 | 鼠标/键盘最后活动 | 持续 | 决定是否进入 Sleeping |
| 系统时间 | 系统 API | 按需 | 上下文注入、提醒触发 |

### 8.2 感知触发策略

```
ObservableEvent:
  ├─ ActiveWindowChanged(title, process_name)
  ├─ UserIdleTime(duration)
  ├─ UserReturned
  └─ ClipboardChanged(content_preview)

感知结果用途：
  ├─ 注入 Working Memory（当前上下文）
  ├─ 决定 Agent 状态转换（Idle ↔ Sleeping）
  ├─ 触发主动发言（可选，默认关闭）
  └─ 为 memory_recall 提供上下文线索
```

### 8.3 隐私边界

| 数据 | 存储 | 发送给云端模型 |
|------|------|--------------|
| 窗口标题 | 本地 WorkingMemory | 仅当与当前问题相关时 |
| 剪贴板内容 | 不存储 | 仅当用户要求 `clipboard_read` 时 |
| 空闲时长 | 本地 | 不发送 |
| 截图 | 不存储 | 仅当用户要求 `take_screenshot` 时 |

---

## 九、角色表现系统

### 9.1 立绘系统

| 属性 | 当前方案 | 说明 |
|------|---------|------|
| 类型 | PNGTuber (CrystalGirl) | 6 表情 × 待机/说话 + 眨眼 |
| 技术 | CSS Sprite Sheet | `useCrystalGirlSprite` composable |
| 帧率 | 待机 8fps / 说话 12fps | 通过 CSS animation step 控制 |
| 情绪映射 | `[EMOTION:xxx]` 标签解析 | 9 种情绪映射到 6 种表情 |

### 9.2 情绪映射表

| 情绪标签 | CrystalGirl 表情 | 触发场景 |
|---------|-----------------|---------|
| `neutral` | neutral | 默认待机 |
| `happy` | happy | 正向回复、任务完成 |
| `sad` | sad | 用户倾诉负面情绪 |
| `angry` | angry | 工具执行失败 |
| `surprised` | surprised | 用户双击唤出、意外输入 |
| `thinking` | neutral + 眨眼加速 | Agent Thinking 状态 |
| `caring` | happy (soft) | 情绪陪伴模式 |
| `excited` | happy (fast) | 项目进展顺利 |
| `sleepy` | neutral + 长闭眼 | Sleeping 状态 |

### 9.3 立绘 × Agent 状态绑定

| AgentState | 立绘姿态 | 表情 | 特殊动画 |
|-----------|---------|------|---------|
| Idle | 待机 | neutral | 呼吸动画、偶尔眨眼 |
| Thinking | 思考 | neutral + 上望 | 眨眼加速 |
| Talking | 说话 | 跟随情绪标签 | 口型开合 |
| ExecutingTool | 工作 | neutral + 下看 | 低头操作 |
| WaitingConfirm | 等待 | neutral + 歪头 | 疑问姿态 |
| Working | 工作 | neutral | 专注姿态 |
| Sleeping | 睡眠 | sleepy | 闭眼、缓慢呼吸 |
| Observing | 观察 | neutral | 环顾四周 |
| ErrorRecover | 抱歉 | sad | 低头、摇头 |
| Interrupted | 惊讶 | surprised | 抖动 → 恢复 |

### 9.4 默契度（Affection）

唯一保留的软数值，影响 LLM 语气的亲密度：

| 默契度区间 | 语气表现 |
|-----------|---------|
| 0-20 | 礼貌、正式 |
| 20-40 | 友好、适度幽默 |
| 40-60 | 亲近、有昵称 |
| 60-80 | 亲密、主动关心 |
| 80-100 | 深度羁绊、独特称呼 |

增量规则：每次有意义对话（非 CasualChat）+0.2，上限 100。

---

## 十、语音系统

### 10.1 TTS（文字转语音）

| 属性 | 值 |
|------|-----|
| 协议 | OpenAI 兼容 TTS API |
| 默认端点 | 用户配置的 LLM Base URL |
| 默认模型 | `tts-1` |
| 触发方式 | 助手回复完成后自动朗读（需用户开启） |
| 口型同步 | `speechTiming.ts` 估算朗读时长，驱动口型开合 |
| 流式 TTS | Phase C 规划（当前为完整 MP3 下载后播放） |

### 10.2 STT（语音转文字）

| 属性 | 值 |
|------|-----|
| 协议 | OpenAI 兼容 Whisper API |
| 触发方式 | 输入框麦克风按钮，按住录音，松开发送 |
| 本地方案 | Phase C 规划（whisper.cpp 本地推理） |

### 10.3 语音配置

| 配置项 | 默认值 | 说明 |
|--------|--------|------|
| 朗读回复 | 关闭 | 总开关 |
| 语音输入 | 关闭 | STT 开关 |
| TTS API Base URL | 复用 LLM Base URL | 可独立配置 |
| TTS API Key | 复用 LLM Key | 可独立配置 |
| 语速 | 1.0 | 0.25-4.0 |
| 音色 | alloy | OpenAI TTS 音色 |

---

## 十一、数据与存储

### 11.1 存储架构

```
%APPDATA%\CheBo\
├── chebo.db                  # SQLite 主数据库
├── vault\                    # Markdown Vault（人类可读）
│   ├── Chunks\               # Memory Tree L0
│   ├── Daily\                # Memory Tree L1
│   ├── Weekly\               # Memory Tree L2
│   ├── Monthly\              # Memory Tree L3
│   ├── Memories\             # 用户画像 + 人格记忆（Markdown 渲染）
│   └── settings.md           # 设置快照
├── config.json               # 应用配置（LLM Provider 等）
└── cache\                    # 临时文件（TTS MP3 等）
```

### 11.2 数据库核心表

| 表名 | 用途 | 关键字段 |
|------|------|---------|
| `messages` | 全部对话记录 | id, session_id, role, content, emotion, motion, created_at |
| `message_fts` | FTS5 全文索引 | content（目标态新增） |
| `memory_summaries` | 对话摘要 | id, session_id, msg_start_id, msg_end_id, summary |
| `user_profile` | 用户画像 | key, value, confidence, source_session_id, source_msg_id |
| `persona_memory` | Chebo 人格记忆 | key, value, category, confidence, source_session_id |
| `long_term_memories` | 长期记忆 | id, content, category, confidence, source_session_id |
| `memory_vectors` | 向量索引 | source_type, source_id, embedding (BLOB) |
| `working_memory` | 当前工作记忆 | scope, current_project, current_topic, user_goal, decisions, questions, actions |
| `tool_config` | 工具配置 | tool_name, enabled, skip_confirm |
| `agent_tasks` | 长期任务 | id, title, description, status, priority |
| `task_steps` | 任务步骤 | id, task_id, description, status, result |
| `reminders` | 提醒 | id, content, remind_at, status |
| `notes` | 笔记 | id, title, content, created_at, updated_at |
| `pet_status` | 宠物状态（保留兼容） | affection 为主 |

### 11.3 数据保留策略（目标态）

| 数据 | 保留策略 |
|------|---------|
| 聊天消息 | 永久保留（本地存储成本极低） |
| 对话摘要 | 永久保留 |
| 用户画像 | 永久保留（过时条目标记 archived） |
| 人格记忆 | 永久保留（低置信条目标记 archived） |
| Memory Tree | 永久保留（Markdown 文件，用户可手动删除） |
| TTS 缓存 | 最近 50 条，自动清理 |
| 截图缓存 | 会话结束后删除 |

---

## 十二、安全沙盒

### 12.1 文件访问控制

```rust
struct SandboxPolicy {
    allowed_paths: Vec<PathBuf>,     // 白名单路径
    denied_paths: Vec<PathBuf>,      // 黑名单路径
    max_file_size: u64,              // 单文件最大读取大小 (默认 1MB)
    allowed_extensions: Vec<String>, // 允许的文件扩展名
}
```

默认白名单：`%USERPROFILE%\Documents`、`%USERPROFILE%\Desktop`、当前工作目录

### 12.2 Shell 命令控制

| 控制项 | 策略 |
|--------|------|
| 命令黑名单 | `format`, `del /f`, `rm -rf`, `shutdown`, `taskkill` 等 |
| 超时限制 | 单次命令最长 30 秒 |
| 输出限制 | 最大 64KB 输出截断 |
| 环境隔离 | 不继承用户环境变量中的敏感值（如 API Key） |

### 12.3 审计日志（目标态）

所有 L2/L3 工具调用记录审计日志：
- 时间、工具名、参数摘要、结果（成功/失败）、用户是否确认

---

## 十三、演进路线

### 13.1 Phase 总览

```
Phase A ✅  基础架构搭建
  └─ Tauri + Vue 3 框架、双模式 UI、基础对话、Agent 状态机

Phase B ✅  桌宠形态重构
  └─ Ambient Agent 定位、去除养成、双模式分工、立绘全状态绑定

Phase C 🎯  记忆与关系深度（当前阶段）
  ├─ System Prompt 冻结 + 动态内容注入
  ├─ FTS5 跨会话快速检索
  ├─ 记忆 Provenance 追踪字段
  ├─ 会话恢复消息序列修复
  └─ Memory Tree TTL 强制封存

Phase D 📋  任务与 Agent 深度
  ├─ Agent Checkpoint / Event Journal
  ├─ 上下文 Microcompact 策略
  ├─ 任务执行回放（前端）
  └─ Human-in-the-loop 任务暂停恢复

Phase E 🔮  感知与主动智能
  ├─ 窗口上下文感知增强
  ├─ 智能提醒（基于对话内容自动建议提醒）
  ├─ 本地 STT（whisper.cpp）
  └─ 流式 TTS

Phase F 🌟  体验极致化
  ├─ Live2D / Spine 骨骼动画
  ├─ 唤醒词检测
  ├─ 多角色支持
  └─ 社区角色市场
```

### 13.2 Phase C 详细任务

| # | 任务 | 优先级 | 涉及模块 |
|---|------|--------|---------|
| C1 | System Prompt 冻结：首次生成后不变，动态内容迁移到 User Message | P0 | `character.rs`, `context_builder.rs`, `commands.rs` |
| C2 | FTS5 全文索引：`messages` 表建立 FTS5，memory_recall 增加跨会话搜索 | P0 | `db.rs`, `memory_vector.rs`, `tool_registry.rs` |
| C3 | Provenance 字段：所有记忆表增加来源追踪字段 | P0 | `db.rs`, `memory_controller.rs`, `memory.rs` |
| C4 | 会话恢复修复：加载历史时修复不完整工具调用链 | P1 | `memory.rs`（`load_history_for_context`） |
| C5 | Memory Tree TTL：Chunk 增加 2 小时强制封存 | P1 | `memory_tree.rs` |
| C6 | Microcompact：旧工具结果正文替换为摘要 | P2 | `tool_dispatcher.rs` |
| C7 | 偏好分两条 Lane：System Prompt 显式偏好 + 每轮动态偏好 | P2 | `character.rs`, `context_builder.rs` |

---

## 附录 A：模块职责映射

| 模块文件 | 职责 | 对应产品功能 |
|---------|------|-------------|
| `agent.rs` | Agent 10 态状态机 | 桌宠状态显示、立绘绑定 |
| `chat_intent.rs` | P1 三层意图路由 | 智能判断用户意图 |
| `context_builder.rs` | P2 按意图构建上下文 | 精确 token 控制 |
| `working_memory.rs` | P3 当前进行中状态 | 跨轮对话连续性 |
| `memory_controller.rs` | P4 统一记忆写入 | 长期记忆质量 |
| `memory.rs` | 四层记忆 + 摘要 | 记忆系统核心 |
| `memory_vector.rs` | 向量语义检索 | 智能记忆召回 |
| `memory_tree.rs` | 分层摘要树 L0-L3 | 长期知识压缩 |
| `vault.rs` | Markdown Vault 读写 | 人类可读记忆 |
| `tool_registry.rs` | 工具注册表 | 工具系统 |
| `tool_dispatcher.rs` | 工具调度执行 | Agent 行动力 |
| `tool_trait.rs` | Tool trait 定义 | 工具接口规范 |
| `intent_router.rs` | 语义工具路由 | 智能工具选择 |
| `sandbox.rs` | 安全沙盒 | 用户信任 |
| `task/` | 长期任务系统 | 复杂任务自动化 |
| `perception.rs` | 环境感知 | 上下文感知 |
| `character.rs` | 角色人设 + Prompt | 人格一致性 |
| `voice.rs` | TTS/STT | 语音交互 |
| `llm.rs` | LLM 调用 | AI 能力核心 |
| `provider_registry.rs` | 模型接入 | 多 Provider 支持 |
| `local_embed.rs` | 本地向量模型 | 零配置向量检索 |
| `db.rs` | 数据库 CRUD | 持久化 |
| `commands.rs` | Tauri IPC 命令 | 前后端通信 |
| `event_bus.rs` | 事件广播 | 模块解耦 |

---

## 附录 B：关键设计决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| System Prompt 冻结 | ✅ 采用 | KV Cache 友好，降低延迟和成本 |
| 动态内容放 User Message | ✅ 采用 | 同上，且每轮可灵活调整召回量 |
| 跨会话 FTS5 快速通道 | ✅ 采用 | 不等后台摘要，新会话立即能搜到历史 |
| 记忆 Provenance 追踪 | ✅ 采用 | 用户可追问"你为什么记得这个" |
| Agent Checkpoint | Phase D | 当前任务系统足够简单，暂不需要 |
| 多套存储格式 | ❌ 不采用 | 保持 SQLite + Markdown Vault 双轨，不引入 JSONL |
| Topic/Global Tree | ❌ 不采用 | 维护成本高，当前 L0-L3 已足够 |
| 定时自言自语 | ❌ 永久禁止 | 事件驱动发言已足够，定时触发破坏信任感 |
| 云端同步 | ❌ 不做 | 本地优先是核心差异化 |
| MCP / 插件市场 | ❌ 不做 | 保持工具可控，降低安全风险 |

---

> **本文档为 Chebo 终极版产品与系统设计的唯一基准。所有后续功能开发、架构变更需以此文档为参照。如有偏离，需更新本文档并记录决策理由。**
