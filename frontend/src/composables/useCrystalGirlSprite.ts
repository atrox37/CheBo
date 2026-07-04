import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useChatStore } from '@/stores/chat'
import { usePetStore } from '@/stores/pet'
import {
  mapAgentStateToMood,
  agentStateUsesTalkSprite,
  resolveCrystalGirlSprite,
} from '@/config/crystalGirl'

/**
 * 根据聊天情绪、Agent 状态驱动 CrystalGirl PNG 切换。
 * - talking：流式输出或 Talking 状态 → Talk 立绘
 * - blink：待机时随机眨眼
 * - neutral talk：NeutralTalk / NeutralTalk2 交替
 */
export function useCrystalGirlSprite() {
  const chatStore = useChatStore()
  const petStore  = usePetStore()

  const isBlinking     = ref(false)
  const talkAltFrame   = ref(false)

  let blinkTimer: ReturnType<typeof setTimeout> | null = null
  let talkTimer: ReturnType<typeof setInterval> | null = null

  const mood = computed(() =>
    mapAgentStateToMood(chatStore.agentState, chatStore.currentEmotion),
  )

  const isTalking = computed(() =>
    agentStateUsesTalkSprite(chatStore.agentState, chatStore.isTalkingVisual),
  )

  const spriteSrc = computed(() => {
    const state = chatStore.agentState
    const resting = state === 'sleeping' || petStore.currentAction === 'resting'
    if (resting) {
      return resolveCrystalGirlSprite('neutral', {
        isTalking: false,
        isBlinking: false,
        talkAltFrame: false,
      })
    }
    return resolveCrystalGirlSprite(mood.value, {
      isTalking: isTalking.value,
      isBlinking: isBlinking.value && state === 'idle',
      talkAltFrame: talkAltFrame.value,
    })
  })

  function scheduleBlink() {
    if (blinkTimer) clearTimeout(blinkTimer)
    const delay = 2800 + Math.random() * 3200
    blinkTimer = setTimeout(() => {
      if (!isTalking.value && chatStore.agentState === 'idle') {
        isBlinking.value = true
        setTimeout(() => {
          isBlinking.value = false
          scheduleBlink()
        }, 120)
      } else {
        scheduleBlink()
      }
    }, delay)
  }

  onMounted(() => {
    scheduleBlink()
    talkTimer = setInterval(() => {
      talkAltFrame.value = !talkAltFrame.value
    }, 450)
  })

  onUnmounted(() => {
    if (blinkTimer) clearTimeout(blinkTimer)
    if (talkTimer) clearInterval(talkTimer)
  })

  return { spriteSrc, mood, isTalking }
}
