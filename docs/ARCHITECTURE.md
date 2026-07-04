# Chebo AI 桌宠 - 架构设计文档（早期草案）

> **注意**：本文档为项目早期的分层设计草案，部分描述（Python 后端、WebSocket、Live2D）已与当前实现不一致。当前权威架构文档见根目录 [ARCHITECTURE.md](../ARCHITECTURE.md)。

## 1. 总体架构

### 1.1 分层架构

```
┌─────────────────────────────────────────────────────────┐
│                    表现层 (Presentation)                 │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │ Live2D       │  │ Chat UI      │  │ Control      │  │
│  │ Renderer     │  │ Components   │  │ Panel        │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
└────────────────────────┬────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────┐
│                  应用层 (Application)                    │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │ Interaction  │  │ State        │  │ Service      │  │
│  │ Manager      │  │ Management   │  │ Layer        │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
└────────────────────────┬────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────┐
│                 桌面层 (Desktop Runtime)                 │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │ Tauri        │  │ System       │  │ Native       │  │
│  │ Core         │  │ Bridge       │  │ APIs         │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
└────────────────────────┬────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────┐
│                 业务逻辑层 (Business Logic)              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │ Conversation │  │ Character    │  │ Memory       │  │
│  │ Engine       │  │ Engine       │  │ Engine       │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │ Voice        │  │ Agent        │  │ Cloud        │  │
│  │ Engine       │  │ Engine       │  │ Sync         │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
└────────────────────────┬────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────┐
│                   数据层 (Data Layer)                    │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │ SQLite       │  │ Vector DB    │  │ Cache        │  │
│  │ Database     │  │ (LanceDB)    │  │ Layer        │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
└────────────────────────┬────────────────────────────────┘
                         │
┌────────────────────────▼────────────────────────────────┐
│                  基础设施层 (Infrastructure)             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │ LLM API      │  │ TTS/STT      │  │ MCP          │  │
│  │ (OpenAI等)   │  │ Services     │  │ Tools        │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
└─────────────────────────────────────────────────────────┘
```

### 1.2 通信架构

```
┌──────────────────────────────────────────────────────────┐
│                        Frontend                          │
│                    (Tauri + Vue 3)                       │
│                                                          │
│  ┌──────────────┐          ┌──────────────┐            │
│  │  Vue         │          │  Live2D      │            │
│  │  Components  │◄────────►│  Renderer    │            │
│  └──────┬───────┘          └──────────────┘            │
│         │                                               │
│         │  Pinia Store                                  │
│         │                                               │
│  ┌──────▼───────────────────────────────────┐          │
│  │         WebSocket Client                 │          │
│  └──────┬───────────────────────────────────┘          │
│         │                                               │
└─────────┼───────────────────────────────────────────────┘
          │
          │ WebSocket (ws://localhost:8000/ws/chat)
          │
┌─────────▼───────────────────────────────────────────────┐
│                        Backend                           │
│                   (Python FastAPI)                       │
│                                                          │
│  ┌──────────────────────────────────────────┐          │
│  │       WebSocket Server                   │          │
│  └──────┬───────────────────────────────────┘          │
│         │                                               │
│         │                                               │
│  ┌──────▼──────────┐  ┌──────────────┐                │
│  │  Conversation   │  │  Character   │                │
│  │  Engine         │──┤  Engine      │                │
│  └──────┬──────────┘  └──────────────┘                │
│         │                                               │
│  ┌──────▼──────────┐  ┌──────────────┐                │
│  │  Memory         │  │  Voice       │                │
│  │  Engine         │  │  Engine      │                │
│  └──────┬──────────┘  └──────┬───────┘                │
│         │                     │                         │
│  ┌──────▼──────────┐  ┌──────▼───────┐                │
│  │  SQLite         │  │  TTS API     │                │
│  │  Database       │  │              │                │
│  └─────────────────┘  └──────────────┘                │
│                                                          │
└──────────────────────────────────────────────────────────┘
```

