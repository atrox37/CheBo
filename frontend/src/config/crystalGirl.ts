/** CrystalGirl PNGTuber 资源（CC0 by Tsuchikabi） */
export const CRYSTAL_GIRL_BASE = '/chebo/crystalgirl/PNGTuber_CrystalGirl'

export type CrystalGirlMood =
  | 'neutral'
  | 'happy'
  | 'sad'
  | 'angry'
  | 'smug'
  | 'surprise'

/** Chebo 情绪 / Agent 状态 → CrystalGirl 情绪组 */
export function mapCheboEmotionToMood(
  emotion: string,
  agentState: string,
): CrystalGirlMood {
  if (agentState === 'sleeping') return 'neutral'

  switch (emotion) {
    case 'happy':
      return 'happy'
    case 'sad':
      return 'sad'
    case 'angry':
      return 'angry'
    case 'shy':
    case 'proud':
      return 'smug'
    case 'surprised':
      return 'surprise'
    default:
      return 'neutral'
  }
}

/** AgentState 优先于情绪的立绘基调（Phase B 10.4） */
export function mapAgentStateToMood(
  agentState: string,
  emotion: string,
): CrystalGirlMood {
  switch (agentState) {
    case 'sleeping':
      return 'neutral'
    case 'errorRecover':
      return 'sad'
    case 'waitingConfirm':
      return 'surprise'
    case 'observing':
      return 'surprise'
    case 'executingTool':
    case 'working':
      return 'smug'
    case 'thinking':
      return mapCheboEmotionToMood(emotion, agentState)
    default:
      return mapCheboEmotionToMood(emotion, agentState)
  }
}

/** 哪些 AgentState 应使用 Talk 口型立绘 */
export function agentStateUsesTalkSprite(agentState: string, isTalkingVisual: boolean): boolean {
  if (isTalkingVisual) return true
  return ['talking', 'executingTool', 'working'].includes(agentState)
}

const MOOD_FILES: Record<CrystalGirlMood, { idle: string; talk: string; talkAlt?: string }> = {
  neutral: {
    idle: `${CRYSTAL_GIRL_BASE}/CrystalGirl_NeutralIdle.png`,
    talk: `${CRYSTAL_GIRL_BASE}/CrystalGirl_NeutralTalk.png`,
    talkAlt: `${CRYSTAL_GIRL_BASE}/CrystalGirl_NeutralTalk2.png`,
  },
  happy: {
    idle: `${CRYSTAL_GIRL_BASE}/CrystalGirl_HappyIdle.png`,
    talk: `${CRYSTAL_GIRL_BASE}/CrystalGirl_HappyTalk.png`,
  },
  sad: {
    idle: `${CRYSTAL_GIRL_BASE}/CrystalGirl_SadIdle.png`,
    talk: `${CRYSTAL_GIRL_BASE}/CrystalGirl_SadTalk.png`,
  },
  angry: {
    idle: `${CRYSTAL_GIRL_BASE}/CrystalGirl_AngryIdle.png`,
    talk: `${CRYSTAL_GIRL_BASE}/CrystalGirl_AngryTalk.png`,
  },
  smug: {
    idle: `${CRYSTAL_GIRL_BASE}/CrystalGirl_SmugIdle.png`,
    talk: `${CRYSTAL_GIRL_BASE}/CrystalGirl_SmugTalk.png`,
  },
  surprise: {
    idle: `${CRYSTAL_GIRL_BASE}/CrystalGirl_SurpriseIdle.png`,
    talk: `${CRYSTAL_GIRL_BASE}/CrystalGirl_SurpriseTalk.png`,
  },
}

export const CRYSTAL_GIRL_BLINK = {
  idle: `${CRYSTAL_GIRL_BASE}/CrystalGirl_BlinkIdle.png`,
  talk: `${CRYSTAL_GIRL_BASE}/CrystalGirl_BlinkTalk.png`,
}

export function resolveCrystalGirlSprite(
  mood: CrystalGirlMood,
  opts: {
    isTalking: boolean
    isBlinking: boolean
    talkAltFrame: boolean
  },
): string {
  if (opts.isBlinking) {
    return opts.isTalking ? CRYSTAL_GIRL_BLINK.talk : CRYSTAL_GIRL_BLINK.idle
  }

  const set = MOOD_FILES[mood]
  if (opts.isTalking) {
    if (mood === 'neutral' && opts.talkAltFrame && set.talkAlt) {
      return set.talkAlt
    }
    return set.talk
  }
  return set.idle
}

/** 工作台侧栏：固定展示 normal 待机立绘 */
export const CRYSTAL_GIRL_NEUTRAL_IDLE = MOOD_FILES.neutral.idle
