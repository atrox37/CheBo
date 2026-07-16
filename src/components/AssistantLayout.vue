<script setup lang="ts">
/**
 * AssistantLayout — 助手模式（大窗口双栏布局）
 *
 * 左侧：导航 + Chebo 迷你状态
 * 右侧：聊天 / 记忆 / 设置
 *
 * 窗口尺寸: 1000×680，有原生边框，不置顶
 */
import { ref, computed, onMounted, onUnmounted, watch, nextTick } from 'vue'
import {
  MessageCircle, Brain, Settings,
  Minimize2,
} from 'lucide-vue-next'
import { invoke } from '@tauri-apps/api/core'
import { useChatStore }      from '../stores/chat'
import { useAppMode }        from '../composables/useAppMode'
import * as tauriService     from '../services/tauriService'
import type { ModelCapabilities } from '../services/tauriService'
import ChatInput             from './ChatInput.vue'
import SettingsPanel         from './panels/SettingsPanel.vue'
import ToolConfirmDialog     from './ToolConfirmDialog.vue'
import { CHEBO_NAME }        from '../config/chebo'
import { CRYSTAL_GIRL_BLINK, CRYSTAL_GIRL_NEUTRAL_IDLE } from '../config/crystalGirl'
import { renderMarkdown }    from '../utils/emotionTag'

// ─── Store 初始化（放在最前面，后面代码都可引用） ──────────────────────────────
const chatStore = useChatStore()
const { switchToPet, switching } = useAppMode()

// ─── 记忆浏览（内嵌在此文件中，避免新建文件） ────────────────────────────────

interface MemoryItem  { id: number; content: string; created_at: string }
interface ProfileItem { key: string; value: string; confidence: number; source: string; updated_at: string }

const memories     = ref<MemoryItem[]>([])
const profiles     = ref<ProfileItem[]>([])
const memLoading   = ref(false)
const memTab       = ref<'profile' | 'chebo' | 'ltm'>('profile')

interface CheboProfileItem {
  key: string; value: string; category: string
  confidence: number; updated_at: string
}
const cheboProfiles   = ref<CheboProfileItem[]>([])
const cheboEditingKey = ref<string | null>(null)
const cheboEditingVal = ref('')

const CHEBO_CAT_LABEL: Record<string, string> = {
  trait: '性格', relationship: '关系', experience: '经历', mood: '情绪',
}

// 编辑状态
const editingKey   = ref<string | null>(null)
const editingValue = ref('')

async function loadMemories() {
  memLoading.value = true
  try {
    const [ltm, prof, chebo] = await Promise.all([
      invoke<MemoryItem[]>('get_long_term_memories').catch(() => []),
      invoke<ProfileItem[]>('get_user_profile').catch(() => []),
      tauriService.getCheboProfile().catch(() => []),
    ])
    memories.value = ltm.slice(0, 30)
    profiles.value = prof
    cheboProfiles.value = chebo
  } finally {
    memLoading.value = false
  }
}

async function deleteProfile(key: string) {
  try {
    await invoke('delete_memory_entry', { key })
    profiles.value = profiles.value.filter(p => p.key !== key)
  } catch (err) { console.error(err) }
}

async function saveProfile(key: string) {
  try {
    await invoke('update_memory_entry', { key, value: editingValue.value })
    const item = profiles.value.find(p => p.key === key)
    if (item) { item.value = editingValue.value; item.confidence = 1.0; item.source = 'user' }
    editingKey.value = null
  } catch (err) { console.error(err) }
}

function startEdit(item: ProfileItem) {
  editingKey.value   = item.key
  editingValue.value = item.value
}

function startCheboEdit(item: CheboProfileItem) {
  cheboEditingKey.value = item.key
  cheboEditingVal.value = item.value
}

async function saveCheboProfile(key: string) {
  try {
    await tauriService.updateCheboProfileEntry(key, cheboEditingVal.value)
    const item = cheboProfiles.value.find(p => p.key === key)
    if (item) { item.value = cheboEditingVal.value; item.confidence = 1.0 }
    cheboEditingKey.value = null
  } catch (err) { console.error(err) }
}

async function deleteCheboProfile(key: string) {
  if (!confirm(`删除 Chebo 画像「${key}」？`)) return
  try {
    await tauriService.deleteCheboProfileEntry(key)
    cheboProfiles.value = cheboProfiles.value.filter(p => p.key !== key)
  } catch (err) { console.error(err) }
}

onMounted(loadMemories)
onMounted(loadLlmConfig)
onMounted(() => {
  chatStore.setSpeechPresentation('instant')
  chatStore.setEmotion('normal')
})

// ─── 侧栏立绘：待机 + 随机眨眼 ───────────────────────────────────────────────

const sidebarBlinking = ref(false)
let sidebarBlinkTimer: ReturnType<typeof setTimeout> | null = null

const sidebarSpriteSrc = computed(() => {
  if (sidebarBlinking.value && !chatStore.isTalkingVisual) {
    return CRYSTAL_GIRL_BLINK.idle
  }
  return CRYSTAL_GIRL_NEUTRAL_IDLE
})

function scheduleSidebarBlink() {
  if (sidebarBlinkTimer) clearTimeout(sidebarBlinkTimer)
  const delay = 2800 + Math.random() * 3200
  sidebarBlinkTimer = setTimeout(() => {
    if (!chatStore.isTalkingVisual) {
      sidebarBlinking.value = true
      setTimeout(() => {
        sidebarBlinking.value = false
        scheduleSidebarBlink()
      }, 120)
    } else {
      scheduleSidebarBlink()
    }
  }, delay)
}

onMounted(scheduleSidebarBlink)
onUnmounted(() => {
  if (sidebarBlinkTimer) clearTimeout(sidebarBlinkTimer)
})

// ─── LLM 配置表单 ─────────────────────────────────────────────────────────────

const llmFormKey    = ref('')
const llmFormUrl    = ref('')
const llmFormModel  = ref('deepseek-v4-flash')
const visionFormKey = ref('')
const visionFormUrl = ref('')
const visionFormModel = ref('')
const llmSaving     = ref(false)
const llmSaveMsg    = ref('')
const llmSaveOk     = ref(true)
const mainModelCaps = ref<ModelCapabilities | null>(null)

