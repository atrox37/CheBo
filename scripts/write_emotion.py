from pathlib import Path
emotion = r'''const EMOTION_TAG_RE = /\[EMOTION:\s*([a-zA-Z_]+)\s*\]/gi

const PARTIAL_EMOTION_SUFFIX = /\[(?:E(?:M(?:O(?:T(?:I(?:O(?:N(?::[\w]*)?)?)?)?)?)?)?)?$/

const VALID_EMOTIONS = new Set([
  'normal', 'happy', 'proud', 'shy', 'angry', 'sad', 'surprised',
])

export function normalizeEmotion(raw: string): string {
  const lower = raw.trim().toLowerCase()
  return VALID_EMOTIONS.has(lower) ? lower : 'normal'
}

export function extractEmotionFromText(text: string): { clean: string; emotion: string | null } {
  let emotion: string | null = null
  const clean = text
    .replace(EMOTION_TAG_RE, (_, raw: string) => {
      emotion = normalizeEmotion(raw)
      return ''
    })
    .replace(/\s+$/g, '')
  return { clean, emotion }
}

export function stripEmotionForDisplay(raw: string): { display: string; emotion: string | null } {
  const { clean, emotion } = extractEmotionFromText(raw)
  const display = clean.replace(PARTIAL_EMOTION_SUFFIX, '').replace(/\s+$/g, '')
  return { display, emotion }
}
'''
Path("frontend/src/utils/emotionTag.ts").write_text(emotion, encoding="utf-8", newline="\n")
print("emotionTag ok")