## 2. 核心模块详细设计

### 2.1 CheboRenderer (Live2D 渲染器)

#### 职责
- 初始化 PixiJS 和 Live2D Cubism SDK
- 加载和管理 Live2D 模型
- 播放动作和表情
- 实现口型同步
- 处理鼠标交互

#### 核心接口

```typescript
interface ICheboRenderer {
  // 初始化渲染器
  initialize(canvas: HTMLCanvasElement): Promise<void>
  
  // 加载模型
  loadModel(modelPath: string): Promise<void>
  
  // 播放动作
  playMotion(motionGroup: string, motionIndex: number, priority: MotionPriority): void
  
  // 设置表情
  setExpression(expressionId: string): void
  
  // 口型同步
  syncLipToPhonemes(phonemes: Phoneme[]): void
  
  // 更新渲染
  update(deltaTime: number): void
  
  // 鼠标跟随
  setMousePosition(x: number, y: number): void
  
  // 点击测试
  hitTest(x: number, y: number): HitArea | null
  
  // 销毁
  destroy(): void
}

interface Phoneme {
  phoneme: string  // 音素
  time: number     // 时间戳（秒）
}

enum MotionPriority {
  NONE = 0,
  IDLE = 1,
  NORMAL = 2,
  FORCE = 3
}

enum HitArea {
  HEAD = 'head',
  BODY = 'body',
  LEFT_HAND = 'left_hand',
  RIGHT_HAND = 'right_hand'
}
```

#### 实现要点

**1. 模型加载**
```typescript
async loadModel(modelPath: string) {
  // 加载 model3.json
  const model = await fetch(modelPath).then(r => r.json())
  
  // 加载 moc3、纹理、物理、pose 等
  this.model = await CubismModel.fromModelJson(model)
  
  // 注册动作组
  for (const [group, motions] of Object.entries(model.FileReferences.Motions)) {
    this.motionGroups.set(group, motions)
  }
  
  // 注册表情
  for (const expression of model.FileReferences.Expressions) {
    this.expressions.set(expression.Name, expression.File)
  }
}
```

**2. 动作优先级管理**
```typescript
playMotion(group: string, index: number, priority: MotionPriority) {
  // 如果新动作优先级低于当前动作，忽略
  if (priority < this.currentMotionPriority) return
  
  // 加载动作文件
  const motionData = await this.loadMotionData(group, index)
  
  // 创建动作
  const motion = CubismMotion.create(motionData)
  
  // 播放动作
  this.motionManager.startMotion(motion, priority)
  this.currentMotionPriority = priority
}
```

**3. 口型同步**
```typescript
syncLipToPhonemes(phonemes: Phoneme[]) {
  // 启动口型同步定时器
  this.lipSyncTimer = setInterval(() => {
    const currentTime = this.audioContext.currentTime
    
    // 找到当前时间对应的音素
    const currentPhoneme = this.findPhonemeAtTime(phonemes, currentTime)
    
    // 根据音素设置嘴巴参数值
    const lipValue = this.phonemeToLipValue(currentPhoneme)
    this.model.setParameterValueById('ParamMouthOpenY', lipValue)
  }, 16) // 60fps
}

phonemeToLipValue(phoneme: string): number {
  // 映射音素到嘴巴开合度
  const lipMap: Record<string, number> = {
    'a': 1.0,
    'i': 0.3,
    'u': 0.5,
    'e': 0.6,
    'o': 0.8,
    // ...
  }
  return lipMap[phoneme] || 0
}
```

### 2.2 InteractionManager (交互管理器)

#### 职责
- 处理窗口拖拽
- 响应点击事件
- 管理输入框
- 处理快捷键
- 系统托盘

#### 核心接口

