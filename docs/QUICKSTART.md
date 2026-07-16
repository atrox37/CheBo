# Chebo AI 桌面宠物 — 快速启动指南

> 基于 **Tauri v2 + Vue 3 + Rust** 的智能 AI 桌面伴侣，开箱即用，无需单独启动 Python 后端。

---

## 环境要求

- **Node.js** 18+
- **Rust** 1.75+（[rustup.rs](https://rustup.rs/)）
- **pnpm** 9+（`npm install -g pnpm`）
- **Windows 10/11** + Visual Studio C++ Build Tools（WebView2，Win11 通常已自带）

---

## 1. 安装依赖

```bash
cd frontend
pnpm install
```

---

## 2. 配置 API Key

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

生产环境也可将 `.env` 放在 `%APPDATA%\Chebo\.env`。

---

## 3. 启动开发

```bash
cd frontend
pnpm install          # 首次或报 vite 找不到时必跑
pnpm tauri:dev
```

仓库根目录也可：

```bash
pnpm install:frontend
pnpm dev
```

首次启动会编译 Rust，可能需要几分钟。

---

## 4. 构建发布包

```bash
cd frontend
# Windows 下防止编译器栈溢出：
$env:RUST_MIN_STACK="67108864"
pnpm tauri build
```

安装包位于 `frontend/src-tauri/target/release/bundle/`。

---

## 关于角色显示

当前使用 CrystalGirl PNGTuber 立绘（静态 PNG），支持情绪映射和口型/眨眼。Live2D 动画尚未接入。

---

## AI 模型选择

| 提供商 | 获取地址 | 说明 |
|--------|---------|------|
| **DeepSeek**（默认） | [platform.deepseek.com](https://platform.deepseek.com/) | 国内速度快，推荐 |
| OpenAI | [platform.openai.com](https://platform.openai.com/) | 需配置代理 |
| Anthropic | [console.anthropic.com](https://console.anthropic.com/) | Claude 系列 |
| Google | [ai.google.dev](https://ai.google.dev/) | Gemini 系列 |
| OpenRouter | [openrouter.ai](https://openrouter.ai/) | 一个 Key 访问所有模型 |
| Ollama | [ollama.com](https://ollama.com/) | 本地运行，可完全离线推理 |

在 `.env` 中修改 `LLM_PROVIDER` 即可切换，无需改代码。

---

## 数据目录

应用数据存储在：

```
C:\Users\{用户名}\AppData\Roaming\Chebo\
```

- `chebo.db` — SQLite 数据库
- `vault/` — Markdown 记忆 Vault
- `.env` — 运行时 API Key 配置

---

## 常见报错

### `vite` 不是内部或外部命令

```bash
cd frontend
pnpm install
```

### `failed to read plugin permissions` / 路径指向旧目录

项目若从其他路径移动过，需清理 Rust 编译缓存：

```powershell
Remove-Item -Recurse -Force frontend\src-tauri\target
cd frontend
pnpm tauri:dev
```

或根目录：`pnpm clean:rust`

### `pnpm tauri dev` 编译失败

```text
确认 Rust 已安装：rustc --version
Windows 需安装 Visual Studio C++ Build Tools
```

栈溢出时可手动设置：

```powershell
$env:RUST_MIN_STACK="67108864"
pnpm tauri:dev
```

### PowerShell 权限问题

```powershell
Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser
```

### 窗口无法打开

```text
Win10 需安装 WebView2 运行时
查看终端完整错误日志
```

---

## 项目结构

```
erii-ai-desktop-pet/          # 仓库目录
├── frontend/                 # Tauri + Vue 3 前端
│   ├── src/                  # Vue 源码
│   └── src-tauri/            # Rust 后端（Agent / 记忆 / 工具 / 40+ 命令）
├── docs/                     # 产品文档
├── README.md                 # 项目说明
├── PROJECT_SUMMARY.md        # 完整技术总结
└── QUICKSTART.md             # 本文件
```

完整架构说明见 [PROJECT_SUMMARY.md](./PROJECT_SUMMARY.md)。

---

**产品名称**：Chebo  
**最后更新**：2026-07-06