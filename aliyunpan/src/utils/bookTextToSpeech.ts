interface SpeechHost {
  speechSynthesis?: {
    cancel: () => void
    getVoices?: () => any[]
    pause?: () => void
    paused?: boolean
    resume?: () => void
    speak: (utterance: any) => void
  }
  SpeechSynthesisUtterance?: new (text: string) => any
}

export interface SpeechSession {
  getCurrentIndex: () => number
  getTotal: () => number
  isPaused: () => boolean
  next: () => boolean
  pause: () => boolean
  previous: () => boolean
  resume: () => boolean
  stop: () => void
  updateOptions: (options: SpeechSequenceOptions) => void
}

export interface SpeechSequenceOptions {
  onChunkStart?: (text: string, index: number, total: number) => void
  onComplete?: () => void
  onError?: (error: unknown) => void
  lang?: string
  voiceURI?: string
  voiceName?: string
  rate?: number
  volume?: number
  maxLength?: number
  combineSentences?: boolean
}

export const SPEECH_SPEED_VALUES = [0.5, 0.75, 1, 1.25, 1.5, 1.75, 2, 3, 4, 5, 6, 7, 8] as const

const CJK_RE = /[\u4e00-\u9fff\u3040-\u309f\u30a0-\u30ff\uac00-\ud7af]+/g

function detectLanguage(text: string): 'cjk' | 'other' {
  let cjk = 0
  let total = 0
  for (const match of text.matchAll(CJK_RE)) {
    cjk += match[0].length
  }
  total = text.replace(/\s/g, '').length || 1
  return cjk / total > 0.3 ? 'cjk' : 'other'
}

export function buildSpeechText(parts: string[]): string {
  return parts.join(' ').replace(/\s+/g, ' ').trim()
}

export function splitSpeechText(text: string, maxLength?: number, combineSentences = true): string[] {
  const normalized = buildSpeechText([text])
  if (!normalized) return []
  
  let sentences: string[]
  try {
    const segmenter = new (Intl as any).Segmenter(detectLanguage(normalized) === 'cjk' ? 'zh-CN' : 'en', { granularity: 'sentence' })
    sentences = Array.from(segmenter.segment(normalized)).map((s: any) => s.segment.trim()).filter(Boolean)
  } catch {
    sentences = normalized.match(/[^.!?。！？；;]+[.!?。！？；;]*/g) || [normalized]
  }
  
  const lang = detectLanguage(normalized)
  const limit = maxLength ?? (lang === 'cjk' ? 50 : 150)
  
  const chunks: string[] = []
  let current = ''
  for (const sentence of sentences) {
    if (!combineSentences && current) {
      chunks.push(current)
      current = sentence
    } else if (!current) {
      current = sentence
    } else if ((current + ' ' + sentence).length <= limit) {
      current += ' ' + sentence
    } else {
      // Try splitting by comma/semicolon
      const parts = sentence.split(/(?<=[,，;；:：、…])/).filter(Boolean)
      if (parts.length > 1 && (current + ' ' + parts[0]).length <= limit) {
        current += ' ' + parts[0]
        chunks.push(current)
        current = parts.slice(1).join('').trim()
      } else {
        chunks.push(current)
        current = sentence
      }
    }
    while (current.length > limit) {
      chunks.push(current.slice(0, limit).trim())
      current = current.slice(limit).trim()
    }
  }
  if (current) chunks.push(current)
  return chunks
}

export function buildSpeechStartText(selectionText: string, sentenceText = '', visibleText = ''): string {
  const fallback = buildSpeechText([sentenceText || selectionText])
  const source = buildSpeechText([visibleText])
  if (!source || !fallback) return fallback
  const sentence = buildSpeechText([sentenceText])
  const selection = buildSpeechText([selectionText])
  const anchor = sentence && source.includes(sentence) ? sentence : selection
  if (!anchor) return fallback
  const index = source.indexOf(anchor)
  return index >= 0 ? source.slice(index).trim() : fallback
}

function getSpeechHost(host?: SpeechHost): SpeechHost | undefined {
  if (host) return host
  return typeof window !== 'undefined' ? window : undefined
}

export function hasSpeechSupport(host = getSpeechHost()): boolean {
  return !!host?.speechSynthesis && typeof host.SpeechSynthesisUtterance === 'function'
}

export function canSpeakText(text: string, supported = hasSpeechSupport()): boolean {
  return !!text.trim() && supported
}

