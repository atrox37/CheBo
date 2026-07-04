<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { Plus, Play, Pause, X, RotateCcw, CheckCircle2, Clock, AlertCircle, Loader2, Zap } from 'lucide-vue-next'

// ─── 类型定义（与 Rust 对齐） ─────────────────────────────────────────────────

interface TaskStep {
  id:              string
  task_id:         string
  step_index:      number
  title:           string
  description:     string
  status:          string
  tool_name:       string | null
  result:          string | null
  error:           string | null
  requires_confirm: boolean
}

interface AgentTask {
  id:               string
  title:            string
  goal:             string
  status:           string
  steps:            TaskStep[]
  current_step:     number
  created_at:       number
  updated_at:       number
  retry_count:      number
  result_summary:   string | null
  error_message:    string | null
}

interface TaskSummary {
  id:            string
  title:         string
  status:        string
  status_label:  string
  total_steps:   number
  done_steps:    number
  progress:      number
  created_at:    number
  updated_at:    number
}

// ─── 活动流类型 ───────────────────────────────────────────────────────────────

interface ActivityEntry {
  ts:      string
  kind:    'started' | 'thinking' | 'finished' | 'failed' | 'confirm'
  message: string
  taskId:  string
}

// ─── 状态 ──────────────────────────────────────────────────────────────────────

const tasks       = ref<TaskSummary[]>([])
const selectedId  = ref<string | null>(null)
const detail      = ref<AgentTask | null>(null)
const creating    = ref(false)
const goalInput   = ref('')
const showCreate  = ref(false)
const unlisten    = ref<UnlistenFn[]>([])

// 实时活动流（全局，不限于单任务）
const activityFeed = ref<ActivityEntry[]>([])

function pushActivity(kind: ActivityEntry['kind'], message: string, taskId = '') {
  const now = new Date()
  const ts  = `${now.getHours().toString().padStart(2,'0')}:${now.getMinutes().toString().padStart(2,'0')}:${now.getSeconds().toString().padStart(2,'0')}`
  activityFeed.value.unshift({ ts, kind, message, taskId })
  if (activityFeed.value.length > 60) activityFeed.value.pop()
}

// ─── 计算属性 ─────────────────────────────────────────────────────────────────

const sortedTasks = computed(() =>
  [...tasks.value].sort((a, b) => b.created_at - a.created_at)
)

const activeTasks = computed(() =>
  sortedTasks.value.filter(t => !['completed','cancelled'].includes(t.status))
)

const doneTasks = computed(() =>
  sortedTasks.value.filter(t => ['completed','cancelled'].includes(t.status))
)

// ─── 生命周期 ─────────────────────────────────────────────────────────────────

onMounted(async () => {
  await fetchTasks()
  await setupListeners()
})

onUnmounted(() => {
  unlisten.value.forEach(fn => fn())
})

// ─── 事件监听 ─────────────────────────────────────────────────────────────────

async function setupListeners() {
  unlisten.value.push(
    await listen('task_created', (e: any) => {
      fetchTasks()
      pushActivity('started', `任务创建：${e.payload.title}`, e.payload.task_id)
    }),
    await listen('task_updated', async (e: any) => {
      await fetchTasks()
      if (selectedId.value === e.payload.task_id) {
        await fetchDetail(e.payload.task_id)
      }
    }),
    await listen('task_step_started', (e: any) => {
      const p = e.payload
      pushActivity('started', `▶ 步骤 ${p.step_index + 1}：${p.title}`, p.task_id)
    }),
    await listen('task_step_thinking', (e: any) => {
      const p = e.payload
      pushActivity('thinking', `💭 ${p.hint}`, p.task_id)
    }),
    await listen('task_step_finished', (e: any) => {
      const p = e.payload
      if (p.status === 'success') {
        pushActivity('finished', `✓ ${p.title}${p.result ? '：' + p.result.slice(0, 60) : ''}`, p.task_id)
      } else {
        pushActivity('failed', `✗ ${p.title}：${p.error || '失败'}`, p.task_id)
      }
      if (selectedId.value === p.task_id) fetchDetail(p.task_id)
    }),
    await listen('task_waiting_confirm', (e: any) => {
      const p = e.payload
      pushActivity('confirm', `⚠ 需要确认：${p.title}`, p.task_id)
    }),
    await listen('task_completed', (e: any) => {
      fetchTasks()
      pushActivity('finished', `🎉 任务完成：${e.payload.title}`, e.payload.task_id)
    }),
    await listen('task_failed', (e: any) => {
      fetchTasks()
      pushActivity('failed', `❌ 任务失败：${e.payload.title}：${e.payload.error_message || ''}`, e.payload.task_id)
    }),
  )
}

