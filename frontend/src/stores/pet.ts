import { defineStore }          from 'pinia'
import { ref, computed, watch } from 'vue'
import { invoke }               from '@tauri-apps/api/core'
import * as tauriService        from '@/services/tauriService'
import { STORAGE_KEYS, migrateLegacyStorageKey } from '@/utils/storageKeys'

// ── localStorage 缓存 key ──────────────────────────────────────────────────
const LS_STATUS    = 'chebo_status'
const LS_FOODS     = 'chebo_foods'
const LS_INVENTORY = 'chebo_inventory'
const LS_TASKS_S   = 'chebo_tasks_study'
const LS_TASKS_W   = 'chebo_tasks_work'

function saveLS(key: string, value: unknown) {
  try { localStorage.setItem(key, JSON.stringify(value)) } catch { /**/ }
}
function loadLS<T>(key: string, fallback: T): T {
  try {
    const s = localStorage.getItem(key)
    return s ? (JSON.parse(s) as T) : fallback
  } catch { return fallback }
}

// ── 类型定义（与 Rust db::Food / db::Task 对应） ──────────────────────────

export interface FoodItem {
  id:           string
  name:         string
  price:        number
  hunger:       number
  energy:       number
  mood:         number
  unlock_level: number
  enabled:      number
}

export interface TaskItem {
  id:           string
  task_type:    string
  name:         string
  duration:     number      // 秒
  energy_cost:  number
  exp:          number
  coins:        number
  mood_delta:   number
  unlock_level: number
  enabled:      number
  locked?:      boolean     // 前端派生：unlock_level > 当前 level
}

interface CachedStatus {
  hunger: number; energy: number; mood: number; affection: number
  level: number; exp: number; coins: number
  current_action: string
  active_task_id: string | null; task_ends_at: string | null; task_type: string | null
}

// ── 默认值（离线初始值） ──────────────────────────────────────────────────
const DEFAULT_FOODS: FoodItem[] = [
  { id: 'bread', name: '面包',   price: 10, hunger: 20, energy: 0,  mood: 1,  unlock_level: 1, enabled: 1 },
  { id: 'milk',  name: '牛奶',   price: 12, hunger: 12, energy: 5,  mood: 2,  unlock_level: 1, enabled: 1 },
  { id: 'cake',  name: '小蛋糕', price: 25, hunger: 15, energy: 0,  mood: 10, unlock_level: 1, enabled: 1 },
]

