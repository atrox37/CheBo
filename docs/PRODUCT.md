# Chebo 产品说明（当前版本）

> 本文档只描述**当前产品能力**，不保留历史版本。功能大改时请同步更新本文档。  
> 维护规则见 `.cursor/rules/product-doc.mdc`。

**最后更新**：2026-07-03

---

## 产品定位

**Chebo** 是一款 Windows **AI 桌面伙伴（Ambient Agent）**：桌宠形态为轻量入口，助手模式为重操作台。技术基准见 docs/PET_AMBIENT_AGENT.md。本地 SQLite + Rust/Tauri + OpenAI 兼容 LLM（默认 DeepSeek），支持 Agent 工具循环。

---

## 核心能力一览

| 模块 | 能力 |
|------|------|
| 双模式 UI | 桌宠（聊天、伙伴面板、轻量工具）↔ 助手（聊天、设置、记忆、Agent 任务） |
| 对话 | 流式回复、多模态图片（Vision 回退）、503/429 自动重试与中文错误提示 |
| Agent | 工具注册表、意图路由、L0–L3 权限、用户确认 L2/L3 工具 |
| 记忆 | 四层记忆 + Vault Markdown 双写 + **向量语义检索** |
| 伙伴 | 默契度（affection）、Agent 状态、情绪表现 |
| 表现 | CrystalGirl PNGTuber 立绘、情绪映射、口型/眨眼 |

---

## 记忆系统

### 四层记忆（自动注入每次聊天）

1. **Working**：当前会话最近 N 条 messages
2. **Episode**：完整对话落库
3. **Summary**：每约 20 条触发 LLM 摘要
4. **Core**：user_profile、persona_memory、long_term_memories（置信 ≥ 0.7）

每次聊天通过 `build_rich_context_string` 注入画像与最近摘要（**按时间/条数，非语义检索**）。

### 向量记忆（按需语义检索）

- 索引表：`memory_vectors`（SQLite BLOB）
- 来源：摘要、长期记忆、用户画像、人格记忆
- 触发：Agent 调用 **memory_recall** 工具
- 检索：余弦相似度 Top-K；失败时回退关键词

### DeepSeek 与 Embedding（聊天 / 向量分离）

**DeepSeek 官方不提供 Embedding API。** Chebo 默认策略：

| 优先级 | 来源 | 说明 |
|--------|------|------|
| 1 | 用户配置 `embedding_base_url` / `embedding_model` | SQLite config 表 |
| 2 | 本机 Ollama（127.0.0.1:11434） | DeepSeek 聊天且 Ollama 在线 → `nomic-embed-text` |
| 3 | **内置 `chebo-local-v1`** | 零配置离线向量，DeepSeek 用户默认可用 |

可选 config：`embedding_base_url`（`local` 强制内置）、`embedding_model`、`embedding_api_key`。

---

## 事件与通信

- 已移除 WebSocket；统一 **Tauri invoke + event listen**（`tauriService.setupListeners`）

---

## 构建与运行

```bash
cd frontend && pnpm install && pnpm tauri dev
```

数据目录：`%APPDATA%\Chebo\`

---

## 文档索引

| 文档 | 用途 |
|------|------|
| docs/PRODUCT.md | 当前产品能力（本文档） |
| docs/AI_PRODUCT_ANALYSIS.md | 模块分析与路线图 |
| docs/ARCHITECTURE.md | 技术架构 |

---

## 已知限制

- 向量未接入每次聊天的自动上下文
- 设置页尚无 Embedding UI
- 内置向量质量弱于 Ollama/OpenAI embedding