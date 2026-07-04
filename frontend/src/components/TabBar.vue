<script setup lang="ts">
export type TabId = 'status' | 'feed' | 'action' | 'shop' | 'settings'

const props = defineProps<{ active: TabId | null }>()
const emit  = defineEmits<{ (e: 'change', tab: TabId | null): void }>()

const tabs: { id: TabId; icon: string; label: string }[] = [
  { id: 'status',   icon: '❤',  label: '状态' },
  { id: 'feed',     icon: '🍜', label: '投喂' },
  { id: 'action',   icon: '✨', label: '动作' },
  { id: 'shop',     icon: '🛍', label: '商店' },
  { id: 'settings', icon: '⚙',  label: '设置' },
]

function onTab(id: TabId) {
  // 再次点击当前激活的 tab → 收起
  emit('change', props.active === id ? null : id)
}
</script>

<template>
  <div class="tab-bar" @click.stop @mousedown.stop>
    <button
      v-for="t in tabs"
      :key="t.id"
      class="tab-btn"
      :class="{ active: active === t.id }"
      @click.stop="onTab(t.id)"
    >
      <span class="tab-icon">{{ t.icon }}</span>
      <span class="tab-label">{{ t.label }}</span>
    </button>
  </div>
</template>

<style scoped>
.tab-bar {
  display: flex;
  align-items: stretch;
  height: 32px;
  background: linear-gradient(180deg, #ffe8f2 0%, #ffd6e8 100%);
  border-top: 1px solid rgba(255,255,255,0.9);
  box-shadow: 0 -2px 8px rgba(200,80,130,0.15),
              inset 0 1px 0 rgba(255,255,255,0.8);
  flex-shrink: 0;
}

.tab-btn {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 1px;
  flex: 1;
  background: none;
  border: none;
  border-right: 1px solid rgba(220,140,180,0.3);
  cursor: pointer;
  padding: 0;
  transition: background 0.12s;
  position: relative;
}
.tab-btn:last-child { border-right: none; }
.tab-btn:hover { background: rgba(255,200,220,0.5); }

.tab-btn.active {
  background: linear-gradient(180deg, rgba(255,255,255,0.6) 0%, rgba(255,220,235,0.8) 100%);
}
.tab-btn.active::after {
  content: '';
  position: absolute;
  top: 0; left: 15%; right: 15%;
  height: 2px;
  background: #e8729a;
  border-radius: 0 0 3px 3px;
}

.tab-icon  { font-size: 12px; line-height: 1; }
.tab-label { font-size: 8px; color: #a06880; letter-spacing: 0.2px; }
.tab-btn.active .tab-label { color: #c0446e; font-weight: 700; }
</style>
