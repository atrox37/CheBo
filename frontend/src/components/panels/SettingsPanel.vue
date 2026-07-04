<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { Volume2, Pin, Palette, Globe, HelpCircle, X, MessageSquare, Database, FolderOpen, RefreshCw, Mic } from 'lucide-vue-next'
import * as tauriService from '@/services/tauriService'
import { useVoiceStore } from '@/utils/speechTiming'
import { STORAGE_KEYS } from '@/utils/storageKeys'
import { invoke } from '@tauri-apps/api/core'

const voiceStore = useVoiceStore()
const voiceKeyInput = ref('')
const voiceSaving = ref(false)

onMounted(() => {
  voiceStore.loadConfig().catch(() => {})
})

async function saveVoice() {
  voiceSaving.value = true
  try {
    await voiceStore.saveConfig({
      tts_enabled: voiceStore.ttsEnabled,
      stt_enabled: voiceStore.sttEnabled,
      tts_voice: voiceStore.ttsVoice,
      tts_model: voiceStore.ttsModel,
      tts_base_url: voiceStore.ttsBaseUrl,
      tts_api_key: voiceKeyInput.value || undefined,
    })
    voiceKeyInput.value = ''
  } finally {
    voiceSaving.value = false
  }
}

const soundOn     = ref(true)
const alwaysOnTop = ref(true)

async function toggleTop(v: boolean) {
  alwaysOnTop.value = v
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window')
    await getCurrentWindow().setAlwaysOnTop(v)
  } catch { /* web preview */ }
}

const toggleItems = computed(() => [
  { id: 'sound', icon: Volume2, title: '声音', sub: '桌宠音效', val: soundOn.value, set: (v: boolean) => { soundOn.value = v } },
  { id: 'top',   icon: Pin,    title: '始终置顶', sub: '保持在最上层', val: alwaysOnTop.value, set: toggleTop },
])

function clearSession() {
  if (confirm('清除本地会话记录，下次重启为新会话？')) {
    localStorage.removeItem(STORAGE_KEYS.sessionId)
    location.reload()
  }
}

/* ── 说明弹窗 ── */
const showGuide = ref(false)

/* ── 聊天历史弹窗 ── */
const showHistory  = ref(false)
const historyLoading = ref(false)

interface HistoryMsg { id: string; role: string; content: string; emotion?: string; created_at: string }
interface DayGroup   { date: string; label: string; messages: HistoryMsg[] }

const historyGroups = ref<DayGroup[]>([])

function _dateLabel(dateStr: string): string {
  const d   = new Date(dateStr)
  const now = new Date()
  const today     = new Date(now.getFullYear(), now.getMonth(), now.getDate())
  const yesterday = new Date(today); yesterday.setDate(today.getDate() - 1)
  const target    = new Date(d.getFullYear(), d.getMonth(), d.getDate())
  if (target.getTime() === today.getTime())     return '今天'
  if (target.getTime() === yesterday.getTime()) return '昨天'
  return `${d.getMonth() + 1}月${d.getDate()}日`
}

async function openHistory() {
  showHistory.value = true
  if (historyGroups.value.length > 0) return
  historyLoading.value = true
  try {
    const msgs = await tauriService.getChatHistory() as HistoryMsg[]

    const map = new Map<string, HistoryMsg[]>()
    for (const m of msgs) {
      const day = m.created_at.slice(0, 10)
      if (!map.has(day)) map.set(day, [])
      map.get(day)!.push(m)
    }
    historyGroups.value = [...map.entries()]
      .sort((a, b) => b[0].localeCompare(a[0]))
      .map(([date, messages]) => ({ date, label: _dateLabel(messages[0].created_at), messages }))
  } catch {
    historyGroups.value = []
  } finally {
    historyLoading.value = false
  }
}

function closeHistory() {
  showHistory.value = false
  historyGroups.value = []
}