async function loadLlmConfig() {
  try {
    const cfg = await tauriService.getAppConfig()
    if (cfg.llm_model)       llmFormModel.value  = cfg.llm_model
    if (cfg.llm_base_url)    llmFormUrl.value    = cfg.llm_base_url
    if (cfg.vision_model)    visionFormModel.value = cfg.vision_model
    if (cfg.vision_base_url) visionFormUrl.value = cfg.vision_base_url
    // 不回显 key，只显示是否已配置
    llmFormKey.value    = cfg.has_api_key    ? '••••••••' : ''
    visionFormKey.value = cfg.has_vision_key ? '••••••••' : ''
    // 加载主模型能力
    mainModelCaps.value = await tauriService.getModelCapabilities(cfg.llm_model)
  } catch (err) { console.warn('加载 LLM 配置失败', err) }
}

// 当主模型变化时实时更新能力徽章
watch(llmFormModel, async (m) => {
  if (m) mainModelCaps.value = await tauriService.getModelCapabilities(m)
})

async function saveLlmConfig() {
  llmSaving.value = true
  llmSaveMsg.value = ''
  try {
    const payload: Record<string, string> = {
      model:        llmFormModel.value,
      base_url:     llmFormUrl.value,
      vision_model: visionFormModel.value,
    }
    // 只有用户改动了 key（不是占位符 ••）才提交
    if (llmFormKey.value && !llmFormKey.value.startsWith('•')) {
      payload.api_key = llmFormKey.value
    }
    if (visionFormKey.value && !visionFormKey.value.startsWith('•')) {
      payload.vision_api_key = visionFormKey.value
    }
    if (visionFormUrl.value) {
      payload.vision_base_url = visionFormUrl.value
    }
    await tauriService.updateAppConfig(payload)
    llmSaveOk.value  = true
    llmSaveMsg.value = '✓ 保存成功，立即生效'
    // 热更新能力徽章
    mainModelCaps.value = await tauriService.getModelCapabilities(llmFormModel.value)
  } catch (err) {
    llmSaveOk.value  = false
    llmSaveMsg.value = `保存失败：${err}`
  } finally {
    llmSaving.value = false
  }
}

// ─── 沙盒路径配置 ─────────────────────────────────────────────────────────────

const sandboxPaths    = ref<string[]>([])
const sandboxPathInput = ref('')
const sandboxSaving   = ref(false)
const sandboxSaveMsg  = ref('')

async function loadSandboxPaths() {
  try {
    sandboxPaths.value = await tauriService.getSandboxPaths()
  } catch (e) { console.warn('加载沙盒路径失败', e) }
}

function addSandboxPath() {
  const p = sandboxPathInput.value.trim()
  if (p && !sandboxPaths.value.includes(p)) {
    sandboxPaths.value.push(p)
    sandboxPathInput.value = ''
  }
}

function removeSandboxPath(i: number) {
  sandboxPaths.value.splice(i, 1)
}

async function saveSandboxPaths() {
  sandboxSaving.value = true
  sandboxSaveMsg.value = ''
  try {
    await tauriService.setSandboxPaths(sandboxPaths.value)
    sandboxSaveMsg.value = '✓ 路径已更新，立即生效'
  } catch (err) {
    sandboxSaveMsg.value = `保存失败：${err}`
  } finally {
    sandboxSaving.value = false
  }
}

onMounted(loadSandboxPaths)

// ─── 聊天相关：自动滚动 + 时间戳 + 日期过滤 ─────────────────────────────────

const historyEl = ref<HTMLElement | null>(null)

function scrollToBottom() {
  nextTick(() => {
    if (historyEl.value) historyEl.value.scrollTop = historyEl.value.scrollHeight
  })
}

// 消息更新或打字时自动滚底
watch(() => chatStore.messages.length, scrollToBottom)
watch(() => chatStore.isTyping, (v) => { if (v) scrollToBottom() })

// 带时间戳的消息列表（无过滤，直接显示当前会话所有消息）
type DisplayItem =
  | { type: 'ts';  key: string; text: string }
  | { type: 'msg'; key: string; msg: typeof chatStore.messages[0] }

function formatMsgTime(ts: number): string {
  const d   = new Date(ts)
  const now = new Date()
  const hm  = `${String(d.getHours()).padStart(2,'0')}:${String(d.getMinutes()).padStart(2,'0')}`
  const sameDay = (a: Date, b: Date) =>
    a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate()
  const yesterday = new Date(now.getTime() - 86400000)
  if (sameDay(d, now))       return `今天 ${hm}`
  if (sameDay(d, yesterday)) return `昨天 ${hm}`
  return `${d.getMonth()+1}月${d.getDate()}日 ${hm}`
}

const messagesWithTs = computed((): DisplayItem[] => {
  const items: DisplayItem[] = []
  let lastTs = 0
  for (const msg of chatStore.messages) {
    if (msg.timestamp - lastTs > 5 * 60 * 1000 || lastTs === 0) {
      items.push({ type: 'ts', key: `ts-${msg.id}`, text: formatMsgTime(msg.timestamp) })
    }
    items.push({ type: 'msg', key: msg.id, msg })
    lastTs = msg.timestamp
  }
  return items
})

// ─── 历史记录弹窗（读 SQLite 全量，与 SettingsPanel 保持一致） ──────────────

const showHistoryModal = ref(false)
const historyModalLoading = ref(false)

interface HistMsg  { id: string; role: string; content: string; emotion?: string; created_at: string }
interface DayGroup { date: string; label: string; messages: HistMsg[] }
const historyGroups = ref<DayGroup[]>([])

function histDateLabel(created_at: string): string {
  const d = new Date(created_at)
  const now = new Date()
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate())
  const yest  = new Date(today); yest.setDate(today.getDate() - 1)
  const tgt   = new Date(d.getFullYear(), d.getMonth(), d.getDate())
  if (tgt.getTime() === today.getTime()) return '今天'
  if (tgt.getTime() === yest.getTime())  return '昨天'
  return `${d.getMonth()+1}月${d.getDate()}日`
}

async function openHistoryModal() {
  showHistoryModal.value = true
  if (historyGroups.value.length > 0) return
  historyModalLoading.value = true
  try {
    const msgs = await tauriService.getChatHistory() as HistMsg[]
    const map = new Map<string, HistMsg[]>()
    for (const m of msgs) {
      const day = m.created_at.slice(0, 10)
      if (!map.has(day)) map.set(day, [])
      map.get(day)!.push(m)
    }
    historyGroups.value = [...map.entries()]
      .sort((a, b) => b[0].localeCompare(a[0]))
      .map(([date, messages]) => ({
        date,
        label: histDateLabel(messages[0].created_at),
        messages,
      }))
  } catch {
    historyGroups.value = []
  } finally {
    historyModalLoading.value = false
  }
}

