// ─── tauriService.ts ──────────────────────────────────────────────────────────
// 替代 websocket.ts：使用 Tauri invoke() 调用 Rust 命令
//                    使用 listen() 接收 Rust emit() 的事件
// ─────────────────────────────────────────────────────────────────────────────

import { invoke }                  from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useChatStore }            from '@/stores/chat'
import { usePetStore }             from '@/stores/pet'

import { getOrCreateSessionId } from '@/utils/storageKeys'

const SESSION_ID: string = getOrCreateSessionId()

export function getSessionId(): string { return SESSION_ID }

// ─── 事件监听器句柄（teardown 时清理） ───────────────────────────────────────
const _unlisteners: UnlistenFn[] = []

/** 注册所有 Rust → 前端事件监听，替代 WebSocket.onmessage */
export async function setupListeners(): Promise<void> {
  const chatStore = useChatStore()
  const petStore  = usePetStore()

  // LLM 流式 token
  _unlisteners.push(
    await listen<{ content: string }>('assistant_chunk', (e) => {
      if (!chatStore.isTyping) chatStore.startStream(SESSION_ID)
      chatStore.appendStreamChunk(e.payload.content)
    }),
  )

  // 工具轮检测到 tool_call：重置 UI 流
  _unlisteners.push(
    await listen('assistant_stream_reset', () => {
      chatStore.abandonStream()
    }),
  )

  // 用户停止生成
  _unlisteners.push(
    await listen<{ partial?: string }>('generation_cancelled', (e) => {
      chatStore.handleGenerationCancelled(e.payload.partial)
    }),
  )

  // LLM 流式完成
  _unlisteners.push(
    await listen<{ full_content: string; emotion: string }>('assistant_done', async (e) => {
      chatStore.finishStream(e.payload.emotion)
      const { speakAssistantReply } = await import('@/composables/useVoice')
      const last = chatStore.latestAssistantMessage
      if (last?.content.trim()) {
        speakAssistantReply(last.content).catch(() => {})
      }
    }),
  )

  // 宠物状态更新（tick / 投喂 / 任务完成等后台推送）
  _unlisteners.push(
    await listen<Record<string, unknown>>('status_update', (e) => {
      petStore.applyUpdate(e.payload)
    }),
  )

  // Chebo 主动发言（定时 / 阈值触发）
  _unlisteners.push(
    await listen<{ content: string; emotion: string }>('status_comment', (e) => {
      chatStore.addMessage({
        role:    'assistant',
        content: e.payload.content,
        emotion: e.payload.emotion ?? 'normal',
      })
      chatStore.setEmotion(e.payload.emotion ?? 'normal')
    }),
  )

  // P0: Agent 状态机变化
  _unlisteners.push(
    await listen<AgentStateType>('agent_state', (e) => {
      chatStore.setAgentState(e.payload)
    }),
  )

  // P1: 感知事件（活跃窗口 / 剪贴板）
  _unlisteners.push(
    await listen<PerceptionEvent>('perception_event', (e) => {
      _perceptionCb?.(e.payload)
    }),
  )

  // P1: 工具结果（send_message 自动触发文件读取时推送）
  _unlisteners.push(
    await listen<{ tool: string; success: boolean; content: string; trigger: string }>(
      'tool_result',
      (e) => {
        _toolResultCb?.(e.payload)
      },
    ),
  )

  // Tool Registry: Agent 工具调用需用户确认（L2/L3）
  _unlisteners.push(
    await listen<{ id: string; name: string; arguments: unknown; level: number }>(
      'tool_call_pending',
      (e) => {
        _agentToolPendingCb?.(e.payload)
      },
    ),
  )

  // Tool Registry: Agent 中间思考文字（工具循环中）
  _unlisteners.push(
    await listen<{ content: string; turn: number }>(
      'assistant_thinking',
      (_e) => {
        // 暂时静默，后续可接入 UI 展示思考过程
      },
    ),
  )

  // P2: 托盘重置信号
  _unlisteners.push(
    await listen('tray_reset', () => {
      _trayResetCb?.()
    }),
  )

  // 双模式：托盘「切换到助手模式」信号
  _unlisteners.push(
    await listen('open_assistant', () => {
      _openAssistantCb?.()
    }),
  )

  // 双模式：Rust 通知前端切回桌宠模式（关闭事件拦截后发出）
  _unlisteners.push(
    await listen('switch_to_pet', () => {
      _switchToPetCb?.()
    }),
  )

  // Task System: 任务完成时通知
  _unlisteners.push(
    await listen<{ task_id: string; title: string; result_summary: string | null }>(
      'task_completed',
      (e) => {
        chatStore.addMessage({
          role:    'assistant',
          content: `任务「${e.payload.title}」完成了！${e.payload.result_summary ?? ''}`,
          emotion: 'happy',
        })
      },
    ),
  )

  // 任务进度（Agent 长期任务）
  _unlisteners.push(
    await listen<{ task_id: string; remaining_secs: number; progress: number }>('task_progress', (e) => {
      _taskProgressCb?.(e.payload.progress, e.payload.remaining_secs)
    }),
  )

  // 后端报错（在聊天界面显示友好提示）
  _unlisteners.push(
    await listen<string>('backend_error', (e) => {
      console.error('[Tauri] 后端错误:', e.payload)
      chatStore.finishStream()  // 中断流

      // 将错误原因显示给用户
      const raw = e.payload ?? ''
      let hint = ''
      const lower = raw.toLowerCase()
      if (raw.includes('401') || lower.includes('unauthorized') || lower.includes('authentication') || raw.includes('API Key 无效')) {
        hint = '⚠️ API Key 无效或已过期，请到「助手模式 → 设置」重新填写正确的 API Key。'
      } else if (raw.includes('429') || lower.includes('rate limit') || raw.includes('过于频繁')) {
        hint = '⚠️ API 请求频率超限，请稍等几秒后再试。'
      } else if (raw.includes('503') || raw.includes('502') || lower.includes('too busy') || lower.includes('unavailable') || raw.includes('服务繁忙') || raw.includes('暂时不可用')) {
        hint = '⚠️ AI 服务当前繁忙（已自动重试），请稍后再试，或换一个时段 / 备用模型。'
      } else if (raw.includes('connect') || raw.includes('network') || raw.includes('timeout')) {
        hint = '⚠️ 网络连接失败，请检查网络或 Base URL 是否正确。'
      } else if (raw.includes('model') || raw.includes('404') || raw.includes('不存在')) {
        hint = '⚠️ 模型名称或接口地址不正确，请到设置页检查模型与 Base URL。'
      } else if (raw.startsWith('LLM 调用失败：')) {
        hint = `⚠️ ${raw.replace(/^LLM 调用失败：/, '')}`
      } else {
        hint = `⚠️ 响应失败：${raw.slice(0, 120)}`
      }

      chatStore.addMessage({ role: 'assistant', content: hint, emotion: 'sad' })
    }),
  )

}

