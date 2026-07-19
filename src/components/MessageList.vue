<script setup lang="ts">
import { ref, watch, nextTick, computed } from 'vue'
import { useChatStore } from '@/stores/chat'
import { renderMarkdown } from '@/utils/emotionTag'
import { invoke } from '@tauri-apps/api/core'

const chatStore = useChatStore()
const listRef = ref<HTMLElement | null>(null)

/** 格式化时间戳为 HH:mm */
function formatTime(ts: number): string {
  return new Date(ts).toLocaleTimeString('zh-CN', {
    hour: '2-digit',
    minute: '2-digit',
  })
}

/** 消息总数 */
const messageCount = computed(() => chatStore.messages.length)

/** 有新消息时自动滚到底部 */
watch(messageCount, async () => {
  await nextTick()
  listRef.value?.scrollTo({ top: listRef.value.scrollHeight, behavior: 'smooth' })
})

// 🆕 Ticket 10: 内嵌确认按钮处理
async function handleInlineConfirm(token: string, approved: boolean) {
  try {
    await invoke('confirm_tool_call', { token, confirm: approved })
    chatStore.resolveToolConfirm(token, approved)
  } catch (e) {
    console.error('confirm_tool_call failed:', e)
  }
}
</script>

<template>
  <div class="message-list" ref="listRef">
    <!-- 空状态提示 -->
    <div v-if="chatStore.messages.length === 0" class="empty-hint">
      <span>和 Chebo 说点什么吧～</span>
    </div>

    <!-- 消息列表 -->
    <div
      v-for="msg in chatStore.messages"
      :key="msg.id"
      class="msg-row"
      :class="msg.role === 'user' ? 'msg-user' : msg.role === 'confirm' ? 'msg-confirm' : 'msg-assistant'"
    >
      <!-- 🆕 Ticket 10: 内嵌工具确认气泡 -->
      <div v-if="msg.role === 'confirm' && !msg.confirmResolved" class="confirm-inline">
        <div class="confirm-header">
          <span class="confirm-badge" :class="'level-' + (msg.confirmLevel ?? 2)">
            L{{ msg.confirmLevel }}
          </span>
          <span>{{ msg.confirmTool }}</span>
        </div>
        <pre class="confirm-args">{{ msg.confirmArgs?.slice(0, 200) }}</pre>
        <div class="confirm-actions">
          <button class="btn-confirm-yes" @click="handleInlineConfirm(msg.confirmToken!, true)">确认执行</button>
          <button class="btn-confirm-no" @click="handleInlineConfirm(msg.confirmToken!, false)">拒绝</button>
        </div>
      </div>
      <div v-else-if="msg.role === 'confirm' && msg.confirmResolved" class="confirm-resolved">
        {{ msg.content }}
      </div>

      <div v-else class="msg-body">
        <div
          class="msg-bubble"
          :class="{
            typing: msg.role === 'assistant' && chatStore.isTyping && !msg.content,
          }"
        >
          <div
            v-if="msg.content"
            class="chat-md"
            v-html="renderMarkdown(msg.content)"
          />
          <template v-else-if="msg.role === 'assistant' && chatStore.isTyping">
            <span class="dot" /><span class="dot" /><span class="dot" />
          </template>
        </div>
        <div class="msg-time">{{ formatTime(msg.timestamp) }}</div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.message-list {
  width: 100%;
  flex: 1;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 8px 10px 4px;
  scrollbar-width: thin;
  scrollbar-color: rgba(255,255,255,0.2) transparent;
}

.message-list::-webkit-scrollbar {
  width: 4px;
}
.message-list::-webkit-scrollbar-thumb {
  background: rgba(255,255,255,0.2);
  border-radius: 2px;
}

.empty-hint {
  text-align: center;
  color: rgba(255,255,255,0.4);
  font-size: 13px;
  padding: 20px 0;
}

/* 每行消息 */
.msg-row {
  display: flex;
  gap: 8px;
  align-items: flex-end;
}

.msg-user {
  flex-direction: row-reverse;
}

