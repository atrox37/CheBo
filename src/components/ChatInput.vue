<script setup lang="ts">
/**
 * ChatInput — 聊天输入框
 *
 * 桌宠模式（默认）：单行输入，Enter 发送
 * 助手模式（inline=true）：
 *   - 多行 textarea，Ctrl+Enter 发送
 *   - 支持图片/文件：点击 📎 / 拖入区域 / Ctrl+V 粘贴
 *   - 附件预览行：图片缩略图 or 文件名 chip，可删除
 *   - 发送时把文本文件内容/图片 base64 拼入消息
 */
import { ref, computed, onMounted } from 'vue'
import { Paperclip, Send, X, FileText, Square, Mic, Brain } from 'lucide-vue-next'
import { useChatStore }  from '@/stores/chat'
import { useVoiceStore } from '@/utils/speechTiming'
import { useVoiceInput } from '@/composables/useVoice'
import * as tauriService from '@/services/tauriService'
import { STORAGE_KEYS } from '@/utils/storageKeys'

const props = defineProps<{ inline?: boolean }>()

const chatStore = useChatStore()
const voiceStore = useVoiceStore()
const { error: voiceError, startRecording, stopRecording } = useVoiceInput()
const inputText = ref('')
const deepThink = ref(localStorage.getItem(STORAGE_KEYS.deepThink) === '1')

// ─── 附件 ─────────────────────────────────────────────────────────────────────

interface Attachment {
  id:        string
  name:      string
  type:      'image' | 'text' | 'other'
  preview?:  string      // 图片：data URL，文本：截断内容
  dataUrl?:  string      // 图片完整 data URL（发送用）
  content?:  string      // 文本文件完整内容（发送用）
  size:      number      // 字节数
}

const attachments = ref<Attachment[]>([])
const isDragging  = ref(false)
const fileInputEl = ref<HTMLInputElement | null>(null)

function guessType(mime: string, name: string): Attachment['type'] {
  if (mime.startsWith('image/')) return 'image'
  if (
    mime.startsWith('text/') ||
    /\.(txt|md|py|js|ts|vue|json|yaml|yml|toml|rs|go|java|c|cpp|h|css|html|xml|sh|bat)$/i.test(name)
  ) return 'text'
  return 'other'
}

async function processFile(file: File) {
  // 限制：图片最大 5MB，文本文件最大 500KB
  const MAX_IMAGE = 5 * 1024 * 1024
  const MAX_TEXT  = 500 * 1024
  const type = guessType(file.type, file.name)

  if (type === 'image' && file.size > MAX_IMAGE) {
    alert(`图片太大（最大 5MB）：${file.name}`)
    return
  }
  if (type === 'text' && file.size > MAX_TEXT) {
    alert(`文件太大（最大 500KB）：${file.name}`)
    return
  }

  const id = crypto.randomUUID()

  if (type === 'image') {
    const dataUrl = await readAsDataURL(file)
    attachments.value.push({
      id, name: file.name, type,
      preview: dataUrl,
      dataUrl,
      size: file.size,
    })
  } else if (type === 'text') {
    const content = await readAsText(file)
    attachments.value.push({
      id, name: file.name, type,
      preview: content.slice(0, 80).replace(/\n/g, ' '),
      content,
      size: file.size,
    })
  } else {
    attachments.value.push({
      id, name: file.name, type,
      preview: `${(file.size / 1024).toFixed(1)} KB`,
      size: file.size,
    })
  }
}

function readAsDataURL(file: File): Promise<string> {
  return new Promise((res, rej) => {
    const r = new FileReader()
    r.onload  = () => res(r.result as string)
    r.onerror = rej
    r.readAsDataURL(file)
  })
}

function readAsText(file: File): Promise<string> {
  return new Promise((res, rej) => {
    const r = new FileReader()
    r.onload  = () => res(r.result as string)
    r.onerror = rej
    r.readAsText(file, 'utf-8')
  })
}

function removeAttachment(id: string) {
  attachments.value = attachments.value.filter(a => a.id !== id)
}

// ── 点击选择文件 ────────────────────────────────────────────────────────────
function openFilePicker() {
  fileInputEl.value?.click()
}
function onFileInputChange(e: Event) {
  const files = (e.target as HTMLInputElement).files
  if (!files) return
  for (const f of Array.from(files)) processFile(f)
  ;(e.target as HTMLInputElement).value = ''
}

