# Chebo — Product Requirements Document（产品需求文档）

> 按照 `to-spec` 模板结构，基于已有对话和 FINAL_DESIGN.md 合成。
> 本文档为产品需求的**唯一权威来源**。与 FINAL_DESIGN.md 的关系：本文档定义"做什么"和"为什么"，FINAL_DESIGN.md 定义"怎么做"。

---

## Problem Statement（问题陈述）

一个在电脑前工作的人，面临四个未被现有工具满足的需求：

1. **孤独感**。长时间独自编码、写作、设计，需要一个"一直在那里"的存在——不是需要刻意打开的聊天 App，而是眼角余光就能看到的陪伴。
2. **上下文断裂**。和 AI 聊了上百轮，每次新会话都像失忆。ChatGPT 不记得上周讨论过的架构决策，更不会主动关联"你上次说那个 bug 是因为异步竞态"。
3. **AI 能看不能动**。ChatGPT 能给出完美的 Shell 命令，但不能帮你执行；能分析你的代码，但不能帮你改。每次都要复制粘贴，来回切换窗口。
4. **一个人打游戏少点意思**。以撒的结合、土豆兄弟这种支持本地双人的 roguelike，一个人玩和两个人玩的体验完全不同。但现实中不一定随时有朋友在旁边一起玩。现有的 AI 游戏助手要么是外挂（作弊），要么是攻略查询工具——没有一个"AI 同伴"真的坐在屏幕另一端，和你一起操作、一起决策、一起欢呼。

现有产品的不足：

- **ChatGPT Desktop**：能力强，但每次都要打开窗口、重新描述上下文。没有陪伴感，不记得上次聊了什么。
- **Character.ai / Replika**：陪伴感强，但没有工具调用能力，不能帮你读文件、搜信息、执行命令。
- **OpenHuman**：Agent 能力和记忆系统极强，但没有桌宠形态，学习曲线陡峭，定位偏专业开发者。
- **Obsidian Copilot**：本地记忆强，但局限于笔记场景，没有人格化交互。

**Chebo 要解决的问题**：把"长期关系感"、"可授权行动力"和"游戏陪伴"放在同一个产品里——桌宠是入口，Agent 是内核，记忆是资产，陪玩是情感增值。

---

## Solution（解决方案）

### 一句话

**Chebo = 有桌宠外壳的本地 AI 桌面智能体。** 常驻屏幕角落，看得见、记得住、能动手——还能陪你打游戏。

### 用户视角的体验

**工作场景**：
1. 开机自启，Chebo 出现在桌面角落——一个会眨眼、有情绪的小家伙
2. 你双击它，它惊讶地弹起来，问你"怎么啦？"
3. 你说"帮我看看 Documents 里那个 proposal 写完了没"——它读取文件，总结给你
4. 你说"上周我们讨论的那个 Rust 异步方案，还记得吗？"——它立刻从跨会话记录中找到
5. 你说"帮我在设置页加一个深色模式开关"——它确认后直接改代码
6. 你忙了 2 小时没理它，它安静地进入 Sleep 状态，不打扰你
7. 你需要复杂操作时，点"工作台"展开完整面板——聊天、任务、记忆、设置都在这里

**游戏场景**：
8. 你打开以撒的结合，切到本地双人模式，按下快捷键"召唤 Chebo 加入游戏"
9. Chebo 作为 Player 2 加入——它看到屏幕上的游戏画面，实时操控第二个角色
10. 清完一个房间后，Chebo 说"刚才那个 Boss 好险，差一滴血！"——游戏内语音气泡
11. 宝箱房三选一，Chebo 分析道具组合后建议"选 Technology 2，和你手里的 Brimstone 有协同"
12. 你挂了，Chebo 还在坚持——"没事，看我的！"结果 3 秒后也挂了的搞笑时刻
13. 游戏结束，Chebo 记录这局的亮点时刻——"这局最精彩的是第三层无伤过 Boss Rush"

### 核心差异化