// ─── 数据获取 ─────────────────────────────────────────────────────────────────

async function fetchTasks() {
  try {
    tasks.value = await invoke<TaskSummary[]>('task_list')
  } catch (err) {
    console.error('task_list 失败:', err)
  }
}

async function fetchDetail(id: string) {
  try {
    detail.value = await invoke<AgentTask>('task_detail', { taskId: id })
  } catch {
    detail.value = null
  }
}

async function selectTask(id: string) {
  selectedId.value = id
  await fetchDetail(id)
}

// ─── 任务操作 ─────────────────────────────────────────────────────────────────

async function createTask() {
  if (!goalInput.value.trim()) return
  creating.value = true
  try {
    await invoke('task_create', { goal: goalInput.value.trim(), sessionId: null })
    goalInput.value = ''
    showCreate.value = false
    await fetchTasks()
  } catch (err) {
    console.error('创建任务失败:', err)
  } finally {
    creating.value = false
  }
}

async function pauseTask(id: string) {
  try { await invoke('task_pause', { taskId: id }); await fetchTasks() } catch {}
}

async function resumeTask(id: string) {
  try { await invoke('task_resume', { taskId: id }); await fetchTasks() } catch {}
}

async function cancelTask(id: string) {
  try { await invoke('task_cancel_agent', { taskId: id }); await fetchTasks() } catch {}
}

async function retryTask(id: string) {
  try { await invoke('task_retry', { taskId: id }); await fetchTasks() } catch {}
}

async function approveStep(taskId: string, stepId: string, approved: boolean) {
  try {
    await invoke('task_approve_step', { taskId, stepId, approved })
    await fetchDetail(taskId)
    await fetchTasks()
  } catch {}
}

// ─── 工具函数 ─────────────────────────────────────────────────────────────────

function statusColor(s: string) {
  switch (s) {
    case 'running':        return '#2b9a6a'
    case 'completed':      return '#2b9a6a'
    case 'planning':       return '#7c6cd8'
    case 'waiting_confirm': return '#d08040'
    case 'paused':         return '#8090a8'
    case 'failed':         return '#d04040'
    case 'cancelled':      return '#8090a8'
    default:               return '#8090a8'
  }
}

function statusLabel(s: string) {
  const map: Record<string, string> = {
    created:         '已创建', planning:       '规划中',
    waiting_confirm: '等待确认', running:       '执行中',
    paused:          '已暂停', interrupted:    '已中断',
    failed:          '失败', completed:        '已完成',
    cancelled:       '已取消',
  }
  return map[s] ?? s
}

function stepIcon(s: string) {
  switch (s) {
    case 'success':          return CheckCircle2
    case 'running':          return Loader2
    case 'waiting_confirm':  return Clock
    case 'failed':           return AlertCircle
    default:                 return null
  }
}

function stepIconColor(s: string) {
  switch (s) {
    case 'success':          return '#2b9a6a'
    case 'running':          return '#7c6cd8'
    case 'waiting_confirm':  return '#d08040'
    case 'failed':           return '#d04040'
    default:                 return '#8090a8'
  }
}

function fmtTime(ts: number) {
  return new Date(ts * 1000).toLocaleString('zh-CN', {
    month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit'
  })
}

function canPause(s: string)  { return s === 'running' || s === 'planning' }
function canResume(s: string) { return s === 'paused' || s === 'interrupted' }
function canCancel(s: string) { return !['completed','cancelled','failed'].includes(s) }
function canRetry(s: string)  { return s === 'failed' }
</script>

