import { ref } from 'vue'
import { useVoiceStore } from '@/utils/speechTiming'
import * as tauriService from '@/services/tauriService'

export function useVoiceInput() {
  const voiceStore = useVoiceStore()
  const error = ref<string | null>(null)
  let mediaRecorder: MediaRecorder | null = null
  let chunks: Blob[] = []

  async function startRecording(): Promise<boolean> {
    error.value = null
    if (!voiceStore.sttEnabled) {
      error.value = '请先在设置中启用语音输入'
      return false
    }
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true })
      const mime = MediaRecorder.isTypeSupported('audio/webm;codecs=opus')
        ? 'audio/webm;codecs=opus' : 'audio/webm'
      chunks = []
      mediaRecorder = new MediaRecorder(stream, { mimeType: mime })
      mediaRecorder.ondataavailable = (e) => { if (e.data.size > 0) chunks.push(e.data) }
      mediaRecorder.start()
      voiceStore.isRecording = true
      return true
    } catch (err) {
      error.value = '无法访问麦克风'
      return false
    }
  }

  async function stopRecording(): Promise<string> {
    if (!mediaRecorder || mediaRecorder.state === 'inactive') {
      voiceStore.isRecording = false
      return ''
    }
    const recorder = mediaRecorder
    const mime = recorder.mimeType || 'audio/webm'
    const blob = await new Promise<Blob>((resolve) => {
      recorder.onstop = () => resolve(new Blob(chunks, { type: mime }))
      recorder.stop()
      recorder.stream.getTracks().forEach((t) => t.stop())
    })
    mediaRecorder = null
    chunks = []
    voiceStore.isRecording = false
    if (blob.size === 0) return ''
    try {
      const b64 = await blobToBase64(blob)
      return await tauriService.transcribeAudio(b64, mime)
    } catch (err) {
      error.value = String(err)
      return ''
    }
  }
  return { error, startRecording, stopRecording }
}

function blobToBase64(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => {
      const data = reader.result as string
      const idx = data.indexOf(',')
      resolve(idx >= 0 ? data.slice(idx + 1) : data)
    }
    reader.onerror = reject
    reader.readAsDataURL(blob)
  })
}

export async function speakAssistantReply(text: string): Promise<void> {
  const voiceStore = useVoiceStore()
  if (!voiceStore.loaded) await voiceStore.loadConfig().catch(() => {})
  if (!voiceStore.ttsEnabled) return
  const { useChatStore } = await import('@/stores/chat')
  const chat = useChatStore()
  const durationMs = await voiceStore.speak(text)
  if (durationMs > 0) chat.beginSpeakHold(text, durationMs)
}