// ── 拖入 ────────────────────────────────────────────────────────────────────
function onDragover(e: DragEvent) {
  e.preventDefault()
  isDragging.value = true
}
function onDragleave() {
  isDragging.value = false
}
function onDrop(e: DragEvent) {
  e.preventDefault()
  isDragging.value = false
  if (!e.dataTransfer?.files) return
  for (const f of Array.from(e.dataTransfer.files)) processFile(f)
}

// ── 粘贴 ────────────────────────────────────────────────────────────────────
function onPaste(e: ClipboardEvent) {
  if (!props.inline) return
  const items = e.clipboardData?.items
  if (!items) return
  for (const item of Array.from(items)) {
    if (item.kind === 'file') {
      const file = item.getAsFile()
      if (file) {
        e.preventDefault()   // 不插入 blob URL 到文本框
        processFile(file)
      }
    }
  }
}

// ─── 构建发送内容 ─────────────────────────────────────────────────────────────
// 图片附件单独提取为 data URL 数组，由后端 Vision Router 处理（不内嵌到文本）
// 文本附件内容内嵌进消息正文

function buildSendPayload(): { content: string; images: string[] } {
  const textParts: string[] = []
  const images: string[]    = []

  if (inputText.value.trim()) textParts.push(inputText.value.trim())

  for (const att of attachments.value) {
    if (att.type === 'image' && att.dataUrl) {
      images.push(att.dataUrl)  // 单独传，不内嵌进文本
    } else if (att.type === 'text' && att.content) {
      const snippet = att.content.length > 3000
        ? att.content.slice(0, 3000) + '\n…（内容过长，已截断）'
        : att.content
      textParts.push(`\n[文件附件: ${att.name}]\n\`\`\`\n${snippet}\n\`\`\``)
    } else if (att.type === 'image') {
      textParts.push(`\n[图片附件: ${att.name}，读取失败]`)
    } else {
      textParts.push(`\n[附件: ${att.name}，${(att.size/1024).toFixed(1)} KB]`)
    }
  }

  return {
    content: textParts.join('\n') || (images.length ? `请分析这${images.length}张图片` : ''),
    images,
  }
}

// ─── 发送 ─────────────────────────────────────────────────────────────────────

function send() {
  const { content, images } = buildSendPayload()
  if (!content && images.length === 0) return

  // 🆕 Ticket 11: 生成中 → 排队
  if (chatStore.isGenerating) {
    chatStore.addQueuedMessage(content || '(图片消息)')
    inputText.value = ''
    attachments.value = []
    return
  }

  localStorage.setItem(STORAGE_KEYS.deepThink, deepThink.value ? '1' : '0')
  tauriService.sendMessage(content, images, deepThink.value, !!props.inline)
  inputText.value = ''
  attachments.value = []
}

// 🆕 Ticket 11: 引导 — 取消当前生成 + 注入方向修正
async function steerMessage(text: string) {
  await tauriService.cancelChatGeneration()
  // 等待取消生效
  await new Promise(r => setTimeout(r, 200))
  tauriService.sendMessage(
    `【方向修正】${text}`,
    [], deepThink.value, !!props.inline
  )
}

function toggleDeepThink() {
  deepThink.value = !deepThink.value
  localStorage.setItem(STORAGE_KEYS.deepThink, deepThink.value ? '1' : '0')
}

function stopGeneration() {
  tauriService.cancelChatGeneration()
}

function onKey(e: KeyboardEvent) {
  if (props.inline) {
    if (e.key === 'Enter' && !e.shiftKey && !e.ctrlKey && !e.metaKey) {
      e.preventDefault()
      send()
    }
    // Ctrl+Enter / Cmd+Enter：换行（textarea 默认行为，不拦截）
  } else {
    if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); send() }
  }
}

const canSend = computed(() =>
  (inputText.value.trim() || attachments.value.length > 0) && !chatStore.isGenerating
)

onMounted(() => {
  voiceStore.loadConfig().catch(() => {})
})

const micActive = computed(() => voiceStore.isRecording)

async function onMicDown() {
  await startRecording()
}

async function onMicUp() {
  const text = await stopRecording()
  if (text.trim()) {
    inputText.value = inputText.value ? `${inputText.value} ${text.trim()}` : text.trim()
    if (!props.inline) send()
  }
}
</script>