export function speakText(text: string, host = getSpeechHost()) {
  const speechText = buildSpeechText([text])
  const Utterance = host?.SpeechSynthesisUtterance
  if (!host?.speechSynthesis || typeof Utterance !== 'function' || !canSpeakText(speechText, true)) return false
  host.speechSynthesis.cancel()
  const utterance = new Utterance(speechText)
  host.speechSynthesis.speak(utterance)
  return true
}

function clampSpeechNumber(value: unknown, fallback: number, min: number, max: number): number {
  const numberValue = Number(value)
  if (!Number.isFinite(numberValue)) return fallback
  return Math.max(min, Math.min(max, numberValue))
}

function applySpeechOptions(utterance: any, options: SpeechSequenceOptions, host: SpeechHost) {
  if (options.lang) utterance.lang = options.lang
  if ((options.voiceURI || options.voiceName) && typeof host.speechSynthesis?.getVoices === 'function') {
    const voices = host.speechSynthesis.getVoices()
    const voice = voices.find((item: any) => options.voiceURI && item?.voiceURI === options.voiceURI)
      || voices.find((item: any) => options.voiceName && options.lang && item?.name === options.voiceName && item?.lang === options.lang)
      || voices.find((item: any) => options.voiceName && item?.name === options.voiceName)
    if (voice) utterance.voice = voice
  }
  utterance.rate = clampSpeechNumber(options.rate, 1, 0.5, 8)
  utterance.volume = clampSpeechNumber(options.volume, 1, 0, 1)
}

export function speakTextSequence(
  text: string,
  options: SpeechSequenceOptions = {},
  host = getSpeechHost()
): SpeechSession | null {
  let chunks = splitSpeechText(text, options.maxLength, options.combineSentences)
  const Utterance = host?.SpeechSynthesisUtterance
  if (!host?.speechSynthesis || typeof Utterance !== 'function' || !chunks.length) return null
  let stopped = false
  let paused = false
  let index = 0
  let currentIndex = -1
  let utteranceRunId = 0
  let currentOptions = { ...options }
  
  const speakNext = () => {
    if (stopped) return
    paused = false
    const chunk = chunks[index++]
    if (!chunk) {
      stopped = true
      paused = false
      currentOptions.onComplete?.()
      return
    }
    currentIndex = index - 1
    currentOptions.onChunkStart?.(chunk, currentIndex, chunks.length)
    const utterance = new Utterance(chunk)
    const runId = ++utteranceRunId
    applySpeechOptions(utterance, currentOptions, host)
    utterance.onend = () => {
      if (runId === utteranceRunId) speakNext()
    }
    utterance.onerror = (event: unknown) => {
      if (stopped || runId !== utteranceRunId) return
      stopped = true
      paused = false
      currentOptions.onError?.(event)
    }
    host.speechSynthesis?.speak(utterance)
  }
  const speakFrom = (targetIndex: number) => {
    if (stopped) return false
    utteranceRunId++
    host.speechSynthesis?.cancel()
    index = Math.max(0, Math.min(chunks.length, targetIndex))
    speakNext()
    return true
  }
  host.speechSynthesis.cancel()
  speakNext()
  return {
    getCurrentIndex: () => currentIndex,
    getTotal: () => chunks.length,
    isPaused: () => paused,
    next: () => speakFrom(currentIndex + 1),
    pause: () => {
      if (stopped || paused || typeof host.speechSynthesis?.pause !== 'function') return false
      host.speechSynthesis.pause()
      paused = true
      return true
    },
    previous: () => speakFrom(currentIndex - 1),
    resume: () => {
      if (stopped || !paused || typeof host.speechSynthesis?.resume !== 'function') return false
      host.speechSynthesis.resume()
      paused = false
      return true
    },
    stop: () => {
      stopped = true
      paused = false
      host.speechSynthesis?.cancel()
    },
    updateOptions: (newOptions: SpeechSequenceOptions) => {
      currentOptions = { ...currentOptions, ...newOptions }
      // Re-chunk if maxLength or combineSentences changed
      if (newOptions.maxLength !== undefined || newOptions.combineSentences !== undefined) {
        chunks = splitSpeechText(text, currentOptions.maxLength, currentOptions.combineSentences)
      }
      speakFrom(currentIndex + 1)
    }
  }
}

export function stopSpeaking(session?: SpeechSession | null) {
  if (session) {
    session.stop()
    return
  }
  if (hasSpeechSupport()) window.speechSynthesis.cancel()
}