/* ── Vault 记忆存档 ── */
interface VaultStats {
  chunk_count: number
  summary_count: number
  vault_path: string
  last_sync_at: string | null
}
const showVault       = ref(false)
const vaultStats      = ref<VaultStats | null>(null)
const vaultSyncing    = ref(false)
const vaultSyncMsg    = ref('')

async function openVault() {
  showVault.value = true
  try {
    vaultStats.value = await invoke<VaultStats>('get_vault_stats')
  } catch { vaultStats.value = null }
}

async function triggerSync() {
  vaultSyncing.value = true
  vaultSyncMsg.value = ''
  try {
    const msg = await invoke<string>('trigger_vault_sync')
    vaultSyncMsg.value = msg
    // 等待后台同步完成事件后刷新统计
    setTimeout(async () => {
      try { vaultStats.value = await invoke<VaultStats>('get_vault_stats') } catch {}
    }, 3000)
  } catch (err) {
    vaultSyncMsg.value = String(err)
  } finally {
    vaultSyncing.value = false
  }
}

function openVaultFolder() {
  invoke('open_vault_folder').catch(() => {})
}
</script>

<template>
  <div class="sp">

    <!-- Toggle 行 -->
    <div v-for="item in toggleItems" :key="item.id" class="s-row">
      <div class="s-left">
        <div class="s-ico-wrap">
          <component v-if="item.icon" :is="item.icon" :size="15" color="#9080a8" />
          <span v-else class="s-star">✦</span>
        </div>
        <div>
          <div class="s-title">{{ item.title }}</div>
          <div class="s-sub">{{ item.sub }}</div>
        </div>
      </div>
      <button class="toggle" :class="{ on: item.val }" @click.stop="item.set(!item.val)">
        <span class="knob" />
      </button>
    </div>

    <div class="divider" />

    <!-- 语音 TTS / STT -->
    <div class="voice-block">
      <div class="voice-hd">
        <Mic :size="15" color="#9080a8" />
        <span>语音（TTS / 听写）</span>
      </div>
      <div class="s-row compact">
        <div class="s-left"><div class="s-title">朗读回复</div><div class="s-sub">助手说完后播放语音</div></div>
        <button class="toggle" :class="{ on: voiceStore.ttsEnabled }" @click.stop="voiceStore.ttsEnabled = !voiceStore.ttsEnabled">
          <span class="knob" />
        </button>
      </div>
      <div class="s-row compact">
        <div class="s-left"><div class="s-title">语音输入</div><div class="s-sub">输入框按住麦克风说话</div></div>
        <button class="toggle" :class="{ on: voiceStore.sttEnabled }" @click.stop="voiceStore.sttEnabled = !voiceStore.sttEnabled">
          <span class="knob" />
        </button>
      </div>
      <div class="voice-fields">
        <label class="vf-label">音色</label>
        <input v-model="voiceStore.ttsVoice" class="vf-input" placeholder="nova" />
        <label class="vf-label">TTS 模型</label>
        <input v-model="voiceStore.ttsModel" class="vf-input" placeholder="tts-1" />
        <label class="vf-label">API Base URL</label>
        <input v-model="voiceStore.ttsBaseUrl" class="vf-input" placeholder="https://api.openai.com/v1" />
        <label class="vf-label">语音 API Key（可空，复用 LLM Key）</label>
        <input v-model="voiceKeyInput" type="password" class="vf-input" :placeholder="voiceStore.hasApiKey ? '已配置，留空不修改' : 'OpenAI 或兼容 TTS 的 Key'" />
        <p class="vf-hint">DeepSeek 等聊天 Key 不能用于 TTS；若留空则尝试复用 LLM Key（仅 OpenAI 可用）。</p>
        <p v-if="voiceStore.lastTtsError" class="vf-error">{{ voiceStore.lastTtsError }}</p>
      </div>
      <button class="voice-save" :disabled="voiceSaving" @click.stop="saveVoice">
        {{ voiceSaving ? '保存中…' : '保存语音设置' }}
      </button>
    </div>

    <div class="divider" />

    <!-- 导航行 -->
    <button class="nav-row" @click.stop>
      <Palette :size="15" color="#9080a8" />
      <div class="s-left2"><div class="s-title">主题色</div><div class="s-sub">粉色 · 樱花</div></div>
      <span class="arrow">›</span>
    </button>
    <button class="nav-row" @click.stop>
      <Globe :size="15" color="#9080a8" />
      <div class="s-left2"><div class="s-title">语言</div><div class="s-sub">简体中文</div></div>
      <span class="arrow">›</span>
    </button>
    <button class="nav-row" @click.stop="openHistory">
      <MessageSquare :size="15" color="#9080a8" />
      <div class="s-left2"><div class="s-title">聊天记录</div><div class="s-sub">按日期浏览历史对话</div></div>
      <span class="arrow">›</span>
    </button>
    <button class="nav-row" @click.stop="openVault">
      <Database :size="15" color="#9080a8" />
      <div class="s-left2"><div class="s-title">记忆存档</div><div class="s-sub">Vault · 双存储长期记忆</div></div>
      <span class="arrow">›</span>
    </button>
    <button class="nav-row" @click.stop="showGuide = true">
      <HelpCircle :size="15" color="#9080a8" />
      <div class="s-left2"><div class="s-title">桌宠说明</div><div class="s-sub">玩法与数值规则</div></div>
      <span class="arrow">›</span>
    </button>

    <div class="divider" />

    <div class="info-row">
      <span class="info-label">连接</span>
      <span class="info-val ok">已连接（本地）</span>
    </div>
    <div class="info-row">
      <span class="info-label">会话</span>
      <span class="info-val mono">{{ tauriService.getSessionId().slice(0, 8) }}…</span>
    </div>
    <button class="clear-btn" @click.stop="clearSession">重置会话</button>
    <div class="version">Chebo v0.1 · Phase 5.5</div>

  </div>

  <!-- 聊天历史弹窗 -->
  <Teleport to="body">
    <transition name="guide">
      <div v-if="showHistory" class="guide-mask" @click.self="closeHistory">
        <div class="guide-card history-card">
          <div class="guide-hd">
            <span class="guide-title">💬 聊天记录</span>
            <button class="guide-close" @click.stop="closeHistory"><X :size="14" /></button>
          </div>
          <div class="guide-body history-body">
            <div v-if="historyLoading" class="hist-empty">加载中…</div>
            <div v-else-if="historyGroups.length === 0" class="hist-empty">暂无聊天记录</div>
            <template v-else>
              <div v-for="group in historyGroups" :key="group.date" class="hist-group">
                <div class="hist-day-label">{{ group.label }}</div>
                <div
                  v-for="m in group.messages" :key="m.id"
                  :class="['hist-msg', m.role]"
                >
                  <div class="hist-bubble">{{ m.content }}</div>
                  <div class="hist-time">{{ m.created_at.slice(11, 16) }}</div>
                </div>
              </div>
            </template>
          </div>
        </div>
      </div>
    </transition>
  </Teleport>

  <!-- 说明弹窗：用 Teleport 挂到 body，覆盖整个透明窗口 -->
  <Teleport to="body">
    <transition name="guide">
      <div v-if="showGuide" class="guide-mask" @click.self="showGuide = false">
        <div class="guide-card">
          <div class="guide-hd">
            <span class="guide-title">🐾 Chebo 桌宠说明</span>
            <button class="guide-close" @click.stop="showGuide = false">
              <X :size="14" />
            </button>
          </div>
          <div class="guide-body">
            <section>
              <h4>📊 核心数值</h4>
              <p><b>饥饱</b> — 每分钟自然降低，降到 0 会影响心情。通过 <em>投喂</em> 补充。</p>
              <p><b>活力</b> — 执行学习 / 工作会消耗。休息或等待自然恢复。</p>
              <p><b>心情</b> — 受饥饿和孤独影响；聊天、摸头等互动可以提升。</p>
              <p><b>亲密</b> — 累计互动后缓慢增加，共分 5 阶段：陌生→熟悉→信任→亲近→默契。</p>
            </section>
            <section>
              <h4>🍞 投喂系统</h4>
              <p>在 <em>投喂</em> 面板直接点击食物即可喂食（从背包消耗或直接花金币购买）。</p>
              <p>在 <em>商店</em> 购买食物存入背包，背包食物可随时喂食不消耗金币。</p>
            </section>
            <section>
              <h4>📚 学习 / 工作</h4>
              <p>在 <em>动作 → 学习/工作</em> 选择任务后 Chebo 开始计时，完成后获得金币和经验。</p>
              <p>饥饱 &lt; 40 或活力 &lt; 25 时无法开始任务。</p>
            </section>
            <section>
              <h4>🌟 升级</h4>
              <p>累计经验值达到 <em>Lv × 100</em> 时自动升级，每级解锁更多任务和商品。</p>
            </section>
            <section>
              <h4>💬 AI 对话</h4>
              <p>双击角色打开聊天框，和 Chebo 正常聊天。Chebo 的性格会随好感阶段而变化。</p>
              <p>Chebo 还会根据当前状态主动发言，气泡显示在她的左侧。</p>
            </section>
            <section>
              <h4>✦ 保持最佳状态</h4>
              <p>开启后饥饱/活力/心情固定满值，适合只想聊天不想养成的用户。</p>
            </section>
          </div>
        </div>
      </div>
    </transition>
  </Teleport>

  <!-- Vault 记忆存档弹窗 -->
  <Teleport to="body">
    <transition name="guide">
      <div v-if="showVault" class="guide-mask" @click.self="showVault = false">
        <div class="guide-box vault-box">
          <button class="guide-close" @click="showVault = false"><X :size="15" /></button>
          <div class="guide-title">
            <Database :size="16" color="#c87090" />
            <span>记忆存档 · Memory Vault</span>
          </div>

          <div class="vault-stats" v-if="vaultStats">
            <div class="vault-stat-item">
              <span class="vault-stat-num">{{ vaultStats.chunk_count }}</span>
              <span class="vault-stat-label">对话片段</span>
            </div>
            <div class="vault-stat-div" />
            <div class="vault-stat-item">
              <span class="vault-stat-num">{{ vaultStats.summary_count }}</span>
              <span class="vault-stat-label">摘要节点</span>
            </div>
            <div class="vault-stat-div" />
            <div class="vault-stat-item">
              <span class="vault-stat-num">{{ vaultStats.last_sync_at ? vaultStats.last_sync_at.slice(0, 10) : '—' }}</span>
              <span class="vault-stat-label">最近同步</span>
            </div>
          </div>
          <div class="vault-no-stats" v-else>暂无统计数据</div>

          <div class="vault-tree">
            <div class="vault-tree-title">层级摘要树</div>
            <div class="vault-tree-row"><span class="vault-lvl l0">L0</span><span class="vault-lvl-desc">Chunks — 原始对话片段（每 10 条一组）</span></div>
            <div class="vault-tree-arrow">↓</div>
            <div class="vault-tree-row"><span class="vault-lvl l1">L1</span><span class="vault-lvl-desc">Daily — 每日对话摘要</span></div>
            <div class="vault-tree-arrow">↓</div>
            <div class="vault-tree-row"><span class="vault-lvl l2">L2</span><span class="vault-lvl-desc">Weekly — 每周综合摘要</span></div>
            <div class="vault-tree-arrow">↓</div>
            <div class="vault-tree-row"><span class="vault-lvl l3">L3</span><span class="vault-lvl-desc">Monthly — 每月长期记忆</span></div>
          </div>

          <div class="vault-path" v-if="vaultStats">
            <span class="vault-path-label">存储路径：</span>
            <span class="vault-path-val">{{ vaultStats.vault_path }}</span>
          </div>

          <div class="vault-sync-msg" v-if="vaultSyncMsg">{{ vaultSyncMsg }}</div>

          <div class="vault-actions">
            <button class="vault-btn primary" @click="triggerSync" :disabled="vaultSyncing">
              <RefreshCw :size="13" :class="{ spinning: vaultSyncing }" />
              {{ vaultSyncing ? '同步中…' : '立即同步' }}
            </button>
            <button class="vault-btn" @click="openVaultFolder">
              <FolderOpen :size="13" />
              打开文件夹
            </button>
          </div>

          <div class="vault-tip">
            可用 <a href="https://obsidian.md" target="_blank" class="vault-link">Obsidian</a>
            打开此目录，查看完整记忆图谱 ✦
          </div>
        </div>
      </div>
    </transition>
  </Teleport>