<template>
  <div
    class="ci-wrap"
    :class="{ inline, dragging: isDragging }"
    @dragover="inline ? onDragover($event) : undefined"
    @dragleave="inline ? onDragleave() : undefined"
    @drop="inline ? onDrop($event) : undefined"
    @click.stop
    @mousedown.stop
  >

    <!-- 拖入提示遮罩 -->
    <div v-if="inline && isDragging" class="drag-overlay">
      <div class="drag-hint">
        <Paperclip :size="28" />
        <span>松开鼠标添加附件</span>
      </div>
    </div>

    <!-- 🆕 Ticket 11: 排队消息区 -->
    <div v-if="chatStore.queuedMessages.length" class="queued-area">
      <div v-for="msg in chatStore.queuedMessages" :key="msg.id" class="queued-msg">
        <span class="queued-text">{{ msg.text }}</span>
        <button class="steer-btn" @click="steerMessage(msg.text); chatStore.removeQueuedMessage(msg.id)">
          引导 →
        </button>
        <button class="queue-remove" @click="chatStore.removeQueuedMessage(msg.id)">✕</button>
      </div>
    </div>

    <!-- 附件预览行 -->
    <div v-if="inline && attachments.length" class="attach-list">
      <div
        v-for="att in attachments"
        :key="att.id"
        class="attach-chip"
        :class="att.type"
      >
        <!-- 图片预览 -->
        <img v-if="att.type === 'image'" :src="att.preview" class="chip-thumb" :alt="att.name" />
        <FileText v-else :size="14" class="chip-icon" />

        <span class="chip-name" :title="att.name">{{ att.name }}</span>
        <button class="chip-del" @click.stop="removeAttachment(att.id)" title="移除">
          <X :size="10" />
        </button>
      </div>
    </div>

    <!-- 主输入区：工作台 -->
    <div v-if="inline" class="ci-inline">
      <textarea
        v-model="inputText"
        class="ci-textarea-full"
        placeholder="说点什么…（Enter 发送 · Ctrl+Enter 换行 · 可拖入或粘贴文件）"
        rows="3"
        :disabled="chatStore.isGenerating"
        @keydown="onKey"
        @paste="onPaste"
      />

      <input
        ref="fileInputEl"
        type="file"
        multiple
        accept="image/*,text/*,.md,.json,.yaml,.yml,.toml,.rs,.py,.js,.ts,.vue,.go,.java,.c,.cpp,.h,.sh"
        style="display:none"
        @change="onFileInputChange"
      />

      <div class="ci-toolbar">
        <div class="ci-toolbar-left">
          <button
            class="tool-chip"
            title="添加图片或文件"
            @click.stop="openFilePicker"
          >
            <Paperclip :size="14" />
            <span>附件</span>
          </button>
          <button
            class="tool-chip"
            :class="{ on: deepThink }"
            title="深度思考"
            @click.stop="toggleDeepThink"
          >
            <Brain :size="14" />
            <span>深度思考</span>
          </button>
        </div>
        <div class="ci-toolbar-right">
          <button
            v-if="voiceStore.sttEnabled"
            class="mic-btn inline-mic"
            :class="{ active: micActive }"
            title="按住说话"
            @mousedown.stop.prevent="onMicDown"
            @mouseup.stop.prevent="onMicUp"
            @mouseleave.stop="micActive ? onMicUp() : undefined"
            @touchstart.stop.prevent="onMicDown"
            @touchend.stop.prevent="onMicUp"
          >
            <Mic :size="15" />
          </button>
          <button
            v-if="chatStore.isGenerating"
            class="ci-send ci-stop inline"
            title="停止生成"
            @click.stop="stopGeneration"
          >
            <Square :size="12" fill="currentColor" />
          </button>
          <button
            v-else
            class="ci-send inline"
            :disabled="!canSend"
            title="发送"
            @click.stop="send"
          >
            <Send :size="14" />
          </button>
        </div>
      </div>
    </div>

    <!-- 主输入区：桌宠 -->
    <div v-else class="ci-bar">
      <input
        v-model="inputText"
        type="text"
        class="ci-input"
        placeholder="说点什么…"
        maxlength="500"
        :disabled="chatStore.isGenerating"
        @keydown="onKey"
      />
      <button
        v-if="voiceStore.sttEnabled"
        class="mic-btn"
        :class="{ active: micActive }"
        title="按住说话"
        @mousedown.stop.prevent="onMicDown"
        @mouseup.stop.prevent="onMicUp"
        @mouseleave.stop="micActive ? onMicUp() : undefined"
      >
        <Mic :size="14" />
      </button>
      <button
        class="deep-btn pet"
        :class="{ on: deepThink }"
        title="深度思考"
        @click.stop="toggleDeepThink"
      >
        <Brain :size="13" />
      </button>
      <button
        v-if="chatStore.isGenerating"
        class="ci-send ci-stop"
        title="停止生成"
        @click.stop="stopGeneration"
      >
        <Square :size="12" fill="currentColor" />
      </button>
      <button
        v-else
        class="ci-send"
        :disabled="!canSend"
        @click.stop="send"
      >
        <Send :size="13" />
      </button>
    </div>

    <!-- 桌宠模式提示 -->
    <div v-if="!inline" class="send-hint">
      Enter 发送 · 双击角色收起
      <span v-if="voiceStore.sttEnabled"> · 按住麦克风说话</span>
      <span v-if="voiceError" class="voice-err">{{ voiceError }}</span>
    </div>

  </div>