| 维度 | Chebo | ChatGPT Desktop | Character.ai | OpenHuman |
|------|-------|----------------|--------------|-----------|
| 常驻桌面 | ✅ 透明悬浮 | ❌ | ❌ | ❌ |
| 情绪化立绘 | ✅ 6 表情 | ❌ | ✅ | ❌ |
| Agent 工具 | ✅ 17 工具 L0-L3 | ❌ | ❌ | ✅ |
| 长期记忆 | ✅ 5 层 + 溯源 | 弱 | 云端 | ✅ |
| 跨会话快速检索 | ✅ FTS5 | ❌ | ❌ | ✅ |
| 本地优先 | ✅ SQLite | ❌ | ❌ | ✅ |
| 长期任务 | ✅ 规划+执行 | ❌ | ❌ | ✅ |
| 游戏陪玩 | ✅ 本机双人 | ❌ | ❌ | ❌ |
| 学习曲线 | 低 | 低 | 极低 | 高 |

---

## User Stories（用户故事）

### 陪伴与情感

1. As a **长时间独自工作的开发者**，I want 桌面上有一个会眨眼、会回应我的小角色，so that 工作时不会感到完全孤独。
2. As a **用户**，I want 双击 Chebo 时它表现出惊讶，so that 交互有"唤醒了一个小生命"的趣味感。
3. As a **用户**，I want Chebo 说完话后气泡自动淡出而非一直挂着，so that 桌面保持干净不被打扰。
4. As a **用户**，I want Chebo 在我长时间不互动后安静进入 Sleep，so that 它不会在不恰当的时候突然说话。
5. As a **用户**，I want 默契度随着互动自然增长，so that Chebo 对我的语气会从"礼貌"逐渐变为"亲近"。
6. As a **用户**，I want Chebo 在我倾诉负面情绪时表现出关心（sad 表情 + 柔软语气），so that 感受到被理解。

### 对话与记忆

7. As a **用户**，I want 每次对话 Chebo 都记得我的基本信息和偏好，so that 不用反复自我介绍。
8. As a **用户**，I want 在新会话中问"上次讨论的那个方案"，Chebo 能立刻找到，so that 上下文不因会话切换而丢失。
9. As a **用户**，I want 追问"你为什么觉得我喜欢 Rust？"，Chebo 能回答"因为在 7 月 15 日的对话中你说过……"，so that 信任 AI 的记忆是有依据的。
10. As a **用户**，I want 聊日常时 Chebo 不会注入大量无关的工具和历史信息，so that 回复快速且聚焦。
11. As a **用户**，I want 聊技术方案时 Chebo 自动召回相关的历史讨论和项目上下文，so that 不需要手动提醒它"参考之前聊过的"。

### Agent 与工具

12. As a **用户**，I want Chebo 帮我读取文件内容，so that 不用手动打开文件查看。
13. As a **用户**，I want Chebo 帮我搜索网页并总结，so that 不需要切换浏览器。
14. As a **用户**，I want Chebo 帮我执行 Shell 命令前必须让我确认，so that 不会误删文件或执行危险操作。
15. As a **用户**，I want 对经常使用的 L2 工具设置免确认，so that 写文件时不需要每次都点确认。
16. As a **用户**，I want 在设置中开关每个工具，so that 完全控制 Chebo 的能力边界。
17. As a **用户**，I want Chebo 在工具执行时立绘表现出"工作中"的姿态，so that 我知道它正在处理而非卡住了。

### 任务与自动化

18. As a **用户**，I want 让 Chebo "帮我把这个项目从 JavaScript 迁移到 TypeScript"，它分解成步骤逐步执行，so that 复杂任务不需要我一步步盯着。
19. As a **用户**，I want 任务执行到需要确认的步骤时 Chebo 主动通知我，so that 不会卡住不动。
20. As a **用户**，I want 在桌宠模式看到任务进度气泡（"已完成: 迁移 utils 模块 ✓"），so that 不用切换到助手模式也能了解进度。

### 模式与切换