function closeHistoryModal() {
  showHistoryModal.value = false
  historyGroups.value    = []
  historyModalDate.value = ''
}

// 弹窗内日期筛选
const historyModalDate = ref('')

const filteredHistoryGroups = computed(() => {
  if (!historyModalDate.value) return historyGroups.value
  return historyGroups.value.filter(g => g.date === historyModalDate.value)
})

// ─── 类型 ─────────────────────────────────────────────────────────────────────

type PageId = 'chat' | 'memory' | 'settings'

interface NavItem {
  id:    PageId
  label: string
  icon:  typeof MessageCircle
  color: string
}

// ─── 状态 ─────────────────────────────────────────────────────────────────────

const activePage = ref<PageId>('chat')

const navItems: NavItem[] = [
  { id: 'chat',      label: '聊天',     icon: MessageCircle, color: '#7c6cd8' },
  { id: 'memory',    label: '记忆浏览', icon: Brain,         color: '#d08040' },
  { id: 'settings',  label: '设置',     icon: Settings,      color: '#8090a8' },
]
</script>

<template>
  <div class="assistant-root">

    <!-- ── 左侧边栏 ─────────────────────────────────────────────────────────── -->
    <aside class="sidebar">

      <!-- 标题 -->
      <div class="sidebar-header">
        <div class="pet-name">{{ CHEBO_NAME }}</div>
        <div class="pet-level">AI 桌面伙伴</div>
      </div>

      <!-- 导航菜单 -->
      <nav class="nav-list">
        <button
          v-for="item in navItems"
          :key="item.id"
          class="nav-item"
          :class="{ active: activePage === item.id }"
          :style="{ '--ac': item.color }"
          @click="activePage = item.id"
        >
          <component
            :is="item.icon"
            :size="16"
            :color="activePage === item.id ? item.color : '#8090a8'"
          />
          <span>{{ item.label }}</span>
        </button>
      </nav>

      <!-- 立绘区（normal 待机 + 随机眨眼） -->
      <div class="sidebar-figure">
        <img
          :src="sidebarSpriteSrc"
          class="sidebar-figure-img"
          alt="Chebo"
          draggable="false"
        />
      </div>

      <!-- 底部：返回桌宠按钮 -->
      <div class="sidebar-footer">
        <button
          class="back-btn"
          :disabled="switching"
          @click="switchToPet"
        >
          <Minimize2 :size="14" />
          <span>{{ switching ? '切换中…' : '返回桌宠' }}</span>
        </button>
      </div>

    </aside>

    <!-- ── 主内容区 ────────────────────────────────────────────────────────── -->
    <main class="content-area">

      <!-- 顶部面包屑 -->
      <div class="content-header">
        <div class="page-title">
          <component
            :is="navItems.find(n => n.id === activePage)?.icon"
            :size="18"
            :color="navItems.find(n => n.id === activePage)?.color"
          />
          <span>{{ navItems.find(n => n.id === activePage)?.label }}</span>
        </div>
      </div>

      <!-- 内容主区 -->
      <div class="content-body">

        <!-- 聊天页 -->
        <div v-if="activePage === 'chat'" class="chat-page">

          <!-- 工具栏：仅历史记录入口 -->
          <div class="chat-toolbar">
            <button class="toolbar-history-btn" @click="openHistoryModal">
              历史记录
            </button>
          </div>

          <!-- 消息流 -->
          <div class="chat-history" ref="historyEl">
            <template v-for="item in messagesWithTs" :key="item.key">

              <!-- 时间戳分隔 -->
              <div v-if="item.type === 'ts'" class="ts-divider">
                <span class="ts-text">{{ item.text }}</span>
              </div>

              <!-- 消息气泡 -->
              <div
                v-else-if="item.type === 'msg'"
                class="msg-row"
                :class="item.msg.role"
              >
                <div
                  class="msg-bubble"
                  :class="{
                    typing: item.msg.role === 'assistant'
                      && chatStore.isTyping
                      && !item.msg.content,
                  }"
                >
                  <div
                    v-if="item.msg.content"
                    class="msg-content chat-md"
                    v-html="renderMarkdown(item.msg.content)"
                  />
                  <template
                    v-else-if="item.msg.role === 'assistant' && chatStore.isTyping"
                  >
                    <span class="dot" /><span class="dot" /><span class="dot" />
                  </template>
                </div>
              </div>
            </template>

            <!-- 空状态 -->
            <div v-if="chatStore.messages.length === 0 && !chatStore.isTyping" class="empty-hint">
              和 Chebo 说点什么吧~
            </div>
          </div>

          <div class="chat-input-wrap">
            <ChatInput :inline="true" />
          </div>
        </div>

        <!-- 记忆浏览页 -->
        <div v-else-if="activePage === 'memory'" class="memory-page">
          <div v-if="memLoading" class="mem-empty">加载中…</div>
          <template v-else>

            <!-- 子标签切换 -->
            <div class="mem-tabs">
              <button
                class="mem-tab"
                :class="{ active: memTab === 'profile' }"
                @click="memTab = 'profile'"
              >用户画像（{{ profiles.length }}）</button>
              <button
                class="mem-tab"
                :class="{ active: memTab === 'chebo' }"
                @click="memTab = 'chebo'"
              >Chebo 画像（{{ cheboProfiles.length }}）</button>
              <button
                class="mem-tab"
                :class="{ active: memTab === 'ltm' }"
                @click="memTab = 'ltm'"
              >长期记忆（{{ memories.length }}）</button>
              <button class="mem-refresh" @click="loadMemories" title="刷新">↻</button>
            </div>

            <!-- 用户画像 -->
            <div v-if="memTab === 'profile'">
              <div v-if="!profiles.length" class="mem-empty">暂无画像数据，多聊聊吧！</div>
              <div v-for="p in profiles" :key="p.key" class="profile-card">
                <div class="profile-key">{{ p.key }}</div>

                <!-- 编辑状态 -->
                <div v-if="editingKey === p.key" class="profile-edit-row">
                  <input
                    v-model="editingValue"
                    class="profile-edit-input"
                    @keydown.enter="saveProfile(p.key)"
                    @keydown.esc="editingKey = null"
                    autofocus
                  />
                  <button class="mem-btn save"  @click="saveProfile(p.key)">保存</button>
                  <button class="mem-btn ghost" @click="editingKey = null">取消</button>
                </div>

                <!-- 展示状态 -->
                <div v-else class="profile-value-row">
                  <span class="profile-value">{{ p.value }}</span>
                  <div class="profile-meta">
                    <span
                      class="confidence-badge"
                      :style="{ background: p.confidence >= 0.8 ? '#e6f5ef' : p.confidence >= 0.5 ? '#fff8e8' : '#ffeaea',
                                color:      p.confidence >= 0.8 ? '#2b9a6a' : p.confidence >= 0.5 ? '#c08000' : '#d04040' }"
                    >{{ Math.round(p.confidence * 100) }}%</span>
                    <span class="source-badge">{{ p.source }}</span>
                  </div>
                  <div class="profile-actions">
                    <button class="mem-btn ghost" @click="startEdit(p)" title="纠正">✏</button>
                    <button class="mem-btn danger" @click="deleteProfile(p.key)" title="删除">✕</button>
                  </div>
                </div>
              </div>
            </div>

            <!-- Chebo 画像 -->
            <div v-else-if="memTab === 'chebo'">
              <p class="chebo-profile-hint">Chebo 对自己的认知；对话与人格记忆会注入系统提示，你也可以在这里了解和纠正。</p>
              <div v-if="!cheboProfiles.length" class="mem-empty">暂无 Chebo 画像</div>
              <div v-for="p in cheboProfiles" :key="p.key" class="profile-card chebo-card">
                <div class="profile-key">
                  {{ p.key }}
                  <span class="cat-badge">{{ CHEBO_CAT_LABEL[p.category] ?? p.category }}</span>
                </div>
                <div v-if="cheboEditingKey === p.key" class="profile-edit-row">
                  <input
                    v-model="cheboEditingVal"
                    class="profile-edit-input"
                    @keydown.enter="saveCheboProfile(p.key)"
                    @keydown.esc="cheboEditingKey = null"
                  />
                  <button class="mem-btn save" @click="saveCheboProfile(p.key)">保存</button>
                  <button class="mem-btn ghost" @click="cheboEditingKey = null">取消</button>
                </div>
                <div v-else class="profile-value-row">
                  <span class="profile-value">{{ p.value }}</span>
                  <div class="profile-meta">
                    <span class="confidence-badge">{{ Math.round(p.confidence * 100) }}%</span>
                  </div>
                  <div class="profile-actions">
                    <button class="mem-btn ghost" @click="startCheboEdit(p)" title="纠正">✏</button>
                    <button class="mem-btn danger" @click="deleteCheboProfile(p.key)" title="删除">✕</button>
                  </div>
                </div>
              </div>
            </div>

            <!-- 长期记忆 -->
            <div v-else-if="memTab === 'ltm'">
              <div v-if="!memories.length" class="mem-empty">暂无长期记忆，多聊聊吧！</div>
              <div v-for="m in memories" :key="m.id" class="mem-card">
                <div class="mem-content">{{ m.content }}</div>
                <div class="mem-time">{{ m.created_at }}</div>
              </div>
            </div>
          </template>
        </div>

        <!-- 设置页 -->
        <div v-else-if="activePage === 'settings'" class="full-panel settings-page">

          <!-- ── LLM 模型配置 ─────────────────────────────────────── -->
          <div class="llm-config-section">
            <div class="lcs-title">
              <span>🤖 AI 模型配置</span>
            </div>

            <!-- 主模型 -->
            <div class="lcs-block">
              <div class="lcs-label">主模型</div>
              <div class="lcs-row">
                <select v-model="llmFormModel" class="lcs-select">
                  <optgroup label="DeepSeek">
                    <option value="deepseek-v4-flash">DeepSeek V4 Flash（推荐）</option>
                    <option value="deepseek-v4-pro">DeepSeek V4 Pro</option>
                    <option value="deepseek-reasoner">DeepSeek Reasoner（思维链）</option>
                  </optgroup>
                  <optgroup label="OpenAI">
                    <option value="gpt-4o">GPT-4o（支持视觉）</option>
                    <option value="gpt-4o-mini">GPT-4o Mini（视觉·便宜）</option>
                    <option value="gpt-4.1">GPT-4.1（超长上下文）</option>
                  </optgroup>
                  <optgroup label="Anthropic">
                    <option value="claude-sonnet-4-5">Claude Sonnet 4.5（视觉）</option>
                    <option value="claude-haiku-3-5">Claude Haiku 3.5（视觉·快）</option>
                  </optgroup>
                  <optgroup label="OpenRouter（一个 Key 访问所有）">
                    <option value="openai/gpt-4o">GPT-4o via OpenRouter</option>
                    <option value="anthropic/claude-sonnet-4-5">Claude Sonnet via OpenRouter</option>
                    <option value="meta-llama/llama-4-scout">Llama 4 Scout（免费）</option>
                    <option value="deepseek/deepseek-chat-v3-0324">DeepSeek V3 via OpenRouter</option>
                  </optgroup>
                  <optgroup label="本地 Ollama">
                    <option value="llama3">Llama 3（本地）</option>
                    <option value="llava">LLaVA（本地·视觉）</option>
                  </optgroup>
                </select>
                <!-- 能力标签 -->
                <div class="cap-badges">
                  <span v-if="mainModelCaps?.supports_vision" class="cap-badge vision">👁 视觉</span>
                  <span v-else class="cap-badge no-vision">仅文字</span>
                  <span v-if="mainModelCaps?.supports_tools" class="cap-badge tools">🔧 工具</span>
                  <span class="cap-badge ctx">{{ mainModelCaps ? (mainModelCaps.context_window / 1000).toFixed(0) + 'K ctx' : '' }}</span>
                </div>
              </div>
              <div class="lcs-row">
                <input v-model="llmFormKey" type="password" class="lcs-input" placeholder="API Key" autocomplete="off" />
              </div>
              <div class="lcs-row">
                <input v-model="llmFormUrl" type="text" class="lcs-input" placeholder="Base URL（如 https://api.deepseek.com/v1）" />
              </div>
            </div>

            <!-- 视觉回退模型 -->
            <div class="lcs-block">
              <div class="lcs-label">
                视觉回退模型
                <span class="lcs-sublabel">主模型不支持视觉时，先用此模型「看图」再回答</span>
              </div>
              <div class="lcs-row">
                <select v-model="visionFormModel" class="lcs-select">
                  <option value="">不配置（图片仅附文字说明）</option>
                  <optgroup label="OpenAI（推荐）">
                    <option value="gpt-4o">GPT-4o</option>
                    <option value="gpt-4o-mini">GPT-4o Mini（更便宜）</option>
                  </optgroup>
                  <optgroup label="Anthropic">
                    <option value="claude-sonnet-4-5">Claude Sonnet 4.5</option>
                    <option value="claude-haiku-3-5">Claude Haiku 3.5（最便宜）</option>
                  </optgroup>
                  <optgroup label="Google">
                    <option value="gemini-2.5-flash">Gemini 2.5 Flash（超便宜）</option>
                  </optgroup>
                  <optgroup label="OpenRouter（一个 Key 全覆盖）">
                    <option value="openai/gpt-4o">GPT-4o via OpenRouter</option>
                    <option value="meta-llama/llama-4-scout">Llama 4 Scout（免费）</option>
                  </optgroup>
                  <optgroup label="本地 Ollama">
                    <option value="llava">LLaVA（本地）</option>
                  </optgroup>
                </select>
              </div>
              <template v-if="visionFormModel">
                <div class="lcs-row">
                  <input v-model="visionFormKey" type="password" class="lcs-input" placeholder="视觉模型 API Key（可与主模型不同）" autocomplete="off" />
                </div>
                <div class="lcs-row">
                  <input v-model="visionFormUrl" type="text" class="lcs-input" placeholder="视觉模型 Base URL（默认 OpenAI）" />
                </div>
              </template>
            </div>

            <!-- 保存按钮 -->
            <div class="lcs-actions">
              <div v-if="llmSaveMsg" class="lcs-msg" :class="llmSaveOk ? 'ok' : 'err'">{{ llmSaveMsg }}</div>
              <button class="lcs-save-btn" :disabled="llmSaving" @click="saveLlmConfig">
                {{ llmSaving ? '保存中…' : '保存配置' }}
              </button>
            </div>
          </div>

          <!-- ── 沙盒路径配置 ─────────────────────────────────────── -->
          <div class="llm-config-section">
            <div class="lcs-title">🔒 Chebo 可访问的文件路径</div>
            <p class="lcs-sublabel" style="padding: 0 16px 8px; color: var(--text-muted, #888); font-size: 12px;">
              只有列表内的目录，Chebo 才能读取文件。修改后立即生效，无需重启。
            </p>

            <!-- 路径列表 -->
            <div style="padding: 0 16px 8px;">
              <div v-for="(path, idx) in sandboxPaths" :key="idx" class="sandbox-path-row">
                <span class="sandbox-path-text">{{ path }}</span>
                <button class="sandbox-path-del" @click="removeSandboxPath(idx)">✕</button>
              </div>
              <div v-if="sandboxPaths.length === 0" style="color: #888; font-size: 12px;">（无限制，允许访问所有路径）</div>
            </div>

            <!-- 新增输入 -->
            <div class="lcs-row" style="padding: 0 16px 12px; gap: 6px;">
              <input
                v-model="sandboxPathInput"
                class="lcs-input"
                style="flex: 1;"
                placeholder="输入绝对路径，如 C:\Users\me\Projects"
                @keydown.enter="addSandboxPath"
              />
              <button class="lcs-save-btn" style="padding: 6px 12px;" @click="addSandboxPath">+ 添加</button>
            </div>

            <div class="lcs-actions">
              <div v-if="sandboxSaveMsg" class="lcs-msg ok">{{ sandboxSaveMsg }}</div>
              <button class="lcs-save-btn" :disabled="sandboxSaving" @click="saveSandboxPaths">
                {{ sandboxSaving ? '保存中…' : '保存路径' }}
              </button>
            </div>
          </div>

          <!-- ── 其他设置 ──────────────────────────────────────────── -->
          <SettingsPanel />
        </div>

      </div>
    </main>

    <!-- 工具确认弹窗（助手模式也需要） -->
    <ToolConfirmDialog />

    <!-- 全量历史记录弹窗 -->
    <transition name="modal-fade">
      <div v-if="showHistoryModal" class="hist-overlay" @click.self="closeHistoryModal">
        <div class="hist-modal">
          <div class="hist-header">
            <span class="hist-title">历史记录</span>
            <button class="hist-close" @click="closeHistoryModal">×</button>
          </div>

          <!-- 弹窗内日期筛选条 -->
          <div class="hist-filter-bar">
            <input
              v-model="historyModalDate"
              type="date"
              class="hist-date-input"
              title="按日期筛选"
            />
            <button
              v-if="historyModalDate"
              class="hist-date-clear"
              @click="historyModalDate = ''"
            >× 清除</button>
            <span v-if="historyModalDate" class="hist-date-hint">
              {{ filteredHistoryGroups.length ? filteredHistoryGroups[0].label + '，共 ' + filteredHistoryGroups.reduce((s,g)=>s+g.messages.length,0) + ' 条' : '该日期无记录' }}
            </span>
          </div>

          <div class="hist-body">
            <div v-if="historyModalLoading" class="hist-loading">加载中…</div>
            <div v-else-if="!historyGroups.length" class="hist-empty">暂无历史记录</div>
            <template v-else>
              <div v-for="group in filteredHistoryGroups" :key="group.date" class="hist-day-group">
                <div class="hist-day-label">{{ group.label }}</div>
                <div
                  v-for="msg in group.messages"
                  :key="msg.id"
                  class="hist-msg"
                  :class="msg.role"
                >
                  <div class="hist-role">{{ msg.role === 'user' ? '你' : 'Chebo' }}</div>
                  <div class="hist-content">{{ msg.content }}</div>
                  <div class="hist-time">{{ msg.created_at.slice(11, 16) }}</div>
                </div>
              </div>
            </template>
          </div>
        </div>
      </div>
    </transition>
  </div>
