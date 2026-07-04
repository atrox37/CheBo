/** Chebo 前端 localStorage 键名（统一口径） */
export const STORAGE_KEYS = {
  sessionId: 'chebo_session_id',
  keepPerfect: 'chebo_keepPerfect',
  careMode: 'chebo_careMode',
  deepThink: 'chebo_deepThink',
} as const

const LEGACY_MAP: Record<string, string> = {
  [STORAGE_KEYS.sessionId]: 'erii_session_id',
  [STORAGE_KEYS.keepPerfect]: 'erii_keepPerfect',
  [STORAGE_KEYS.careMode]: 'erii_careMode',
}

/** 从旧版 Erii 键名迁移到 Chebo 键名（仅迁移一次） */
export function migrateLegacyStorageKey(key: string): void {
  const legacy = LEGACY_MAP[key]
  if (!legacy) return
  const legacyValue = localStorage.getItem(legacy)
  if (legacyValue !== null && localStorage.getItem(key) === null) {
    localStorage.setItem(key, legacyValue)
  }
  if (legacyValue !== null) {
    localStorage.removeItem(legacy)
  }
}

export function getOrCreateSessionId(): string {
  migrateLegacyStorageKey(STORAGE_KEYS.sessionId)
  const existing = localStorage.getItem(STORAGE_KEYS.sessionId)
  if (existing) return existing
  const id = crypto.randomUUID()
  localStorage.setItem(STORAGE_KEYS.sessionId, id)
  return id
}