export function teardownListeners(): void {
  _unlisteners.forEach(fn => fn())
  _unlisteners.length = 0
}

// ─── 聊天命令 ─────────────────────────────────────────────────────────────────

/**
 * 发送聊天消息。
 * 先在前端显示用户消息气泡，再 invoke Rust（后台流式 LLM 调用，通过事件推送结果）。
 */
export function sendMessage(
  content: string,
  images: string[] = [],
  deepThink = false,
  assistantMode = false,
): void {
  const chatStore = useChatStore()
  chatStore.addMessage({ role: 'user', content })
  chatStore.prepareAssistantReply()
  invoke('send_message', {
    content,
    sessionId: SESSION_ID,
    images,
    deepThink,
    assistantMode,
  }).catch((err) => {
    console.error('[Tauri] send_message 失败:', err)
  })
}

/** P1: 中断当前聊天生成 */
export function cancelChatGeneration(): void {
  invoke('cancel_chat_generation').catch((err) => {
    console.error('[Tauri] cancel_chat_generation 失败:', err)
  })
}

// ─── Provider 能力注册表 ──────────────────────────────────────────────────────

export interface ModelCapabilities {
  model_id:           string
  display_name:       string
  provider:           string
  supports_vision:    boolean
  supports_tools:     boolean
  context_window:     number
  cost_input_per_1k:  number
  cost_output_per_1k: number
  notes:              string
}