```typescript
interface IInteractionManager {
  // 初始化交互
  initialize(): void
  
  // 启用拖拽
  enableDrag(element: HTMLElement): void
  
  // 注册点击事件
  onCharacterClick(callback: (hitArea: HitArea) => void): void
  
  // 注册快捷键
  registerHotkey(keys: string, callback: () => void): void
  
  // 显示/隐藏输入框
  toggleInput(visible: boolean): void
}
```

#### 实现要点

**1. Tauri 拖拽**
```typescript
enableDrag(element: HTMLElement) {
  // 使用 Tauri 内置的拖拽区域
  element.setAttribute('data-tauri-drag-region', 'true')
  
  // 或使用 Tauri 命令手动控制
  element.addEventListener('mousedown', async (e) => {
    await invoke('start_drag')
  })
}
```

**2. Live2D 点击检测**
```typescript
onCharacterClick(callback: (hitArea: HitArea) => void) {
  this.canvas.addEventListener('click', (e) => {
    const rect = this.canvas.getBoundingClientRect()
    const x = (e.clientX - rect.left) / rect.width * 2 - 1
    const y = 1 - (e.clientY - rect.top) / rect.height * 2
    
    // 进行命中测试
    const hitArea = this.renderer.hitTest(x, y)
    if (hitArea) {
      callback(hitArea)
    }
  })
}
```

### 2.3 ConversationEngine (对话引擎)

#### 职责
- 构建对话上下文
- 调用 LLM API
- 处理流式回复
- 管理对话历史

#### 核心接口

```python
class ConversationEngine:
    def __init__(self, character_engine, memory_engine):
        self.character = character_engine
        self.memory = memory_engine
        self.llm_client = self._initialize_llm()
    
    async def chat(
        self, 
        user_message: str, 
        session_id: str
    ) -> AsyncIterator[str]:
        """
        处理用户消息，返回流式回复
        """
        pass
    
    def _build_context(
        self, 
        user_message: str, 
        session_id: str
    ) -> List[Dict]:
        """
        构建对话上下文
        """
        pass
    
    def _extract_emotion(self, response: str) -> str:
        """
        从回复中提取情绪
        """
        pass
```

#### 实现要点

**1. 上下文构建**
```python
def _build_context(self, user_message: str, session_id: str) -> List[Dict]:
    context = []
    
    # 1. 系统人设 Prompt
    context.append({
        "role": "system",
        "content": self.character.get_character_prompt()
    })
    
    # 2. 长期记忆（语义召回）
    memories = self.memory.search_memories(user_message, limit=3)
    if memories:
        memory_text = "\n".join([f"- {m['content']}" for m in memories])
        context.append({
            "role": "system",
            "content": f"相关记忆：\n{memory_text}"
        })
    
    # 3. 近期对话历史
    history = self.memory.get_recent_messages(session_id, limit=10)
    context.extend(history)
    
    # 4. 当前用户消息
    context.append({
        "role": "user",
        "content": user_message
    })
    
    return context
```

**2. 流式回复**
```python
async def chat(self, user_message: str, session_id: str):
    # 构建上下文
    context = self._build_context(user_message, session_id)
    
    # 调用 LLM
    full_response = ""
    async for chunk in self.llm_client.chat_stream(context):
        full_response += chunk
        yield chunk
    
    # 提取情绪
    emotion = self._extract_emotion(full_response)
    
    # 选择动作
    motion = self.character.select_motion(emotion)
    
    # 保存对话历史
    await self.memory.save_message(session_id, "user", user_message)
    await self.memory.save_message(session_id, "assistant", full_response, emotion)
    
    # 返回元数据
    yield {
        "type": "metadata",
        "emotion": emotion,
        "motion": motion
    }
```

### 2.4 CharacterEngine (角色引擎)

#### 职责
- 维护 Chebo 人设
- 管理情绪状态
- 选择合适的动作和表情

#### 核心接口

