# Chebo AI 桌宠 - MVP 开发计划

## 📌 MVP 版本目标

开发一个可运行的基础版本 AI 桌宠，实现核心的对话和展示功能。

### 核心功能清单
- ✅ Tauri 透明悬浮窗口
- ✅ Vue 3 响应式界面
- ✅ Live2D 角色渲染（Chebo 模型）
- ✅ 基础交互（拖拽、点击）
- ✅ 文本聊天对话
- ✅ 流式回复显示
- ✅ TTS 语音合成
- ✅ 表情/动作联动
- ✅ SQLite 本地存储
- ✅ 简单长期记忆

## 🗓️ 开发阶段

### 第一阶段：项目初始化（第 1-2 天）✅ 已完成

#### 目标
搭建基础框架，确保开发环境可用。

#### 任务清单

**1. 前端项目初始化**
- [x] 使用 Vite 创建 Vue 3 + TypeScript 项目
- [x] 安装 Tauri 2.x CLI 和依赖
- [x] 配置 `tauri.conf.json`（透明窗口、无边框、置顶 400×600）
- [x] 安装必要的依赖：pinia / tailwindcss 4 / @tauri-apps/api

**2. 后端项目初始化**
- [x] 创建 Python 虚拟环境
- [x] 安装 FastAPI / uvicorn / websockets / openai / pydantic-settings
- [x] 创建基础项目结构
- [x] 配置 `.env` + 多 LLM 提供商支持（DeepSeek / OpenAI / Ollama）

**3. 前端核心组件**
- [x] App.vue 透明根组件（data-tauri-drag-region 拖拽）
- [x] CharacterDisplay.vue（PNG 占位图）
- [x] ChatBubble.vue（毛玻璃气泡 + 打字机效果）
- [x] ChatInput.vue（底部输入框，点击角色弹出）
- [x] WebSocket 服务（自动重连）
- [x] Pinia stores（chat / chebo）

**4. 后端 AI 对话接入**
- [x] ConversationEngine（DeepSeek 流式调用）
- [x] CharacterEngine（Chebo 人设 Prompt + 情绪识别）

**验收标准**
- [x] 能够启动 Tauri 透明窗口
- [x] 后端 FastAPI 服务运行在 `http://localhost:8000`
- [x] 前端能够连接 WebSocket `ws://localhost:8000/ws/chat`
- [x] pnpm run build 编译通过

---

### 第二阶段：Live2D 渲染（第 3-5 天）⏭️ 暂时跳过（等待模型资源）

> **跳过原因**：Live2D 模型文件（.moc3 / .model3.json）尚未就绪。
> 当前使用 `public/chebo-placeholder.png` 作为占位图。
> 等模型资源准备好后再回来实现本阶段。

#### 目标
成功加载和显示 Live2D 模型，实现基础动画。

#### 任务清单

**1. Live2D 环境搭建**
- [ ] 下载 Live2D Cubism SDK for Web
- [ ] 集成 SDK 到 Vue 项目
- [ ] 创建 `CheboRenderer` 服务类

**2. 模型资源准备**
- [ ] 准备 Chebo Live2D 模型资源：
  - `chebo.model3.json` - 模型配置
  - `chebo.moc3` - 模型数据
  - `textures/` - 纹理文件
  - `motions/` - 动作文件
  - `expressions/` - 表情文件
- [ ] 将资源放置到 `frontend/src/assets/live2d/chebo/`

**3. 渲染器实现**
- [ ] 创建 PixiJS Canvas
- [ ] 加载 Live2D 模型
- [ ] 实现渲染循环（60fps）
- [ ] 实现待机动作（idle）
- [ ] 实现鼠标跟随效果

**4. 表情和动作系统**
- [ ] 实现表情切换 API
- [ ] 实现动作播放 API
- [ ] 实现动作优先级管理
- [ ] 测试不同表情和动作的切换

**验收标准**
- Live2D 模型在透明窗口中正常显示
- 模型会播放待机动作
- 模型眼睛会跟随鼠标移动
- 可以通过代码切换表情和播放动作