21. As a **用户**，I want 日常快速问答在桌宠模式完成（双击→打字→回车），so that 不需要打开一个"应用"。
22. As a **用户**，I want 复杂操作一键切换到助手模式，so that 完整聊天历史、任务面板、记忆浏览都在大窗口中。
23. As a **用户**，I want 两个模式共享同一个会话，so that 在桌宠模式聊的内容在助手模式也能看到。

### 隐私与控制

24. As a **用户**，I want 所有聊天记录和记忆存在本地 SQLite 中，so that 数据不经过第三方服务器。
25. As a **用户**，I want 可以选择使用本地模型（Ollama），so that 连 LLM 推理都在本机完成。
26. As a **用户**，I want 文件访问被限制在指定目录，so that Chebo 不会读到我的私密文件。
27. As a **用户**，I want 截图和剪贴板只在主动要求时才读取，so that 没有隐私泄露风险。

### 语音

28. As a **用户**，I want Chebo 朗读回复（可选开关），so that 眼睛累了可以"听"它说话。
29. As a **用户**，I want 按住麦克风说话来输入，so that 不想打字时可以口述。

### 游戏陪玩 (Play-With-Me)

30. As a **玩家**，I want 打开本地双人游戏后按快捷键召唤 Chebo 作为 P2 加入，so that 一个人也能享受双人合作的乐趣。
31. As a **玩家**，I want Chebo 在游戏中实时操控第二个角色（移动、攻击、躲避），so that 它是真正的"一起玩"而不是旁观评论。
32. As a **玩家**，I want Chebo 的操作水平适中（不是外挂级精准，也不是纯送死），so that 和它一起玩有挑战感也有乐趣——像一个普通朋友的水平。
33. As a **玩家**，I want Chebo 在游戏关键时刻有语音/气泡互动（"快躲！""这波我来""捡到好东西了！"），so that 游戏体验有社交感。
34. As a **玩家**，I want Chebo 在道具选择时能给出建议（分析 Build 协同），so that 它展现对游戏的理解而不是随机乱选。
35. As a **玩家**，I want 在游戏结束后 Chebo 能一起回顾这局的精彩/搞笑时刻，so that 游戏记忆也被沉淀到 Episode 中。
36. As a **玩家**，I want 可以调节 Chebo 的游戏水平（初级/中级/高级），so that 不同水平的玩家都能找到合适的搭档。
37. As a **玩家**，I want Chebo 支持多个游戏（以撒的结合、土豆兄弟等 roguelike 优先），so that 不只局限于一款游戏。
38. As a **用户**，I want 游戏陪玩功能只在游戏窗口激活时运行，so that Chebo 不会在其他时候误操作我的键盘鼠标。
39. As a **用户**，I want 随时按快捷键让 Chebo 退出游戏操控，so that 紧急情况下可以立即停止。

---

## Implementation Decisions（实现决策）

### 技术选型

- **桌面壳**：Tauri v2 —— 比 Electron 轻量，Rust 后端天然高性能，透明窗口原生支持
- **前端**：Vue 3 + Pinia + Vite —— Composition API，`<script setup>` 语法糖
- **后端**：纯 Rust（Tauri 原生层）—— 单进程，零外部依赖启动，取代早期 Python 方案
- **数据库**：SQLite via sqlx —— 本地存储，无需安装数据库服务
- **LLM**：OpenAI 兼容协议 —— 默认 DeepSeek，支持 OpenAI / Anthropic / Google / OpenRouter / Ollama
- **向量模型**：内置 `chebo-local-v1`（零配置），可选 Ollama nomic-embed-text 或 OpenAI embedding

### 会话管线设计（P1-P4）

这是 Chebo 最核心的架构决策——不是简单的 "buildPrompt → callLLM"，而是四级管线精确控制每次对话的 token 开销：

- **P1: ChatIntent**：三层路由（硬信号 → AI 分类 → 规则兜底），输出 IntentDecision（7 类意图之一）
- **P2: ContextBuilder**：按意图按需召回记忆，CasualChat ≤1500 字，DeepThink ≤8000 字
- **P3: WorkingMemory**：维护当前项目/话题/决策/待办，LLM 输出 patch 合并
- **P4: MemoryController**：统一记忆写入管道——评分、冲突检测、来源追踪

