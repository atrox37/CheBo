/**
 * Estimate speech duration (ms) for pet talking animation.
 * TTS can pass measured duration later via beginSpeakHold(text, durationMs).
 */
export function estimateSpeechDurationMs(text: string): number {
  const MIN_MS = 2200
  const MAX_MS = 45000
  const trimmed = text.trim()
  if (!trimmed) return MIN_MS

  let units = 0
  for (const ch of trimmed) {
    units += /[\u4e00-\u9fff\u3400-\u4dbf\uff00-\uffef]/.test(ch) ? 1 : 0.35
  }

  const ms = 600 + units * 260
  return Math.min(MAX_MS, Math.max(MIN_MS, Math.round(ms)))
}

import { ref } from 'vue'
import { defineStore } from 'pinia'
import * as tauriService from '@/services/tauriService'

export const useVoiceStore = defineStore('voice', () => {
  const ttsEnabled  = ref(false)
  const sttEnabled  = ref(false)
  const ttsVoice    = ref('nova')
  const ttsModel    = ref('tts-1')
  const ttsBaseUrl  = ref('https://api.openai.com/v1')
  const hasApiKey   = ref(false)
  const loaded      = ref(false)
  const isPlaying   = ref(false)
  const isRecording = ref(false)
  const lastTtsError = ref<string | null>(null)

  let audioEl: HTMLAudioElement | null = null
  let objectUrl: string | null = null
  let lastTtsErrorAt = 0

  function parseTtsError(err: unknown): string {
    const raw = String(err)
    if (raw.includes('401') || raw.includes('invalid_api_key') || raw.includes('Incorrect API key')) {
      return 'TTS needs OpenAI API key. DeepSeek chat keys cannot be used for TTS; set a separate TTS key in Settings > Voice.'
    }
    if (raw.includes('403')) return 'TTS access denied. Check API key permissions or balance.'
    if (raw.includes('429')) return 'TTS rate limited. Try again later.'
    return 'Speech synthesis failed. Check TTS settings.'
  }

  function revokeUrl() {
    if (objectUrl) {
      URL.revokeObjectURL(objectUrl)
      objectUrl = null
    }
  }

  async function loadConfig() {
    const cfg = await tauriService.getVoiceConfig()
    ttsEnabled.value  = cfg.tts_enabled
    sttEnabled.value  = cfg.stt_enabled
    ttsVoice.value    = cfg.tts_voice
    ttsModel.value    = cfg.tts_model
    ttsBaseUrl.value  = cfg.tts_base_url
    hasApiKey.value   = cfg.has_tts_api_key
    loaded.value      = true
  }

  async function saveConfig(patch: tauriService.VoiceUpdatePayload) {
    await tauriService.updateVoiceConfig(patch)
    await loadConfig()
  }

  function stopPlayback() {
    if (audioEl) {
      audioEl.pause()
      audioEl.currentTime = 0
      audioEl = null
    }
    revokeUrl()
    isPlaying.value = false
  }

  async function speak(text: string): Promise<number> {
    const trimmed = text.trim()
    if (!trimmed || !ttsEnabled.value) return 0

    stopPlayback()
    lastTtsError.value = null
    try {
      const b64 = await tauriService.synthesizeSpeech(trimmed)
      const binary = atob(b64)
      const bytes = new Uint8Array(binary.length)
      for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i)
      const blob = new Blob([bytes], { type: 'audio/mpeg' })
      objectUrl = URL.createObjectURL(blob)

      const durationMs = await new Promise<number>((resolve, reject) => {
        const audio = new Audio(objectUrl!)
        audioEl = audio
        isPlaying.value = true
        let settled = false
        const finish = () => {
          isPlaying.value = false
          stopPlayback()
        }
        audio.onloadedmetadata = () => {
          if (!settled) {
            settled = true
            resolve(Math.round((audio.duration || 3) * 1000))
          }
        }
        audio.onended = finish
        audio.onerror = () => {
          finish()
          reject(new Error('audio playback failed'))
        }
        audio.play().catch((e) => {
          finish()
          reject(e)
        })
        setTimeout(() => {
          if (!settled) {
            settled = true
            resolve(3000)
          }
        }, 800)
      })

      return durationMs
    } catch (err) {
      const msg = parseTtsError(err)
      lastTtsError.value = msg
      const now = Date.now()
      if (now - lastTtsErrorAt > 15000) {
        lastTtsErrorAt = now
        console.warn('[Voice] TTS failed:', msg)
      }
      isPlaying.value = false
      stopPlayback()
      return 0
    }
  }

  return {
    ttsEnabled, sttEnabled, ttsVoice, ttsModel, ttsBaseUrl, hasApiKey,
    loaded, isPlaying, isRecording, lastTtsError,
    loadConfig, saveConfig, speak, stopPlayback,
  }
})