---

### 第三阶段：基础交互（第 6-7 天）✅ 已完成（Live2D 点击区域除外）

#### 目标
实现窗口拖拽、点击反馈等基础交互。

#### 任务清单

**1. 窗口拖拽**
- [ ] 创建 `InteractionManager` 类
- [ ] 实现 Tauri 窗口拖拽
- [ ] 限制窗口拖拽范围（不超出屏幕）

**2. 点击交互**
- [ ] 实现 Live2D 点击检测（Hit Test）
- [ ] 定义点击区域（头部、身体等）
- [ ] 实现点击反馈动作
  - 点击头部：播放 `touch_head` 动作
  - 点击身体：播放 `touch_body` 动作

**3. 输入框设计**
- [ ] 创建聊天输入框组件 `ChatInput.vue`
- [ ] 实现输入框的显示/隐藏
- [ ] 点击 Chebo 显示输入框
- [ ] 输入框失焦自动隐藏

**验收标准**
- 可以拖拽窗口移动
- 点击 Chebo 不同部位有不同反馈
- 输入框可以正常显示和输入

---

### 第四阶段：对话功能（第 8-11 天）✅ 已完成（Live2D 表情联动除外）

#### 目标
实现完整的 AI 对话功能。

#### 任务清单

**1. 后端对话引擎**
- [ ] 创建 `ConversationEngine` 类
- [ ] 实现 OpenAI API 调用
- [ ] 实现流式回复处理
- [ ] 创建 `CharacterEngine` 类
- [ ] 编写 Chebo 人设 Prompt

**2. WebSocket 通信**
- [ ] 实现后端 WebSocket 端点 `/ws/chat`
- [ ] 实现消息接收和处理
- [ ] 实现流式回复推送
- [ ] 前端实现 WebSocket 连接管理

**3. 前端对话 UI**
- [ ] 创建聊天气泡组件 `ChatBubble.vue`
- [ ] 实现消息列表组件 `MessageList.vue`
- [ ] 实现流式回复显示（打字机效果）
- [ ] 实现聊天历史滚动

**4. 情绪和动作联动**
- [ ] 后端识别回复中的情绪
- [ ] 根据情绪选择表情和动作
- [ ] 前端接收情绪数据
- [ ] 触发对应的 Live2D 表情和动作

**5. Pinia 状态管理**
- [ ] 创建 `chatStore` 管理对话状态
- [ ] 创建 `cheboStore` 管理 Chebo 状态（表情、动作）
- [ ] 实现状态持久化

**验收标准**
- 用户可以发送消息给 Chebo
- Chebo 会流式返回回复（打字机效果）
- Chebo 的表情和动作会根据情绪变化
- 聊天历史正常显示

---

### 第五阶段：记忆系统（第 12-14 天）🔄 进行中

#### 目标
实现对话历史存储和简单的长期记忆。

#### 任务清单

**1. 数据库设计**
- [ ] 设计 SQLite 数据库 Schema
  - `messages` 表：聊天历史
  - `config` 表：配置项
  - `long_term_memories` 表：长期记忆
- [ ] 创建数据库初始化脚本

**2. 记忆引擎**
- [ ] 创建 `MemoryEngine` 类
- [ ] 实现消息存储 API
- [ ] 实现消息查询 API
- [ ] 实现长期记忆提取逻辑

**3. 上下文管理**
- [ ] 对话引擎集成记忆引擎
- [ ] 构建对话上下文时包含历史消息
- [ ] 实现对话历史的滑动窗口（最近 10 条）

**4. 简单长期记忆**
- [ ] 识别重要信息（用户偏好、个人信息等）
- [ ] 自动提取并保存为长期记忆
- [ ] 在对话中召回相关记忆

**验收标准**
- 对话历史保存到数据库
- 重启应用后历史仍然存在
- Chebo 能够记住之前对话中的重要信息
- 对话中能够体现长期记忆（如用户名字、偏好等）

