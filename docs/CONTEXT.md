# Chebo — Ubiquitous Language（领域通用语言）

> 本文档建立 Chebo 项目所有参与方（你、AI Agent、代码）之间的共享术语。
> 一个词只有一个意思，一个意思只有一个词。
> 更新规则：每次发现术语歧义或新概念时即时更新。

---

## 产品形态

| 术语 | 定义 | 避免使用 |
|------|------|---------|
| **桌宠模式 (Pet Mode)** | 320×285 透明悬浮窗，含角色立绘、对话气泡、双击唤出输入 | 宠物模式、小窗 |
| **助手模式 (Assistant Mode)** | 1000×680 标准窗口，含聊天/任务/记忆/设置完整面板 | 工作台、大窗、主窗口 |
| **双模式 (Dual Mode)** | 桌宠模式与助手模式一键切换的架构设计 | 大小窗切换 |
| **Ambient Agent** | 常驻桌面、事件驱动、不喧哗的 AI 智能体——Chebo 的产品品类 | 桌面宠物、桌宠助手 |

## 角色与关系

| 术语 | 定义 | 避免使用 |
|------|------|---------|
| **Chebo** | AI 伴侣角色的名字，也是产品名 | 宠物、机器人、助手 |
| **CrystalGirl** | 当前的 PNGTuber 立绘角色名 | 立绘、模型、皮肤 |
| **默契度 (Affection)** | 0-100 的软数值，影响 LLM 语气的亲密度——唯一保留的养成数值 | 好感度、亲密度 |
| **人格记忆 (Persona Memory)** | Chebo 对角色的自我认知——"我是一个什么样的 Chebo" | 角色设定、人设 |

## 记忆系统

