/**
 * useAppMode — 全局双模式管理
 *
 * 桌宠模式 (pet)：透明·无边框·置顶·320×285 悬浮窗
 * 助手模式 (assistant)：标准窗口·有边框·不置顶·1000×680 居中
 */
import { ref } from 'vue'
import { getCurrentWindow, currentMonitor } from '@tauri-apps/api/window'
import { LogicalSize, PhysicalPosition } from '@tauri-apps/api/dpi'

/** setOpacity 在此 Tauri 版本中不存在，以空函数占位避免引用错误 */
// eslint-disable-next-line @typescript-eslint/no-unused-vars
async function fadeOut(_win: unknown) { /* no-op */ }
// eslint-disable-next-line @typescript-eslint/no-unused-vars
async function fadeIn(_win: unknown) { /* no-op */ }

export const ASSISTANT_WIDTH  = 1000
export const ASSISTANT_HEIGHT = 680

export type AppMode = 'pet' | 'assistant'
// 全局单例（模块级 ref，所有组件共享同一个实例）
const mode = ref<AppMode>('pet')
const switching = ref(false)

// 切换前保存桌宠位置
let savedPos: PhysicalPosition | null = null

// ─── 切换到助手模式 ────────────────────────────────────────────────────────────

export async function switchToAssistant(): Promise<void> {
  if (mode.value === 'assistant' || switching.value) return
  switching.value = true
  try {
    const win = getCurrentWindow()

    // 先保存当前坐标（桌宠位置）
    try {
      savedPos = await win.outerPosition()
    } catch { /* 非关键，忽略 */ }

    // 先切 Vue mode（确保 UI 立即响应），再调整窗口属性
    mode.value = 'assistant'

    await fadeOut(win)

    // 逐步调用，每步独立 try/catch，确保 setSize 即使在某个步骤失败后仍执行
    try { await win.setDecorations(true)  } catch (e) { console.warn('setDecorations', e) }
    try { await win.setAlwaysOnTop(false) } catch (e) { console.warn('setAlwaysOnTop', e) }
    try { await win.setResizable(true)    } catch (e) { console.warn('setResizable', e) }
    try { await win.setMinSize(new LogicalSize(ASSISTANT_WIDTH, ASSISTANT_HEIGHT)) } catch (e) { console.warn('setMinSize', e) }
    try { await win.setSize(new LogicalSize(ASSISTANT_WIDTH, ASSISTANT_HEIGHT)) } catch (e) { console.warn('setSize', e) }
    try { await win.center()              } catch (e) { console.warn('center', e) }

    await sleep(80)
    await fadeIn(win)
  } catch (err) {
    console.error('[useAppMode] switchToAssistant failed:', err)
    // 回滚
    mode.value = 'pet'
  } finally {
    switching.value = false
  }
}

// ─── 切回桌宠模式 ──────────────────────────────────────────────────────────────

export async function switchToPet(): Promise<void> {
  if (mode.value === 'pet' || switching.value) return
  switching.value = true
  try {
    const win = getCurrentWindow()

    await fadeOut(win)

    // 先切 Vue mode
    mode.value = 'pet'

    try { await win.unminimize()                      } catch (e) { console.warn('unminimize', e) }
    try { await win.show()                            } catch (e) { console.warn('show', e) }
    try { await win.setDecorations(false)             } catch (e) { console.warn('setDecorations', e) }
    try { await win.setAlwaysOnTop(true)              } catch (e) { console.warn('setAlwaysOnTop', e) }
    try { await win.setResizable(false)               } catch (e) { console.warn('setResizable', e) }
    try { await win.setMinSize(new LogicalSize(320, 285)) } catch (e) { console.warn('setMinSize', e) }
    try { await win.setShadow(false)                  } catch (e) { console.warn('setShadow', e) }
    try { await win.setSize(new LogicalSize(320, 285)) } catch (e) { console.warn('setSize', e) }

    // 恢复之前的桌宠位置（确保仍在可见工作区内）
    if (savedPos) {
      try {
        await win.setPosition(await clampToWorkArea(savedPos, win))
      } catch (e) { console.warn('setPosition', e) }
    }

    await sleep(80)
    await fadeIn(win)
    try { await win.setFocus() } catch (e) { console.warn('setFocus', e) }
  } catch (err) {
    console.error('[useAppMode] switchToPet failed:', err)
    mode.value = 'assistant'
  } finally {
    switching.value = false
  }
}

// ─── Composable hook ──────────────────────────────────────────────────────────

export function useAppMode() {
  return {
    mode,
    switching,
    switchToAssistant,
    switchToPet,
  }
}

// ─── 工具 ────────────────────────────────────────────────────────────────────

function sleep(ms: number): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, ms))
}

/** 将窗口位置限制在当前显示器工作区内，避免高 DPI 下坐标错位后飞出屏幕 */
async function clampToWorkArea(
  pos: PhysicalPosition,
  win: ReturnType<typeof getCurrentWindow>,
): Promise<PhysicalPosition> {
  const monitor = await currentMonitor()
  if (!monitor) return pos

  const size = await win.outerSize()
  const area = monitor.workArea
  const margin = 24

  const minX = area.position.x + margin
  const minY = area.position.y + margin
  const maxX = area.position.x + area.size.width  - size.width  - margin
  const maxY = area.position.y + area.size.height - size.height - margin

  return new PhysicalPosition(
    Math.min(Math.max(pos.x, minX), Math.max(minX, maxX)),
    Math.min(Math.max(pos.y, minY), Math.max(minY, maxY)),
  )
}
