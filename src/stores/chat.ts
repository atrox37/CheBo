import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { AgentStateType } from '@/services/tauriService'
import { estimateSpeechDurationMs } from '@/utils/speechTiming'
import { extractEmotionFromText, stripEmotionForDisplay } from '@/utils/emotionTag'
/** 单条消息的结构 */
export interface ChatMessage {
  id:        string
  role:      'user' | 'assistant'
  content:   string
  emotion?:  string
  motion?:   string
  timestamp: number
}

/** 处于生成中的 Agent 状态 */
const GENERATING_AGENT_STATES: AgentStateType[] = [
  'thinking', 'talking', 'executingTool', 'waitingConfirm',
]

export const useChatStore = defineStore('chat', () => {
  // ── 状态 ────────────────────────────────────────────────────────────────
  const messages       = ref<ChatMessage[]>([])
  /** Tauri IPC 始终可用，保留字段供 UI 兼容 */
  const isConnected = ref(true)
  const isTyping       = ref(false)
  const streamBuffer   = ref('')
  /** 流式原始缓冲（含 EMOTION 标签，仅用于解析） */
  let streamRawBuffer  = ''
  const currentEmotion = ref<string>('normal')
  /** P0: Agent 状态机当前状态 */
  const agentState     = ref<AgentStateType>('idle')
  /**
   * 桌宠「正在说话」展示态：流式结束后仍保持口型/光标，时长可接 TTS。
   * 助手模式用 instant，不延长。
   */
  const isSpeaking     = ref(false)
  const speechPresentation = ref<'instant' | 'pet'>('pet')
  let speakHoldTimer: ReturnType<typeof setTimeout> | null = null

  /** 桌宠气泡：L2/L3 工具确认弹窗打开时保持显示 */
  const toolConfirmOpen = ref(false)
  /** 桌宠气泡可见（由发言/确认/输入驱动，普通聊天结束后自动隐藏） */
  const petBubbleVisible = ref(false)
  const PET_BUBBLE_LINGER_MS = 3500
  let petBubbleDismissTimer: ReturnType<typeof setTimeout> | null = null

  const bubblePinned = computed(() =>
    toolConfirmOpen.value || agentState.value === 'waitingConfirm'
  )

  // ── 计算属性 ──────────────────────────────────────────────────────────
  const latestAssistantMessage = computed(() =>
    [...messages.value].reverse().find(m => m.role === 'assistant')
  )

  const isGenerating = computed(() =>
    isTyping.value || GENERATING_AGENT_STATES.includes(agentState.value)
  )

  /** 立绘/气泡：流式中或桌宠发言尾韵 */
  const isTalkingVisual = computed(() =>
    isTyping.value || isSpeaking.value || agentState.value === 'talking'
  )

  // ── 发言展示（桌宠） ───────────────────────────────────────────────────
  function setSpeechPresentation(mode: 'instant' | 'pet') {
    speechPresentation.value = mode
    if (mode === 'instant') {
      endSpeakHold()
      hidePetBubble()
    }
  }

  function endSpeakHold() {
    if (speakHoldTimer) {
      clearTimeout(speakHoldTimer)
      speakHoldTimer = null
    }
    isSpeaking.value = false
    schedulePetBubbleDismiss()
  }

  function clearPetBubbleDismissTimer() {
    if (petBubbleDismissTimer) {
      clearTimeout(petBubbleDismissTimer)
      petBubbleDismissTimer = null
    }
  }

  function showPetBubble() {
    if (speechPresentation.value !== 'pet') return
    petBubbleVisible.value = true
    clearPetBubbleDismissTimer()
  }

  function hidePetBubble() {
    clearPetBubbleDismissTimer()
    petBubbleVisible.value = false
  }

  /** 发言尾韵结束后，普通聊天再等几秒隐藏；确认类消息保持 */
  function schedulePetBubbleDismiss() {
    if (speechPresentation.value !== 'pet') return
    if (bubblePinned.value || isTyping.value) return
    clearPetBubbleDismissTimer()
    petBubbleDismissTimer = setTimeout(() => {
      if (!bubblePinned.value && !isTyping.value && !isSpeaking.value) {
        petBubbleVisible.value = false
      }
    }, PET_BUBBLE_LINGER_MS)
  }

  function setToolConfirmOpen(open: boolean) {
    toolConfirmOpen.value = open
    if (open) showPetBubble()
    else if (!isTyping.value && !isSpeaking.value) schedulePetBubbleDismiss()
  }

  /**
   * 延长说话动画；TTS 接入时传入实测时长（毫秒）即可覆盖估算值。
   */
  function beginSpeakHold(text: string, durationMs?: number) {
    if (speechPresentation.value !== 'pet') return
    if (speakHoldTimer) {
      clearTimeout(speakHoldTimer)
      speakHoldTimer = null
    }
    const content = text.trim()
    if (!content) return
    showPetBubble()
    isSpeaking.value = true
    const ms = durationMs ?? estimateSpeechDurationMs(content)
    speakHoldTimer = setTimeout(() => endSpeakHold(), ms)
  }

  // ── 方法 ──────────────────────────────────────────────────────────────
  function addMessage(msg: Omit<ChatMessage, 'id' | 'timestamp'>) {
    let content = msg.content
    let emotion = msg.emotion
    if (msg.role === 'assistant') {
      const parsed = extractEmotionFromText(content)
      content = parsed.clean
      if (!emotion && parsed.emotion) emotion = parsed.emotion
      if (emotion) currentEmotion.value = emotion
    }
    messages.value.push({
      ...msg,
      content,
      emotion,
      id:        crypto.randomUUID(),
      timestamp: Date.now(),
    })
    if (msg.role === 'assistant' && content.trim()) {
      beginSpeakHold(content)
    }
  }
  function startStream(_sessionId?: string): string {
    if (speakHoldTimer) {
      clearTimeout(speakHoldTimer)
      speakHoldTimer = null
    }
    isSpeaking.value = false
    showPetBubble()
    const id = crypto.randomUUID()
    messages.value.push({ id, role: 'assistant', content: '', timestamp: Date.now() })
    isTyping.value = true
    isSpeaking.value = speechPresentation.value === 'pet'
    streamRawBuffer = ''
    streamBuffer.value = ''
    return id
  }

  function appendStreamChunk(chunk: string) {
    streamRawBuffer += chunk
    const { display, emotion } = stripEmotionForDisplay(streamRawBuffer)
    streamBuffer.value = display
    if (emotion) currentEmotion.value = emotion
    const last = messages.value.at(-1)
    if (last && last.role === 'assistant') {
      last.content = display
    }
  }

  function finishStream(emotion?: string, motion?: string) {
    const last = messages.value.at(-1)
    if (last && last.role === 'assistant') {
      const parsed = extractEmotionFromText(streamRawBuffer || last.content)
      last.content = parsed.clean
      const finalEmotion = emotion || parsed.emotion || 'normal'
      last.emotion = finalEmotion
      currentEmotion.value = finalEmotion
      if (motion) last.motion = motion
    } else if (emotion) {
      currentEmotion.value = emotion
    }
    isTyping.value = false
    streamRawBuffer = ''
    streamBuffer.value = ''
    if (last?.role === 'assistant' && last.content.trim()) {
      beginSpeakHold(last.content)
    } else {
      endSpeakHold()
    }
  }
  /** 工具轮检测到 tool_call：丢弃当前未完成的流式气泡 */
  function abandonStream() {
    const last = messages.value.at(-1)
    if (last?.role === 'assistant') {
      messages.value.pop()
    }
    isTyping.value = false
    streamRawBuffer = ''
    streamBuffer.value = ''
    endSpeakHold()
  }

  /** 用户主动停止生成 */
  function handleGenerationCancelled(partial?: string) {
    const last = messages.value.at(-1)
    if (last?.role === 'assistant') {
      if (partial?.trim()) {
        const parsed = extractEmotionFromText(partial)
        last.content = parsed.clean
        if (parsed.emotion) currentEmotion.value = parsed.emotion
      }      if (!last.content.trim()) {
        messages.value.pop()
        endSpeakHold()
      } else {
        beginSpeakHold(last.content)
      }
    } else if (partial?.trim()) {
      addMessage({ role: 'assistant', content: partial })
    } else {
      endSpeakHold()
    }
    isTyping.value = false
    streamRawBuffer = ''
    streamBuffer.value = ''
  }
  function prepareAssistantReply() {
    if (isTyping.value) return
    startStream()
  }

  /** P0: 更新 Agent 状态（由 Rust agent_state 事件触发） */
  function setAgentState(state: AgentStateType) {
    agentState.value = state
    if (state === 'thinking' && !isTyping.value) {
      prepareAssistantReply()
    }
    if (state === 'idle') {
      const last = messages.value.at(-1)
      if (last?.role === 'assistant' && !last.content.trim() && isTyping.value) {
        messages.value.pop()
        isTyping.value = false
        streamRawBuffer = ''
        streamBuffer.value = ''
      }
    }
    if (state === 'waitingConfirm') showPetBubble()
    else if (!isTyping.value && !isSpeaking.value) schedulePetBubbleDismiss()
  }

  function setEmotion(emotion: string) {
    currentEmotion.value = emotion
  }

  function clearMessages() { messages.value = [] }

  return {
    messages, isConnected, isTyping, isGenerating, isSpeaking, isTalkingVisual,
    streamBuffer, currentEmotion, latestAssistantMessage,
    agentState, toolConfirmOpen, petBubbleVisible, bubblePinned,
    addMessage, startStream, appendStreamChunk, finishStream,
    abandonStream, handleGenerationCancelled, prepareAssistantReply,
    setSpeechPresentation, beginSpeakHold, endSpeakHold,
    showPetBubble, hidePetBubble, setToolConfirmOpen,
    clearMessages, setAgentState, setEmotion,
  }
})