---

### 第五点五阶段：Chebo 养成系统 v1（第 14-20 天）⏳ 待开发

> **核心理念**：Chebo = 桌宠数值养成系统 + AI 人格反馈系统 + 未来 Agent 能力入口
>
> 数值不要太复杂，先保证反馈明显；Chebo 的人格反馈比数值本身更重要；每个行为都要能触发一句角色化台词。

---

#### UI 架构：Tab 操作栏

窗口 **300×420**，底部常驻 **Tab 操作栏 + 聊天输入框**，点击 Tab 弹出对应功能面板。

```
┌────────────────────────────┐  420px
│  [气泡：最新消息 absolute]   │
│                            │
│       [角色图片]             │  character-layer: flex:1 ≈ 220px
│                            │
│  ❤── 🍜── ⚡── Lv1 💰100  │  status-bar: 28px (absolute overlay)
├────────────────────────────┤
│  [💬][🍜 投喂][📚 学习][💼 工作][🏪 商店]  │  tab-bar: 40px
├────────────────────────────┤
│                            │  tab-panel: 0 ↔ 130px (transition)
│  [当前 Tab 内容区域]          │  仅选中 Tab 时展开
│                            │
├────────────────────────────┤
│  [输入框…]        [发送 ▶] │  chat-input: 46px (始终可见)
└────────────────────────────┘
```

**Tab 说明：**
| Tab | 图标 | 功能面板 |
|-----|------|---------|
| 聊天 | 💬 | 无面板（panel 关闭），保持默认交互 |
| 投喂 | 🍜 | 食物列表（面包/牛奶/小蛋糕）+ 当前金币 |
| 学习 | 📚 | 学习任务列表 + 当前进度条 |
| 工作 | 💼 | 工作任务列表 + 当前进度条 |
| 商店 | 🏪 | MVP 阶段：仅食物商店（未来扩展装扮/家具） |

---

#### 1. 核心数值系统（MVP 子集）

所有数值范围 0 ～ 100：

```typescript
// MVP 初始值（完整系统见设计文档）
CheboMVPStats {
  hunger:    80,   // 饱腹度，越高越不饿
  energy:    80,   // 精力
  mood:      70,   // 心情
  affection: 20,   // 好感度
  level:     1,
  exp:       0,
  coins:     100
}
```

---

#### 2. 时间衰减规则（每 1 分钟 tick）

**普通待机：**
```
hunger  -0.5 / min
energy  -0.25 / min
mood    根据 hunger 和 energy 微调
```

**阈值联动：**
```
hunger < 30  →  mood -5（每 tick）
hunger < 20  →  energy -10（每 tick）
energy < 25  →  工作/学习收益降低 30%
```

**长时间未互动：**
```
30 分钟未互动  →  主动发言触发检测
2 小时未互动   →  mood -5，触发 lonely_idle
```

---

#### 3. 行为状态系统

**MVP 行为枚举：**
```
ActionState = idle | hungry | sleepy | happy | eating | studying | working
```

**自动行为判断（每 3 分钟）：**
```
hunger < 30    →  currentAction = hungry
energy < 25    →  currentAction = sleepy
mood > 80      →  currentAction = happy
（任务进行中）  →  currentAction = studying / working
otherwise      →  currentAction = idle
```

**状态优先级（高 → 低）：** eating > sleeping > hungry > sleepy > sad > working > studying > happy > idle

---

#### 4. 投喂系统

**MVP 食物列表：**
```typescript
foods = [
  { id: "bread",  name: "面包",   price: 10, hungerRestore: 20, moodChange: +1 },
  { id: "milk",   name: "牛奶",   price: 12, hungerRestore: 12, energyChange: +5, moodChange: +2 },
  { id: "cake",   name: "小蛋糕", price: 25, hungerRestore: 15, moodChange: +10 }
]
```

**规则：**
- 扣除 coins → 更新对应数值 → 播放 eating 状态 → 触发 Chebo 反馈台词
- 过度投喂（hunger > 95）：`mood -3`，`health -1`，Chebo 说"不用再喂啦……"