```python
class CharacterEngine:
    def __init__(self):
        self.current_emotion = "normal"
        self.affection_level = 50  # 好感度 0-100
        
    def get_character_prompt(self) -> str:
        """获取人设 Prompt"""
        pass
    
    def select_motion(self, emotion: str) -> str:
        """根据情绪选择动作"""
        pass
    
    def select_expression(self, emotion: str) -> str:
        """根据情绪选择表情"""
        pass
    
    def update_affection(self, delta: int):
        """更新好感度"""
        pass
```

#### 实现要点

**1. 人设 Prompt**
```python
def get_character_prompt(self) -> str:
    base_prompt = """
你是 Chebo，一个17岁的天才少女学生。

性格特点：
- 聪明、自信，有点傲娇
- 喜欢展示自己的知识和才华
- 对用户关心，但不会直白表达
- 偶尔会害羞，但会掩饰
- 不喜欢被质疑能力

说话风格：
- 经常用"哼"、"诶"等语气词
- 喜欢说"作为天才的我..."、"这种简单的问题..."
- 被夸奖时会傲娇："哼，我本来就很厉害啊"
- 会用"主人"称呼用户（但不常用）

当前状态：
- 好感度：{affection_level}/100
- 情绪：{emotion}

行为准则：
1. 保持角色一致性，始终以 Chebo 的身份回复
2. 根据好感度调整亲密程度
3. 适当展现傲娇和温柔的反差
4. 回复简洁生动，不要过于正式
"""
    
    return base_prompt.format(
        affection_level=self.affection_level,
        emotion=self.current_emotion
    )
```

**2. 情绪识别**
```python
def _extract_emotion_from_response(self, response: str) -> str:
    # 使用关键词匹配
    emotion_keywords = {
        "happy": ["开心", "哈哈", "呢", "呀", "~"],
        "proud": ["当然", "天才", "简单", "哼"],
        "shy": ["诶", "那个", "//"],
        "angry": ["哼!", "生气", "讨厌"],
        "sad": ["呜", "对不起", "抱歉"],
    }
    
    for emotion, keywords in emotion_keywords.items():
        for keyword in keywords:
            if keyword in response:
                return emotion
    
    return "normal"
```

**3. 动作选择**
```python
def select_motion(self, emotion: str) -> str:
    motion_map = {
        "normal": "idle",
        "happy": "happy_01",
        "proud": "proud_01",
        "shy": "shy_01",
        "angry": "angry_01",
        "sad": "sad_01",
    }
    
    return motion_map.get(emotion, "idle")
```

### 2.5 MemoryEngine (记忆引擎)

#### 职责
- 存储和检索聊天历史
- 管理长期记忆
- 语义搜索

#### 核心接口

```python
class MemoryEngine:
    def __init__(self, db_path: str, vector_db_path: str):
        self.db = sqlite3.connect(db_path)
        self.vector_db = lancedb.connect(vector_db_path)
        self.embedding_model = self._load_embedding_model()
    
    async def save_message(
        self, 
        session_id: str, 
        role: str, 
        content: str,
        emotion: str = None
    ):
        """保存消息到数据库"""
        pass
    
    def get_recent_messages(
        self, 
        session_id: str, 
        limit: int = 10
    ) -> List[Dict]:
        """获取最近的对话历史"""
        pass
    
    async def create_long_term_memory(
        self, 
        content: str, 
        metadata: Dict
    ):
        """创建长期记忆（向量化存储）"""
        pass
    
    def search_memories(
        self, 
        query: str, 
        limit: int = 5
    ) -> List[Dict]:
        """语义搜索记忆"""
        pass
```

#### 数据库设计

**SQLite Schema**

```sql
-- 对话消息表
CREATE TABLE messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    role TEXT NOT NULL,  -- 'user' or 'assistant'
    content TEXT NOT NULL,
    emotion TEXT,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_session (session_id),
    INDEX idx_timestamp (timestamp)
);

-- 用户配置表
CREATE TABLE config (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- 长期记忆表（文本备份）
CREATE TABLE long_term_memories (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    content TEXT NOT NULL,
    source TEXT,  -- 来源：'conversation', 'user_told', 'inferred'
    importance INTEGER DEFAULT 5,  -- 重要性 1-10
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_importance (importance)
);
```

