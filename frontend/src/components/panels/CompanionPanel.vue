<script setup lang="ts">
import { computed, onMounted } from 'vue'
import CheboAvatar from '@/components/CheboAvatar.vue'
import { CHEBO_NAME } from '@/config/chebo'
import { useChatStore } from '@/stores/chat'
import { usePetStore } from '@/stores/pet'
import type { AgentStateType } from '@/services/tauriService'

const chat = useChatStore()
const pet = usePetStore()

const AGENT_LABELS: Record<AgentStateType, string> = {
  idle: '待机陪伴',
  thinking: '思考中',
  talking: '说话中',
  working: '处理任务',
  sleeping: '休息中',
  observing: '观察环境',
  waitingConfirm: '等你确认',
  executingTool: '执行工具',
  interrupted: '被打断',
  errorRecover: '恢复中',
}

const agentLabel = computed(() => AGENT_LABELS[chat.agentState] ?? chat.agentState)
const rapport = computed(() => Math.min(100, Math.round(pet.affection)))

onMounted(() => { pet.fetchStatus().catch(() => {}) })
</script>

<template>
  <div class="companion">
    <section class="card hero">
      <CheboAvatar :size="56" ring />
      <div class="hero-text">
        <div class="hero-name">{{ CHEBO_NAME }}</div>
        <div class="hero-sub">你的桌面 AI 伙伴</div>
      </div>
    </section>
    <section class="card">
      <div class="card-title">当前状态</div>
      <div class="state-row">
        <span class="dot" :class="chat.agentState" />
        <span class="state-text">{{ agentLabel }}</span>
      </div>
      <p class="hint">立绘与气泡会随 AI 状态变化，不再依赖喂食或任务面板。</p>
    </section>
    <section class="card">
      <div class="card-title">默契</div>
      <div class="bar-wrap"><div class="bar-fill" :style="{ width: rapport + '%' }" /></div>
      <div class="bar-meta"><span>{{ rapport }} / 100</span><span class="muted">长期对话自然累积</span></div>
    </section>
    <section class="card muted-card">
      <div class="card-title">桌宠能做什么</div>
      <ul>
        <li>聊天、提醒、简单问答</li>
        <li>轻量工具（读文件、搜索、记忆召回）</li>
        <li>危险操作会弹出确认</li>
        <li>复杂任务请点右上角展开助手模式</li>
      </ul>
    </section>
  </div>
</template>

<style scoped>
.companion { display: flex; flex-direction: column; gap: 10px; font-size: 12px; color: #4a4058; }
.hero { display: flex; align-items: center; gap: 12px; }
.hero-text { min-width: 0; }
.hero-name { font-size: 16px; font-weight: 700; color: #3a2848; }
.hero-sub { font-size: 11px; color: #907898; margin-top: 2px; }
.card { background: #fff; border-radius: 10px; padding: 10px 12px; border: 1px solid #f0e8f4; }
.muted-card { background: #faf8fc; }
.card-title { font-weight: 600; font-size: 11px; color: #9070a0; margin-bottom: 6px; }
.state-row { display: flex; align-items: center; gap: 8px; }
.dot { width: 8px; height: 8px; border-radius: 50%; background: #c0b0c8; }
.dot.thinking, .dot.executingTool, .dot.working { background: #7090e0; animation: pulse 1.2s infinite; }
.dot.talking { background: #e07090; }
.dot.waitingConfirm { background: #d0a030; }
.dot.errorRecover { background: #d05050; }
.dot.sleeping { background: #9090a8; }
.dot.observing { background: #60a880; }
.state-text { font-size: 13px; font-weight: 600; }
.hint { margin: 6px 0 0; line-height: 1.5; color: #887898; font-size: 11px; }
.bar-wrap { height: 6px; background: #f0e8f4; border-radius: 3px; overflow: hidden; }
.bar-fill { height: 100%; background: linear-gradient(90deg, #e06080, #f0a0b8); border-radius: 3px; transition: width 0.4s; }
.bar-meta { display: flex; justify-content: space-between; margin-top: 4px; font-size: 11px; }
.muted { color: #a090a8; }
ul { margin: 0; padding-left: 16px; line-height: 1.6; color: #686078; }
@keyframes pulse { 0%,100% { opacity: 1; } 50% { opacity: 0.45; } }
</style>
