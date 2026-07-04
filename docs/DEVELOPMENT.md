# Chebo AI 桌宠 - 开发指南

> **注意**：本文档部分章节仍保留早期 Python 后端的描述，仅供参考。当前版本已迁移为 **Rust 全栈（Tauri IPC）**，请以 [README.md](../README.md) 与 [PROJECT_SUMMARY.md](../PROJECT_SUMMARY.md) 为准。

## 📚 目录

- [环境准备](#环境准备)
- [项目结构](#项目结构)
- [开发流程](#开发流程)
- [常见任务](#常见任务)
- [调试技巧](#调试技巧)
- [部署指南](#部署指南)
- [常见问题](#常见问题)

## 🛠️ 环境准备

### 系统要求

- **操作系统**: Windows 10+ / macOS 10.15+ / Linux
- **Node.js**: 18.0.0+
- **Rust**: 1.70.0+
- **Python**: 3.11.0+

### 安装开发工具

#### 1. 安装 Node.js 和 pnpm

```bash
# 安装 Node.js (https://nodejs.org/)
# 安装 pnpm
npm install -g pnpm
```

#### 2. 安装 Rust

```bash
# Windows (PowerShell)
winget install --id=Rustlang.Rustup -e

# macOS / Linux
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

#### 3. 安装 Tauri 依赖

**Windows**:
```bash
# 安装 Microsoft C++ Build Tools
# https://visualstudio.microsoft.com/visual-cpp-build-tools/

# 安装 WebView2
# 通常 Windows 10/11 已预装
```

**macOS**:
```bash
xcode-select --install
```

**Linux (Debian/Ubuntu)**:
```bash
sudo apt update
sudo apt install libwebkit2gtk-4.0-dev \
    build-essential \
    curl \
    wget \
    file \
    libssl-dev \
    libgtk-3-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev
```

#### 4. 安装 Python 依赖

```bash
# 创建虚拟环境
cd backend
python -m venv venv

# 激活虚拟环境
# Windows
venv\Scripts\activate
# macOS/Linux
source venv/bin/activate

# 安装依赖
pip install -r requirements.txt
```

### 获取 API Key

#### OpenAI API
1. 访问 https://platform.openai.com/
2. 注册并获取 API Key
3. 在 `backend/.env` 中配置

```env
OPENAI_API_KEY=sk-xxx
OPENAI_BASE_URL=https://api.openai.com/v1
```

#### 或使用本地模型（Ollama）

```bash
# 安装 Ollama
# https://ollama.ai/

# 下载模型
ollama pull llama2

# 配置 .env
USE_LOCAL_MODEL=true
OLLAMA_BASE_URL=http://localhost:11434
```

## 📁 项目结构

```
chebo-ai-desktop-pet/
├── frontend/                 # 前端项目
│   ├── src/
│   │   ├── main.ts           # 入口文件
│   │   ├── App.vue           # 根组件
│   │   ├── components/       # 组件目录
│   │   ├── stores/           # Pinia 状态
│   │   ├── services/         # 服务层
│   │   ├── composables/      # 组合式函数
│   │   ├── types/            # TypeScript 类型
│   │   └── assets/           # 静态资源
│   ├── src-tauri/            # Tauri Rust 代码
│   │   ├── src/
│   │   │   └── main.rs       # Rust 主程序
│   │   ├── Cargo.toml        # Rust 依赖
│   │   └── tauri.conf.json   # Tauri 配置
│   ├── package.json
│   ├── vite.config.ts
│   └── tsconfig.json
│
├── backend/                  # 后端项目
│   ├── main.py               # FastAPI 入口
│   ├── config.py             # 配置
│   ├── models/               # 数据模型
│   ├── services/             # 业务逻辑
│   ├── api/                  # API 路由
│   ├── database/             # 数据库
│   └── requirements.txt      # Python 依赖
│
├── assets/                   # 共享资源
│   └── live2d/               # Live2D 模型
│
├── docs/                     # 文档
│   ├── ARCHITECTURE.md       # 架构设计
│   ├── API.md                # API 文档
│   ├── DEVELOPMENT.md        # 开发指南
│   └── MVP_PLAN.md           # MVP 计划
│
└── README.md                 # 项目说明
```

## 🚀 开发流程

### 1. 克隆项目并安装依赖

```bash
# 安装前端依赖
cd frontend
pnpm install

# 安装后端依赖
cd ../backend
python -m venv venv
source venv/bin/activate  # Windows: venv\Scripts\activate
pip install -r requirements.txt
```

### 2. 配置环境变量

在 `backend/` 目录下创建 `.env` 文件：

```env
# AI 模型配置
OPENAI_API_KEY=your_api_key_here
OPENAI_BASE_URL=https://api.openai.com/v1
MODEL_NAME=gpt-4o-mini

# 或使用本地模型
# USE_LOCAL_MODEL=true
# OLLAMA_BASE_URL=http://localhost:11434
# LOCAL_MODEL_NAME=llama2

# 数据库路径
DATABASE_PATH=./data/chebo.db
VECTOR_DB_PATH=./data/vector_db

# 语音配置
TTS_PROVIDER=openai
TTS_VOICE=nova
TTS_MODEL=tts-1

# 服务器配置
HOST=127.0.0.1
PORT=8000

# 日志级别
LOG_LEVEL=INFO
```

### 3. 启动开发服务

**启动后端服务**（终端 1）:
```bash
cd backend
source venv/bin/activate  # Windows: venv\Scripts\activate
python main.py
```

后端服务将运行在 http://localhost:8000

**启动前端服务**（终端 2）:
```bash
cd frontend
pnpm tauri dev
```

这将启动 Vite 开发服务器并打开 Tauri 窗口。

### 4. 开发过程

- 前端代码修改会自动热重载
- 后端代码修改需要重启服务（或使用 `uvicorn --reload`）
- Rust 代码修改需要重新编译

## 🔧 常见任务

### 添加新的 Vue 组件

```typescript
// frontend/src/components/MyComponent.vue
<script setup lang="ts">
import { ref } from 'vue'

const message = ref('Hello Chebo!')
</script>

<template>
  <div class="my-component">
    {{ message }}
  </div>
</template>

<style scoped>
.my-component {
  /* 样式 */
}
</style>
```

### 添加 Pinia Store

```typescript
// frontend/src/stores/myStore.ts
import { defineStore } from 'pinia'
import { ref } from 'vue'

export const useMyStore = defineStore('my', () => {
  const count = ref(0)
  
  function increment() {
    count.value++
  }
  
  return { count, increment }
})
```

### 调用 Tauri Command

**Rust 侧定义命令**:
```rust
// frontend/src-tauri/src/main.rs
#[tauri::command]
fn my_custom_command(input: String) -> String {
    format!("Hello, {}!", input)
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![my_custom_command])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

**前端调用**:
```typescript
// frontend/src/services/tauri.ts
import { invoke } from '@tauri-apps/api/tauri'

export async function myCustomCommand(input: string): Promise<string> {
  return await invoke('my_custom_command', { input })
}
```

### 添加后端 API 端点

```python
# backend/api/my_router.py
from fastapi import APIRouter

router = APIRouter(prefix="/api/my", tags=["My API"])

@router.get("/hello")
async def hello(name: str = "World"):
    return {"message": f"Hello, {name}!"}
```

```python
# backend/main.py
from api.my_router import router as my_router

app.include_router(my_router)
```

### 添加 Live2D 动作

1. 在 Live2D Cubism Editor 中创建动作
2. 导出 `.motion3.json` 文件
3. 放置到 `assets/live2d/chebo/motions/`
4. 在 `model3.json` 中注册动作：

```json
{
  "FileReferences": {
    "Motions": {
      "MyMotionGroup": [
        { "File": "motions/my_motion.motion3.json" }
      ]
    }
  }
}
```

5. 在代码中播放：

```typescript
// frontend/src/services/live2d.ts
cheboRenderer.playMotion('MyMotionGroup', 0, MotionPriority.NORMAL)
```

### 数据库操作

```python
# backend/database/sqlite_db.py
import sqlite3
from typing import List, Dict

class Database:
    def __init__(self, db_path: str):
        self.conn = sqlite3.connect(db_path)
        self.conn.row_factory = sqlite3.Row
        self._init_tables()
    
    def _init_tables(self):
        cursor = self.conn.cursor()
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                emotion TEXT,
                timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
            )
        """)
        self.conn.commit()
    
    def save_message(self, session_id: str, role: str, content: str, emotion: str = None):
        cursor = self.conn.cursor()
        cursor.execute(
            "INSERT INTO messages (session_id, role, content, emotion) VALUES (?, ?, ?, ?)",
            (session_id, role, content, emotion)
        )
        self.conn.commit()
    
    def get_recent_messages(self, session_id: str, limit: int = 10) -> List[Dict]:
        cursor = self.conn.cursor()
        cursor.execute(
            "SELECT * FROM messages WHERE session_id = ? ORDER BY timestamp DESC LIMIT ?",
            (session_id, limit)
        )
        rows = cursor.fetchall()
        return [dict(row) for row in reversed(rows)]
```

## 🐛 调试技巧

### 前端调试

**使用 Vue DevTools**:
1. 安装浏览器扩展：[Vue DevTools](https://devtools.vuejs.org/)
2. 在 Tauri 开发模式下按 `F12` 打开开发者工具
3. 切换到 Vue 面板

**调试 Live2D**:
```typescript
// 启用 Live2D 日志
window.Live2DCubismCore.Logging.csmSetLogFunction((message: string) => {
  console.log('[Live2D]', message)
})
```

**调试 WebSocket**:
```typescript
// frontend/src/services/websocket.ts
const ws = new WebSocket('ws://localhost:8000/ws/chat')

ws.onopen = () => console.log('WebSocket 连接已建立')
ws.onmessage = (event) => console.log('收到消息:', event.data)
ws.onerror = (error) => console.error('WebSocket 错误:', error)
ws.onclose = () => console.log('WebSocket 连接已关闭')
```

### 后端调试

**启用详细日志**:
```python
# backend/main.py
import logging

logging.basicConfig(
    level=logging.DEBUG,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
```

**使用 pdb 调试器**:
```python
# 在需要调试的地方插入
import pdb; pdb.set_trace()
```

**测试 API 端点**:
```bash
# 使用 curl
curl http://localhost:8000/api/chat/history?limit=10

# 或使用 httpie
http GET http://localhost:8000/api/chat/history limit==10
```

### Rust 调试

**打印调试信息**:
```rust
println!("Debug: {:?}", some_variable);
```

**使用 Rust 调试器**:
```bash
# 在 VS Code 中安装 "rust-analyzer" 和 "CodeLLDB" 扩展
# 然后可以在代码中设置断点
```

## 📦 部署指南

### 构建生产版本

```bash
cd frontend
pnpm tauri build
```

构建产物位于 `frontend/src-tauri/target/release/bundle/`

### 支持的平台

- **Windows**: `.msi` 安装包
- **macOS**: `.dmg` 磁盘镜像、`.app` 应用包
- **Linux**: `.deb`, `.AppImage`

### 打包后端

**方式 1: PyInstaller**
```bash
cd backend
pip install pyinstaller
pyinstaller --onefile main.py
```

**方式 2: Docker**
```dockerfile
FROM python:3.11-slim

WORKDIR /app
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

COPY . .

CMD ["python", "main.py"]
```

### 分发

1. 将前端可执行文件和后端打包在一起
2. 包含必要的资源文件（Live2D 模型、配置模板等）
3. 提供安装说明和配置指南

## ❓ 常见问题

### Q: Tauri 窗口无法启动

**A**: 检查以下几点：
1. 确保安装了 WebView2 (Windows) 或对应的依赖
2. 检查 `tauri.conf.json` 配置是否正确
3. 查看终端错误信息

### Q: Live2D 模型加载失败

**A**: 可能的原因：
1. 模型文件路径不正确
2. 模型文件损坏
3. Live2D SDK 版本不兼容
4. 纹理文件缺失

解决方法：
```typescript
// 检查模型路径
console.log('模型路径:', modelPath)

// 添加错误处理
try {
  await live2dModel.load(modelPath)
} catch (error) {
  console.error('加载模型失败:', error)
}
```

### Q: WebSocket 连接失败

**A**: 检查：
1. 后端服务是否启动
2. 端口是否被占用
3. 防火墙是否阻止连接
4. WebSocket URL 是否正确

```typescript
// 测试连接
const testWebSocket = () => {
  const ws = new WebSocket('ws://localhost:8000/ws/chat')
  ws.onopen = () => console.log('✅ 连接成功')
  ws.onerror = (error) => console.error('❌ 连接失败:', error)
}
```

### Q: LLM API 调用失败

**A**: 检查：
1. API Key 是否正确
2. 网络是否畅通
3. API 配额是否用尽
4. API URL 是否正确

```python
# 测试 API
import openai

try:
    response = openai.ChatCompletion.create(
        model="gpt-4",
        messages=[{"role": "user", "content": "Hello"}]
    )
    print("✅ API 正常:", response)
except Exception as e:
    print("❌ API 错误:", e)
```

### Q: 内存占用过高

**A**: 优化方法：
1. 限制聊天历史加载数量
2. 定期清理旧的音频文件
3. 优化 Live2D 渲染（降低帧率）
4. 使用对象池复用资源

```typescript
// 限制渲染帧率
const targetFPS = 30
const frameTime = 1000 / targetFPS

let lastTime = 0
function render(currentTime: number) {
  if (currentTime - lastTime >= frameTime) {
    // 渲染逻辑
    lastTime = currentTime
  }
  requestAnimationFrame(render)
}
```

### Q: 口型同步不准确

**A**: 改进方法：
1. 使用更精确的音素识别（如 Montreal Forced Aligner）
2. 调整音素到口型参数的映射
3. 添加插值平滑过渡
4. 根据语速调整时间轴

```typescript
// 平滑口型过渡
function updateLipSync(targetValue: number) {
  const current = model.getParameterValueById('ParamMouthOpenY')
  const smooth = current + (targetValue - current) * 0.3
  model.setParameterValueById('ParamMouthOpenY', smooth)
}
```

## 📚 参考资源

### 官方文档
- [Vue.js 文档](https://vuejs.org/)
- [Tauri 文档](https://tauri.app/)
- [FastAPI 文档](https://fastapi.tiangolo.com/)
- [Live2D Cubism SDK](https://www.live2d.com/en/download/cubism-sdk/)
- [OpenAI API 文档](https://platform.openai.com/docs/)

### 社区资源
- [Tauri Discord](https://discord.com/invite/tauri)
- [Vue.js Discord](https://discord.com/invite/vue)
- [Live2D 论坛](https://community.live2d.com/)

### 相关项目
- [VTuber 应用](https://github.com/topics/vtuber)
- [桌面宠物项目](https://github.com/topics/desktop-pet)
- [AI 助手项目](https://github.com/topics/ai-assistant)

---

**文档版本**: 1.0  
**最后更新**: 2026-05-11
