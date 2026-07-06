# Chebo AI 桌面宠物

一个基于 **Tauri v2 + Vue 3 + Rust** 的智能 AI 桌面伴侣。  
后端完全由 Rust 实现（无 Python、无独立进程），开箱即用。

> 详细技术文档见 [PROJECT_SUMMARY.md](./PROJECT_SUMMARY.md)

---

## 快速开始

### 环境要求

- Node.js 18+
- Rust 1.75+（`rustup update stable`）
- pnpm（`npm i -g pnpm`）
- Windows 10/11 + Visual Studio C++ Build Tools

### 1. 安装依赖

```bash
cd frontend
pnpm install
```

### 2. 配置 API Key

在项目根目录创建 `.env` 文件：

```env
# 使用 DeepSeek（推荐）
LLM_PROVIDER=deepseek
DEEPSEEK_API_KEY=sk-xxxxxxxx

# 或使用 OpenAI
# LLM_PROVIDER=openai
# OPENAI_API_KEY=sk-xxxxxxxx

# 或使用本地 Ollama
# LLM_PROVIDER=ollama
# OLLAMA_BASE_URL=http://localhost:11434
# OLLAMA_MODEL=llama3
```

### 3. 启动开发

```bash
cd frontend
pnpm tauri dev
```

### 4. 构建发布包

```bash
cd frontend
$env:RUST_MIN_STACK="67108864"   # Windows 下防止编译器栈溢出
pnpm tauri build
```

安装包位于 `frontend/src-tauri/target/release/bundle/`

---

## 核心特性

| 功能 | 说明 |
|------|------|
| 双模式窗口 | 桌宠模式（320×285 透明悬浮，隐藏任务栏）↔ 助手模式（1000×680 标准窗口） |
| AI 对话 | 流式回复 + 工具调用循环（最多8轮） + 多步任务自动识别 |
| 工具系统 | 17 个工具：读/写/替换文件、Web 搜索/抓取、Shell、Git、剪贴板、截图、系统信息、进程列表、提醒、笔记、记忆检索 |
| 工具权限 | L0-L3 分级 + 用户可开关 + 自动批准设置 |
| 语义路由 | 三级路由自动选择工具，纯聊天 0 工具 token 消耗 |
| 工具沙盒 | 路径白名单 + 命令黑名单 + 速率限制 + 审计日志 |
| 长期任务 | LLM 分解任务 → 步骤化执行 → 可暂停/续跑 |
| 记忆系统 | 四层记忆 + 向量语义检索 + Markdown Vault 双存储，每 10 条消息自动生成详细摘要 |
| 宠物养成 | 饥饿/精力/心情/好感度/等级，时间衰减，投喂/商店 |
| 感知系统 | 检测前台窗口切换 / 剪贴板变化 / 用户空闲 |
| 系统托盘 | 常驻通知区，`Ctrl+Shift+Space` 全局快捷键 |

---

## 数据目录

应用数据存储在：`C:\Users\{用户名}\AppData\Roaming\Chebo\`

- `chebo.db` — SQLite 数据库（宠物状态、聊天记录、任务、记忆等）
- `.env` — API Key 配置（可在此覆盖项目根目录的 .env）
- `vault/` — Markdown 记忆 Vault

---

*详细架构说明见 [PROJECT_SUMMARY.md](./PROJECT_SUMMARY.md)*