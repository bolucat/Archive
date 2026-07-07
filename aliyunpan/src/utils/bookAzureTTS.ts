import useSettingStore from '../setting/settingstore'

export interface AzureTTSConfig {
  key: string
  region: string
}

export interface AzureVoice {
  name: string
  displayName: string
  localName: string
  locale: string
  gender: string
  voiceType: string
}

const LS_AZURE_TTS_ENABLED = 'bookReader.azureTTSEnabled'
const LS_AZURE_VOICE = 'bookReader.azureVoice'

const ROLE_VOICES: Record<string, AzureVoice[]> = {
  'zh-CN': [
    { name: 'zh-CN-XiaoxiaoNeural', displayName: '晓晓', localName: '晓晓', locale: 'zh-CN', gender: 'Female', voiceType: 'Neural' },
    { name: 'zh-CN-YunxiNeural', displayName: '云希', localName: '云希', locale: 'zh-CN', gender: 'Male', voiceType: 'Neural' },
    { name: 'zh-CN-YunyangNeural', displayName: '云扬', localName: '云扬', locale: 'zh-CN', gender: 'Male', voiceType: 'Neural' },
    { name: 'zh-CN-XiaoyiNeural', displayName: '晓伊', localName: '晓伊', locale: 'zh-CN', gender: 'Female', voiceType: 'Neural' },
    { name: 'zh-CN-YunjianNeural', displayName: '云健', localName: '云健', locale: 'zh-CN', gender: 'Male', voiceType: 'Neural' },
    { name: 'zh-CN-XiaochenNeural', displayName: '晓辰', localName: '晓辰', locale: 'zh-CN', gender: 'Female', voiceType: 'Neural' },
    { name: 'zh-CN-XiaohanNeural', displayName: '晓涵', localName: '晓涵', locale: 'zh-CN', gender: 'Female', voiceType: 'Neural' },
    { name: 'zh-CN-XiaomengNeural', displayName: '晓梦', localName: '晓梦', locale: 'zh-CN', gender: 'Female', voiceType: 'Neural' },
    { name: 'zh-CN-XiamoNeural', displayName: '晓墨', localName: '晓墨', locale: 'zh-CN', gender: 'Female', voiceType: 'Neural' },
    { name: 'zh-CN-XiaoqiuNeural', displayName: '晓秋', localName: '晓秋', locale: 'zh-CN', gender: 'Female', voiceType: 'Neural' },
    { name: 'zh-CN-XiaoruiNeural', displayName: '晓睿', localName: '晓睿', locale: 'zh-CN', gender: 'Female', voiceType: 'Neural' },
    { name: 'zh-CN-XiaoshuangNeural', displayName: '晓双', localName: '晓双', locale: 'zh-CN', gender: 'Female', voiceType: 'Neural' },
    { name: 'zh-CN-XiaoxuanNeural', displayName: '晓萱', localName: '晓萱', locale: 'zh-CN', gender: 'Female', voiceType: 'Neural' },
    { name: 'zh-CN-XiaoyanNeural', displayName: '晓颜', localName: '晓颜', locale: 'zh-CN', gender: 'Female', voiceType: 'Neural' },
    { name: 'zh-CN-XiaoyouNeural', displayName: '晓悠', localName: '晓悠', locale: 'zh-CN', gender: 'Female', voiceType: 'Neural' },
    { name: 'zh-CN-XiaozhenNeural', displayName: '晓甄', localName: '晓甄', locale: 'zh-CN', gender: 'Female', voiceType: 'Neural' },
  ],
  'en-US': [
    { name: 'en-US-JennyNeural', displayName: 'Jenny', localName: 'Jenny', locale: 'en-US', gender: 'Female', voiceType: 'Neural' },
    { name: 'en-US-GuyNeural', displayName: 'Guy', localName: 'Guy', locale: 'en-US', gender: 'Male', voiceType: 'Neural' },
    { name: 'en-US-AriaNeural', displayName: 'Aria', localName: 'Aria', locale: 'en-US', gender: 'Female', voiceType: 'Neural' },
    { name: 'en-US-DavisNeural', displayName: 'Davis', localName: 'Davis', locale: 'en-US', gender: 'Male', voiceType: 'Neural' },
    { name: 'en-US-EmmaNeural', displayName: 'Emma', localName: 'Emma', locale: 'en-US', gender: 'Female', voiceType: 'Neural' },
  ],
}

export function getAzureTTSConfig(): AzureTTSConfig | null {
  const store = useSettingStore()
  if (!store.apiAzureSpeechKey || !store.apiAzureSpeechRegion) return null
  return { key: store.apiAzureSpeechKey, region: store.apiAzureSpeechRegion }
}