export async function getModelCapabilities(model: string): Promise<ModelCapabilities> {
  return await invoke<ModelCapabilities>('get_model_capabilities', { model })
}

export async function listKnownModels(): Promise<ModelCapabilities[]> {
  return await invoke<ModelCapabilities[]>('list_known_models')
}

// ─── App 配置（含视觉模型）────────────────────────────────────────────────────

export interface AppConfigDto {
  llm_provider:    string
  llm_base_url:    string
  llm_model:       string
  has_api_key:     boolean
  vision_model:    string
  vision_base_url: string
  has_vision_key:  boolean
}

export async function getAppConfig(): Promise<AppConfigDto> {
  return await invoke<AppConfigDto>('get_app_config')
}

export async function updateAppConfig(payload: {
  api_key?:       string
  base_url?:      string
  model?:         string
  llm_provider?:  string
  vision_api_key?:  string
  vision_base_url?: string
  vision_model?:    string
}): Promise<void> {
  await invoke('update_app_config', { payload })
}

// ─── 沙盒路径配置 ─────────────────────────────────────────────────────────────

export async function getSandboxPaths(): Promise<string[]> {
  return await invoke<string[]>('get_sandbox_paths')
}

export async function setSandboxPaths(paths: string[]): Promise<void> {
  await invoke('set_sandbox_paths', { paths })
}

// ─── 宠物操作命令 ─────────────────────────────────────────────────────────────

export interface FeedResult {
  ok:        boolean
  reason?:   string
  food_name?: string
  status?:   Record<string, unknown>
}

export interface PetActionResult {
  ok:        boolean
  reason?:   string
  action_id: string
  chat_hint: string
  emotion:   string
  status?:   Record<string, unknown>
}

export async function feed(foodId: string): Promise<FeedResult> {
  return await invoke<FeedResult>('feed', { foodId })
}

export async function petAction(actionId: string): Promise<PetActionResult> {
  return await invoke<PetActionResult>('pet_action', { actionId })
}

export interface BuyResult {
  ok:        boolean
  reason?:   string
  item_name?: string
}

export async function buyItem(itemId: string): Promise<BuyResult> {
  return await invoke<BuyResult>('buy_item', { itemId })
}

export interface TaskStartResult {
  ok:         boolean
  reason?:    string
  task_name?: string
  ends_at?:   string
}

export async function startTask(taskId: string, taskType: string): Promise<TaskStartResult> {
  return await invoke<TaskStartResult>('start_task', { taskId, taskType })
}

export async function cancelTask(): Promise<void> {
  await invoke('cancel_task')
}

export async function setKeepPerfect(enabled: boolean): Promise<void> {
  await invoke('set_keep_perfect', { enabled })
}

// ─── 数据查询命令 ─────────────────────────────────────────────────────────────

export async function getStatus() {
  return await invoke('get_status')
}

export async function getFoods() {
  return await invoke('get_foods')
}

export async function getTasks(taskType: string) {
  return await invoke('get_tasks', { taskType })
}

export async function getInventory() {
  return await invoke('get_inventory')
}

export async function getChatHistory() {
  return await invoke('get_chat_history')
}

// ─── 类型定义 ─────────────────────────────────────────────────────────────────

/** P0: Agent 状态枚举（与 Rust agent.rs 的 AgentState 对应，camelCase） */
export type AgentStateType =
  | 'idle' | 'thinking' | 'talking' | 'working' | 'sleeping'
  | 'observing' | 'waitingConfirm' | 'executingTool'
  | 'interrupted' | 'errorRecover'

/** P1: 感知事件 payload */
export interface PerceptionEvent {
  kind:      string           // 'window_switch' | 'clipboard'
  data:      string           // 事件内容
  category?: string           // 应用分类: coding/browsing/office/gaming/other
}

// ─── 数据查询扩展 ─────────────────────────────────────────────────────────────

/** P0: 获取当前 AgentState */
export async function getAgentState(): Promise<AgentStateType> {
  return await invoke<AgentStateType>('get_agent_state')
}