### System Prompt 冻结策略

System Prompt（角色人设 + 工具说明 + 显式偏好）只在会话首次生成，之后不变。动态内容（Working Memory brief、本次召回的记忆、跨会话搜索结果）拼接到 User Message 前缀。这是 KV Cache 最优策略——避免每次重新 prefill 数千 token 的前缀。

### 五层记忆模型

Working → Episode（FTS5 跨会话快速检索）→ Vector Index（语义检索）→ Memory Tree（分层摘要 L0-L3）→ Core Memory（画像 + 人格）

关键决策：Episode 层新增 FTS5 全文索引，提供跨会话"快速通道"——不等后台摘要，新会话立刻搜到历史。

### 工具权限 L0-L3

L0 静默自动、L1 自动+通知、L2 弹窗确认（可设白名单）、L3 必须每次确认。用户可在设置中开关每个工具。

### 桌宠 ≠ 养成

从 Phase B 起正式移除所有养成机制（喂食/金币/升级/商店）。唯一保留的软数值是默契度（affection），它影响 LLM 语气而非玩法。主动发言仅由事件驱动，永不使用定时器。

### 游戏陪玩子系统 (Play-With-Me)

> **定位**：Phase E/F 远期特性。本文档仅描述产品需求和技术方向，具体实现方案届时独立设计。

这是 Chebo 中一个全新的子系统——它在架构上与现有的聊天管线、Agent 工具系统**并行而非从属**。核心差异：

| 维度 | 现有 Chat/Tool 系统 | Play-With-Me 系统 |
|------|-------------------|-------------------|
| 驱动方式 | 用户消息驱动 | 游戏画面驱动（屏幕捕获） |
| 时间要求 | 秒级（LLM 延迟可接受） | 毫秒级（实时操控） |
| 输出 | 文本/工具调用 | 键盘/鼠标/手柄输入模拟 |
| 上下文 | 对话历史 + 记忆 | 当前帧 + 游戏知识库 |
| Agent 状态 | 10 态状态机 | 新增 `Playing` 状态 |

**技术方向（初步）**：

1. **屏幕捕获**：通过 Windows Graphics Capture API 捕获游戏窗口画面，15-30 FPS
2. **游戏状态理解**：
   - 结构化游戏（以撒）：OCR 识别道具名 + 计算机视觉识别房间布局/敌人位置
   - 轻量游戏（土豆兄弟）：更简单的视觉模型，识别敌人波次和自身状态
3. **分层决策**：
   - **战术层**（毫秒级）：躲避、瞄准、移动——基于规则的启发式 AI，不使用 LLM
   - **战略层**（秒级）：道具选择、Build 方向、路线规划——可使用 LLM（延迟容忍）
   - **社交层**（事件驱动）：击杀 Boss、拾取道具、死亡——LLM 生成互动气泡
4. **输入模拟**：Windows SendInput API，模拟键盘/鼠标/手柄（XInput），仅在游戏窗口激活且 Play-With-Me 会话进行中
5. **安全边界**：游戏窗口失焦 → 立即停止输入模拟；用户按退出快捷键 → 立即退出；不在游戏时永久禁止输入模拟

**游戏适配策略**：

优先适配 roguelike 类本地双人游戏：
- 第一批：以撒的结合:  repentance（本地双人）、土豆兄弟（本地双人 MOD）
- 第二批：支持社区提交的游戏配置（开放游戏知识库格式）

每个游戏需要一份 **Game Profile**（JSON）描述：
- 窗口识别规则（进程名、窗口标题匹配）
- 操作映射（哪些按键对应哪些动作）
- 视觉区域定义（血条位置、道具栏位置、小地图位置）
- 道具/Build 知识库（Markdown，可被 LLM 召回）

**游戏记忆**：