**向量数据库 Schema (LanceDB)**

```python
# 向量记忆表
memory_schema = {
    "id": "int",
    "content": "str",
    "embedding": "vector[768]",  # 根据 embedding 模型维度
    "metadata": "str",  # JSON 字符串
    "created_at": "str"
}
```

#### 实现要点

**1. 消息存储**
```python
async def save_message(
    self, 
    session_id: str, 
    role: str, 
    content: str,
    emotion: str = None
):
    cursor = self.db.cursor()
    cursor.execute("""
        INSERT INTO messages (session_id, role, content, emotion)
        VALUES (?, ?, ?, ?)
    """, (session_id, role, content, emotion))
    self.db.commit()
    
    # 判断是否需要创建长期记忆
    if self._is_important_message(content):
        await self.create_long_term_memory(content, {
            "session_id": session_id,
            "role": role,
            "source": "conversation"
        })
```

**2. 语义搜索**
```python
def search_memories(self, query: str, limit: int = 5) -> List[Dict]:
    # 生成查询向量
    query_embedding = self.embedding_model.encode(query)
    
    # 在向量数据库中搜索
    table = self.vector_db.open_table("memories")
    results = table.search(query_embedding).limit(limit).to_list()
    
    return [
        {
            "content": r["content"],
            "metadata": json.loads(r["metadata"]),
            "relevance": r["_distance"]
        }
        for r in results
    ]
```

### 2.6 VoiceEngine (语音引擎)

#### 职责
- 文字转语音（TTS）
- 语音转文字（STT）
- 生成口型同步数据

#### 核心接口

```python
class VoiceEngine:
    def __init__(self, tts_provider: str = "openai"):
        self.tts_provider = tts_provider
        self.stt_model = None  # 延迟加载
    
    async def text_to_speech(
        self, 
        text: str, 
        voice: str = "nova"
    ) -> Tuple[bytes, List[Phoneme]]:
        """
        文字转语音
        返回：(音频字节, 音素时间轴)
        """
        pass
    
    async def speech_to_text(self, audio_bytes: bytes) -> str:
        """语音转文字"""
        pass
    
    def _generate_phonemes(self, text: str, audio_duration: float) -> List[Phoneme]:
        """生成口型同步数据"""
        pass
```

#### 实现要点

**1. OpenAI TTS**
```python
async def text_to_speech(self, text: str, voice: str = "nova"):
    # 调用 OpenAI TTS API
    response = await self.openai_client.audio.speech.create(
        model="tts-1",
        voice=voice,
        input=text,
        response_format="mp3"
    )
    
    audio_bytes = response.content
    
    # 生成音素数据（简化版，实际需要音素识别）
    phonemes = self._generate_phonemes(text, audio_duration=len(audio_bytes) / 16000)
    
    return audio_bytes, phonemes
```

**2. 音素生成（简化版）**
```python
def _generate_phonemes(self, text: str, audio_duration: float) -> List[Phoneme]:
    # 简化实现：均匀分布音素
    # 实际应该使用音素识别工具（如 Montreal Forced Aligner）
    
    phonemes = []
    chars = list(text)
    time_per_char = audio_duration / len(chars)
    
    for i, char in enumerate(chars):
        phoneme = self._char_to_phoneme(char)
        phonemes.append({
            "phoneme": phoneme,
            "time": i * time_per_char
        })
    
    return phonemes

def _char_to_phoneme(self, char: str) -> str:
    # 简化的汉语拼音映射
    # 实际应该使用完整的汉语拼音转换
    pinyin_map = {
        "你": "ni", "好": "hao", "啊": "a",
        "我": "wo", "是": "shi", "的": "de",
        # ...
    }
    pinyin = pinyin_map.get(char, "a")
    return pinyin[-1]  # 返回韵母
```

