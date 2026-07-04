<template>
  <Teleport to="body">
    <Transition name="dialog-fade">
      <div v-if="visible" class="dialog-overlay" @click.self="handleReject">
        <div class="dialog-box" :class="`level-${pendingCall?.level ?? 0}`">
          <!-- 标题栏 -->
          <div class="dialog-header">
            <div class="level-badge" :class="`badge-${levelTag}`">
              {{ levelLabel }}
            </div>
            <span class="dialog-title">工具调用确认</span>
          </div>

          <!-- 工具信息 -->
          <div class="dialog-body">
            <div class="tool-info">
              <span class="tool-name">{{ pendingCall?.tool }}</span>
              <pre class="tool-args">{{ formattedArgs }}</pre>
            </div>

            <!-- 风险说明 -->
            <div class="risk-desc" :class="`risk-${levelTag}`">
              <i class="ci ci-info-circle" />
              {{ pendingCall?.riskDesc }}
            </div>
          </div>

          <!-- 操作按钮 -->
          <div class="dialog-footer">
            <button class="btn-reject" @click="handleReject">取消</button>
            <button
              class="btn-confirm"
              :class="`confirm-${levelTag}`"
              @click="handleConfirm"
            >
              {{ isHighRisk ? '我已了解风险，确认执行' : '确认执行' }}
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useChatStore } from '@/stores/chat';

const chatStore = useChatStore();

interface PendingCall {
  token:     string;
  tool:      string;
  args:      string;
  level:     number;
  levelTag:  string;
  riskDesc:  string;
}

const visible     = ref(false);
const pendingCall = ref<PendingCall | null>(null);

// 监听来自 Rust 的确认请求
listen<PendingCall>('tool_confirm_required', (event) => {
  pendingCall.value = event.payload;
  visible.value     = true;
  chatStore.setToolConfirmOpen(true);
});

const levelLabel = computed(() => {
  const map: Record<number, string> = {
    0: 'L0 只读',
    1: 'L1 查询',
    2: 'L2 系统操作',
    3: 'L3 高危操作',
  };
  return map[pendingCall.value?.level ?? 0] ?? '未知';
});

const levelTag    = computed(() => pendingCall.value?.levelTag ?? 'green');
const isHighRisk  = computed(() => (pendingCall.value?.level ?? 0) >= 3);

const formattedArgs = computed(() => {
  try {
    const parsed = JSON.parse(pendingCall.value?.args ?? '{}');
    return JSON.stringify(parsed, null, 2);
  } catch {
    return pendingCall.value?.args ?? '';
  }
});

async function handleConfirm() {
  if (!pendingCall.value) return;
  try {
    await invoke('confirm_tool_call', {
      token:   pendingCall.value.token,
      confirm: true,
    });
  } catch (err) {
    console.error('[ToolConfirm] confirm failed:', err);
  }
  close();
}

async function handleReject() {
  if (!pendingCall.value) return;
  try {
    await invoke('confirm_tool_call', {
      token:   pendingCall.value.token,
      confirm: false,
    });
  } catch (err) {
    console.error('[ToolConfirm] reject failed:', err);
  }
  close();
}

function close() {
  visible.value     = false;
  pendingCall.value = null;
  chatStore.setToolConfirmOpen(false);
}
</script>

<style scoped>
.dialog-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.55);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 9999;
}

.dialog-box {
  background: var(--color-surface, #1e2330);
  border: 1px solid var(--color-border, #2d3448);
  border-radius: 12px;
  width: 380px;
  max-width: 90vw;
  overflow: hidden;
}

/* 高危操作时加红色左边框 */
.level-3 { border-left: 4px solid #ef4444; }
.level-2 { border-left: 4px solid #f59e0b; }

.dialog-header {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 14px 16px 10px;
  border-bottom: 1px solid var(--color-border, #2d3448);
}

.dialog-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--color-text, #e2e8f0);
}

.level-badge {
  font-size: 11px;
  font-weight: 700;
  padding: 2px 8px;
  border-radius: 99px;
}
.badge-green  { background: #16a34a22; color: #4ade80; border: 1px solid #16a34a55; }
.badge-blue   { background: #2563eb22; color: #60a5fa; border: 1px solid #2563eb55; }
.badge-yellow { background: #d9770622; color: #fbbf24; border: 1px solid #d9770655; }
.badge-red    { background: #dc262622; color: #f87171; border: 1px solid #dc262655; }

.dialog-body {
  padding: 14px 16px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.tool-info {
  background: var(--color-bg, #0f1117);
  border-radius: 8px;
  padding: 10px 12px;
}

.tool-name {
  display: block;
  font-size: 13px;
  font-weight: 700;
  color: #a78bfa;
  margin-bottom: 6px;
}

.tool-args {
  font-size: 11px;
  color: var(--color-text-secondary, #94a3b8);
  white-space: pre-wrap;
  word-break: break-all;
  margin: 0;
  max-height: 100px;
  overflow-y: auto;
}

.risk-desc {
  font-size: 12px;
  padding: 8px 12px;
  border-radius: 8px;
  display: flex;
  align-items: flex-start;
  gap: 6px;
  line-height: 1.5;
}
.risk-green  { background: #16a34a11; color: #4ade80; }
.risk-blue   { background: #2563eb11; color: #93c5fd; }
.risk-yellow { background: #d9770611; color: #fcd34d; }
.risk-red    { background: #dc262611; color: #fca5a5; }

.dialog-footer {
  display: flex;
  gap: 10px;
  padding: 12px 16px;
  border-top: 1px solid var(--color-border, #2d3448);
}

.btn-reject,
.btn-confirm {
  flex: 1;
  padding: 8px 0;
  border-radius: 8px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  border: none;
  transition: opacity 0.15s;
}

.btn-reject {
  background: var(--color-bg, #0f1117);
  color: var(--color-text-secondary, #94a3b8);
  border: 1px solid var(--color-border, #2d3448);
}

.btn-confirm { color: #fff; }
.confirm-green  { background: #16a34a; }
.confirm-blue   { background: #2563eb; }
.confirm-yellow { background: #d97706; }
.confirm-red    { background: #dc2626; }

.btn-reject:hover  { opacity: 0.75; }
.btn-confirm:hover { opacity: 0.85; }

/* 过渡动画 */
.dialog-fade-enter-active,
.dialog-fade-leave-active { transition: opacity 0.2s; }
.dialog-fade-enter-from,
.dialog-fade-leave-to    { opacity: 0; }
</style>