// ─── P1: 工具系统 ─────────────────────────────────────────────────────────────

export interface ToolResult {
  tool:    string
  success: boolean
  content: string
}

/** 执行工具（read_file / web_search / git_status / safe_shell / list_dir） */
export async function executeTool(tool: string, args: Record<string, string>): Promise<ToolResult> {
  return await invoke<ToolResult>('execute_tool', { tool, args })
}

// ─── P2: 托盘 ─────────────────────────────────────────────────────────────────

/** 切换主窗口显示/隐藏 */
export function toggleWindow(): void {
  invoke('toggle_window').catch(() => {})
}

// ─── 回调钩子（供面板组件注册，替代 wsService.onXxx） ─────────────────────────

let _taskProgressCb:     ((progress: number, secsLeft: number) => void) | null = null
let _levelUpCb:          ((level: number) => void) | null = null
let _perceptionCb:       ((event: PerceptionEvent) => void) | null = null
let _toolResultCb:       ((result: { tool: string; success: boolean; content: string; trigger: string }) => void) | null = null
let _trayResetCb:        (() => void) | null = null
let _openAssistantCb:    (() => void) | null = null
let _switchToPetCb:      (() => void) | null = null
let _agentToolPendingCb: ((call: { id: string; name: string; arguments: unknown; level: number }) => void) | null = null

export function onTaskProgress(fn: typeof _taskProgressCb):         void { _taskProgressCb = fn }
export function onLevelUp(fn: typeof _levelUpCb):                   void { _levelUpCb = fn }
export function onPerception(fn: typeof _perceptionCb):             void { _perceptionCb = fn }
export function onToolResult(fn: typeof _toolResultCb):             void { _toolResultCb = fn }
export function onTrayReset(fn: typeof _trayResetCb):               void { _trayResetCb = fn }
export function onOpenAssistant(fn: typeof _openAssistantCb):       void { _openAssistantCb = fn }
export function onSwitchToPet(fn: typeof _switchToPetCb):           void { _switchToPetCb = fn }
export function onAgentToolPending(fn: typeof _agentToolPendingCb): void { _agentToolPendingCb = fn }

/** 批准/拒绝 Agent 循环中的 L2/L3 工具调用 */
export async function approveAgentTool(id: string, approved: boolean): Promise<void> {
  await invoke('approve_agent_tool', { id, approved })
}

// ─── Voice: TTS / STT ─────────────────────────────────────────────────────────

export interface VoiceConfigDto {
  tts_enabled:     boolean
  stt_enabled:     boolean
  tts_voice:       string
  tts_model:       string
  tts_base_url:    string
  has_tts_api_key: boolean
}

export interface VoiceUpdatePayload {
  tts_enabled?:  boolean
  stt_enabled?:  boolean
  tts_voice?:    string
  tts_model?:    string
  tts_base_url?: string
  tts_api_key?:  string
}

export async function getVoiceConfig(): Promise<VoiceConfigDto> {
  return await invoke<VoiceConfigDto>('voice_get_config')
}

export async function updateVoiceConfig(payload: VoiceUpdatePayload): Promise<void> {
  await invoke('voice_update_config', { payload })
}

export async function synthesizeSpeech(text: string): Promise<string> {
  return await invoke<string>('voice_synthesize', { text })
}

export async function transcribeAudio(audioBase64: string, mimeType?: string): Promise<string> {
  return await invoke<string>('voice_transcribe', {
    audioBase64,
    mimeType: mimeType ?? null,
  })
}

export interface CheboProfileItem {
  key:        string
  value:      string
  category:   string
  confidence: number
  updated_at: string
}

export async function getCheboProfile(): Promise<CheboProfileItem[]> {
  return await invoke<CheboProfileItem[]>('get_chebo_profile')
}

export async function updateCheboProfileEntry(key: string, value: string): Promise<void> {
  await invoke('update_chebo_profile_entry', { key, value })
}

export async function deleteCheboProfileEntry(key: string): Promise<void> {
  await invoke('delete_chebo_profile_entry', { key })
}

// Tauri IPC 始终可用，无需重连逻辑
export const isConnected = true
