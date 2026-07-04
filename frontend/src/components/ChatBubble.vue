<script setup lang="ts">
import { computed } from 'vue'
import { useChatStore } from '@/stores/chat'
import { extractEmotionFromText } from '@/utils/emotionTag'
import { CHEBO_NAME } from '@/config/chebo'
import { renderMarkdown } from '@/utils/emotionTag'

const chatStore = useChatStore()

const text = computed(() => {
  const raw = chatStore.isTyping
    ? chatStore.streamBuffer
    : (chatStore.latestAssistantMessage?.content ?? '')
  return extractEmotionFromText(raw).clean
})
const isEmpty = computed(() => !chatStore.isTyping && !text.value)
</script>

<template>
  <div class="bubble-wrap">
    <div class="bubble-head">
      <span class="bubble-name">{{ CHEBO_NAME }}</span>
    </div>
    <div class="bubble" :class="{ empty: isEmpty }">
      <div class="scroll-area">
        <div v-if="!isEmpty" class="txt chat-md" v-html="renderMarkdown(text)" />
        <span v-if="chatStore.isTalkingVisual" class="cursor">▌</span>
        <div v-if="isEmpty" class="placeholder">
          <span class="dot" /><span class="dot" /><span class="dot" />
        </div>
      </div>
      <div class="tail" />
    </div>
  </div>
</template>

<style scoped>
.bubble-wrap {
  display: flex;
  flex-direction: column;
  gap: 4px;
  max-width: 100px;
}

.bubble-head {
  display: flex;
  align-items: center;
  gap: 5px;
  padding-left: 2px;
}

.bubble-name {
  font-size: 10px;
  font-weight: 700;
  color: #806080;
}

.bubble {
  width: 92px;
  background: #ffffff;
  border-radius: 12px;
  box-shadow: 0 3px 14px rgba(0,0,0,0.12);
  padding: 9px 10px 9px;
  position: relative;
}
.scroll-area {
  max-height: 160px;
  overflow-y: auto;
  scrollbar-width: thin;
  scrollbar-color: rgba(180,130,160,0.3) transparent;
}
.scroll-area::-webkit-scrollbar { width: 2px; }
.scroll-area::-webkit-scrollbar-thumb { background: rgba(180,130,160,0.4); border-radius: 2px; }

.txt {
  font-size: 11px;
  line-height: 1.65;
  color: #3a1a2a;
  word-break: break-word;
  white-space: pre-wrap;
  margin: 0;
}

.cursor {
  display: inline-block;
  color: #e8729a;
  font-size: 11px;
  margin-left: 1px;
  animation: blink 0.8s step-end infinite;
}
@keyframes blink { 0%,100%{opacity:1} 50%{opacity:0} }

/* right-pointing tail */
.tail {
  position: absolute;
  top: 16px;
  right: -8px;
  width: 0; height: 0;
  border-top: 6px solid transparent;
  border-bottom: 6px solid transparent;
  border-left: 9px solid #ffffff;
  filter: drop-shadow(2px 1px 1px rgba(0,0,0,0.06));
}

/* 空状态：气泡更小，透明度稍低 */
.bubble.empty {
  opacity: 0.75;
  min-height: 36px;
  display: flex;
  align-items: center;
}

/* 三点等待动画 */
.placeholder {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 2px 0;
}
.dot {
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: #d0a8c0;
  animation: dotPulse 1.4s ease-in-out infinite;
}
.dot:nth-child(2) { animation-delay: 0.2s; }
.dot:nth-child(3) { animation-delay: 0.4s; }
@keyframes dotPulse {
  0%, 80%, 100% { transform: scale(0.7); opacity: 0.4; }
  40%           { transform: scale(1);   opacity: 1; }
}
</style>