</template>

<style scoped>
.sp { padding: 6px 16px 14px; display: flex; flex-direction: column; gap: 6px; }

.s-row { display: flex; align-items: center; justify-content: space-between; padding: 4px 0; }
.s-left { display: flex; align-items: center; gap: 10px; flex: 1; }
.s-ico-wrap { width: 18px; display: flex; align-items: center; justify-content: center; flex-shrink: 0; }
.s-star { font-size: 13px; color: #9080a8; }
.s-title { font-size: 12px; font-weight: 600; color: #3a1a2a; }
.s-sub   { font-size: 10px; color: #b0a0b0; }

.toggle {
  width: 36px; height: 20px; border-radius: 10px;
  border: none; cursor: pointer; position: relative;
  background: #e0d8e8; transition: background 0.2s; flex-shrink: 0;
}
.toggle.on { background: #e8729a; }
.knob {
  position: absolute; top: 2px; left: 2px;
  width: 16px; height: 16px; border-radius: 50%;
  background: #fff; box-shadow: 0 1px 4px rgba(0,0,0,0.15);
  transition: left 0.2s;
}
.toggle.on .knob { left: 18px; }

.divider { height: 1px; background: #f0e8f4; margin: 2px 0; }

.voice-block { padding: 2px 0 4px; }
.voice-hd {
  display: flex; align-items: center; gap: 8px;
  font-size: 11px; font-weight: 600; color: #9070a0;
  margin-bottom: 6px;
}
.s-row.compact { padding: 6px 0; }
.voice-fields { display: flex; flex-direction: column; gap: 4px; margin: 8px 0; }
.vf-label { font-size: 10px; color: #9080a8; }
.vf-hint { font-size: 10px; color: #a090b8; line-height: 1.4; margin: 2px 0 0; }
.vf-error { font-size: 10px; color: #c44; line-height: 1.4; margin: 4px 0 0; }
.vf-input {
  width: 100%; box-sizing: border-box;
  border: 1px solid #e8e0f0; border-radius: 8px;
  padding: 6px 8px; font-size: 11px; color: #3a2848;
  background: #faf8fc;
}
.voice-save {
  width: 100%; border: none; border-radius: 10px;
  padding: 8px 0; font-size: 11px; font-weight: 600;
  background: linear-gradient(135deg, #9080d8, #7c6cd8);
  color: #fff; cursor: pointer;
}
.voice-save:disabled { opacity: 0.6; cursor: wait; }

.nav-row {
  display: flex; align-items: center; gap: 10px;
  padding: 5px 0; background: none; border: none;
  cursor: pointer; width: 100%; text-align: left;
}
.nav-row:hover .s-title { color: #c87090; }
.s-left2 { flex: 1; }
.arrow { font-size: 16px; color: #c8b0c8; font-weight: 300; }

.info-row { display: flex; justify-content: space-between; align-items: center; padding: 1px 0; }
.info-label { font-size: 10px; color: #b0a0b0; }
.info-val   { font-size: 10px; font-weight: 600; color: #5a3050; }
.info-val.mono { font-family: monospace; }
.info-val.ok  { color: #38885a; }
.info-val.err { color: #c04040; }

.clear-btn {
  align-self: center; font-size: 10px; color: #c06080;
  background: #fff0f5; border: 1.5px solid #f0c8d8;
  border-radius: 9px; padding: 4px 16px; cursor: pointer;
  margin-top: 2px; transition: background 0.12s;
}
.clear-btn:hover { background: #ffe0ec; }
.version { text-align: center; font-size: 8.5px; color: #c8b8c8; margin-top: 2px; }
</style>

<!-- 弹窗样式（非 scoped，因为 Teleport 出了组件范围） -->
<style>
.guide-mask {
  position: fixed;
  inset: 0;
  background: rgba(40, 10, 30, 0.38);
  backdrop-filter: blur(3px);
  -webkit-backdrop-filter: blur(3px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 9999;
}
.guide-card {
  width: 300px;
  max-height: 80vh;
  background: #fff;
  border-radius: 18px;
  box-shadow: 0 12px 40px rgba(0,0,0,0.18);
  overflow: hidden;
  display: flex;
  flex-direction: column;
}
.guide-hd {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 16px 10px;
  border-bottom: 1px solid #f0e8f4;
  flex-shrink: 0;
}
.guide-title { font-size: 14px; font-weight: 700; color: #3a1a2a; }
.guide-close {
  width: 26px; height: 26px; border-radius: 50%;
  border: none; background: #f5f0f5; color: #a09090;
  cursor: pointer; display: flex; align-items: center; justify-content: center;
  transition: background 0.15s;
}
.guide-close:hover { background: #ece0ec; color: #604050; }
.guide-body {
  padding: 12px 16px 16px;
  overflow-y: auto;
  scrollbar-width: thin;
  scrollbar-color: rgba(200,150,180,0.3) transparent;
}
.guide-body section { margin-bottom: 14px; }
.guide-body h4 { font-size: 12px; font-weight: 700; color: #5a2040; margin-bottom: 5px; }
.guide-body p  { font-size: 11px; color: #6a5060; line-height: 1.7; margin-bottom: 3px; }
.guide-body em { color: #c87090; font-style: normal; font-weight: 600; }
.guide-body b  { color: #3a1a2a; }

.guide-enter-active, .guide-leave-active { transition: opacity 0.22s, transform 0.22s; }
.guide-enter-from, .guide-leave-to { opacity: 0; transform: scale(0.94); }

/* ── 历史记录弹窗扩展 ── */
.history-card { width: 340px; max-height: 85vh; }
.history-body {
  padding: 10px 14px 14px;
  overflow-y: auto;
  scrollbar-width: thin;
  scrollbar-color: rgba(200,150,180,0.3) transparent;
}
.hist-empty {
  text-align: center;
  font-size: 12px;
  color: #b0a0b0;
  padding: 24px 0;
}
.hist-group { margin-bottom: 18px; }
.hist-day-label {
  font-size: 10px;
  font-weight: 700;
  color: #c8a0c0;
  text-align: center;
  margin-bottom: 8px;
  letter-spacing: 0.05em;
}
.hist-msg {
  display: flex;
  flex-direction: column;
  margin-bottom: 6px;
}
.hist-msg.user  { align-items: flex-end; }
.hist-msg.assistant { align-items: flex-start; }
.hist-bubble {
  max-width: 80%;
  padding: 6px 10px;
  border-radius: 12px;
  font-size: 11px;
  line-height: 1.6;
  word-break: break-word;
}
.hist-msg.user .hist-bubble {
  background: linear-gradient(135deg, #f87090, #e060a0);
  color: #fff;
  border-bottom-right-radius: 4px;
}
.hist-msg.assistant .hist-bubble {
  background: #f8f0f8;
  color: #4a2040;
  border-bottom-left-radius: 4px;
}
.hist-time {
  font-size: 9px;
  color: #c0b0c0;
  margin-top: 2px;
  padding: 0 2px;
}

/* ── Vault 弹窗 ── */
.vault-box {
  max-width: 340px;
  padding: 18px 20px 16px;
}
.vault-stats {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0;
  background: #fdf4f9;
  border-radius: 12px;
  padding: 12px 0;
  margin: 12px 0;
}
.vault-stat-item {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 2px;
}
.vault-stat-num {
  font-size: 20px;
  font-weight: 700;
  color: #c87090;
  line-height: 1;
}
.vault-stat-label {
  font-size: 9px;
  color: #b0a0b0;
}
.vault-stat-div {
  width: 1px;
  height: 32px;
  background: #f0d8e8;
}
.vault-no-stats {
  text-align: center;
  font-size: 11px;
  color: #c0b0c0;
  padding: 12px 0;
}
.vault-tree {
  background: #f8f0fa;
  border-radius: 10px;
  padding: 10px 14px;
  margin-bottom: 10px;
}
.vault-tree-title {
  font-size: 10px;
  font-weight: 700;
  color: #a080a8;
  margin-bottom: 6px;
  letter-spacing: 0.04em;
}
.vault-tree-row {
  display: flex;
  align-items: center;
  gap: 8px;
}
.vault-tree-arrow {
  font-size: 10px;
  color: #d0b8d8;
  text-align: center;
  margin: 1px 0 1px 4px;
}
.vault-lvl {
  font-size: 9px;
  font-weight: 700;
  border-radius: 4px;
  padding: 1px 5px;
  min-width: 22px;
  text-align: center;
}
.vault-lvl.l0 { background: #ffe8f0; color: #e06080; }
.vault-lvl.l1 { background: #e8f8e8; color: #40a040; }
.vault-lvl.l2 { background: #e8f0ff; color: #4060c0; }
.vault-lvl.l3 { background: #fff0d8; color: #b07020; }
.vault-lvl-desc {
  font-size: 10px;
  color: #7a5a7a;
}
.vault-path {
  font-size: 9px;
  color: #b0a0b0;
  word-break: break-all;
  margin-bottom: 10px;
  padding: 0 2px;
}
.vault-path-label { color: #9080a0; font-weight: 600; }
.vault-path-val   { font-family: monospace; color: #8070a0; }
.vault-sync-msg {
  font-size: 10px;
  color: #609050;
  background: #f0faf0;
  border-radius: 6px;
  padding: 4px 8px;
  margin-bottom: 8px;
}
.vault-actions {
  display: flex;
  gap: 8px;
  margin-bottom: 10px;
}
.vault-btn {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 5px;
  font-size: 11px;
  font-weight: 600;
  border-radius: 10px;
  padding: 7px 0;
  cursor: pointer;
  border: 1.5px solid #e8d0e0;
  background: #fff;
  color: #7a5a7a;
  transition: background 0.12s;
}
.vault-btn:hover { background: #fdf0f8; }
.vault-btn.primary {
  background: linear-gradient(135deg, #f87090, #e060a0);
  color: #fff;
  border-color: transparent;
}
.vault-btn.primary:hover { filter: brightness(1.05); }
.vault-btn:disabled { opacity: 0.5; cursor: not-allowed; }
.vault-tip {
  font-size: 10px;
  color: #b0a0b0;
  text-align: center;
}
.vault-link {
  color: #c87090;
  text-decoration: underline;
}
.spinning {
  animation: spin 1s linear infinite;
}
@keyframes spin {
  from { transform: rotate(0deg); }
  to   { transform: rotate(360deg); }
}
</style>