/* 🆕 Ticket 10: 内嵌工具确认气泡 */
.msg-confirm {
  justify-content: center;
}
.confirm-inline {
  background: rgba(255,255,255,0.08);
  border: 1px solid rgba(255,255,255,0.15);
  border-radius: 12px;
  padding: 12px 16px;
  max-width: 80%;
}
.confirm-header {
  display: flex; align-items: center; gap: 8px;
  font-weight: 600; margin-bottom: 8px;
}
.confirm-badge {
  padding: 2px 8px; border-radius: 4px; font-size: 12px; font-weight: 700;
}
.level-2 { background: #e6a81733; color: #e6a817; }
.level-3 { background: #e6505033; color: #e65050; }
.confirm-args {
  font-size: 12px; color: rgba(255,255,255,0.5);
  white-space: pre-wrap; margin-bottom: 12px;
  max-height: 100px; overflow: hidden;
}
.confirm-actions { display: flex; gap: 8px; justify-content: flex-end; }
.btn-confirm-yes, .btn-confirm-no {
  padding: 6px 16px; border-radius: 8px; border: none; cursor: pointer;
  font-size: 13px; font-weight: 500;
}
.btn-confirm-yes { background: #4a9eff; color: white; }
.btn-confirm-yes:hover { background: #3a8eef; }
.btn-confirm-no { background: rgba(255,255,255,0.1); color: rgba(255,255,255,0.7); }
.btn-confirm-no:hover { background: rgba(255,255,255,0.15); }
.confirm-resolved {
  font-size: 13px; color: rgba(255,255,255,0.5); text-align: center;
  padding: 4px 0;
}

/* 头像 */
.msg-avatar {
  width: 28px;
  height: 28px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
  font-weight: 600;
  flex-shrink: 0;
}

.msg-assistant .msg-avatar {
  background: linear-gradient(135deg, #aa3bff, #7b2fff);
  color: #fff;
}

.msg-user .msg-avatar {
  background: rgba(255,255,255,0.2);
  color: rgba(255,255,255,0.9);
}

/* 气泡 + 时间 */
.msg-body {
  display: flex;
  flex-direction: column;
  gap: 3px;
  max-width: 78%;
}

.msg-user .msg-body {
  align-items: flex-end;
}

.msg-bubble {
  padding: 6px 10px;
  border-radius: 12px;
  font-size: 12px;
  line-height: 1.55;
  word-break: break-word;
}

.chat-md :deep(p) { margin: 0 0 0.4em; }
.chat-md :deep(p:last-child) { margin-bottom: 0; }
.chat-md :deep(code) {
  font-family: ui-monospace, monospace;
  font-size: 0.9em;
  padding: 0.1em 0.3em;
  border-radius: 4px;
  background: rgba(0, 0, 0, 0.08);
}

.msg-bubble.typing {
  display: flex;
  gap: 4px;
  align-items: center;
  min-height: 1.5em;
}

.dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: #aa80cc;
  animation: dot-bounce 1.2s infinite ease-in-out;
}

.dot:nth-child(2) { animation-delay: .2s; }
.dot:nth-child(3) { animation-delay: .4s; }

@keyframes dot-bounce {
  0%, 80%, 100% { transform: scale(0.6); opacity: 0.4; }
  40% { transform: scale(1); opacity: 1; }
}

.msg-assistant .msg-bubble {
  background: rgba(255,255,255,0.88);
  backdrop-filter: blur(8px);
  color: #1a1a1a;
  border-bottom-left-radius: 4px;
}

.msg-user .msg-bubble {
  background: #aa3bff;
  color: #fff;
  border-bottom-right-radius: 4px;
}

.msg-time {
  font-size: 10px;
  color: rgba(255,255,255,0.4);
  padding: 0 4px;
}

/* 打字指示符 */
.cursor {
  animation: blink 0.8s step-end infinite;
  color: #aa3bff;
}
@keyframes blink {
  0%, 100% { opacity: 1; }
  50% { opacity: 0; }
}
</style>