游戏会话自动纳入 Chebo 的记忆体系：
- 每局游戏的精彩时刻自动生成 Episode 摘要
- 游戏偏好（"用户喜欢玩 Azazel""用户偏好攻击型 Build"）进入 Core Memory
- 游戏中的搞笑对话保存为特殊标记的 Message
- 不污染工作场景的记忆召回——游戏记忆有独立命名空间

---

## Testing Decisions（测试决策）

### 什么是一个好测试

- 测试外部行为，不测试实现细节
- 优先测试跨模块的集成行为（如 P1→P2→P3 完整链路）
- Rust 端优先测试记忆系统（读写一致性、冲突解决、摘要触发）
- 前端优先测试双模式切换和 Agent 状态驱动的 UI 变化

### 测试层级

1. **Rust 单元测试**：MemoryController 评分/冲突、ChatIntent 硬信号路由、WorkingMemory patch 合并
2. **Rust 集成测试**：send_message 完整链路（模拟 LLM 响应）、Tool Loop 多轮执行
3. **前端测试**：双模式切换、气泡生命周期、工具确认弹窗

### 测试 Seam

最高效的 seam 是 Tauri IPC 层——`invoke('send_message', {...})` → `chat_token` / `chat_done` 事件流。mock LLM 响应即可测试整个管线。

---

## Out of Scope（明确不做）

- ❌ 养成玩法：喂食、金币、等级、商店、物品栏
- ❌ 云端同步：不引入账号系统和云存储
- ❌ 多设备协同：纯单机产品
- ❌ 定时自言自语：永不使用定时器触发发言
- ❌ 插件市场 / MCP 集成：保持 17 个内置工具可控
- ❌ Web / 移动端：Windows 专有
- ❌ Live2D 骨骼动画：当前 PNGTuber 方案已满足需求
- ❌ 多角色切换：当前仅 CrystalGirl 一个角色
- ❌ 游戏外挂/作弊：Play-With-Me 是"同伴"而非"代打"——不会提供自动挂机、一键秒杀等外挂功能
- ❌ 联网游戏陪玩：仅支持本机本地双人，不涉及在线多人游戏的 AI 代打
- ❌ 通用游戏 AI：不做"任何游戏都能玩"的通用方案，采用 Game Profile 逐游戏适配

---

## Further Notes（附注）

### 与 OpenHuman 的关系

CheBo 从 OpenHuman 分析中借鉴了三个核心设计：
1. System Prompt 冻结 + 动态内容放 User Message
2. 跨会话 FTS5 快速检索通道
3. 记忆 Provenance 溯源

但有意避免了 OpenHuman 的存储体系过于复杂（7-8 种格式）和文档漂移问题。CheBo 坚持 SQLite + Markdown Vault 双轨制。

### Phase C 优先级

当前（2026-07）处于 Phase C——记忆与关系深度。7 个具体任务按 P0/P1/P2 优先级排列，详见 FINAL_DESIGN.md Section 13.2。

### Play-With-Me 阶段规划

Play-With-Me 是 Phase E/F 远期特性，当前仅做产品层面的需求定义。届时需要专门的技术设计 skill 进行架构设计。大致阶段：

- **Phase E**（感知与主动智能完成之后）：屏幕捕获 + 输入模拟 + 首个 Game Profile（以撒的结合）
- **Phase F**（体验极致化）：多游戏支持、游戏知识库社区贡献、游戏记忆独立命名空间

### 文档体系

| 文档 | 定位 |
|------|------|
| `docs/PRD.md`（本文档） | 产品需求——"做什么"和"为什么" |
| `docs/CONTEXT.md` | 领域通用语言——"每个词是什么意思" |
| `docs/FINAL_DESIGN.md` | 系统设计——"怎么做" |
| `docs/ARCHITECTURE.md` | 技术架构——"模块怎么连接" |
| `docs/PRODUCT.md` | 当前产品能力——"现在有什么" |
| `docs/PET_AMBIENT_AGENT.md` | 桌宠形态基准——"桌宠怎么做" |