<template>
  <div class="task-panel">

    <!-- Header -->
    <div class="panel-header">
      <span class="panel-title">长期任务</span>
      <button class="btn-create" @click="showCreate = !showCreate" title="新建任务">
        <Plus :size="14" />
        新建
      </button>
    </div>

    <!-- 新建任务输入框 -->
    <transition name="create-box">
      <div v-if="showCreate" class="create-box">
        <textarea
          v-model="goalInput"
          class="goal-input"
          placeholder="描述你的目标，例如：整理下载文件夹，按类型分类"
          rows="3"
          @keydown.ctrl.enter="createTask"
        />
        <div class="create-actions">
          <button class="btn-sm btn-ghost" @click="showCreate = false">取消</button>
          <button
            class="btn-sm btn-primary"
            :disabled="creating || !goalInput.trim()"
            @click="createTask"
          >
            <Loader2 v-if="creating" :size="12" class="spin" />
            {{ creating ? '规划中…' : '开始任务 (Ctrl+↵)' }}
          </button>
        </div>
      </div>
    </transition>

    <!-- 任务列表 -->
    <div class="task-list">

      <!-- 进行中 -->
      <template v-if="activeTasks.length">
        <div class="section-label">进行中 ({{ activeTasks.length }})</div>
        <div
          v-for="t in activeTasks"
          :key="t.id"
          class="task-card"
          :class="{ selected: selectedId === t.id }"
          @click="selectTask(t.id)"
        >
          <div class="task-top">
            <span class="task-title">{{ t.title }}</span>
            <span class="status-pill" :style="{ color: statusColor(t.status) }">
              {{ statusLabel(t.status) }}
            </span>
          </div>
          <div v-if="t.total_steps > 0" class="progress-wrap">
            <div class="progress-bar">
              <div class="progress-fill" :style="{ width: `${t.progress * 100}%`, background: statusColor(t.status) }" />
            </div>
            <span class="progress-text">{{ t.done_steps }}/{{ t.total_steps }}</span>
          </div>
          <!-- 快捷操作 -->
          <div class="task-actions" @click.stop>
            <button v-if="canPause(t.status)"  class="act-btn" title="暂停" @click="pauseTask(t.id)">
              <Pause :size="12" />
            </button>
            <button v-if="canResume(t.status)" class="act-btn" title="继续" @click="resumeTask(t.id)">
              <Play :size="12" />
            </button>
            <button v-if="canRetry(t.status)"  class="act-btn" title="重试" @click="retryTask(t.id)">
              <RotateCcw :size="12" />
            </button>
            <button v-if="canCancel(t.status)" class="act-btn danger" title="取消" @click="cancelTask(t.id)">
              <X :size="12" />
            </button>
          </div>
        </div>
      </template>

      <!-- 已完成/已取消 -->
      <template v-if="doneTasks.length">
        <div class="section-label muted">历史 ({{ doneTasks.length }})</div>
        <div
          v-for="t in doneTasks"
          :key="t.id"
          class="task-card done"
          :class="{ selected: selectedId === t.id }"
          @click="selectTask(t.id)"
        >
          <div class="task-top">
            <span class="task-title muted">{{ t.title }}</span>
            <span class="status-pill" :style="{ color: statusColor(t.status) }">
              {{ statusLabel(t.status) }}
            </span>
          </div>
          <div class="time-hint">{{ fmtTime(t.updated_at) }}</div>
        </div>
      </template>

      <!-- 空状态 -->
      <div v-if="!tasks.length" class="empty-hint">
        <div class="empty-icon">📋</div>
        <div>暂无任务</div>
        <div class="empty-sub">点击「新建」把一个长期目标交给 Chebo</div>
      </div>
    </div>

    <!-- 实时活动流 -->
    <div v-if="activityFeed.length" class="activity-feed">
      <div class="feed-header">
        <Zap :size="11" style="color:#7c6cd8" />
        <span>实时进度</span>
      </div>
      <div
        v-for="(item, i) in activityFeed"
        :key="i"
        class="feed-entry"
        :class="item.kind"
        @click="item.taskId && selectTask(item.taskId)"
      >
        <span class="feed-ts">{{ item.ts }}</span>
        <span class="feed-msg">{{ item.message }}</span>
      </div>
    </div>

    <!-- 任务详情 -->
    <transition name="detail-slide">
      <div v-if="detail" class="task-detail">
        <div class="detail-header">
          <span class="detail-title">{{ detail.title }}</span>
          <button class="close-btn" @click="detail = null; selectedId = null">
            <X :size="14" />
          </button>
        </div>

        <div class="detail-goal">{{ detail.goal }}</div>

        <!-- 步骤列表 -->
        <div class="steps-list">
          <div
            v-for="(step, idx) in detail.steps"
            :key="step.id"
            class="step-item"
            :class="step.status"
          >
            <div class="step-left">
              <div class="step-num" :style="{ color: stepIconColor(step.status) }">
                <component
                  :is="stepIcon(step.status)"
                  v-if="stepIcon(step.status)"
                  :size="14"
                  :class="{ spin: step.status === 'running' }"
                />
                <span v-else>{{ idx + 1 }}</span>
              </div>
              <div class="step-body">
                <div class="step-title" :class="{ muted: step.status === 'pending' }">
                  {{ step.title }}
                </div>
                <div v-if="step.description" class="step-desc">{{ step.description }}</div>
                <div v-if="step.result" class="step-result">{{ step.result.slice(0, 120) }}{{ step.result.length > 120 ? '…' : '' }}</div>
                <div v-if="step.error"  class="step-error">{{ step.error }}</div>
              </div>
            </div>

            <!-- 步骤确认操作 -->
            <div v-if="step.status === 'waiting_confirm'" class="confirm-row">
              <div class="confirm-hint">需要你确认才能继续</div>
              <div class="confirm-btns">
                <button class="btn-sm btn-ghost" @click="approveStep(detail!.id, step.id, false)">跳过</button>
                <button class="btn-sm btn-primary" @click="approveStep(detail!.id, step.id, true)">确认执行</button>
              </div>
            </div>
          </div>
        </div>

        <!-- 完成总结 -->
        <div v-if="detail.result_summary" class="result-summary">
          <div class="summary-label">完成总结</div>
          {{ detail.result_summary }}
        </div>

        <!-- 错误信息 -->
        <div v-if="detail.error_message" class="error-msg">
          {{ detail.error_message }}
        </div>

        <!-- 详情底部操作 -->
        <div class="detail-footer">
          <button v-if="canPause(detail.status)"  class="btn-sm btn-ghost" @click="pauseTask(detail.id)">
            <Pause :size="12" /> 暂停
          </button>
          <button v-if="canResume(detail.status)" class="btn-sm btn-primary" @click="resumeTask(detail.id)">
            <Play :size="12" /> 继续
          </button>
          <button v-if="canRetry(detail.status)"  class="btn-sm btn-primary" @click="retryTask(detail.id)">
            <RotateCcw :size="12" /> 重试
          </button>
          <button v-if="canCancel(detail.status)" class="btn-sm btn-danger" @click="cancelTask(detail.id)">
            <X :size="12" /> 取消任务
          </button>
        </div>
      </div>
    </transition>

  </div>