</template>

<style scoped>
/* ── 根容器：填满整个窗口 ── */
.assistant-root {
  display: flex;
  width: 100vw;
  height: 100vh;
  background: #f5f5f8;
  overflow: hidden;
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
  font-size: 13px;
  color: #1a1a2e;
}

/* ── 左侧边栏 ── */
.sidebar {
  width: 200px;
  flex-shrink: 0;
  background: #ffffff;
  border-right: 1px solid #e8e8ee;
  display: flex;
  flex-direction: column;
  padding: 0;
}

.sidebar-header {
  padding: 16px 16px 10px;
  border-bottom: 1px solid #f0f0f5;
}

.sidebar-figure {
  flex: 1;
  min-height: 0;
  display: flex;
  align-items: flex-end;
  justify-content: center;
  padding: 8px 4px 12px;
  overflow: hidden;
}

.sidebar-figure-img {
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
  object-position: bottom center;
  filter: drop-shadow(0 6px 16px rgba(140, 120, 180, 0.22));
  pointer-events: none;
  user-select: none;
}

.pet-name  { font-size: 15px; font-weight: 700; color: #2d1a2a; }
.pet-level { font-size: 11px; color: #a080a0; margin-top: 2px; }

/* 迷你状态条 */
.mini-stats {
  padding: 10px 16px 8px;
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.mini-stat {
  display: flex;
  align-items: center;
  gap: 6px;
}

.mini-stat-label {
  font-size: 10px;
  color: #a0a0b8;
  width: 24px;
  flex-shrink: 0;
}

.mini-stat-bar {
  flex: 1;
  height: 4px;
  background: #f0f0f5;
  border-radius: 2px;
  overflow: hidden;
}

.mini-stat-fill {
  height: 100%;
  border-radius: 2px;
  transition: width .3s;
}

.mini-stat-val {
  font-size: 9px;
  color: #b0b0c8;
  width: 22px;
  text-align: right;
  flex-shrink: 0;
}

.sidebar-divider {
  height: 1px;
  background: #f0f0f5;
  margin: 4px 0;
}

/* 导航 */
.nav-list {
  flex: 1;
  padding: 8px 10px;
  display: flex;
  flex-direction: column;
  gap: 2px;
  overflow-y: auto;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 9px 12px;
  border-radius: 10px;
  border: none;
  background: transparent;
  color: #606070;
  font-size: 13px;
  cursor: pointer;
  transition: background .15s, color .15s;
  position: relative;
  text-align: left;
  width: 100%;
}

.nav-item:hover {
  background: #f5f5fa;
  color: #333;
}

.nav-item.active {
  background: color-mix(in srgb, var(--ac) 12%, transparent);
  color: var(--ac);
  font-weight: 600;
}

.nav-badge {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: #2b9a6a;
  margin-left: auto;
  animation: pulse 2s infinite;
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50%       { opacity: .4; }
}

/* 底部返回按钮 */
.sidebar-footer {
  padding: 12px 10px;
  border-top: 1px solid #f0f0f5;
}

.back-btn {
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 8px 0;
  border-radius: 8px;
  border: 1.5px dashed #c0c8d8;
  background: #f8f8fc;
  color: #8090a8;
  font-size: 12px;
  cursor: pointer;
  transition: background .15s, color .15s, border-color .15s;
}

.back-btn:hover:not(:disabled) {
  background: #e8eef8;
  color: #4060a0;
  border-color: #8090a8;
}

.back-btn:disabled {
  opacity: .6;
  cursor: not-allowed;
}

/* ── 主内容区 ── */
.content-area {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.content-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 20px 12px;
  background: #ffffff;
  border-bottom: 1px solid #e8e8ee;
  flex-shrink: 0;
}

.page-title {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 15px;
  font-weight: 600;
  color: #1a1a2e;
}

.header-right { display: flex; align-items: center; gap: 10px; }

.coins-badge {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 3px 10px;
  background: #fff8ec;
  border: 1px solid #f0d890;
  border-radius: 20px;
  font-size: 11px;
  font-weight: 600;
  color: #b07810;
}

.content-body {
  flex: 1;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

/* ── 聊天页 ── */
.chat-page {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

/* ── 工具栏（仅历史记录入口）── */
.chat-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 16px 0;
  flex-shrink: 0;
}

.toolbar-history-btn {
  height: 28px;
  padding: 0 12px;
  border-radius: 6px;
  border: 1px solid #d0c8f0;
  background: #f5f0ff;
  color: #7c6cd8;
  font-size: 11px;
  font-weight: 500;
  cursor: pointer;
  white-space: nowrap;
  flex-shrink: 0;
  margin-left: auto;
  transition: background .1s;
}
.toolbar-history-btn:hover { background: #ece4ff; }

/* ── 历史记录弹窗 ── */
.hist-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0,0,0,0.35);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 999;
}

.hist-modal {
  width: 620px;
  max-width: 90vw;
  max-height: 80vh;
  background: #fff;
  border-radius: 16px;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.hist-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px 20px 12px;
  border-bottom: 1px solid #eee;
  flex-shrink: 0;
}

.hist-title {
  font-size: 15px;
  font-weight: 600;
  color: #1a1a2e;
}

.hist-close {
  width: 28px; height: 28px;
  border-radius: 50%; border: none;
  background: #f0f0f8; color: #888;
  font-size: 18px; line-height: 1;
  cursor: pointer; display: flex;
  align-items: center; justify-content: center;
}
.hist-close:hover { background: #e0e0f0; color: #444; }

.hist-filter-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 20px 8px;
  border-bottom: 1px solid #f0f0f8;
  flex-shrink: 0;
}

.hist-date-input {
  height: 28px;
  padding: 0 10px;
  border: 1px solid #e0e0f0;
  border-radius: 7px;
  font-size: 12px;
  color: #555;
  background: #fafaff;
  outline: none;
  cursor: pointer;
}
.hist-date-input:focus { border-color: #7c6cd8; }

.hist-date-clear {
  height: 26px;
  padding: 0 9px;
  border-radius: 6px;
  border: 1px solid #ddd;
  background: #fff;
  color: #888;
  font-size: 11px;
  cursor: pointer;
}
.hist-date-clear:hover { background: #f0ecff; color: #7c6cd8; }

.hist-date-hint {
  font-size: 11px;
  color: #aaa;
}

.hist-body {
  flex: 1;
  overflow-y: auto;
  padding: 12px 20px 20px;
  scrollbar-width: thin;
  scrollbar-color: rgba(124,108,216,0.2) transparent;
}
.hist-body::-webkit-scrollbar { width: 4px; }
.hist-body::-webkit-scrollbar-thumb { background: rgba(124,108,216,0.25); border-radius: 4px; }

.hist-loading, .hist-empty {
  text-align: center;
  color: #aaa;
  padding: 30px;
  font-size: 13px;
}

.hist-day-group {
  margin-bottom: 20px;
}

.hist-day-label {
  font-size: 11px;
  font-weight: 700;
  color: #7c6cd8;
  text-transform: uppercase;
  letter-spacing: .5px;
  margin-bottom: 8px;
  padding-left: 2px;
}

.hist-msg {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  padding: 6px 0;
  border-bottom: 1px solid #f5f5fa;
}
.hist-msg:last-child { border-bottom: none; }

.hist-role {
  font-size: 10.5px;
  font-weight: 600;
  color: #8090a8;
  width: 36px;
  flex-shrink: 0;
  padding-top: 1px;
}
.hist-msg.user .hist-role    { color: #2b9a6a; }
.hist-msg.assistant .hist-role { color: #7c6cd8; }

.hist-content {
  flex: 1;
  font-size: 12.5px;
  color: #333;
  line-height: 1.55;
  word-break: break-word;
}

.hist-time {
  font-size: 10px;
  color: #c0c0d0;
  flex-shrink: 0;
  padding-top: 2px;
  font-variant-numeric: tabular-nums;
}

.modal-fade-enter-active, .modal-fade-leave-active { transition: opacity .2s; }
.modal-fade-enter-from,  .modal-fade-leave-to      { opacity: 0; }

/* ── 时间戳分隔 ── */
.ts-divider {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 4px 0;
  flex-shrink: 0;
}
.ts-text {
  font-size: 10.5px;
  color: #b0b0c0;
  background: #f5f5fa;
  padding: 2px 10px;
  border-radius: 8px;
  font-variant-numeric: tabular-nums;
  user-select: none;
}

/* ── 空结果提示 ── */
.empty-hint {
  text-align: center;
  color: #bbb;
  font-size: 12px;
  padding: 20px;
}

.chat-history {
  flex: 1;
  overflow-y: auto;
  padding: 10px 20px 16px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  /* T3: 主题滚动条 */
  scrollbar-width: thin;
  scrollbar-color: rgba(124,108,216,0.25) transparent;
}
.chat-history::-webkit-scrollbar { width: 4px; }
.chat-history::-webkit-scrollbar-thumb { background: rgba(124,108,216,0.3); border-radius: 4px; }
.chat-history::-webkit-scrollbar-thumb:hover { background: rgba(124,108,216,0.5); }

.msg-row {
  display: flex;
  align-items: flex-end;
  gap: 8px;
}

.msg-row.user {
  flex-direction: row-reverse;
}

.msg-avatar {
  width: 30px;
  height: 30px;
  border-radius: 50%;
  background: #f0eeff;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 14px;
  flex-shrink: 0;
}

.user-ava {
  background: #e8f5ef;
  font-size: 10px;
  font-weight: 700;
  color: #2b9a6a;
}

.msg-bubble {
  max-width: 68%;
  padding: 9px 13px;
  border-radius: 14px;
  font-size: 13px;
  line-height: 1.55;
  word-break: break-word;
}

.chat-md :deep(p) { margin: 0 0 0.45em; }
.chat-md :deep(p:last-child) { margin-bottom: 0; }
.chat-md :deep(ul), .chat-md :deep(ol) { margin: 0.2em 0 0.45em; padding-left: 1.2em; }
.chat-md :deep(code) {
  font-family: ui-monospace, monospace;
  font-size: 0.9em;
  padding: 0.1em 0.35em;
  border-radius: 4px;
  background: rgba(0, 0, 0, 0.06);
}
.chat-md :deep(pre) {
  margin: 0.35em 0;
  padding: 0.55em 0.7em;
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.06);
  overflow-x: auto;
}
.chat-md :deep(pre code) { padding: 0; background: none; }
.chat-md :deep(blockquote) {
  margin: 0.35em 0;
  padding-left: 0.7em;
  border-left: 3px solid rgba(120, 100, 200, 0.35);
}
.chat-md :deep(a) { color: #6b5ce7; text-decoration: underline; }

.msg-row.assistant .msg-bubble {
  background: #ffffff;
  border: 1px solid #eee;
  color: #222;
  border-bottom-left-radius: 4px;
  max-width: min(85%, 720px);
}

.msg-row.user .msg-bubble {
  background: #7c6cd8;
  color: #fff;
  border-bottom-right-radius: 4px;
  max-width: min(85%, 720px);
}

.msg-bubble.typing {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 10px 14px;
}

.dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: #bbb;
  animation: blink 1.4s infinite both;
}
.dot:nth-child(2) { animation-delay: .2s; }
.dot:nth-child(3) { animation-delay: .4s; }

@keyframes blink {
  0%, 80%, 100% { opacity: .2; transform: scale(.8); }
  40%           { opacity: 1;  transform: scale(1); }
}

.chat-input-wrap {
  padding: 12px 16px;
  background: #fff;
  border-top: 1px solid #eee;
  flex-shrink: 0;
}

/* ── 全屏面板（任务/设置） ── */
.full-panel {
  flex: 1;
  overflow: hidden;
  background: #fff;
  padding: 0;
}

.settings-page {
  overflow-y: auto;
}

/* ── LLM 配置面板 ── */
.llm-config-section {
  margin: 20px 20px 0;
  background: #fafafe;
  border: 1px solid #e8e0f8;
  border-radius: 14px;
  overflow: hidden;
}

.lcs-title {
  background: linear-gradient(135deg, #7c6cd8 0%, #b06cb4 100%);
  color: #fff;
  padding: 12px 18px;
  font-size: 13.5px;
  font-weight: 600;
  letter-spacing: 0.3px;
}

.lcs-block {
  padding: 14px 18px;
  border-bottom: 1px solid #f0eafc;
}
.lcs-block:last-of-type { border-bottom: none; }

.lcs-label {
  font-size: 12.5px;
  font-weight: 600;
  color: #5545a0;
  margin-bottom: 8px;
  display: flex;
  align-items: center;
  gap: 8px;
}
.lcs-sublabel {
  font-weight: 400;
  color: #aaa;
  font-size: 11px;
}

.lcs-row {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 7px;
}
.lcs-row:last-child { margin-bottom: 0; }

.lcs-select, .lcs-input {
  flex: 1;
  height: 32px;
  padding: 0 10px;
  border: 1px solid #ddd8f0;
  border-radius: 8px;
  font-size: 12.5px;
  color: #333;
  background: #fff;
  outline: none;
  transition: border-color .15s;
}
.lcs-select:focus, .lcs-input:focus { border-color: #7c6cd8; }

.cap-badges {
  display: flex;
  gap: 4px;
  flex-shrink: 0;
  flex-wrap: wrap;
}
.cap-badge {
  padding: 2px 7px;
  border-radius: 10px;
  font-size: 10.5px;
  font-weight: 500;
  white-space: nowrap;
}
.cap-badge.vision    { background: #e8f5e9; color: #2e7d32; }
.cap-badge.no-vision { background: #fff3e0; color: #e65100; }
.cap-badge.tools     { background: #e3f2fd; color: #1565c0; }
.cap-badge.ctx       { background: #f3e5f5; color: #6a1b9a; }

.lcs-actions {
  padding: 12px 18px 16px;
  display: flex;
  align-items: center;
  gap: 12px;
  justify-content: flex-end;
}

.lcs-msg {
  font-size: 12px;
  flex: 1;
}
.lcs-msg.ok  { color: #2e7d32; }
.lcs-msg.err { color: #c62828; }

.lcs-save-btn {
  height: 32px;
  padding: 0 20px;
  border-radius: 8px;
  border: none;
  background: #7c6cd8;
  color: #fff;
  font-size: 13px;
  cursor: pointer;
  transition: filter .15s;
}
.lcs-save-btn:hover:not(:disabled) { filter: brightness(1.08); }
.lcs-save-btn:disabled { opacity: .5; cursor: not-allowed; }

/* ── 沙盒路径 ── */
.sandbox-path-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 5px 8px;
  margin-bottom: 4px;
  background: rgba(0,0,0,.04);
  border-radius: 6px;
  font-size: 12px;
}
.sandbox-path-text { flex: 1; word-break: break-all; color: #444; }
.sandbox-path-del {
  flex-shrink: 0;
  background: none;
  border: none;
  color: #e55;
  cursor: pointer;
  font-size: 13px;
  padding: 0 4px;
}
.sandbox-path-del:hover { color: #c00; }

/* ── 记忆页 ── */
.memory-page {
  flex: 1;
  overflow-y: auto;
  padding: 16px 20px;
  background: #fff;
}

.mem-tabs {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 12px;
  border-bottom: 1px solid #eee;
  padding-bottom: 8px;
}

.mem-tab {
  padding: 4px 12px;
  border-radius: 6px;
  border: 1px solid transparent;
  font-size: 12px;
  font-weight: 500;
  color: #888;
  background: transparent;
  cursor: pointer;
  transition: all .15s;
}
.mem-tab.active {
  background: #fff3e0;
  border-color: #d08040;
  color: #d08040;
}
.mem-tab:hover:not(.active) { background: #f5f5f5; }

.mem-refresh {
  margin-left: auto;
  background: none;
  border: none;
  font-size: 15px;
  color: #aaa;
  cursor: pointer;
  padding: 2px 6px;
}
.mem-refresh:hover { color: #555; }

.profile-card {
  background: #fff;
  padding: 8px 12px;
  border-radius: 8px;
  border: 1px solid #ede8e0;
  background: #fdf8f2;
  margin-bottom: 6px;
}

.profile-key {
  font-size: 10px;
  font-weight: 700;
  color: #d08040;
  text-transform: uppercase;
  letter-spacing: .4px;
  margin-bottom: 4px;
  display: flex;
  align-items: center;
  gap: 6px;
}
.chebo-profile-hint {
  font-size: 11px;
  color: #887898;
  line-height: 1.5;
  margin: 0 0 10px;
  padding: 0 2px;
}
.cat-badge {
  font-size: 9px;
  font-weight: 600;
  text-transform: none;
  letter-spacing: 0;
  color: #7c6cd8;
  background: #f0ecff;
  border-radius: 4px;
  padding: 1px 5px;
}
.chebo-card .profile-key { color: #7c6cd8; }

.profile-value-row {
  display: flex;
  align-items: flex-start;
  gap: 8px;
}

.profile-value {
  font-size: 12px;
  color: #333;
  flex: 1;
  line-height: 1.4;
}

.profile-meta {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 2px;
  flex-shrink: 0;
}

.confidence-badge {
  font-size: 9.5px;
  font-weight: 600;
  padding: 1px 5px;
  border-radius: 4px;
}

.source-badge {
  font-size: 9px;
  color: #bbb;
}

.profile-actions {
  display: flex;
  gap: 3px;
  flex-shrink: 0;
}

.mem-btn {
  font-size: 11px;
  padding: 2px 7px;
  border-radius: 4px;
  border: 1px solid #ddd;
  cursor: pointer;
  background: #f5f5f5;
  color: #555;
  transition: background .1s;
}
.mem-btn:hover { background: #e8e8e8; }
.mem-btn.save   { background: #e6f5ef; border-color: #b8e0cc; color: #2b9a6a; }
.mem-btn.save:hover { background: #d0eddf; }
.mem-btn.danger { background: #fff0f0; border-color: #f5c0c0; color: #d04040; }
.mem-btn.danger:hover { background: #ffe0e0; }

.profile-edit-row {
  display: flex;
  align-items: center;
  gap: 6px;
}

.profile-edit-input {
  flex: 1;
  padding: 4px 8px;
  border-radius: 5px;
  border: 1px solid #d08040;
  font-size: 12px;
  color: #333;
  background: #fff;
  outline: none;
}
</style>

<!-- 记忆视图（非 scoped，全局生效） -->
<style>
.memory-view { display: flex; flex-direction: column; gap: 8px; }
.mem-loading  { text-align: center; color: #aaa; padding: 20px; }
.mem-empty    { text-align: center; color: #aaa; padding: 20px; }
.mem-section-title {
  font-size: 11px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: .5px;
  color: #d08040;
  margin-bottom: 4px;
}
.mem-card {
  padding: 10px 12px;
  border-radius: 8px;
  border: 1px solid #ede8e0;
  background: #fdf8f2;
}
.mem-content {
  font-size: 12px;
  color: #333;
  line-height: 1.5;
}
.mem-time {
  font-size: 10px;
  color: #aaa;
  margin-top: 4px;
}
</style>

<!-- T3: 全局主题滚动条（覆盖 AgentTaskPanel / 设置页等子组件中的滚动区域） -->
<style>
/* 助手模式内所有滚动区域统一样式 */
.assistant-root *::-webkit-scrollbar {
  width: 4px;
  height: 4px;
}
.assistant-root *::-webkit-scrollbar-track {
  background: transparent;
}
.assistant-root *::-webkit-scrollbar-thumb {
  background: rgba(124, 108, 216, 0.22);
  border-radius: 4px;
}
.assistant-root *::-webkit-scrollbar-thumb:hover {
  background: rgba(124, 108, 216, 0.45);
}
/* Firefox */
.assistant-root * {
  scrollbar-width: thin;
  scrollbar-color: rgba(124, 108, 216, 0.22) transparent;
}
</style>