---

#### 5. 学习系统

**MVP 学习任务：**
```typescript
studyTasks = [
  {
    id: "read_book", name: "读书", durationMinutes: 10,
    energyCost: 10,  stressGain: 2,
    expGain: 20,     intelligenceGain: 1,  moodChange: -2
  }
]
```

**限制：**
- `energy < 25`：不能开始学习
- `hunger < 25`：学习收益减半

**完成奖励：** exp +20，mood 根据 energy 状态微调

---

#### 6. 工作系统

**MVP 工作任务：**
```typescript
workTasks = [
  {
    id: "organize_notes", name: "整理笔记", durationMinutes: 10,
    energyCost: 12,  stressGain: 2,
    coinsGain: 30,   expGain: 15,  moodChange: -3
  }
]
```

**限制：**
- `energy < 20`：不能工作
- `hunger < 20`：不能工作

---

#### 7. 等级与经验系统

```
nextLevelExp = level * 100
// Lv1→Lv2: 100exp  |  Lv2→Lv3: 200exp  ...
```

**升级奖励：** `coins +50`，`mood +10`，`energy +10`，`affection +2`

**关键解锁节点：**
```
Lv2：解锁学习任务"解题"
Lv3：解锁工作任务"调试代码"
Lv5：解锁装饰商店（第七阶段）
Lv10：解锁主动 Agent 提醒（第八阶段）
```

---

#### 8. 主动发言系统

**触发条件（带冷却）：**
```
hunger < 30       →  高优先级，5 分钟冷却："肚子好饿……"
energy < 25       →  高优先级，5 分钟冷却："好困啊……"
长时间未互动       →  中优先级，15 分钟冷却：随机想你发言
升级              →  立即触发："哇！我升级啦！"
定时 AI 分析       →  30 分钟冷却：LLM 根据状态生成自言自语
```

---

#### 9. AI 人格联动（对话上下文注入）

每次 Chebo 回复时，system prompt 附带状态摘要：

```python
CheboContext {
  currentStats: { hunger, energy, mood, affection, level },
  currentAction: "idle | studying | ...",
  affectionStage: "陌生 | 熟悉 | 信任 | 亲近 | 默契",
  recentEvents: ["completed_study", "fed", ...]
}
```

**回复风格规则：**
```
mood > 80   →  更活泼，更爱说话
energy < 25 →  更短、更困倦、省字
affection > 60 →  语气更亲近
hunger < 30 →  不时提到饿
```

---

#### 10. 数据持久化

| 保存项 | 频率 |
|--------|------|
| CheboStats（全部数值） | 每 1 分钟自动保存 |
| 关键事件（投喂/升级/完成任务） | 立即保存 |
| 背包/金币 | 每次变动立即保存 |
| 退出前 | 完整快照 |

---

#### 11. 数据库 Schema（完整版）

```sql
-- 扩展 pet_status 表（替换旧版简单表）
CREATE TABLE pet_status (
    id              INTEGER PRIMARY KEY DEFAULT 1,
    -- 生理
    hunger          INTEGER DEFAULT 80,
    energy          INTEGER DEFAULT 80,
    -- 情绪
    mood            INTEGER DEFAULT 70,
    -- 关系
    affection       INTEGER DEFAULT 20,
    -- 成长
    level           INTEGER DEFAULT 1,
    exp             INTEGER DEFAULT 0,
    -- 资源
    coins           INTEGER DEFAULT 100,
    -- 当前行为
    current_action  TEXT    DEFAULT 'idle',
    -- 任务状态
    active_task_id  TEXT,        -- 进行中的任务 ID
    task_ends_at    TEXT,        -- 任务结束时间
    task_type       TEXT,        -- 'study' | 'work' | null
    -- 互动追踪
    last_interaction_at TEXT,
    -- 元数据
    updated_at      TEXT DEFAULT (datetime('now','localtime'))
);
INSERT OR IGNORE INTO pet_status (id) VALUES (1);

-- 背包（食物等消耗品）
CREATE TABLE inventory (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id     TEXT    NOT NULL,
    item_type   TEXT    NOT NULL,   -- 'food' | 'clean' | 'gift'
    count       INTEGER DEFAULT 1,
    acquired_at TEXT    DEFAULT (datetime('now','localtime'))
);

-- 事件日志（用于记忆/成就/统计）
CREATE TABLE pet_events (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    type        TEXT    NOT NULL,
    payload     TEXT,               -- JSON string
    created_at  TEXT    DEFAULT (datetime('now','localtime'))
);
```