</template>

<style scoped>
.task-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
  font-size: 12px;
}

.panel-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px 6px;
  flex-shrink: 0;
}

.panel-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-primary, #1a1a2e);
}

.btn-create {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 4px 10px;
  border-radius: 6px;
  border: 1px solid #2b9a6a;
  background: #e6f5ef;
  color: #2b9a6a;
  font-size: 11px;
  font-weight: 600;
  cursor: pointer;
  transition: background .15s;
}
.btn-create:hover { background: #d0eddf; }

.create-box {
  margin: 0 10px 8px;
  padding: 10px;
  border-radius: 8px;
  border: 1px solid #d0e8da;
  background: #f4fbf7;
}

.goal-input {
  width: 100%;
  box-sizing: border-box;
  resize: none;
  border: 1px solid #c8ddd4;
  border-radius: 6px;
  padding: 6px 8px;
  font-size: 11.5px;
  color: #333;
  background: #fff;
  outline: none;
  font-family: inherit;
}
.goal-input:focus { border-color: #2b9a6a; }

.create-actions {
  display: flex;
  justify-content: flex-end;
  gap: 6px;
  margin-top: 6px;
}

.btn-sm {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 4px 10px;
  border-radius: 5px;
  font-size: 11px;
  font-weight: 500;
  cursor: pointer;
  border: none;
  transition: opacity .15s;
}
.btn-sm:disabled { opacity: .5; cursor: not-allowed; }
.btn-ghost   { background: #eee; color: #555; }
.btn-ghost:hover:not(:disabled) { background: #ddd; }
.btn-primary { background: #2b9a6a; color: #fff; }
.btn-primary:hover:not(:disabled) { background: #247d57; }
.btn-danger  { background: #d04040; color: #fff; }
.btn-danger:hover:not(:disabled) { background: #b03030; }

.task-list {
  flex: 1;
  overflow-y: auto;
  padding: 0 8px;
}

.section-label {
  font-size: 10px;
  font-weight: 600;
  color: #2b9a6a;
  text-transform: uppercase;
  letter-spacing: .5px;
  padding: 6px 4px 2px;
}
.section-label.muted { color: #a0a0b0; }

.task-card {
  padding: 8px 10px;
  border-radius: 8px;
  border: 1px solid #e8e8ee;
  background: #fff;
  margin-bottom: 6px;
  cursor: pointer;
  transition: border-color .15s, background .15s;
}
.task-card:hover { border-color: #c8d8cc; background: #fafffe; }
.task-card.selected { border-color: #2b9a6a; background: #f0fbf5; }
.task-card.done { opacity: .75; }

.task-top {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 6px;
  margin-bottom: 4px;
}

.task-title {
  font-size: 12px;
  font-weight: 500;
  color: #222;
  flex: 1;
  line-height: 1.4;
}
.task-title.muted { color: #888; }

.status-pill {
  font-size: 10px;
  font-weight: 600;
  flex-shrink: 0;
}

.progress-wrap {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 4px;
}

.progress-bar {
  flex: 1;
  height: 3px;
  background: #eee;
  border-radius: 2px;
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  border-radius: 2px;
  transition: width .3s;
}

.progress-text {
  font-size: 10px;
  color: #888;
  flex-shrink: 0;
}

.time-hint {
  font-size: 10px;
  color: #aaa;
}

.task-actions {
  display: flex;
  gap: 4px;
  margin-top: 4px;
}

.act-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  border-radius: 4px;
  border: 1px solid #ddd;
  background: #f5f5f5;
  color: #555;
  cursor: pointer;
  transition: background .15s;
}
.act-btn:hover { background: #e8e8e8; }
.act-btn.danger { color: #d04040; border-color: #f5c0c0; background: #fff0f0; }
.act-btn.danger:hover { background: #ffe0e0; }

.empty-hint {
  text-align: center;
  padding: 24px 12px;
  color: #aaa;
}
.empty-icon { font-size: 28px; margin-bottom: 6px; }
.empty-sub  { font-size: 10px; margin-top: 4px; color: #bbb; }

/* ── 任务详情 ── */
.task-detail {
  border-top: 1px solid #eee;
  background: #fafafa;
  max-height: 55%;
  overflow-y: auto;
  flex-shrink: 0;
  padding: 10px 12px;
}

.detail-header {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  margin-bottom: 4px;
}

.detail-title {
  font-size: 13px;
  font-weight: 600;
  color: #1a1a2e;
  flex: 1;
}

.close-btn {
  background: none;
  border: none;
  color: #aaa;
  cursor: pointer;
  padding: 2px;
  display: flex;
  align-items: center;
}
.close-btn:hover { color: #555; }

.detail-goal {
  font-size: 11px;
  color: #666;
  margin-bottom: 10px;
  line-height: 1.5;
}

.steps-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin-bottom: 10px;
}

.step-item {
  display: flex;
  flex-direction: column;
  padding: 6px 8px;
  border-radius: 6px;
  border: 1px solid transparent;
  background: transparent;
}

.step-item.running        { background: #f0eeff; border-color: #d8d0f8; }
.step-item.waiting_confirm { background: #fff8ec; border-color: #f0d8a0; }
.step-item.failed         { background: #fff0f0; border-color: #f8c0c0; }

.step-left {
  display: flex;
  align-items: flex-start;
  gap: 8px;
}

.step-num {
  width: 18px;
  height: 18px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  font-size: 10px;
  font-weight: 700;
  color: #aaa;
}

.step-body { flex: 1; }

.step-title {
  font-size: 11.5px;
  font-weight: 500;
  color: #222;
  line-height: 1.4;
}
.step-title.muted { color: #aaa; }

.step-desc {
  font-size: 10.5px;
  color: #888;
  margin-top: 2px;
  line-height: 1.4;
}

.step-result {
  font-size: 10.5px;
  color: #2b7a50;
  background: #edf7f2;
  border-radius: 4px;
  padding: 3px 6px;
  margin-top: 4px;
}

.step-error {
  font-size: 10.5px;
  color: #c03030;
  margin-top: 2px;
}

.confirm-row {
  margin-top: 6px;
  padding: 6px 0 2px;
}

.confirm-hint {
  font-size: 10.5px;
  color: #d08040;
  margin-bottom: 5px;
  font-weight: 500;
}

.confirm-btns {
  display: flex;
  gap: 6px;
}

.result-summary {
  background: #edf7f2;
  border: 1px solid #b8e0cc;
  border-radius: 6px;
  padding: 8px 10px;
  font-size: 11px;
  color: #2b7a50;
  margin-bottom: 8px;
  line-height: 1.5;
}

.summary-label {
  font-weight: 600;
  font-size: 10px;
  margin-bottom: 4px;
  color: #2b9a6a;
  text-transform: uppercase;
  letter-spacing: .5px;
}

.error-msg {
  background: #fff0f0;
  border: 1px solid #f8c0c0;
  border-radius: 6px;
  padding: 6px 10px;
  font-size: 11px;
  color: #c03030;
  margin-bottom: 8px;
}

.detail-footer {
  display: flex;
  gap: 6px;
  flex-wrap: wrap;
}

/* ── 实时活动流 ── */
.activity-feed {
  border-top: 1px solid #eee;
  max-height: 140px;
  overflow-y: auto;
  flex-shrink: 0;
  padding: 4px 0;
}

.feed-header {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 4px 12px 2px;
  font-size: 10px;
  font-weight: 600;
  color: #7c6cd8;
  text-transform: uppercase;
  letter-spacing: .4px;
}

.feed-entry {
  display: flex;
  align-items: baseline;
  gap: 6px;
  padding: 2px 12px;
  cursor: pointer;
  transition: background .1s;
}
.feed-entry:hover { background: #f5f0ff; }

.feed-ts {
  font-size: 9.5px;
  color: #bbb;
  flex-shrink: 0;
  font-variant-numeric: tabular-nums;
}

.feed-msg {
  font-size: 10.5px;
  color: #444;
  line-height: 1.4;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.feed-entry.thinking .feed-msg { color: #7c6cd8; font-style: italic; }
.feed-entry.finished .feed-msg { color: #2b9a6a; }
.feed-entry.failed   .feed-msg { color: #d04040; }
.feed-entry.confirm  .feed-msg { color: #d08040; font-weight: 500; }

/* ── 动画 ── */
.create-box-enter-active,
.create-box-leave-active { transition: all .2s ease; }
.create-box-enter-from,
.create-box-leave-to     { opacity: 0; transform: translateY(-6px); }

.detail-slide-enter-active,
.detail-slide-leave-active { transition: all .2s ease; }
.detail-slide-enter-from,
.detail-slide-leave-to     { opacity: 0; transform: translateY(10px); }

.spin {
  animation: spin .8s linear infinite;
}
@keyframes spin {
  from { transform: rotate(0deg); }
  to   { transform: rotate(360deg); }
}
</style>