| 术语 | 定义 | 避免使用 |
|------|------|---------|
| **Working Memory** | 当前会话窗口中最近 20 条消息——纯 SQL 查询，零 LLM 开销 | 短期记忆、上下文窗口 |
| **Episode** | `messages` 表中全部对话记录 + FTS5 全文索引——不可压缩的原始数据 | 聊天记录、对话历史 |
| **Summary** | 每 10 条消息由 LLM 生成的 200-300 字对话摘要 | 中期记忆、对话摘要 |
| **Memory Tree** | L0 Chunk → L1 Daily → L2 Weekly → L3 Monthly 的四层分层摘要树 | 记忆树、摘要树 |
| **Core Memory** | 用户画像 + 人格记忆 + 长期记忆——置信 ≥0.7 才持久化 | 长期记忆、核心记忆 |
| **Vector Index** | `memory_vectors` 表的余弦相似度语义检索——`memory_recall` 工具的数据源 | 向量数据库、embedding 索引 |
| **Vault** | `%APPDATA%\CheBo\vault\` 下的 Markdown 文件——Memory Tree 的人类可读渲染 | 记忆库、知识库 |
| **Provenance** | 每条记忆的来源追踪：session_id、msg_id、提取时间、提取方式 | 溯源、出处、来源 |

## Agent 系统

| 术语 | 定义 | 避免使用 |
|------|------|---------|
| **Agent State** | 10 态状态机（Idle/Thinking/Talking/Working/Sleeping/Observing/WaitingConfirm/ExecutingTool/Interrupted/ErrorRecover） | Agent 状态、运行状态 |
| **Tool Loop** | LLM 输出 → 解析工具调用 → 执行 → 结果注入 → 继续 LLM，最多 8 轮 | 工具循环、Agent 循环 |
| **Intent Decision** | P1 ChatIntent 的输出：意图类型 + 召回策略 + 记忆动作 + 工具策略 + 回复模式 | 意图分类结果、路由决策 |
| **Context Pack** | P2 ContextBuilder 按 IntentDecision 组装的结构化上下文包 | 上下文包、记忆注入包 |
| **Confirmation Flow** | L2/L3 工具的前端确认弹窗 → 用户批准/拒绝 → 继续/终止 | 工具确认、权限确认 |

## 会话管线

| 术语 | 定义 | 避免使用 |
|------|------|---------|
| **P1: ChatIntent** | 三层意图路由：硬信号 → AI 分类 → 规则兜底 | 意图识别、意图分类 |
| **P2: ContextBuilder** | 按 IntentDecision 按需召回记忆——CasualChat 零额外 token | 上下文构建、记忆召回 |
| **P3: WorkingMemory** | 维护当前进行中状态——项目、话题、决策、待办 | 工作状态、当前上下文 |
| **P4: MemoryController** | 统一记忆写入：提取候选 → 分类 → 评分 → 冲突检测 → 写入 | 记忆管理、记忆写入 |
| **System Prompt Freeze** | 会话首次生成 System Prompt 后冻结，动态内容放 User Message | 前缀冻结、KV Cache 优化 |

## 工具系统

| 术语 | 定义 | 避免使用 |
|------|------|---------|
| **L0** | 只读安全工具，静默自动执行（read_file、web_search 等） | 安全工具、只读工具 |
| **L1** | 只读/轻量工具，自动执行 + 气泡通知（clipboard_read、git_status 等） | 轻量工具 |
| **L2** | 写操作工具，弹窗确认后可设免确认白名单（write_file、replace_in_file） | 写工具、确认工具 |
| **L3** | 系统控制工具，必须每次弹窗确认（safe_shell） | 危险工具、系统工具 |
| **Semantic Tool Router** | 三级路由决定注入哪些工具：硬信号匹配 → AI 语义推荐 → 全量兜底 | 工具路由、工具选择 |

## 任务系统

| 术语 | 定义 | 避免使用 |
|------|------|---------|
| **Agent Task** | 由 TaskManager 管理的长期多步骤任务——规划 → 执行 → 完成 | 长期任务、后台任务 |
| **Task Step** | 任务中的一个原子步骤——LLM 决定工具和参数 | 步骤、子任务 |
| **Task Planner** | LLM 把用户目标分解为 TaskStep[] 的模块 | 规划器、任务分解 |

## 感知与表现

| 术语 | 定义 | 避免使用 |
|------|------|---------|
| **Perception** | 环境感知：活动窗口标题、剪贴板变化、用户空闲时间 | 感知、环境监测 |
| **Emotion Tag** | `[EMOTION:xxx]` 格式的流式标记，驱动立绘表情切换 | 情绪标签、表情标记 |
| **Proactive Speech** | 事件驱动的主动发言——Agent 任务进度、提醒触发、用户确认请求 | 主动说话、自言自语 |
| **Bubble Linger** | 对话气泡在说完后的保持时间：普通消息 3.5s 淡出，确认类保持到操作 | 气泡保持、气泡时长 |

## 游戏陪玩 (Play-With-Me)

| 术语 | 定义 | 避免使用 |
|------|------|---------|
| **Play-With-Me** | Chebo 的游戏陪玩子系统——通过屏幕捕获 + 输入模拟实现本机双人游戏中的 AI 同伴 | 游戏模式、AI 陪玩、游戏 AI |
| **Game Session** | 一次从"召唤加入"到"退出游戏"的完整游戏陪玩会话 | 游戏会话、游戏回合 |
| **Game Profile** | 每个游戏的适配配置文件（JSON）——定义窗口识别、操作映射、视觉区域、道具知识库 | 游戏配置、适配器 |
| **Screen Capture** | 通过 Windows Graphics Capture API 实时捕获游戏窗口画面，15-30 FPS | 屏幕截图、截屏 |
| **Input Simulation** | 通过 Windows SendInput API 模拟键盘/鼠标/手柄操作——仅在游戏窗口激活且 Game Session 进行中 | 输入注入、模拟按键 |
| **Tactical Layer** | 毫秒级决策——躲避、瞄准、移动，基于规则/启发式，不使用 LLM | 战术层、实时操作 |
| **Strategic Layer** | 秒级决策——道具选择、Build 方向、路线规划，可使用 LLM（延迟容忍） | 战略层、决策层 |
| **Social Layer** | 事件驱动的互动气泡——击杀 Boss、拾取道具、角色死亡时触发 LLM 生成对话 | 互动层、游戏对话 |
| **Game Memory** | 独立命名空间的游戏记忆——不污染工作场景的召回，但享受同样的五层记忆体系 | 游戏记录、游戏历史 |

---

## 关系

- 一个 **Summary** 覆盖一个 **Episode** 中连续 10 条消息
- 一个 **L0 Chunk** 对应一个 **Episode** 中连续 10 条消息（与 Summary 同触发边界）
- 一个 **L1 Daily** 聚合当日的所有 **L0 Chunk**
- 一个 **Intent Decision** 决定一个 **Context Pack** 的内容范围
- 一个 **Tool Loop** 中可能发生多次 **Confirmation Flow**
- **Persona Memory** 和 **User Profile** 都属于 **Core Memory**
- 一个 **Game Session** 对应一个游戏进程的生命周期——启动游戏窗口到关闭
- 一个 **Game Profile** 包含一个游戏的完整适配信息
- **Game Session** 中的事件产生 **Game Memory**，进入独立命名空间
- **Tactical Layer** 和 **Strategic Layer** 并行运行，**Social Layer** 由两者的事件触发

## 示例对话

> **Dev:** "P2 ContextBuilder 对于 CasualChat 意图只注入画像 2 条 + 人格 4 条，那 Working Memory 呢？"
>
> **PM:** "CasualChat 不需要 Working Memory。用户说'今天天气真好'时，我们不需要告诉他当前项目是 Chebo 重构。"
>
> **Dev:** "那 Vector Index 呢？CasualChat 也不触发向量召回？"
>
> **PM:** "对。CasualChat 的总预算 ≤1500 字。向量召回只在 TechnicalQa、ContinueTask、ProjectReview、ToolOperation、DeepThink 时触发。"
>
> **Dev:** "明白了。所以 P2 的职责就是严格按 Intent Decision 控制每次对话的 token 开销。"

## 已标记的歧义

- "记忆" 在早期代码中同时指 `messages` 表（Episode）和 `long_term_memories` 表（Core Memory）——已统一：前者称 **Episode**，后者称 **Core Memory**
- "长期记忆" 曾被用来指 Summary + Core Memory 的混合——已拆分：**Summary** 是压缩后的对话摘要，**Core Memory** 是提取出的事实/偏好/决策
- "工具循环" 和 "Agent 循环" 曾混用——已统一为 **Tool Loop**，Agent 循环是更上层的概念
- **Play-With-Me** 不是桌面工具的一部分——它是一个独立子系统，有自己的 Agent 状态（Playing）、自己的输入输出（屏幕捕获→输入模拟）、自己的记忆命名空间
