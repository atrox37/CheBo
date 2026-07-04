const EMOTION_TAG_RE = /\[EMOTION:\s*([a-zA-Z_]+)\s*\]/gi

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

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
}

/** 轻量 Markdown 渲染（聊天气泡用） */
export function renderMarkdown(text: string): string {
  const trimmed = text?.trim()
  if (!trimmed) return ''

  let html = escapeHtml(trimmed)
  html = html.replace(/```([\s\S]*?)```/g, (_, code: string) =>
    `<pre><code>${code.trim()}</code></pre>`)
  html = html.replace(/`([^`]+)`/g, '<code>$1</code>')
  html = html.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>')
  html = html.replace(/\*([^*]+)\*/g, '<em>$1</em>')
  html = html.replace(/\[([^\]]+)\]\(([^)]+)\)/g,
    '<a href="$2" target="_blank" rel="noopener noreferrer">$1</a>')
  html = html.replace(/^[-*] (.+)$/gm, '<li>$1</li>')
  html = html.replace(/(<li>.*<\/li>)/gs, '<ul>$1</ul>')
  html = html.replace(/\n/g, '<br>')
  return html
}
