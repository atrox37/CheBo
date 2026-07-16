<script setup lang="ts">
import { computed } from 'vue'
import { useCheboStore } from '@/stores/chebo'
import { useChatStore } from '@/stores/chat'
import { useCrystalGirlSprite } from '@/composables/useCrystalGirlSprite'

const cheboStore = useCheboStore()
const chatStore = useChatStore()
const { spriteSrc } = useCrystalGirlSprite()
const emit = defineEmits<{ (e: 'petClick'): void }>()

const displaySrc = computed(() =>
  cheboStore.useCrystalGirl ? spriteSrc.value : cheboStore.characterImage,
)

/** 有独立立绘后仅保留轻量阴影，避免色相滤镜破坏原画 */
const EMOTION_SHADOW: Record<string, string> = {
  normal:    'drop-shadow(0 4px 14px rgba(160, 120, 180, 0.28))',
  happy:     'drop-shadow(0 4px 18px rgba(255, 180, 80, 0.45))',
  proud:     'drop-shadow(0 4px 16px rgba(140, 100, 220, 0.38))',
  shy:       'drop-shadow(0 4px 16px rgba(240, 120, 180, 0.4))',
  angry:     'drop-shadow(0 4px 18px rgba(220, 80, 80, 0.42))',
  sad:       'drop-shadow(0 4px 14px rgba(100, 120, 200, 0.35))',
  surprised: 'drop-shadow(0 4px 20px rgba(255, 160, 60, 0.48))',
  sleepy:    'drop-shadow(0 4px 12px rgba(120, 130, 200, 0.3))',
}

const appliedFilter = computed(() =>
  EMOTION_SHADOW[chatStore.currentEmotion] ?? EMOTION_SHADOW.normal,
)

const agentState = computed(() => chatStore.agentState)

const STATE_RING_COLOR: Record<string, string> = {
  idle:             'transparent',
  thinking:         'rgba(120, 160, 255, 0.8)',
  talking:          'rgba(100, 220, 140, 0.8)',
  working:          'rgba(250, 180, 60, 0.8)',
  sleeping:         'rgba(160, 140, 220, 0.6)',
  observing:        'rgba(100, 200, 180, 0.75)',
  waitingConfirm:   'rgba(255, 160, 80, 0.85)',
  executingTool:    'rgba(250, 180, 60, 0.85)',
  interrupted:      'rgba(200, 120, 255, 0.7)',
  errorRecover:     'rgba(220, 90, 90, 0.8)',
}

const STATE_ICON: Record<string, string> = {
  idle:             '',
  thinking:         '💭',
  talking:          '💬',
  working:          '⚙️',
  sleeping:         '💤',
  observing:        '👀',
  waitingConfirm:   '❓',
  executingTool:    '🔧',
  interrupted:      '✋',
  errorRecover:     '⚠️',
}
const stateRingColor = computed(() =>
  STATE_RING_COLOR[agentState.value] ?? 'transparent',
)

const stateIcon = computed(() => STATE_ICON[agentState.value] ?? '')

const showStateRing = computed(() => agentState.value !== 'idle')
</script>

<template>
  <div class="char-wrap" data-tauri-drag-region>

    <transition name="ring">
      <div
        v-if="showStateRing"
        class="state-ring"
        :class="`ring-${agentState}`"
        :style="{ '--ring-color': stateRingColor }"
      />
    </transition>

    <img
      :src="displaySrc"
      alt="Chebo"
      class="char-img"
      :class="`state-${agentState}`"
      :style="{ filter: appliedFilter }"
      draggable="false"
      data-tauri-drag-region
      @click.stop="emit('petClick')"
    />

    <transition name="badge">
      <div v-if="stateIcon" class="state-badge" :class="`badge-${agentState}`">
        {{ stateIcon }}
      </div>
    </transition>

  </div>
</template>

<style scoped>
.char-wrap {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: flex-end;
  justify-content: center;
  background: transparent;
  padding-bottom: 2px;
  z-index: 1;
}

.char-img {
  width: 132px;
  height: 250px;
  object-fit: contain;
  object-position: bottom center;
  cursor: pointer;
  transition: filter 0.35s ease, opacity 0.35s ease, transform 0.35s ease;
  position: relative;
  z-index: 2;
  animation: idle-breathe 4.2s ease-in-out infinite;
}