---

#### 12. WebSocket 消息协议（完整版）

| 方向 | type | 说明 |
|------|------|------|
| S→C | `status_update` | 每次状态变化（tick / 操作后）推送完整状态 |
| S→C | `status_comment` | AI 自言自语气泡 |
| S→C | `task_progress` | 任务进度更新（每 10 秒推送一次） |
| S→C | `task_complete` | 任务完成 + 奖励详情 |
| S→C | `level_up` | 升级事件 |
| C→S | `feed` | `{ food_id: "bread" }` |
| C→S | `task_start` | `{ task_id: "read_book", task_type: "study" }` |
| C→S | `task_cancel` | 取消当前任务 |

---

#### 13. 实现任务清单

**后端（`backend/`）：**
- [ ] 扩展 `sqlite_db.py`：新 Schema + inventory / pet_events 表
- [ ] 重构 `status_engine.py`：
  - 完整 MVP 数值 + 衰减规则
  - `feed(food_id)` 支持多种食物
  - `task_start(task_id, task_type)` 通用任务启动
  - 任务进度推送（每 10 秒）
  - `_check_level_up()` 升级检测
  - 状态上下文注入 `conversation_engine`
- [ ] 扩展 `main.py`：新 WS 消息类型 + `/api/inventory` 端点

**前端（`frontend/src/`）：**
- [ ] 扩展 `stores/pet.ts`：完整 MVP 状态 + 任务状态 + 背包
- [ ] 新建 `components/TabBar.vue`：5 个 Tab 图标按钮
- [ ] 新建 `components/panels/FeedPanel.vue`：食物格子 + 购买/使用
- [ ] 新建 `components/panels/StudyPanel.vue`：任务卡片 + 进度条
- [ ] 新建 `components/panels/WorkPanel.vue`：任务卡片 + 进度条
- [ ] 新建 `components/panels/ShopPanel.vue`：食物商店（MVP）
- [ ] 重构 `App.vue`：Tab 布局，移除旧 `PetActions.vue`
- [ ] 更新 `services/websocket.ts`：处理新 WS 事件

---

#### 验收标准
- [ ] 状态栏显示 hunger / energy / mood / Lv / 💰，每分钟自动衰减
- [ ] Tab 操作栏切换流畅，面板弹出/收起有动画
- [ ] 投喂面板：显示食物列表，消耗金币，Chebo 有台词反馈
- [ ] 学习/工作面板：选择任务后显示进度条，完成后获得奖励
- [ ] 商店面板：购买食物添加到背包
- [ ] 每 30 分钟 AI 生成一条状态自言自语
- [ ] 状态影响 Chebo 对话风格（energy 低时回复变短/变困）
- [ ] 升级时推送 level_up 事件，Chebo 有欢呼台词

---

### 第六阶段：语音功能（第 15-17 天）

#### 目标
实现 TTS 语音合成和口型同步。

#### 任务清单

**1. TTS 集成**
- [ ] 创建 `VoiceEngine` 类
- [ ] 集成 OpenAI TTS API
- [ ] 实现文字转语音接口
- [ ] 保存生成的音频文件到临时目录

**2. 音频播放**
- [ ] 前端实现音频播放功能
- [ ] 创建 `useVoice` 组合式函数
- [ ] 实现播放队列管理
- [ ] 实现播放状态监听