export const usePetStore = defineStore('pet', () => {
  // ── 从 localStorage 恢复初始状态 ──────────────────────────────────────
  const cached = loadLS<Partial<CachedStatus>>(LS_STATUS, {})

  const hunger    = ref<number>(cached.hunger    ?? 80)
  const energy    = ref<number>(cached.energy    ?? 80)
  const mood      = ref<number>(cached.mood      ?? 70)
  const affection = ref<number>(cached.affection ?? 20)
  const level     = ref<number>(cached.level     ?? 1)
  const exp       = ref<number>(cached.exp       ?? 0)
  const coins     = ref<number>(cached.coins     ?? 100)

  const currentAction = ref<string>(cached.current_action ?? 'idle')
  const activeTaskId  = ref<string | null>(cached.active_task_id ?? null)
  const taskEndsAt    = ref<string | null>(cached.task_ends_at   ?? null)
  const taskType      = ref<string | null>(cached.task_type      ?? null)

  const inventory  = ref<{ item_id: string; count: number }[]>(loadLS(LS_INVENTORY, []))
  const foods      = ref<FoodItem[]>(loadLS(LS_FOODS, DEFAULT_FOODS))
  const studyTasks = ref<TaskItem[]>(loadLS(LS_TASKS_S, []))
  const workTasks  = ref<TaskItem[]>(loadLS(LS_TASKS_W, []))

  migrateLegacyStorageKey(STORAGE_KEYS.keepPerfect)
  migrateLegacyStorageKey(STORAGE_KEYS.careMode)

  const keepPerfect = ref(localStorage.getItem(STORAGE_KEYS.keepPerfect) === '1')
  // 关怀模式：true = Chebo 会主动发言；false = 静默模式（不主动发消息）
  const careMode = ref(localStorage.getItem(STORAGE_KEYS.careMode) !== '0')

  // ── 计算属性 ──────────────────────────────────────────────────────────
  const nextLevelExp    = computed(() => 100 + (level.value - 1) * 50)
  const expProgress     = computed(() => Math.min(1, exp.value / nextLevelExp.value))
  const isDoingTask     = computed(() => !!activeTaskId.value)
  const taskSecondsLeft = computed(() => {
    if (!taskEndsAt.value) return -1
    const diff = Math.floor((new Date(taskEndsAt.value).getTime() - Date.now()) / 1000)
    return diff > 0 ? diff : 0
  })
  const inventoryCount = computed(() =>
    (itemId: string) => inventory.value.find(i => i.item_id === itemId)?.count ?? 0
  )

  // ── 状态更新（来自 Tauri status_update 事件 / invoke 返回） ─────────
  function applyUpdate(data: Record<string, unknown>) {
    if (data.hunger    !== undefined) hunger.value    = data.hunger    as number
    if (data.energy    !== undefined) energy.value    = data.energy    as number
    if (data.mood      !== undefined) mood.value      = data.mood      as number
    if (data.affection !== undefined) affection.value = data.affection as number
    if (data.level     !== undefined) level.value     = data.level     as number
    if (data.exp       !== undefined) exp.value       = data.exp       as number
    if (data.coins     !== undefined) coins.value     = data.coins     as number
    if (data.current_action !== undefined) currentAction.value = data.current_action as string
    if (data.active_task_id !== undefined) activeTaskId.value  = data.active_task_id as string | null
    if (data.task_ends_at   !== undefined) taskEndsAt.value    = data.task_ends_at   as string | null
    if (data.task_type      !== undefined) taskType.value      = data.task_type      as string | null

    saveLS(LS_STATUS, {
      hunger: hunger.value, energy: energy.value, mood: mood.value,
      affection: affection.value, level: level.value, exp: exp.value,
      coins: coins.value, current_action: currentAction.value,
      active_task_id: activeTaskId.value, task_ends_at: taskEndsAt.value,
      task_type: taskType.value,
    })
  }

  // ── 保持最佳状态 ──────────────────────────────────────────────────────
  async function setKeepPerfect(v: boolean) {
    keepPerfect.value = v
    localStorage.setItem(STORAGE_KEYS.keepPerfect, v ? '1' : '0')
    try { await tauriService.setKeepPerfect(v) } catch { /* 静默 */ }
    if (v) { hunger.value = 100; energy.value = 100; mood.value = 100 }
  }

  async function syncKeepPerfect() {
    if (keepPerfect.value) {
      try { await tauriService.setKeepPerfect(true) } catch { /**/ }
    }
  }

  // ── 关怀/静默模式 ──────────────────────────────────────────────────────
  async function setCareMode(v: boolean) {
    careMode.value = v
    localStorage.setItem(STORAGE_KEYS.careMode, v ? '1' : '0')
    try { await invoke('set_care_mode', { enabled: v }) } catch { /* 静默 */ }
  }

  // ── 数据拉取（invoke 替代 fetch） ─────────────────────────────────────

  async function fetchStatus() {
    try {
      const data = await invoke<Record<string, unknown>>('get_status')
      applyUpdate(data)
    } catch (e) { console.warn('[pet] fetchStatus:', e) }
  }

  async function fetchFoods() {
    try {
      const data = await invoke<FoodItem[]>('get_foods')
      foods.value = data
      saveLS(LS_FOODS, data)
    } catch (e) { console.warn('[pet] fetchFoods:', e) }
  }

  async function fetchTasks(type: 'study' | 'work') {
    try {
      const data = await invoke<TaskItem[]>('get_tasks', { taskType: type })
      // 标记高等级锁定状态
      const marked = data.map(t => ({ ...t, locked: t.unlock_level > level.value }))
      if (type === 'study') {
        studyTasks.value = marked
        saveLS(LS_TASKS_S, marked)
      } else {
        workTasks.value = marked
        saveLS(LS_TASKS_W, marked)
      }
    } catch (e) { console.warn('[pet] fetchTasks:', e) }
  }

  async function fetchInventory() {
    try {
      const data = await invoke<{ item_id: string; item_type: string; count: number }[]>('get_inventory')
      inventory.value = data.map(i => ({ item_id: i.item_id, count: i.count }))
      saveLS(LS_INVENTORY, inventory.value)
    } catch (e) { console.warn('[pet] fetchInventory:', e) }
  }

  watch(inventory, (v) => saveLS(LS_INVENTORY, v), { deep: true })

  return {
    hunger, energy, mood, affection, level, exp, coins,
    currentAction, activeTaskId, taskEndsAt, taskType,
    inventory, foods, studyTasks, workTasks,
    keepPerfect, careMode,
    nextLevelExp, expProgress, isDoingTask, taskSecondsLeft, inventoryCount,
    applyUpdate, setKeepPerfect, syncKeepPerfect, setCareMode,
    fetchStatus, fetchFoods, fetchTasks, fetchInventory,
  }
})
