/**
 * 跨组件导航（遗留：桌宠侧栏已移除，后续可改为工作台页面深链）
 */
import { ref } from 'vue'

export type TabId = 'companion' | 'settings'
const _pendingTab = ref<TabId | null>(null)

export function navigateTo(tab: TabId) {
  _pendingTab.value = tab
}

/** App.vue 调用：消费待跳转 tab，返回目标并清除 */
export function consumePendingTab(): TabId | null {
  const t = _pendingTab.value
  _pendingTab.value = null
  return t
}

export { _pendingTab as pendingTab }