**3. 口型同步**
- [ ] 生成音素时间轴数据（简化版）
- [ ] 实现 Live2D 口型参数控制
- [ ] 根据音频播放进度同步口型
- [ ] 调整口型动画的流畅度

**4. 用户控制**
- [ ] 添加语音开关按钮
- [ ] 添加音量控制
- [ ] 添加语音速度控制

**验收标准**
- Chebo 的回复会自动转换为语音
- 语音播放时嘴巴会动
- 口型与语音基本同步
- 用户可以控制是否启用语音

---

### 第七阶段：UI 美化和优化（第 18-20 天）

#### 目标
完善 UI 设计，优化性能，修复 Bug。

#### 任务清单

**1. UI 美化**
- [ ] 设计并实现聊天气泡样式
- [ ] 设计输入框样式（毛玻璃效果）
- [ ] 添加动画效果（淡入淡出、滑动等）
- [ ] 设计设置面板 UI

**2. 设置面板**
- [ ] 创建设置面板组件 `SettingsPanel.vue`
- [ ] 实现配置项：
  - AI 模型选择
  - 温度参数
  - 语音开关
  - 语音音色选择
- [ ] 实现配置保存和加载

**3. 系统托盘**
- [ ] Tauri 实现系统托盘图标
- [ ] 添加托盘菜单：
  - 显示/隐藏
  - 设置
  - 退出
- [ ] 实现窗口隐藏到托盘

**4. 性能优化**
- [ ] 优化 Live2D 渲染性能
- [ ] 减少不必要的重渲染
- [ ] 优化 WebSocket 消息处理
- [ ] 限制聊天历史加载数量

**5. 错误处理**
- [ ] 添加全局错误捕获
- [ ] 网络错误提示
- [ ] API 调用失败重试
- [ ] 日志记录

**6. Bug 修复和测试**
- [ ] 全面测试所有功能
- [ ] 修复发现的 Bug
- [ ] 测试边界情况
- [ ] 测试长时间运行稳定性

**验收标准**
- UI 美观、动画流畅
- 设置面板功能完整
- 系统托盘正常工作
- 无明显 Bug 和性能问题

---

## 📦 技术实现细节

### Tauri 配置 (`tauri.conf.json`)

```json
{
  "build": {
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build",
    "devPath": "http://localhost:5173",
    "distDir": "../dist"
  },
  "package": {
    "productName": "Chebo",
    "version": "0.1.0"
  },
  "tauri": {
    "allowlist": {
      "all": false,
      "shell": {
        "all": false,
        "open": true
      },
      "dialog": {
        "all": true
      },
      "fs": {
        "all": false,
        "readFile": true,
        "writeFile": true,
        "scope": ["$APPDATA/*", "$RESOURCE/*"]
      },
      "window": {
        "all": false,
        "center": true,
        "close": true,
        "create": false,
        "hide": true,
        "maximize": false,
        "minimize": true,
        "print": false,
        "requestUserAttention": true,
        "setAlwaysOnTop": true,
        "setDecorations": false,
        "setFocus": true,
        "setFullscreen": false,
        "setIcon": false,
        "setPosition": true,
        "setResizable": true,
        "setSize": true,
        "setSkipTaskbar": false,
        "setTitle": false,
        "show": true,
        "startDragging": true,
        "unmaximize": false,
        "unminimize": false
      }
    },
    "bundle": {
      "active": true,
      "category": "Utility",
      "copyright": "",
      "deb": {
        "depends": []
      },
      "externalBin": [],
      "icon": [
        "icons/32x32.png",
        "icons/128x128.png",
        "icons/128x128@2x.png",
        "icons/icon.icns",
        "icons/icon.ico"
      ],
      "identifier": "com.chebo.desktop",
      "longDescription": "",
      "macOS": {
        "entitlements": null,
        "exceptionDomain": "",
        "frameworks": [],
        "providerShortName": null,
        "signingIdentity": null
      },
      "resources": [],
      "shortDescription": "",
      "targets": "all",
      "windows": {
        "certificateThumbprint": null,
        "digestAlgorithm": "sha256",
        "timestampUrl": ""
      }
    },
    "security": {
      "csp": null
    },
    "updater": {
      "active": false
    },
    "windows": [
      {
        "fullscreen": false,
        "height": 600,
        "width": 400,
        "resizable": false,
        "title": "Chebo",
        "decorations": false,
        "transparent": true,
        "alwaysOnTop": true,
        "skipTaskbar": false
      }
    ],
    "systemTray": {
      "iconPath": "icons/icon.png",
      "iconAsTemplate": true
    }
  }
}
```

