# Chebo — Module Architecture Design（模块架构设计）

> 使用 `codebase-design` 方法论：深度模块、接口、缝合点、适配器。
> 本文档定义 **模块如何组织**——每个模块的接口是什么、缝合点放在哪里、为什么这样设计。
> 与 FINAL_DESIGN.md 的关系：FINAL_DESIGN.md 定义系统做什么，本文档定义模块怎么分。

---

## 目录

1. [设计原则](#一设计原则)
2. [模块全景图](#二模块全景图)
3. [缝合点地图](#三缝合点地图)
4. [模块规格：Chat Pipeline](#四模块规格chat-pipeline)
5. [模块规格：Memory Stack](#五模块规格memory-stack)
6. [模块规格：Agent & Tool](#六模块规格agent--tool)
7. [模块规格：Task System](#七模块规格task-system)
8. [模块规格：Perception & Performance](#八模块规格perception--performance)
9. [模块规格：Infrastructure](#九模块规格infrastructure)
10. [模块规格：Play-With-Me (Future)](#十模块规格play-with-me-future)
11. [深度审计：当前→目标](#十一深度审计当前目标)

---

## 一、设计原则

使用 `codebase-design` 的精确词汇——不用 "component"、"service"、"API"、"boundary"：

| 术语 | 定义 |
|------|------|
| **Module** | 任何有接口和实现的东西——函数、结构体、crate 子模块 |
| **Interface** | 调用者使用模块需要知道的**全部**信息：类型签名、不变量、错误模式、性能特征 |
| **Implementation** | 接口后面的代码体 |
| **Depth** | 接口后面的行为量 ÷ 接口的复杂度 —— **深模块** = 小接口 + 大实现 |
| **Seam** | 接口所在的位置——可以不改代码就替换行为的地方 |
| **Adapter** | 在缝合点满足接口的具体实现——描述**角色**而非内部结构 |
| **Leverage** | 调用者从深度中获得的好处：每个接口单元获得更多能力 |
| **Locality** | 维护者从深度中获得的好处：修改、Bug、知识集中在一处 |

### 核心原则

1. **删除测试**。想象删除一个模块。如果复杂度消失了——它是透传。如果复杂度散布到 N 个调用者——它在干活。
2. **接口即测试面**。调用者和测试穿过同一个缝合点。如果你想测试接口后面的东西，模块形状可能不对。
3. **一个适配器 = 假设缝合点。两个适配器 = 真实缝合点。** 只在确实有东西在这个缝合点上变化时才引入。
4. **接收依赖，不要创建依赖。** 模块从外部接收它的依赖，而不是内部 `new` 出来。
5. **返回结果，不要产生副作用。** 纯计算模块总是比有副作用的模块更容易测试。

---

## 二、模块全景图

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Chebo Module Map                              │
│                                                                      │
│  ┌──────────────────────────┐    ┌──────────────────────────────┐   │
│  │     Chat Pipeline         │    │       Agent & Tool           │   │
│  │                           │    │                               │   │
│  │  IntentClassifier         │    │  AgentRuntime               │   │
│  │    → IntentDecision        │    │  ToolRegistry               │   │
│  │                           │    │  ToolDispatcher             │   │
│  │  ContextAssembler         │    │  SandboxPolicy              │   │
│  │    → ContextPack           │    │                               │   │
│  │                           │    │  ┌─────────────────────┐    │   │
│  │  PromptBuilder            │    │  │ Future:              │    │   │
│  │    → (system, user)       │    │  │  GameController      │    │   │
│  │                           │    │  │  ScreenCapture       │    │   │
│  │  WorkingMemoryStore       │    │  │  InputSimulator      │    │   │
│  │                           │    │  │  TacticalEngine      │    │   │
│  │  MemoryWriteRouter        │    │  └─────────────────────┘    │   │
│  └──────────┬───────────────┘    └──────────────┬───────────────┘   │
│             │                                    │                   │
│  ┌──────────┴────────────────────────────────────┴───────────────┐   │
│  │                     Memory Stack                               │   │
│  │                                                                │   │
│  │  EpisodeStore          SummaryEngine        MemoryTreeSync    │   │
│  │  (messages 表 + FTS5)  (LLM 摘要生成)       (L0→L3 级联)      │   │
│  │                                                                │   │
│  │  VectorIndex           CoreMemoryStore      VaultWriter       │   │
│  │  (embedding + 余弦)    (profile + persona)  (Markdown 渲染)    │   │
│  └────────────────────────────┬───────────────────────────────────┘   │
│                               │                                       │
│  ┌────────────────────────────┴───────────────────────────────────┐   │
│  │                     Infrastructure                              │   │
│  │                                                                │   │
│  │  LlmClient          EmbeddingProvider     Database             │   │
│  │  (流式 + 重试)      (chebo-local-v1)     (sqlx SqlitePool)    │   │
│  │                                                                │   │
│  │  EventBus           ConfigStore            VoiceClient         │   │
│  │  (状态广播)          (LLM/Sandbox配置)      (TTS/STT)           │   │
│  └────────────────────────────────────────────────────────────────┘   │
│                                                                      │
│  ┌────────────────────────────────────────────────────────────────┐   │
│  │  Task System        Perception          Character Performance  │   │
│  │  TaskPlanner         WindowObserver       EmotionResolver       │   │
│  │  TaskExecutor        ClipboardWatcher     SpriteController      │   │
│  │  TaskStore           IdleDetector         SpeechTimer           │   │
│  └────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────┘
```

### 模块分类

| 类别 | 模块 | 职责一句话 |
|------|------|-----------|
| **Chat Pipeline** | IntentClassifier, ContextAssembler, PromptBuilder, WorkingMemoryStore, MemoryWriteRouter | 从用户消息到 LLM 调用的完整管线 |
| **Memory Stack** | EpisodeStore, SummaryEngine, MemoryTreeSync, VectorIndex, CoreMemoryStore, VaultWriter | 五层记忆的读写和索引 |
| **Agent & Tool** | AgentRuntime, ToolRegistry, ToolDispatcher, SandboxPolicy | 状态机 + 工具执行 + 安全 |
| **Task System** | TaskPlanner, TaskExecutor, TaskStore | 长期任务的规划、执行、持久化 |
| **Perception** | WindowObserver, ClipboardWatcher, IdleDetector | 环境感知 |
| **Performance** | EmotionResolver, SpriteController, SpeechTimer | 立绘、表情、口型 |
| **Infrastructure** | LlmClient, EmbeddingProvider, Database, EventBus, ConfigStore, VoiceClient | 跨模块共享的基础设施 |
| **Future** | GameController, ScreenCapture, InputSimulator, TacticalEngine | Play-With-Me 子系统 |

---

## 三、缝合点地图

每个缝合点（Seam）是一个可以不改代码就替换行为的地方。

```
┌──────────────────────────────────────────────────────────────────┐
│                        Seam Map                                   │
│                                                                   │
│  S1: Tauri IPC  ←── 前端与后端的唯一缝合点                         │
│      适配器: commands.rs (当前1个) → 目标: 按领域拆分              │
│                                                                   │
│  S2: LLM Provider  ←── 模型切换的缝合点                            │
│      适配器: DeepSeek / OpenAI / Anthropic / Ollama               │
│                                                                   │
│  S3: Embedding Provider  ←── 向量模型的缝合点                      │
│      适配器: chebo-local-v1 / nomic-embed-text / OpenAI embedding │
│                                                                   │
│  S4: Memory Store  ←── 记忆持久化的缝合点                           │
│      适配器: SQLite (当前唯一) → 未来可加 LanceDB                  │
│                                                                   │
│  S5: Tool Execution  ←── 工具执行的缝合点                           │
│      适配器: 17 个内置工具，每个是一个 Adapter                      │
│                                                                   │
│  S6: Voice Provider  ←── TTS/STT 的缝合点                          │
│      适配器: OpenAI TTS / 未来 whisper.cpp                        │
│                                                                   │
│  S7: Game Window (Future)  ←── 游戏陪玩的缝合点                     │
│      适配器: 每个 Game Profile 一个 Adapter                        │
└──────────────────────────────────────────────────────────────────┘
```

**缝合点判定规则**：

- S2/S3/S6：已有多个适配器 → **真实缝合点**（✅ 保留）
- S5：每个工具是一个适配器 → **真实缝合点**（✅ 保留）
- S4：当前只有一个适配器 → **假设缝合点**（⚠ 保持接口清晰，但不急于抽象）
- S7：未来引入 → **计划缝合点**（📋 预留接口设计）

---

## 四、模块规格：Chat Pipeline

### 4.1 IntentClassifier

```
┌─────────────────────────────────────────┐
│ Interface: IntentClassifier              │
│                                          │
│  classify(input: IntentInput,            │
│           ctx: IntentContext)            │
│    → IntentDecision                     │
│                                          │
│  Depth: ★★★★★  Deep                     │
│  背后隐藏: 硬信号匹配 + AI分类调用       │
│           + 规则兜底 + 7种意图枚举       │
│  接口大小: 1个方法, 2个参数              │
└─────────────────────────────────────────┘
```

**接口契约**：
- 输入 `IntentInput`：用户消息内容、是否 DeepThink、是否助手模式、是否有图片
- 输入 `IntentContext`：最近几条对话摘要（≤80 字/条）、当前 WorkingMemory brief
- 输出 `IntentDecision`：意图类型 + 召回策略 + 记忆动作 + 工具策略 + 回复模式
- **不变量**：always returns a valid decision — 最差返回 `CasualChat` fallback
- **错误模式**：AI 分类失败 → 静默降级到规则兜底，不抛出错误

**实现内部**：
```
hard_signal_classify(input)  → Option<IntentDecision>  // 0ms, 覆盖确定场景
    ↓ None
ai_classify(input, ctx)      → Result<IntentDecision>  // ~300ms LLM调用
    ↓ Err
rule_based_fallback(input)   → IntentDecision          // 0ms, CasualChat
```

**为什么是深模块**：调用者只需要知道"给我一个意图决策"，不需要知道三层路由的存在、不需要知道 AI 分类用了哪个模型、不需要处理分类失败——这些全部隐藏在接口后面。

**目标态改进**（当前 `chat_intent.rs` 已接近此设计，主要问题是 `ai_classify` 与 `llm.rs` 的耦合需要通过依赖注入解耦）：

```rust
// 目标态：通过 trait 解耦
pub struct IntentClassifier {
    llm: Arc<dyn LlmClient>,  // ← 注入依赖，不自己创建
}

impl IntentClassifier {
    pub async fn classify(&self, input: &IntentInput, ctx: &IntentContext) -> IntentDecision {
        // ...
    }
}
```

### 4.2 ContextAssembler

```
┌─────────────────────────────────────────┐
│ Interface: ContextAssembler              │
│                                          │
│  assemble(decision: &IntentDecision)     │
│    → ContextPack                        │
│                                          │
│  Depth: ★★★★★  Deep                     │
│  背后隐藏: 按意图差异化的召回逻辑         │
│           7种意图 × (画像/人格/摘要/向量) │
│           每种的token预算精确控制         │
│  接口大小: 1个方法, 1个参数              │
└─────────────────────────────────────────┘
```

**接口契约**：
- 输入：IntentDecision（来自 IntentClassifier）
- 输出：ContextPack（结构化上下文，包含各部分的内容和 token 预算）
- **不变量**：CasualChat ≤1500 字，DeepThink ≤8000 字。任何意图的 ContextPack 都是合法的。
- **性能特征**：除向量召回外，所有查询都是 SQL 级别，不触发 LLM

**为什么是深模块**：调用者只需要传一个 IntentDecision，得到的就是拼好的上下文。不需要知道每种意图应该召回什么、从哪里召回、每条最多取几条——这些知识全部局部化在这个模块内部。

**目标态改进**（当前 `context_builder.rs` 已接近此设计，需要增加 FTS5 跨会话检索通道）：

```rust
pub async fn assemble(&self, decision: &IntentDecision) -> ContextPack {
    let reqs = requirements_for_decision(decision);
    let mut pack = ContextPack::default();

    if reqs.need_working_memory { pack.working_memory = self.wm_store.get_brief(); }
    if reqs.need_profile       { pack.profile_items  = self.core_store.get_profile(reqs.max_profile_items); }
    if reqs.need_persona       { pack.persona_items  = self.core_store.get_persona(reqs.max_persona_items); }
    if reqs.need_summaries     { pack.summaries      = self.episode_store.get_summaries(reqs.max_summaries); }
    if reqs.need_vector_recall { pack.vector_memories = self.vector_index.search(/* ... */, reqs.max_vector_items); }
    
    // 🆕 Phase C: FTS5 跨会话快速检索
    if reqs.need_fts_search    { pack.fts_results = self.episode_store.fts_search(/* ... */); }

    pack
}
```

### 4.3 PromptBuilder

```
┌─────────────────────────────────────────┐
│ Interface: PromptBuilder                 │
│                                          │
│  build_system(session: &SessionSnapshot) │
│    → LlmMessage  // 会话首次调用          │
│                                          │
│  build_user(pack: &ContextPack,          │
│             user_msg: &str,              │
│             history: &[LlmMessage])      │
│    → Vec<LlmMessage>                    │
│                                          │
│  Depth: ★★★★☆  Deep                     │
│  背后隐藏: System Prompt冻结策略          │
│           动态内容放User Message         │
│           KV Cache友好的前缀管理          │
│  接口大小: 2个方法                        │
└─────────────────────────────────────────┘
```

**接口契约**：
- `build_system` 只在会话首次调用，之后 System Prompt 冻结
- `build_user` 每轮调用，动态内容拼接在 User Message 前面
- **不变量**：System Prompt 的格式和顺序保证稳定（以利 KV Cache 命中）
- **错误模式**：构建失败 → 返回最小可用 Prompt（仅角色人设 + 用户消息）

**为什么分离 build_system 和 build_user**：这是从 OpenHuman 分析中学到的核心设计——稳定内容放 System Prompt，动态内容放 User Message。分离两个方法使得调用者可以在会话层面缓存 `build_system` 的结果。

### 4.4 WorkingMemoryStore

```
┌─────────────────────────────────────────┐
│ Interface: WorkingMemoryStore            │
│                                          │
│  get(scope: &str) → WorkingMemory       │
│  apply_patch(patch: WorkingMemoryPatch)  │
│    → Result<()>                         │
│  get_brief(max_chars: usize) → String   │
│                                          │
│  Depth: ★★★★☆  Deep                     │
│  背后隐藏: scope分离、Vec字段管理、       │
│           LLM patch合并、置信度维护       │
│  接口大小: 3个方法                        │
└─────────────────────────────────────────┘
```

### 4.5 MemoryWriteRouter

```
┌─────────────────────────────────────────┐
│ Interface: MemoryWriteRouter             │
│                                          │
│  route(candidates: Vec<MemoryCandidate>) │
│    → WriteReport                        │
│                                          │
│  Depth: ★★★★★  Deep                     │
│  背后隐藏: 分类→评分→冲突检测→写入       │
│           4种存储目标的路由               │
│           write_score公式                │
│           confidence衰减策略              │
│  接口大小: 1个方法                        │
└─────────────────────────────────────────┘
```

**目标态改进**（当前 `memory_controller.rs` 已接近此设计，需要增加 provenance 字段）：

每条 MemoryCandidate 必须携带 `source_session_id`、`source_msg_id`、`extraction_method`——这使得记忆的"为什么记住这个"可追溯。

---

## 五、模块规格：Memory Stack

### 5.1 EpisodeStore

```
┌─────────────────────────────────────────┐
│ Interface: EpisodeStore                  │
│                                          │
│  save_message(msg: NewMessage) → i64     │
│  get_recent(session: &str, n: i64)       │
│    → Vec<Message>                       │
│  fts_search(query: &str,                 │
│             exclude_session: &str,       │
│             limit: usize)                │
│    → Vec<FtsResult>    // 🆕 Phase C     │
│  count_after(session: &str,              │
│              after_id: i64) → i64        │
│                                          │
│  Depth: ★★★★☆  Deep                     │
│  背后隐藏: SQLite CRUD + FTS5全文索引     │
│           会话隔离 + 消息序列修复         │
│  接口大小: 4个方法                        │
└─────────────────────────────────────────┘
```

**目标态改进**：
- 🆕 `fts_search`：跨会话 FTS5 快速检索，不等后台摘要就能搜到历史
- 🆕 消息序列修复：加载历史时自动修复不完整的 tool_call/tool_result 配对

### 5.2 SummaryEngine

```
┌─────────────────────────────────────────┐
│ Interface: SummaryEngine                 │
│                                          │
│  maybe_summarize(session: &str)          │
│    → Option<SummaryId>  // fire&forget   │
│                                          │
│  get_recent(limit: usize)                │
│    → Vec<MemorySummary>                 │
│                                          │
│  Depth: ★★★☆☆  Moderate                 │
│  背后隐藏: 阈值检查(每10条) + LLM摘要调用 │
│           + 去重 + 异步fire-and-forget   │
│  接口大小: 2个方法                        │
└─────────────────────────────────────────┘
```

### 5.3 VectorIndex

```
┌─────────────────────────────────────────┐
│ Interface: VectorIndex                   │
│                                          │
│  index(source: IndexSource,              │
│         content: &str) → Result<()>      │
│  search(query: &str, k: usize)           │
│    → Vec<ScoredMemory>                  │
│                                          │
│  Depth: ★★★★☆  Deep                     │
│  背后隐藏: embedding生成 + 余弦相似度     │
│           + 关键词回退 + 批量索引         │
│  接口大小: 2个方法                        │
└─────────────────────────────────────────┘
```

**目标态改进**（当前 `memory_vector.rs` 在 SQLite BLOB 中做余弦相似度，大规模性能差）：
- 短期：SQLite 方案仍可工作（Chebo 规模不大）
- 中期：预留 `VectorStore` trait，可切换到 LanceDB 或 Qdrant
- 关键：调用者不关心底层用什么存储——这就是缝合点的价值

### 5.4 CoreMemoryStore

```
┌─────────────────────────────────────────┐
│ Interface: CoreMemoryStore               │
│                                          │
│  get_profile(limit: usize)               │
│    → Vec<ProfileEntry>                  │
│  get_persona(limit: usize)               │
│    → Vec<PersonaEntry>                  │
│  upsert_profile(key, value, conf, src)   │
│  upsert_persona(key, value, cat, conf,   │
│                  src)                    │
│  resolve_conflict(key, new_conf)         │
│    → ResolveResult                     │
│                                          │
│  Depth: ★★★☆☆  Moderate                 │
│  背后隐藏: 画像/人格的CRUD + 冲突解决     │
│           + confidence衰减 + provenance  │
│  接口大小: 5个方法                        │
└─────────────────────────────────────────┘
```

### 5.5 MemoryTreeSync

```
┌─────────────────────────────────────────┐
│ Interface: MemoryTreeSync                │
│                                          │
│  sync_session(session: &str)             │
│    → SyncReport   // 增量同步           │
│                                          │
│  Depth: ★★★★★  Deep                     │
│  背后隐藏: L0 Chunk切割 + L1/L2/L3级联   │
│           增量处理 + bucket密封 + TTL刷新 │
│           Markdown文件写入               │
│  接口大小: 1个方法                        │
└─────────────────────────────────────────┘
```

**为什么是深模块**：1 个方法，调用者不需要知道四层结构、密封条件、级联规则、文件路径。SyncReport 只告诉调用者"这次同步产生了几个新 Chunk，几个新 Daily"。

### 5.6 VaultWriter

```
┌─────────────────────────────────────────┐
│ Interface: VaultWriter                   │
│                                          │
│  write_chunk(date, seq, content)         │
│  write_daily(date, content)              │
│  write_weekly(year, week, content)       │
│  write_monthly(year, month, content)     │
│  write_profile(entries)                  │
│  write_persona(entries)                  │
│                                          │
│  Depth: ★★☆☆☆  Shallow                  │
│  背后隐藏: Markdown格式化 + 文件I/O      │
│  接口大小: 6个方法 → 偏浅               │
│  注: 这是Adapter层，浅是可以接受的        │
└─────────────────────────────────────────┘
```

---

## 六、模块规格：Agent & Tool

### 6.1 AgentRuntime

```
┌─────────────────────────────────────────┐
│ Interface: AgentRuntime                  │
│                                          │
│  current() → AgentState                 │
│  can_receive_message() → bool           │
│  is_generating() → bool                 │
│  try_start_thinking() → bool            │
│  cancel_generation()                     │
│  resume_thinking_after_tools()           │
│  mark_activity()                         │
│  idle_secs() → u64                      │
│                                          │
│  Depth: ★★★★☆  Deep                     │
│  背后隐藏: 10态状态机 + 转换守卫          │
│           + 空闲检测 + EventBus广播       │
│  接口大小: 8个方法 → 可接受               │
└─────────────────────────────────────────┘
```

### 6.2 ToolRegistry

```
┌─────────────────────────────────────────┐
│ Interface: ToolRegistry                  │
│                                          │
│  register(tool: Arc<dyn Tool>)           │
│  get(name: &str) → Option<&Arc<dyn Tool>>│
│  build_prompt(tool_policy: ToolPolicy)   │
│    → String                             │
│  to_openai_tools() → Vec<Value>         │
│  all_specs() → Vec<ToolSpec>            │
│                                          │
│  Depth: ★★★★★  Deep                     │
│  背后隐藏: 17个工具的统一注册/发现/提示词  │
│           语义路由 + OpenAI格式兼容       │
│  接口大小: 5个方法                        │
└─────────────────────────────────────────┘
```

**关键设计**：`Tool` trait 是缝合点 S5——每个工具是一个适配器：

```rust
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn permission(&self) -> ToolPermission;
    fn category(&self) -> ToolCategory;
    fn spec(&self) -> ToolSpec;
    async fn execute(&self, args: &Value, sandbox: &SandboxPolicy) -> ToolCallResult;
}
```

**为什么 `execute` 接收 `sandbox` 参数而不是内部持有**：这是"接收依赖，不要创建依赖"原则——工具不应该自己决定安全边界，SandboxPolicy 由 ToolDispatcher 注入。测试时可以直接传一个宽松的 SandboxPolicy。

### 6.3 ToolDispatcher

```
┌─────────────────────────────────────────┐
│ Interface: ToolDispatcher                │
│                                          │
│  dispatch(call: ToolCallRequest,         │
│           sandbox: &SandboxPolicy)       │
│    → ToolCallResult                     │
│                                          │
│  needs_confirmation(tool_name: &str)     │
│    → bool                               │
│                                          │
│  Depth: ★★★★☆  Deep                     │
│  背后隐藏: XML/JSON格式解析               │
│           权限检查 + L2/L3确认路由        │
│           工具超时 + Microcompact压缩     │
│  接口大小: 2个方法                        │
└─────────────────────────────────────────┘
```

**目标态改进**：
- 当前 L2/L3 等待确认使用 300ms sleep 轮询 → 改为 `tokio::sync::oneshot` channel
- 🆕 Microcompact：旧工具结果正文替换为 `[已执行: xxx, 成功]` 摘要

### 6.4 SandboxPolicy

```
┌─────────────────────────────────────────┐
│ Interface: SandboxPolicy                 │
│                                          │
│  is_path_allowed(path: &Path) → bool    │
│  is_command_allowed(cmd: &str) → bool   │
│  resolve_path(path: &str) → PathBuf     │
│  audit(entry: AuditEntry)               │
│                                          │
│  Depth: ★★★☆☆  Moderate                 │
│  背后隐藏: 路径白名单/黑名单 + 命令过滤   │
│           + 路径解析 + 审计日志           │
│  接口大小: 4个方法                        │
└─────────────────────────────────────────┘
```

---

## 七、模块规格：Task System

### 7.1 TaskPlanner

```
┌─────────────────────────────────────────┐
│ Interface: TaskPlanner                   │
│                                          │
│  plan(goal: &str, context: &TaskContext) │
│    → Result<Vec<TaskStep>>              │
│                                          │
│  Depth: ★★★★☆  Deep                     │
│  背后隐藏: LLM调用分解目标               │
│           步骤依赖关系分析               │
│           工具匹配推荐                    │
│  接口大小: 1个方法                        │
└─────────────────────────────────────────┘
```

### 7.2 TaskExecutor

```
┌─────────────────────────────────────────┐
│ Interface: TaskExecutor                  │
│                                          │
│  execute_step(step: &TaskStep,           │
│               context: &TaskContext)     │
│    → StepResult                         │
│                                          │
│  Depth: ★★★★★  Deep                     │
│  背后隐藏: LLM决策工具和参数              │
│           ToolDispatcher调用             │
│           重试逻辑(≤3次)                 │
│           确认等待 + 暂停/恢复            │
│  接口大小: 1个方法                        │
└─────────────────────────────────────────┘
```

### 7.3 TaskStore

```
┌─────────────────────────────────────────┐
│ Interface: TaskStore                     │
│                                          │
│  create(task: NewTask) → TaskId          │
│  get(id: &str) → Option<AgentTask>       │
│  list(status: Option<TaskStatus>)        │
│    → Vec<AgentTask>                     │
│  update_status(id, status)               │
│  save_step_result(id, step_id, result)   │
│                                          │
│  Depth: ★★★☆☆  Moderate                 │
│  接口大小: 5个方法                        │
└─────────────────────────────────────────┘
```

---

## 八、模块规格：Perception & Performance

### 8.1 Perception 模块

```
WindowObserver       ClipboardWatcher      IdleDetector
                                                          
  observe()           on_change()           idle_duration()
    → WindowInfo        → ClipboardSnapshot    → Duration
                                                          
所有模块共享 PerceptionEvent enum，通过 EventBus 广播。
```

### 8.2 Performance 模块

```
EmotionResolver      SpriteController      SpeechTimer
                         
  resolve(tag)        set_state(state)      estimate_duration(text)
    → Emotion           set_emotion(e)        → Duration
                        update_frame()
```

---

## 九、模块规格：Infrastructure

### 9.1 LlmClient

```
┌─────────────────────────────────────────┐
│ Interface: LlmClient (trait)             │
│                                          │
│  chat(messages: &[LlmMessage],           │
│       config: &LlmConfig,                │
│       tools: Option<&[ToolDef]>,         │
│       stream: Sender<StreamChunk>)       │
│    → Result<LlmUsage>                   │
│                                          │
│  Depth: ★★★★★  Deep                     │
│  背后隐藏: HTTP调用 + 流式解析            │
│           + 503/429重试 + 超时           │
│           + 多Provider适配               │
│  接口大小: 1个方法（核心）                │
│  适配器: DeepSeek / OpenAI / Anthropic   │
│           / Google / OpenRouter / Ollama │
└─────────────────────────────────────────┘
```

**为什么是 trait**：这是缝合点 S2——有 6+ 个适配器，是真实缝合点。调用者不需要知道用的是哪个 Provider。

### 9.2 Database

```
┌─────────────────────────────────────────┐
│ Interface: Database                      │
│                                          │
│  pool() → &SqlitePool                   │
│  migrate() → Result<()>                 │
│                                          │
│  Depth: ★★☆☆☆  Shallow (Adapter)        │
│  但这是基础设施层，浅是可以接受的         │
│  所有领域模块通过 SqlitePool 直接操作     │
│  当前 db.rs 15+表CRUD → 目标: 按领域拆分 │
└─────────────────────────────────────────┘
```

**目标态改进**：当前 `db.rs` 是 15+ 表的 CRUD 大杂烩——违反了 Locality 原则。目标态按领域拆分：

```rust
// 目标态：每个领域模块拥有自己的数据访问
impl EpisodeStore {
    // messages 表的所有 CRUD 在这里
}

impl CoreMemoryStore {
    // user_profile, persona_memory 表的所有 CRUD 在这里
}

impl SummaryEngine {
    // memory_summaries 表的所有 CRUD 在这里
}
```

### 9.3 EventBus

```
┌─────────────────────────────────────────┐
│ Interface: EventBus                      │
│                                          │
│  emit(event: AgentEvent)                 │
│  subscribe() → Receiver<AgentEvent>     │
│                                          │
│  Depth: ★★★☆☆  Moderate                 │
│  当前问题: 只用于日志层面，未被消费       │
│  目标态: AgentState变更 → 前端立绘绑定   │
│          Tool执行    → 前端确认弹窗       │
│          Task进度    → 前端任务面板       │
└─────────────────────────────────────────┘
```

---

## 十、模块规格：Play-With-Me (Future)

> Phase E/F 远期子系统。当前仅定义模块接口，不实现。

### 10.1 GameController

```
┌─────────────────────────────────────────┐
│ Interface: GameController                │
│                                          │
│  start(profile: &GameProfile)            │
│    → GameSession                        │
│  stop()                                  │
│  status() → GameState                   │
│                                          │
│  Depth: ★★★★★  Deep (目标)              │
│  背后隐藏: 屏幕捕获 + 输入模拟            │
│           + TacticalEngine + SocialEngine│
│           会话管理 + 安全边界             │
│  接口大小: 3个方法                        │
└─────────────────────────────────────────┘
```

### 10.2 ScreenCapture

```
┌─────────────────────────────────────────┐
│ Interface: ScreenCapture                 │
│                                          │
│  capture(window: &WindowHandle)          │
│    → Frame  // 15-30 FPS                │
│                                          │
│  Depth: ★★★★☆  Deep                     │
│  背后隐藏: Windows Graphics Capture API  │
│           + 帧率控制 + 格式转换           │
│  接口大小: 1个方法                        │
└─────────────────────────────────────────┘
```

---

## 十二、ADR：架构决策记录

> 所有决策于 2026-07-19 通过 grilling 确认。每个 ADR 满足三个条件：难以逆转、没有上下文会让人困惑、存在真实的权衡。

### ADR-0001：EventBus 仅用于 Agent 状态广播

**决策**：EventBus 不做通用模块间通信总线。仅用于 Agent 状态变更 → `app.emit("agent_state_changed", state)` → 前端被动响应。Chat Pipeline、Memory Stack、Tool System 之间的数据流全部使用直接函数调用。

**理由**：状态变更是"通知"（发送者不关心谁来消费），适合 pub/sub。管线中的每一步都是"我需要你的计算结果才能继续"，直接调用让调用链可追踪。

### ADR-0002：前端不感知 P1-P4 管线阶段

**决策**：前端只看到 Agent 状态（Thinking → Talking → ExecutingTool → WaitingConfirm → Idle），不感知 IntentDecision、ContextPack 等管线内部状态。S1（Tauri IPC）缝合点保持窄——`send_message` 进，`chat_token` / `chat_done` / `agent_state_changed` 出。

**理由**：管线总延迟 < 500ms，分阶段通知的 UI 闪烁大于信息价值。内部实现细节暴露给前端会破坏 S1 缝合点的封装。

### ADR-0003：统一数据库迁移 + 分散 CRUD

**决策**：`Database::migrate()` 一处建所有表（Schema 理解的 Locality），但每张表的 CRUD 归属对应领域模块（数据访问的 Locality）。迁移脚本中每个模块的建表语句用注释标注归属。

**理由**：Schema 全貌是原子知识，不应分散到 6 个文件。CRUD 是各模块的私有实现，外部不应直接碰其他模块的表。

### ADR-0004：延迟引入结构化错误类型

**决策**：当前保持 `anyhow::Result`。在 IPC 边界（`commands.rs` 的每个 `#[tauri::command]`）做统一的错误 → 用户友好消息映射。当至少 2 个调用者需要对同一错误做不同处理时，再引入 `CheboError` enum。

**理由**：单进程应用中，能真正确实需要"根据错误类型分支处理"的场景极少。提前引入 enum = 猜测需求，违反 YAGNI。

### ADR-0005：VectorStore 暂不引入 trait

**决策**：`VectorIndex` 直接使用 struct。`VectorIndex::new(pool, embedder)` 签名已支持依赖注入，未来切换后端只需新增构造方法。当有第二个向量后端适配器时再抽 trait。

**理由**：一个适配器 = 假设缝合点。当前只有 SQLite + `chebo-local-v1`，近期无切换计划。

### ADR-0006：会话 = 窗口生命周期

**决策**：窗口关闭 = 会话结束，下次打开 = 新会话，重新生成 System Prompt。桌宠 ↔ 助手模式切换不算新会话——仅改变 `response_mode`，通过 User Message 动态指定。

**理由**：窗口关闭后重开的间隔通常较长（小时级），System Prompt 重建成本（几分钱 + 几百毫秒）远小于"过时的 System Prompt 导致的糟糕回复"。

### ADR-0007：L2/L3 工具确认 = 聊天内嵌消息，非弹窗

**决策**：工具确认为聊天中的一条特殊消息气泡，[确认执行] / [拒绝] / [引导...] 三个按钮。确认气泡永不过期。确认期间用户可继续输入（消息排队到当前 turn 结束后执行）、可停止（终止 Tool Loop）、可引导（见 ADR-0008）。

**理由**：弹窗打断心流。排队消息让用户不会被"卡住"。引导提供 Codex 式的方向修正能力。

### ADR-0008：Steering（引导）= 中断当前流 + 注入指令 + 重新调用

**决策**：用户点击 `[引导 →]` → 立即 `cancel_generation()` → 将引导内容作为补充指令注入对话历史 → 重新调用 LLM。引导不是排队消息，是同一轮对话的即时方向修正。UI：排队消息右侧的 `[引导 →]` 按钮，默认消息排队，点击引导才执行中断。

**理由**：引导的本质是"立刻纠正方向"。等当前输出完成再处理 = 失去引导的实时性价值。

### ADR-0009：Memory Tree 异步 + 限流 + 用户可控

**决策**：摘要全部异步 fire-and-forget，任务进入串行队列（最多 1 个并发）。设置页增加开关"自动生成对话摘要"，标注"约每 10 条消息消耗一次 LLM 调用"。

**理由**：用户需要感知并控制 LLM 成本。摘要的核心价值是长期回顾（三个月后），不是实时反馈。

### ADR-0010：按意图模型路由——保持单一模型，预留配置结构

**决策**：当前保持用户选择的单一模型。`ConfigStore` 中预留 `HashMap<ChatIntent, ModelConfig>`，未来可扩展。不做自动路由——模型选择是成本和质量的 trade-off，决定权给用户。

**理由**：DeepSeek V3 和 R1 价格差异不大，多模型配置的体验收益当前小于复杂度。自动路由侵犯用户对成本的控制感。

### ADR-0011：Phase C 引入按意图的动态对话窗口

**决策**：当前保持 20 条固定窗口。Phase C 在 `ContextRequirements` 中增加 `history_window_size` 字段：CasualChat 10 条，ContinueTask 40 条，其余 20 条。

**理由**：与 ContextBuilder "按意图控制 token 预算"的理念一致。

### ADR-0012：commands.rs 拆分 + 前端单文件命名空间

**决策**：Rust 端 `commands.rs` 按领域拆分为多个文件。前端保持单一 `tauriService.ts`，内部用命名空间分组（`chat.sendMessage()` / `task.create()` / `config.get()`）。

**理由**：Tauri 的 `invoke` 是全局命名空间——Rust 端拆分是组织收益，前端拆分为多文件无强制执行能力。IPC 类型安全应靠 Rust 端导出 TypeScript 类型定义。

### 10.3 TacticalEngine

```
┌─────────────────────────────────────────┐
│ Interface: TacticalEngine                │
│                                          │
│  decide(frame: &Frame,                   │
│          profile: &GameProfile)          │
│    → TacticalAction                     │
│                                          │
│  Depth: ★★★★★  Deep                     │
│  背后隐藏: 视觉解析(OCR/CV)              │
│           规则引擎(躲避/瞄准/移动)        │
│           无LLM——毫秒级决策              │
│  接口大小: 1个方法                        │
└─────────────────────────────────────────┘
```

### 10.4 InputSimulator

```
┌─────────────────────────────────────────┐
│ Interface: InputSimulator                │
│                                          │
│  send(action: TacticalAction)            │
│  emergency_stop()                        │
│                                          │
│  Depth: ★★★★☆  Deep                     │
│  背后隐藏: Windows SendInput API         │
│           + 键盘/鼠标/手柄(XInput)       │
│           + 窗口焦点检测                 │
│           + 安全边界(失焦→停止)           │
│  接口大小: 2个方法                        │
└─────────────────────────────────────────┘
```

**关键安全设计**：`emergency_stop` 是硬件级别的——GameController 在游戏窗口失焦时自动调用，用户快捷键也会触发。

### 10.5 缝合点 S7：Game Profile

```
                    GameController
                          │
                    ┌─────┴─────┐
                    │ GameProfile│  ← Seam S7
                    │   trait    │
                    └─────┬─────┘
                          │
          ┌───────────────┼───────────────┐
          │               │               │
    IsaacProfile    BrotatoProfile    FutureGame...
```

每个 Game Profile 是 S7 上的一个适配器，包含：
- 窗口识别规则
- 操作映射表（游戏动作 → 键盘/手柄按键）
- 视觉 ROI 定义（血条、道具栏、小地图位置）
- 道具/Build 知识库（Markdown，可被 LLM 召回用于 Strategic Layer）

---

## 十一、深度审计：当前→目标

### 当前架构的浅模块问题

基于 Chebo 现有代码分析，存在以下几个需要深化的模块：

| # | 问题 | 当前状态 | 目标态 | 优先级 |
|---|------|---------|--------|--------|
| 1 | `db.rs` 15+ 表 CRUD 集中在一个文件 | **Shallow**: 调用者需要了解 15+ 表的结构 | 按领域拆分到各 Memory 模块内部 | P0 |
| 2 | `commands.rs` 41 个 Tauri 命令于一文件 | **Shallow**: 单个文件承担过多职责 | 按领域拆分为 chat_commands / task_commands / config_commands | P1 |
| 3 | `lib.rs` 30+ 模块初始化 | **Shallow**: 缺乏模块化的初始化 | 每个模块提供 `init(pool, config) → Self` | P1 |
| 4 | L2/L3 确认轮询用 300ms sleep | 浪费 CPU 且延迟不可控 | 改为 `tokio::sync::oneshot` channel | P0 |
| 5 | EventBus 仅用于日志 | 缝合点存在但未被消费 | 前端通过 EventBus 订阅 Agent 状态变更 | P1 |
| 6 | 向量检索在 SQLite BLOB 中计算余弦 | 当前规模可接受 | 预留 VectorStore trait，未来切换 | P2 |
| 7 | 错误处理以 String 为主 | 调用者无法区分错误类型 | 引入结构化错误类型 `CheboError` | P2 |

### 删除测试（对当前架构的评估）

| 模块 | 删除后... | 判断 |
|------|----------|------|
| IntentClassifier | 每个调用者需要自己实现意图路由 → 复杂度散布 | ✅ 深度模块，在干活 |
| ContextAssembler | 每个调用者需要自己决定召回什么 → token 预算失控 | ✅ 深度模块，在干活 |
| MemoryWriteRouter | 每个调用者需要自己评分/冲突检测 → 记忆质量下降 | ✅ 深度模块，在干活 |
| EventBus | 前端仍然可以通过 Tauri emit 通信 → 复杂度不变 | ⚠ 当前是透传，需要深化 |
| `db.rs` 中的 helper 函数 | 复杂度重新出现在各调用者 → 但调用者本来就应该拥有自己的数据访问 | ⚠ 应该重构为 Locality |

### Deepen Priority（深化优先级）

```
P0（阻塞性，当前就影响开发效率）:
  1. db.rs 拆分 → 按领域归属到各 Memory 模块
  2. L2/L3 确认改为 oneshot channel

P1（重要，影响代码可维护性）:
  3. commands.rs 按领域拆分
  4. lib.rs 模块初始化重构
  5. EventBus 作为真正的通信通道

P2（改善，当前规模可接受）:
  6. VectorStore trait 预留
  7. CheboError 结构化错误类型
```

---

## 附录 A：接口清单

| 模块 | 方法数 | 深度评级 | 缝合点 |
|------|--------|---------|--------|
| IntentClassifier | 1 | ★★★★★ Deep | — |
| ContextAssembler | 1 | ★★★★★ Deep | — |
| PromptBuilder | 2 | ★★★★☆ Deep | — |
| WorkingMemoryStore | 3 | ★★★★☆ Deep | — |
| MemoryWriteRouter | 1 | ★★★★★ Deep | — |
| EpisodeStore | 4 | ★★★★☆ Deep | S4 |
| SummaryEngine | 2 | ★★★☆☆ Moderate | — |
| VectorIndex | 2 | ★★★★☆ Deep | S3 |
| CoreMemoryStore | 5 | ★★★☆☆ Moderate | S4 |
| MemoryTreeSync | 1 | ★★★★★ Deep | — |
| VaultWriter | 6 | ★★☆☆☆ Shallow | — |
| AgentRuntime | 8 | ★★★★☆ Deep | — |
| ToolRegistry | 5 | ★★★★★ Deep | S5 |
| ToolDispatcher | 2 | ★★★★☆ Deep | — |
| SandboxPolicy | 4 | ★★★☆☆ Moderate | — |
| TaskPlanner | 1 | ★★★★☆ Deep | — |
| TaskExecutor | 1 | ★★★★★ Deep | — |
| TaskStore | 5 | ★★★☆☆ Moderate | — |
| LlmClient | 1 | ★★★★★ Deep | S2 |
| GameController (Future) | 3 | ★★★★★ Deep | S7 |
| ScreenCapture (Future) | 1 | ★★★★☆ Deep | — |
| TacticalEngine (Future) | 1 | ★★★★★ Deep | — |
| InputSimulator (Future) | 2 | ★★★★☆ Deep | — |

---

> **本文档与 FINAL_DESIGN.md 并行维护**：FINAL_DESIGN.md 回答"系统做什么"，本文档回答"模块怎么分、接口怎么设计、缝合点放哪里"。