</template>

<style scoped>
.ci-wrap {
  width: 100%;
  position: relative;
}

/* ── 拖入遮罩 ── */
.drag-overlay {
  position: absolute;
  inset: 0;
  background: rgba(124, 108, 216, 0.08);
  border: 2px dashed #7c6cd8;
  border-radius: 14px;
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 10;
  pointer-events: none;
}
.drag-hint {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  color: #7c6cd8;
  font-size: 13px;
  font-weight: 500;
}

/* ── 附件预览行 ── */
.attach-list {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  padding: 0 0 8px 2px;
}

.attach-chip {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  height: 28px;
  padding: 0 6px 0 4px;
  border-radius: 7px;
  border: 1px solid #e0dcf0;
  background: #f7f5ff;
  font-size: 11.5px;
  color: #5560a0;
  max-width: 200px;
}
.attach-chip.image { background: #fff5fb; border-color: #f0d0e8; color: #904070; }

.chip-thumb {
  width: 20px;
  height: 20px;
  object-fit: cover;
  border-radius: 4px;
  flex-shrink: 0;
}
.chip-icon {
  color: #7c6cd8;
  flex-shrink: 0;
}

.chip-name {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
}

.chip-del {
  width: 16px; height: 16px;
  border-radius: 50%; border: none;
  background: rgba(0,0,0,0.07);
  color: #888;
  display: flex; align-items: center; justify-content: center;
  cursor: pointer; flex-shrink: 0;
  transition: background .1s;
}
.chip-del:hover { background: rgba(200,0,0,0.12); color: #d04040; }

/* ── 主输入区 ── */
.ci-bar {
  display: flex;
  align-items: center;
  gap: 6px;
  background: rgba(255,255,255,0.92);
  border: 1px solid rgba(210,170,200,0.5);
  border-radius: 12px;
  padding: 6px 6px 6px 10px;
  box-shadow: 0 2px 10px rgba(180,100,150,0.1);
  transition: border-color .15s;
}
.ci-bar:focus-within { border-color: rgba(200,120,170,0.7); }

.ci-bar.multiline {
  align-items: flex-end;
  padding: 8px 8px 8px 10px;
  border-radius: 14px;
  background: #fff;
  border-color: #e0dcf0;
  box-shadow: none;
}
.ci-bar.multiline:focus-within { border-color: #b8a8e8; }

/* ── 工作台输入：文本全宽 + 底栏工具 ── */
.ci-inline {
  background: #fff;
  border: 1px solid #e0dcf0;
  border-radius: 14px;
  overflow: hidden;
  transition: border-color .15s;
}
.ci-inline:focus-within { border-color: #b8a8e8; }

.ci-textarea-full {
  display: block;
  width: 100%;
  box-sizing: border-box;
  border: none;
  outline: none;
  resize: vertical;
  min-height: 72px;
  max-height: 200px;
  padding: 12px 14px 8px;
  font-size: 13px;
  line-height: 1.6;
  color: #1a1a2e;
  font-family: inherit;
  background: transparent;
}
.ci-textarea-full::placeholder { color: #b0a0c0; font-size: 12.5px; }
.ci-textarea-full:disabled { opacity: 0.55; }

.ci-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 6px 10px 8px;
  border-top: 1px solid #f0ecf8;
}
.ci-toolbar-left,
.ci-toolbar-right {
  display: flex;
  align-items: center;
  gap: 6px;
}
.tool-chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  height: 28px;
  padding: 0 8px;
  border: none;
  border-radius: 8px;
  background: transparent;
  color: #8070a0;
  font-size: 11px;
  cursor: pointer;
  transition: background .12s, color .12s;
}
.tool-chip:hover { background: #f5f2ff; color: #6a5ab0; }
.tool-chip.on {
  background: #e8f0ff;
  color: #5070d0;
  box-shadow: inset 0 0 0 1px rgba(80, 112, 208, 0.2);
}
.mic-btn.inline-mic {
  width: 32px;
  height: 32px;
  border-radius: 10px;
}

.attach-btn {
  width: 28px; height: 28px;
  border-radius: 8px; border: none;
  background: transparent;
  color: #9080b8;
  display: flex; align-items: center; justify-content: center;
  cursor: pointer;
  flex-shrink: 0;
  align-self: flex-end;
  margin-bottom: 2px;
  transition: background .12s, color .12s;
}
.attach-btn:hover { background: #f0ecff; color: #7c6cd8; }

.mic-btn {
  width: 28px; height: 28px;
  border-radius: 50%; border: none;
  background: transparent;
  color: #9080b8;
  display: flex; align-items: center; justify-content: center;
  cursor: pointer; flex-shrink: 0;
  transition: background .12s, color .12s, transform .12s;
}
.mic-btn:hover { background: #f0ecff; color: #7c6cd8; }
.mic-btn.active {
  background: #e8729a; color: #fff;
  transform: scale(1.08);
  box-shadow: 0 2px 8px rgba(232, 114, 154, 0.35);
}

.deep-btn {
  width: 28px; height: 28px;
  border-radius: 8px; border: none;
  background: transparent;
  color: #9080b8;
  display: flex; align-items: center; justify-content: center;
  cursor: pointer; flex-shrink: 0;
  transition: background .12s, color .12s;
}
.deep-btn.on {
  background: #e8f0ff;
  color: #5070d0;
  box-shadow: inset 0 0 0 1px rgba(80, 112, 208, 0.25);
}
.deep-btn.pet {
  width: 24px; height: 24px;
  border-radius: 50%;
}
.deep-btn:hover { background: #f0ecff; color: #7c6cd8; }

.ci-input {
  flex: 1;
  border: none; outline: none;
  background: transparent;
  font-size: 13px; color: #3a1a2a;
  min-width: 0;
  font-family: inherit;
}
.ci-input::placeholder { color: rgba(160, 120, 150, 0.5); }
.ci-input:disabled     { opacity: 0.5; }

.ci-textarea {
  resize: none;
  line-height: 1.6;
  color: #1a1a2e;
  padding: 2px 0;
}
.ci-textarea::placeholder { color: #b0a0c0; font-size: 12.5px; }

.ci-send {
  width: 28px; height: 28px;
  border-radius: 50%; border: none;
  background: #e8729a; color: #fff;
  display: flex; align-items: center; justify-content: center;
  cursor: pointer; flex-shrink: 0;
  box-shadow: 0 2px 6px rgba(220,80,130,0.3);
  transition: filter .15s, transform .1s;
}
.ci-send.inline {
  background: #7c6cd8;
  box-shadow: 0 2px 8px rgba(124,108,216,0.3);
  align-self: flex-end;
  margin-bottom: 2px;
}
.ci-send:hover:not(:disabled)  { filter: brightness(1.1); }
.ci-send:active:not(:disabled) { transform: scale(0.92); }
.ci-send:disabled { opacity: .4; cursor: not-allowed; box-shadow: none; }

.ci-stop {
  background: #6b7280;
  box-shadow: 0 2px 6px rgba(80, 90, 110, 0.25);
}
.ci-stop.inline {
  background: #6b7280;
  box-shadow: 0 2px 8px rgba(80, 90, 110, 0.25);
}
.ci-stop:hover { filter: brightness(1.08); }

/* ── 桌宠模式提示 ── */
.send-hint {
  text-align: center;
  font-size: 9px;
  color: rgba(160, 110, 140, 0.4);
  margin-top: 3px;
}
.voice-err { display: block; color: #d05050; margin-top: 2px; }
</style>

/* ?? Ticket 11: �Ŷ���Ϣ�� */
.queued-area {
  padding: 4px 10px 0;
  display: flex; flex-direction: column; gap: 4px;
}
.queued-msg {
  display: flex; align-items: center; gap: 8px;
  background: rgba(255,255,255,0.06);
  border-radius: 8px; padding: 4px 8px;
  font-size: 13px;
}
.queued-text {
  flex: 1; color: rgba(255,255,255,0.6);
  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}
.steer-btn {
  padding: 2px 10px; border: 1px solid rgba(255,255,255,0.15);
  border-radius: 6px; background: transparent;
  color: #4a9eff; font-size: 12px; cursor: pointer;
  white-space: nowrap;
}
.steer-btn:hover { background: rgba(74,158,255,0.15); }
.queue-remove {
  padding: 2px 6px; border: none; background: transparent;
  color: rgba(255,255,255,0.3); cursor: pointer; font-size: 14px;
}
.queue-remove:hover { color: rgba(255,255,255,0.7); }