### 依赖版本

**前端 (package.json)**
```json
{
  "dependencies": {
    "vue": "^3.4.0",
    "pinia": "^2.1.7",
    "@pixi/app": "^7.3.0",
    "@pixi/display": "^7.3.0",
    "live2dcubismcore": "^4.0.0",
    "@cubism/cubismwebframework": "^4-r.7"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^1.5.0",
    "@tauri-apps/api": "^1.5.0",
    "@vitejs/plugin-vue": "^5.0.0",
    "typescript": "^5.3.0",
    "vite": "^5.0.0",
    "tailwindcss": "^3.4.0"
  }
}
```

**后端 (requirements.txt)**
```txt
fastapi==0.109.0
uvicorn[standard]==0.27.0
websockets==12.0
openai==1.10.0
python-dotenv==1.0.0
pydantic==2.5.0
pydantic-settings==2.1.0
```

---

## 🧪 测试计划

### 单元测试
- [ ] Live2D 渲染器测试
- [ ] WebSocket 连接测试
- [ ] 对话引擎测试
- [ ] 记忆引擎测试

### 集成测试
- [ ] 完整对话流程测试
- [ ] 情绪识别和动作联动测试
- [ ] 记忆存储和召回测试
- [ ] TTS 和口型同步测试

### 性能测试
- [ ] 长时间运行测试（24 小时）
- [ ] 内存占用测试
- [ ] CPU 占用测试
- [ ] 渲染帧率测试

### 用户测试
- [ ] 邀请 3-5 位用户试用
- [ ] 收集反馈
- [ ] 修复体验问题

---

## 📊 进度追踪

| 阶段 | 预计时间 | 状态 | 完成度 |
|------|---------|------|--------|
| 第一阶段：项目初始化 | 1-2 天 | 🔄 进行中 | 10% |
| 第二阶段：Live2D 渲染 | 3-5 天 | ⏳ 未开始 | 0% |
| 第三阶段：基础交互 | 6-7 天 | ⏳ 未开始 | 0% |
| 第四阶段：对话功能 | 8-11 天 | ⏳ 未开始 | 0% |
| 第五阶段：记忆系统 | 12-14 天 | ⏳ 未开始 | 0% |
| 第六阶段：语音功能 | 15-17 天 | ⏳ 未开始 | 0% |
| 第七阶段：优化打磨 | 18-20 天 | ⏳ 未开始 | 0% |

**总体进度**: 10%

---

## 🚀 下一步行动

### 立即开始
1. **创建前端项目**
   ```bash
   cd frontend
   npm create vite@latest . -- --template vue-ts
   npm install
   npm install @tauri-apps/cli @tauri-apps/api
   npm install pinia tailwindcss
   ```

2. **初始化 Tauri**
   ```bash
   npm install -D @tauri-apps/cli
   npx tauri init
   ```

3. **创建后端项目**
   ```bash
   cd backend
   python -m venv venv
   venv\Scripts\activate  # Windows
   pip install fastapi uvicorn websockets openai python-dotenv
   ```

4. **创建基础文件结构**
   - 前端：`src/main.ts`, `src/App.vue`
   - 后端：`main.py`, `config.py`

5. **测试运行**
   - 前端：`npm run tauri dev`
   - 后端：`python main.py`

---

**文档版本**: 1.0  
**创建日期**: 2026-05-11  
**预计完成日期**: 2026-06-01