export function isAzureTTSEnabled(): boolean {
  return localStorage.getItem(LS_AZURE_TTS_ENABLED) === 'true'
}

export function setAzureTTSEnabled(v: boolean) {
  try { localStorage.setItem(LS_AZURE_TTS_ENABLED, String(v)) } catch {}
}

export function isAzureConfigured(): boolean {
  return !!getAzureTTSConfig()
}

export function getSavedAzureVoice(): string {
  return localStorage.getItem(LS_AZURE_VOICE) || ''
}

export function saveAzureVoice(voiceName: string) {
  try { localStorage.setItem(LS_AZURE_VOICE, voiceName) } catch {}
}

export function getAzureVoices(): AzureVoice[] {
  return Object.values(ROLE_VOICES).flat()
}

export function getAzureVoiceLabel(voice: AzureVoice): string {
  const gender = voice.gender === 'Female' ? '女' : voice.gender === 'Male' ? '男' : ''
  return `${voice.displayName}${gender ? ` · ${gender}` : ''}`
}

// --- Speech Synthesis ---

let sdkImport: any = null

async function getSDK() {
  if (sdkImport) return sdkImport
  sdkImport = await import('microsoft-cognitiveservices-speech-sdk')
  return sdkImport
}

export async function speakWithAzure(
  text: string,
  config: AzureTTSConfig,
  voiceName: string,
  rate: number,
  onStart?: () => void,
  onEnd?: () => void,
  onError?: (err: string) => void,
) {
  try {
    const sdk = await getSDK()
    const speechConfig = sdk.SpeechConfig.fromSubscription(config.key, config.region)
    speechConfig.speechSynthesisVoiceName = voiceName || 'zh-CN-XiaoxiaoNeural'

    const ratePercent = Math.round((rate - 1) * 100)
    const ssml = `<speak version="1.0" xmlns="http://www.w3.org/2001/10/synthesis" xml:lang="zh-CN">
      <voice name="${voiceName || 'zh-CN-XiaoxiaoNeural'}">
        <prosody rate="${ratePercent >= 0 ? '+' : ''}${ratePercent}%">${text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')}</prosody>
      </voice>
    </speak>`

    const audioConfig = sdk.AudioConfig.fromDefaultSpeakerOutput()
    const synthesizer = new sdk.SpeechSynthesizer(speechConfig, audioConfig)
    onStart?.()
    synthesizer.speakSsmlAsync(ssml, () => { synthesizer.close(); onEnd?.() }, (error: any) => { synthesizer.close(); onError?.(error?.toString() || 'Azure TTS 错误') })
  } catch (e: any) {
    onError?.(e?.message || 'Azure TTS 加载失败')
  }
}

let activeAzureSynthesizer: any = null

export function stopAzureSpeech() {
  if (activeAzureSynthesizer) { try { activeAzureSynthesizer.close() } catch {}; activeAzureSynthesizer = null }
}

export async function speakChunkedWithAzure(
  chunks: string[],
  config: AzureTTSConfig,
  voiceName: string,
  rate: number,
  onChunkStart?: (text: string, index: number, total: number) => void,
  onComplete?: () => void,
  onError?: (err: string) => void,
): Promise<void> {
  try {
    const sdk = await getSDK()
    const speechConfig = sdk.SpeechConfig.fromSubscription(config.key, config.region)
    speechConfig.speechSynthesisVoiceName = voiceName || 'zh-CN-XiaoxiaoNeural'
    const audioConfig = sdk.AudioConfig.fromDefaultSpeakerOutput()

    const ratePercent = Math.round((rate - 1) * 100)
    const rateStr = `${ratePercent >= 0 ? '+' : ''}${ratePercent}%`
    let currentIdx = 0
    let stopped = false

    const playNext = () => {
      if (stopped || currentIdx >= chunks.length) { onComplete?.(); return }
      const chunk = chunks[currentIdx]
      onChunkStart?.(chunk, currentIdx, chunks.length)
      const ssml = `<speak version="1.0" xmlns="http://www.w3.org/2001/10/synthesis">
        <voice name="${voiceName}"><prosody rate="${rateStr}">${chunk.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')}</prosody></voice>
      </speak>`
      const synthesizer = new sdk.SpeechSynthesizer(speechConfig, audioConfig)
      activeAzureSynthesizer = synthesizer
      currentIdx++
      synthesizer.speakSsmlAsync(ssml, () => { synthesizer.close(); if (!stopped) playNext() }, (error: any) => { synthesizer.close(); if (!stopped) onError?.(error?.toString() || 'Azure TTS 错误') })
    }
    playNext()
  } catch (e: any) { onError?.(e?.message || 'Azure TTS 加载失败') }
}