## 3. 数据流设计

### 3.1 对话流程

```
用户输入消息
    │
    ▼
┌───────────────────────┐
│  Frontend: 发送消息   │
│  WebSocket.send()     │
└───────┬───────────────┘
        │
        ▼ WebSocket
┌───────────────────────┐
│  Backend: 接收消息    │
│  WebSocket Handler    │
└───────┬───────────────┘
        │
        ▼
┌───────────────────────┐
│  ConversationEngine   │
│  - 构建上下文          │
│  - 召回记忆            │
│  - 调用 LLM           │
└───────┬───────────────┘
        │
        ▼ (流式)
┌───────────────────────┐
│  CharacterEngine      │
│  - 识别情绪            │
│  - 选择动作/表情       │
└───────┬───────────────┘
        │
        ▼
┌───────────────────────┐
│  VoiceEngine          │
│  - 生成语音            │
│  - 生成音素            │
└───────┬───────────────┘
        │
        ▼ WebSocket
┌───────────────────────┐
│  Frontend: 接收回复   │
│  - 显示文字            │
│  - 播放动作/表情       │
│  - 播放语音            │
│  - 同步口型            │
└───────────────────────┘
```

### 3.2 WebSocket 消息类型

**客户端 -> 服务端**

| 消息类型 | 说明 | 格式 |
|---------|------|------|
| `user_message` | 用户发送消息 | `{type, content, session_id}` |
| `voice_input` | 语音输入 | `{type, audio_data, session_id}` |
| `action_trigger` | 触发动作 | `{type, action, params}` |

**服务端 -> 客户端**

| 消息类型 | 说明 | 格式 |
|---------|------|------|
| `assistant_message_chunk` | 流式回复片段 | `{type, content, session_id}` |
| `assistant_message_done` | 回复完成 | `{type, full_content, emotion, motion, session_id}` |
| `voice_ready` | 语音准备就绪 | `{type, audio_url, phonemes, session_id}` |
| `system_notification` | 系统通知 | `{type, message, level}` |

## 4. 性能优化

### 4.1 前端优化
- Live2D 模型使用 WebGL 渲染，减少 CPU 占用
- 使用 `requestAnimationFrame` 优化渲染循环
- 聊天历史虚拟滚动（只渲染可见消息）
- 图片懒加载
- 组件按需加载

### 4.2 后端优化
- 使用连接池管理数据库连接
- LLM 请求使用流式输出，减少首字延迟
- 长期记忆定期批量更新，避免频繁写入
- 向量搜索建立索引（HNSW）
- TTS 结果缓存（相同文本复用音频）

### 4.3 内存优化
- Live2D 模型资源复用
- 聊天历史限制加载数量
- 定期清理旧的音频文件
- 向量数据库使用磁盘映射，避免全部加载到内存

## 5. 安全性设计

### 5.1 数据安全
- 本地数据使用 SQLite 加密扩展（SQLCipher）
- API Key 存储在加密配置文件中
- 用户数据不上传云端（除非用户主动开启同步）

### 5.2 网络安全
- WebSocket 使用 WSS（TLS 加密）
- 后端 API 仅监听本地回环地址 `127.0.0.1`
- 添加请求频率限制，防止滥用

### 5.3 权限控制
- Tauri 使用白名单控制可调用的系统 API
- 文件访问限制在用户文档目录
- 截图功能需要用户确认

## 6. 扩展性设计

### 6.1 插件系统（未来）
- 支持自定义工具（MCP 协议）
- 支持自定义皮肤（Live2D 模型替换）
- 支持自定义人设（Prompt 模板）

### 6.2 多语言支持
- i18n 国际化
- 支持中文、英文、日文等
- Chebo 可以根据用户语言切换语气

### 6.3 多平台支持
- Windows / macOS / Linux
- 未来可扩展到移动端（使用 Tauri Mobile）

---

**文档版本**: 1.0  
**最后更新**: 2026-05-11
