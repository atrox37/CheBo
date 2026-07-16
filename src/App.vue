<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted, nextTick } from 'vue'
import { Maximize2 } from 'lucide-vue-next'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { LogicalSize }      from '@tauri-apps/api/dpi'
import CharacterDisplay    from './components/CharacterDisplay.vue'
import ChatBubble          from './components/ChatBubble.vue'
import ChatInput           from './components/ChatInput.vue'
import ToolConfirmDialog   from './components/ToolConfirmDialog.vue'
import AssistantLayout     from './components/AssistantLayout.vue'
import { useChatStore }  from './stores/chat'
import { usePetStore }   from './stores/pet'
import * as tauriService from './services/tauriService'
import { useAppMode }    from './composables/useAppMode'

const chatStore = useChatStore()
const petStore  = usePetStore()
const { mode, switching, switchToAssistant, switchToPet } = useAppMode()

const W_PET      = 320
const H_BASE     = 285
const H_CHAT     = 340

function resizeWindow(chatOpen: boolean): void {
  getCurrentWindow().setSize(
    new LogicalSize(W_PET, chatOpen ? H_CHAT : H_BASE)
  ).catch(() => {})
}

const isHovered = ref(false)
let leaveTimer: ReturnType<typeof setTimeout> | null = null
function onEnter() {
  if (leaveTimer) { clearTimeout(leaveTimer); leaveTimer = null }
  isHovered.value = true
}
function onLeave() {
  leaveTimer = setTimeout(() => { isHovered.value = false }, 400)
}

const chatInputVisible = ref(false)
const charInteractPulse = ref(0)

function onCardDblClick() {
  const next = !chatInputVisible.value
  chatInputVisible.value = next
  resizeWindow(next)
  charInteractPulse.value++
  chatStore.setEmotion(next ? 'surprised' : 'normal')
  if (next) chatStore.showPetBubble()
  else if (!chatStore.bubblePinned && !chatStore.isGenerating) chatStore.hidePetBubble()
}

const bubbleVisible = computed(() => {
  if (switching.value) return false
  if (chatInputVisible.value) return true
  if (chatStore.isGenerating || chatStore.isTalkingVisual) return true
  if (chatStore.bubblePinned) return chatStore.petBubbleVisible
  return chatStore.petBubbleVisible
})

onMounted(async () => {
  chatStore.setSpeechPresentation('pet')
  await tauriService.setupListeners()
  await petStore.fetchStatus()

  try {
    const state = await tauriService.getAgentState()
    chatStore.setAgentState(state)
  } catch { /* 静默 */ }

  if (chatStore.messages.length === 0) {
    await nextTick()
    setTimeout(() => {
      chatStore.addMessage({
        role: 'assistant',
        content: '你好！我是 Chebo，有什么我可以帮你的吗~',
        emotion: 'happy',
      })
    }, 800)
  }

  tauriService.onTrayReset(() => {
    petStore.fetchStatus()
    chatStore.addMessage({
      role: 'assistant',
      content: '（状态已重置，重新出发~）',
      emotion: 'normal',
    })
  })

  tauriService.onOpenAssistant(() => switchToAssistant())
  tauriService.onSwitchToPet(() => switchToPet())
})

watch(mode, (m) => {
  if (m === 'pet') {
    chatStore.setSpeechPresentation('pet')
    resizeWindow(chatInputVisible.value)
  }
})

onUnmounted(() => {
  tauriService.teardownListeners()
  if (leaveTimer) clearTimeout(leaveTimer)
})
</script>

<template>
  <AssistantLayout v-if="mode === 'assistant'" />

  <template v-else>
    <div class="pet-root">
      <transition name="bbl">
        <ChatBubble v-if="bubbleVisible" class="bubble-pos" />
      </transition>

      <div class="interact" @mouseenter="onEnter" @mouseleave="onLeave">
        <div
          class="char-card"
          :class="{ 'char-interact-pulse': charInteractPulse > 0 }"
          data-tauri-drag-region
          @dblclick.stop="onCardDblClick"
          @animationend="charInteractPulse = 0"
        >
          <CharacterDisplay />
        </div>

        <transition name="btns">
          <div v-if="isHovered || chatInputVisible" class="btn-col">
            <button
              class="tab-btn expand-btn"
              title="展开工作台"
              :disabled="switching"
              @click.stop="switchToAssistant"
              @mousedown.stop
            >
              <Maximize2 :size="14" stroke-width="2" color="#8090a8" />
            </button>
          </div>
        </transition>
      </div>

      <transition name="ci">
        <div
          v-if="chatInputVisible"
          class="chat-wrap"
          @mouseenter="onEnter"
          @mouseleave="onLeave"
          @mousedown.stop
        >
          <ChatInput @close="onCardDblClick" />
        </div>
      </transition>
    </div>

    <ToolConfirmDialog />
  </template>
</template>

<style scoped>
.pet-root {
  position: fixed;
  inset: 0;
  background: transparent;
}

.bubble-pos {
  position: absolute;
  left: 4px;
  top: 20px;
}

.interact {
  position: absolute;
  left: 108px;
  top: 10px;
  display: flex;
  align-items: flex-start;
  gap: 8px;
}

.char-card {
  position: relative;
  width: 162px;
  height: 260px;
  flex-shrink: 0;
  overflow: hidden;
}

.char-card.char-interact-pulse {
  animation: char-dblclick-pop 0.45s ease-out;
}

@keyframes char-dblclick-pop {
  0%   { transform: scale(1); }
  35%  { transform: scale(1.06) translateY(-6px); }
  100% { transform: scale(1); }
}

.btn-col {
  display: flex;
  flex-direction: column;
  gap: 9px;
  flex-shrink: 0;
}

.tab-btn {
  width: 36px;
  height: 36px;
  border-radius: 50%;
  border: none;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
  transition: transform 0.15s, box-shadow 0.15s, background 0.15s;
}

.tab-btn:hover:not(:disabled) {
  transform: scale(1.12);
  box-shadow: 0 4px 14px rgba(0, 0, 0, 0.15);
}

.tab-btn:disabled {
  opacity: 0.5;
  cursor: wait;
}

.expand-btn {
  background: #f0f0f8 !important;
  border: 1.5px dashed #c0c8d8 !important;
  opacity: 0.85;
}

.expand-btn:hover:not(:disabled) {
  opacity: 1;
  background: #e8e8f4 !important;
  border-color: #8090a8 !important;
}

.chat-wrap {
  position: absolute;
  left: 68px;
  top: 274px;
  width: 242px;
}

.bbl-enter-active, .bbl-leave-active { transition: opacity 0.25s, transform 0.25s; }
.bbl-enter-from, .bbl-leave-to { opacity: 0; transform: scale(0.9) translateX(-6px); }

.ci-enter-active, .ci-leave-active { transition: opacity 0.2s, transform 0.2s; }
.ci-enter-from, .ci-leave-to { opacity: 0; transform: translateY(8px); }

.btns-enter-active { transition: opacity 0.2s ease, transform 0.2s ease; }
.btns-leave-active { transition: opacity 0.15s ease, transform 0.15s ease; }
.btns-enter-from, .btns-leave-to { opacity: 0; transform: translateX(-6px); }
</style>
