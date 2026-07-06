# Chebo 产品说明（当前版本）

> 本文档只描述**当前产品能力**，不保留历史版本。功能大改时请同步更新本文档。  
> 维护规则见 `.cursor/rules/product-doc.mdc`。

**最后更新**：2026-07-06

---

## 产品定位

**Chebo** 是一款 Windows **AI 桌面伙伴（Ambient Agent）**：桌宠形态为轻量入口，助手模式为重操作台。本地 SQLite + Rust/Tauri + OpenAI 兼容 LLM（默认 DeepSeek），支持 Agent 工具循环。

---

## 核心能力一览

| 模块 | 能力 |
|------|------|
| 双模式 UI | 桌宠（320×285 透明悬浮，隐藏任务栏）↔ 助手（1000×680 标准窗口，显示任务栏） |
| 对话 | 流式回复、多模态图片（Vision 回退）、503/429 自动重试、桌宠 30-50 字短回复 / 助手不限字数 |
| Agent | 10 态状态机、工具注册表、三级意图路由、L0-L3 权限、用户确认 L2/L3 工具 |
| 工具系统 | 17 个工具，用户可开关，L2/L3 可设免确认 |
| 记忆 | 四层记忆 + 向量语义检索 + Markdown Vault，每 10 条消息自动生成 200-300 字详细摘要 |
| 画像 | 关键词提取用户画像 + 对话提取 Chebo 人格，system prompt 引导主动调用 memory_recall |
| 伙伴 | 默契度（affection）、Agent 状态、情绪表现 |
| 表现 | CrystalGirl PNGTuber 立绘、情绪映射、口型/眨眼 |

---

## 记忆系统

### 四层记忆（自动注入每次聊天）

1. **Working**：当前会话最近 20 条 messages
2. **Episode**：完整对话落库
3. **Summary**：每约 **10 条**触发 LLM 摘要（200-300 字详细格式，含项目/技术/个人信息）
4. **Core**：user_profile、persona_memory、long_term_memories（置信 ≥ 0.7）

每次聊天通过 `build_rich_context_string` 注入：
- 人格记忆最多 6 条 + 用户画像最多 8 条 + 历史摘要最近 **10 条** + 记忆片段最近 8 条

system prompt 含主动记忆召回指令（规则 6/7）。

### 向量记忆（按需语义检索）

- 索引表：`memory_vectors`（SQLite BLOB）
- 来源：摘要、长期记忆、用户画像、人格记忆
- 触发：Agent 调用 **memory_recall** 工具
- 检索：余弦相似度 Top-K；失败时回退关键词

### 内置向量模型

DeepSeek 用户默认使用内置 `chebo-local-v1` 零配置向量；支持 Ollama `nomic-embed-text` 或 OpenAI embedding 替代。

---

## 工具系统（17 个）

用户可在设置 → **工具管理** 中开关每个工具，并为 L2/L3 工具开启免确认模式。

| 工具 | 权限 | 默认 |
|------|------|------|
| read_file, list_dir, search_files | L0 | ✅ |
| write_file, replace_in_file | L2 | ❌ |
| safe_shell | L3 | ✅ |
| open_file, get_system_info, process_list | L0/L1 | ✅ |
| set_reminder | L1 | ❌ |
| web_search, web_fetch | L0 | ✅ |
| memory_recall, note_take | L0/L1 | ✅ |
| clipboard_read | L1 | ✅ |
| take_screenshot | L1 | ✅ |
| git_status | L1 | ✅ |

---

## 事件与通信

- 统一 **Tauri invoke + event listen**（`tauriService.setupListeners`）

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

- 向量未接入每次聊天的自动上下文（需 LLM 主动调用 memory_recall）
- 用户画像为关键词硬匹配（未接入 LLM 深度提取）
- 无定时任务系统（仅简化版 set_reminder）
- 无 Live2D 动画（静态 PNG 立绘）