.char-img:hover {
  filter: drop-shadow(0 6px 20px rgba(160, 120, 180, 0.42)) !important;
}

@keyframes idle-breathe {
  0%, 100% { transform: translateY(0); }
  50%       { transform: translateY(-4px); }
}

.char-img.state-sleeping {
  opacity: 0.78;
  transform: rotate(-2deg) translateY(4px);
  animation: none;
}

.char-img.state-thinking {
  animation: float-thinking 2.4s ease-in-out infinite;
}

@keyframes float-thinking {
  0%, 100% { transform: translateY(0); }
  50%       { transform: translateY(-5px); }
}

.char-img.state-executingTool {
  animation: sway-talking 0.45s ease-in-out infinite alternate;
}

.char-img.state-waitingConfirm {
  animation: float-thinking 2s ease-in-out infinite;
}

.char-img.state-observing {
  opacity: 0.92;
  animation: idle-breathe 3s ease-in-out infinite;
}

.char-img.state-errorRecover {
  opacity: 0.85;
  transform: translateY(2px);
  animation: none;
}

.char-img.state-interrupted {
  opacity: 0.9;
  animation: none;
}

.ring-executingTool {
  animation: spin-ring 3s linear infinite;
  border-style: dotted;
}

.ring-waitingConfirm {
  animation: pulse-ring 1.2s ease-out infinite;
  border-style: solid;
}

.ring-observing {
  animation: breathe-ring 2.5s ease-in-out infinite;
}

.ring-errorRecover {
  animation: pulse-ring 0.8s ease-out infinite;
  border-color: rgba(220, 90, 90, 0.85) !important;
}

.char-img.state-talking {
  animation: sway-talking 0.55s ease-in-out infinite alternate;
}

@keyframes sway-talking {
  from { transform: rotate(-1.2deg) translateY(-2px); }
  to   { transform: rotate(1.2deg) translateY(-2px); }
}

.state-ring {
  position: absolute;
  bottom: 8px;
  left: 50%;
  transform: translateX(-50%);
  width: 120px;
  height: 120px;
  border-radius: 50%;
  border: 3px solid var(--ring-color, transparent);
  pointer-events: none;
  z-index: 0;
}

.ring-thinking {
  animation: spin-ring 2s linear infinite;
  border-style: dashed;
}

@keyframes spin-ring {
  to { transform: translateX(-50%) rotate(360deg); }
}

.ring-talking {
  animation: pulse-ring 1s ease-out infinite;
}

@keyframes pulse-ring {
  0%   { transform: translateX(-50%) scale(1);    opacity: 1; }
  100% { transform: translateX(-50%) scale(1.25); opacity: 0; }
}

.ring-working {
  animation: spin-ring 4s linear infinite;
  border-style: dotted;
}

.ring-sleeping {
  animation: breathe-ring 3s ease-in-out infinite;
}

@keyframes breathe-ring {
  0%, 100% { opacity: 0.3; transform: translateX(-50%) scale(0.95); }
  50%       { opacity: 0.7; transform: translateX(-50%) scale(1.05); }
}

.state-badge {
  position: absolute;
  top: 14px;
  right: 10px;
  font-size: 15px;
  line-height: 1;
  filter: drop-shadow(0 1px 4px rgba(0,0,0,0.3));
  z-index: 3;
  pointer-events: none;
}

.badge-sleeping {
  animation: float-zzz 3s ease-in-out infinite;
}

@keyframes float-zzz {
  0%, 100% { transform: translateY(0);    opacity: 0.8; }
  50%       { transform: translateY(-6px); opacity: 1; }
}

.badge-thinking {
  animation: scale-bubble 1.4s ease-in-out infinite;
}

@keyframes scale-bubble {
  0%, 100% { transform: scale(1); }
  50%       { transform: scale(1.25); }
}

.ring-enter-active,  .ring-leave-active  { transition: opacity 0.4s ease; }
.ring-enter-from,    .ring-leave-to      { opacity: 0; }
.badge-enter-active, .badge-leave-active { transition: opacity 0.3s, transform 0.3s; }
.badge-enter-from,   .badge-leave-to     { opacity: 0; transform: scale(0.5); }
</style>
