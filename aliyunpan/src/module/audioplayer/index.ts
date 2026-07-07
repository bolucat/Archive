/**
 * AudioContext-based player engine with effects chain.
 * Adapted from lx-music-desktop src/renderer/plugins/player/index.ts
 *
 * Signal chain: source -> analyser -> 10-band EQ -> pitchShifter -> panner -> gain -> destination
 */
import { reactive, watch } from 'vue'

// ---- Constants ----

export const freqs = [31, 62, 125, 250, 500, 1000, 2000, 4000, 8000, 16000] as const
type FreqBand = (typeof freqs)[number]

export const freqsPreset = [
  { name: 'pop', hz31: 6, hz62: 5, hz125: -3, hz250: -2, hz500: 5, hz1000: 4, hz2000: -4, hz4000: -3, hz8000: 6, hz16000: 4 },
  { name: 'dance', hz31: 4, hz62: 3, hz125: -4, hz250: -6, hz500: 0, hz1000: 0, hz2000: 3, hz4000: 4, hz8000: 4, hz16000: 5 },
  { name: 'rock', hz31: 7, hz62: 6, hz125: 2, hz250: 1, hz500: -3, hz1000: -4, hz2000: 2, hz4000: 1, hz8000: 4, hz16000: 5 },
  { name: 'classical', hz31: 6, hz62: 7, hz125: 1, hz250: 2, hz500: -1, hz1000: 1, hz2000: -4, hz4000: -6, hz8000: -7, hz16000: -8 },
  { name: 'vocal', hz31: -5, hz62: -6, hz125: -4, hz250: -3, hz500: 3, hz1000: 4, hz2000: 5, hz4000: 4, hz8000: -3, hz16000: -3 },
  { name: 'slow', hz31: 5, hz62: 4, hz125: 2, hz250: 0, hz500: -2, hz1000: 0, hz2000: 3, hz4000: 6, hz8000: 7, hz16000: 8 },
  { name: 'electronic', hz31: 6, hz62: 5, hz125: 0, hz250: -5, hz500: -4, hz1000: 0, hz2000: 6, hz4000: 8, hz8000: 8, hz16000: 7 },
  { name: 'subwoofer', hz31: 8, hz62: 7, hz125: 5, hz250: 4, hz500: 0, hz1000: 0, hz2000: 0, hz4000: 0, hz8000: 0, hz16000: 0 },
  { name: 'soft', hz31: -5, hz62: -5, hz125: -4, hz250: -4, hz500: 3, hz1000: 2, hz2000: 4, hz4000: 4, hz8000: 0, hz16000: 0 },
] as const

export type FreqPreset = typeof freqsPreset[number] | { name: string } & Record<`hz${FreqBand}`, number>

// ---- Effect Settings (reactive) ----

export interface SoundEffectSettings {
  eq: Record<`hz${FreqBand}`, number>
  panner: {
    enabled: boolean
    soundR: number
    speed: number
  }
  pitchShifter: {
    playbackRate: number
  }
}

export const defaultSettings: SoundEffectSettings = {
  eq: { hz31: 0, hz62: 0, hz125: 0, hz250: 0, hz500: 0, hz1000: 0, hz2000: 0, hz4000: 0, hz8000: 0, hz16000: 0 },
  panner: { enabled: false, soundR: 5, speed: 25 },
  pitchShifter: { playbackRate: 1.0 },
}

// ---- Internal State ----

let audio: HTMLAudioElement | null = null
let audioContext: AudioContext | null = null
let mediaSource: MediaElementAudioSourceNode | null = null
let analyser: AnalyserNode | null = null
let biquads: Map<string, BiquadFilterNode> | null = null
let gainNode: GainNode | null = null
let panner: PannerNode | null = null
let pitchShifterNode: AudioWorkletNode | null = null
let pitchShifterNodePitchFactor: AudioParam | null = null
let pitchShifterNodeLoadStatus: 'none' | 'loading' | 'unconnect' | 'connected' = 'none'
let pitchShifterNodeTempValue = 1

let pannerTimer: ReturnType<typeof setInterval> | null = null
let pannerRad = 0

// ---- Public API ----

export const effectSettings = reactive<SoundEffectSettings>({
  ...defaultSettings,
  eq: { ...defaultSettings.eq },
  panner: { ...defaultSettings.panner },
  pitchShifter: { ...defaultSettings.pitchShifter },
})

/**
 * Bind an existing HTMLAudioElement to the AudioContext effects chain.
 * Must be called after the audio element exists in DOM.
 */
export function bindAudio(audioEl: HTMLAudioElement) {
  if (audio === audioEl) return
  audio = audioEl
  audioContext = null
  mediaSource = null
}

function initAdvancedAudioFeatures() {
  if (!audio) throw new Error('bindAudio() must be called first')
  if (audioContext) return

  audioContext = new AudioContext({ latencyHint: 'playback' })

  // Analyser
  analyser = audioContext.createAnalyser()
  analyser.fftSize = 256

  // 10-band EQ
  biquads = new Map()
  for (const freq of freqs) {
    const filter = audioContext.createBiquadFilter()
    biquads.set(`hz${freq}`, filter)
    filter.type = 'peaking'
    filter.frequency.value = freq
    filter.Q.value = 1.4
    filter.gain.value = effectSettings.eq[`hz${freq}`] ?? 0
  }
  for (let i = 1; i < freqs.length; i++) {
    biquads.get(`hz${freqs[i - 1]}`)!.connect(biquads.get(`hz${freqs[i]}`)!)
  }

  // Panner
  panner = audioContext.createPanner()

  // Gain
  gainNode = audioContext.createGain()

  // Connect chain: source -> analyser -> EQ -> panner -> gain -> destination
  mediaSource = audioContext.createMediaElementSource(audio)
  mediaSource.connect(analyser)
  analyser.connect(biquads.get(`hz${freqs[0]}`)!)
  const lastBiquadFilter = biquads.get(`hz${freqs[freqs.length - 1]}`)!
  lastBiquadFilter.connect(panner)
  panner.connect(gainNode)
  gainNode.connect(audioContext.destination)

  // Apply initial EQ settings
  applyAllEq()

  // Resume AudioContext on user interaction
  audio.addEventListener('playing', () => {
    if (audioContext?.state === 'suspended') {
      audioContext.resume().catch(console.error)
    }
  })
}

// ---- EQ ----

export function getBiquadFilters() {
  initAdvancedAudioFeatures()
  return biquads!
}

function applyAllEq() {
  if (!biquads) return
  for (const freq of freqs) {
    const filter = biquads.get(`hz${freq}`)
    if (filter) filter.gain.value = effectSettings.eq[`hz${freq}`] ?? 0
  }
}

export function setEqBand(freq: FreqBand, gain: number) {
  effectSettings.eq[`hz${freq}`] = gain
  if (biquads) {
    const filter = biquads.get(`hz${freq}`)
    if (filter) filter.gain.setTargetAtTime(gain, audioContext!.currentTime, 0.02)
  }
}

export function setEqPreset(preset: FreqPreset) {
  for (const freq of freqs) {
    const key = `hz${freq}` as const
    effectSettings.eq[key as keyof typeof effectSettings.eq] = (preset as any)[key] ?? 0
  }
  applyAllEq()
}

export function resetEq() {
  for (const freq of freqs) {
    effectSettings.eq[`hz${freq}`] = 0
  }
  applyAllEq()
}

// ---- Panner ----

function applyPanner() {
  if (!panner) return
  if (effectSettings.panner.enabled) {
    startPannerRotation()
  } else {
    stopPannerRotation()
  }
}

function startPannerRotation() {
  stopPannerRotation()
  pannerRad = 0
  const speed = effectSettings.panner.speed
  const soundR = effectSettings.panner.soundR
  pannerTimer = setInterval(() => {
    pannerRad += 1
    if (pannerRad > 360) pannerRad -= 360
    const r = soundR * 0.1
    if (panner) {
      panner.positionX.value = Math.sin(pannerRad * Math.PI / 180) * r
      panner.positionY.value = Math.cos(pannerRad * Math.PI / 180) * r
      panner.positionZ.value = Math.cos(pannerRad * Math.PI / 180) * r
    }
  }, speed * 10)
}

function stopPannerRotation() {
  if (pannerTimer) {
    clearInterval(pannerTimer)
    pannerTimer = null
  }
  if (panner) {
    panner.positionX.value = 0
    panner.positionY.value = 0
    panner.positionZ.value = 0
  }
}

export function setPannerEnabled(enabled: boolean) {
  initAdvancedAudioFeatures()
  effectSettings.panner.enabled = enabled
  applyPanner()
}

export function setPannerSoundR(soundR: number) {
  effectSettings.panner.soundR = soundR
}

export function setPannerSpeed(speed: number) {
  effectSettings.panner.speed = speed
  if (effectSettings.panner.enabled) applyPanner()
}

// ---- Pitch Shifter ----

function connectPitchShifterNode() {
  if (!audio || !biquads || !pitchShifterNode) return
  const lastBiquad = biquads.get(`hz${freqs[freqs.length - 1]}`)!
  lastBiquad.disconnect()
  lastBiquad.connect(pitchShifterNode)
  pitchShifterNode.connect(panner!)
  pitchShifterNodeLoadStatus = 'connected'
  pitchShifterNodePitchFactor!.value = pitchShifterNodeTempValue
}

function disconnectPitchShifterNode() {
  if (!biquads || !pitchShifterNode) return
  const lastBiquad = biquads.get(`hz${freqs[freqs.length - 1]}`)!
  lastBiquad.disconnect()
  lastBiquad.connect(panner!)
  pitchShifterNodeLoadStatus = 'unconnect'
}

async function loadPitchShifterNode() {
  pitchShifterNodeLoadStatus = 'loading'
  initAdvancedAudioFeatures()

  try {
    await audioContext!.audioWorklet.addModule('/pitch-shifter.worklet.js')
    pitchShifterNode = new AudioWorkletNode(audioContext!, 'phase-vocoder-processor', {
      outputChannelCount: [2],
    })
    const param = pitchShifterNode.parameters.get('pitchFactor')
    if (!param) return
    pitchShifterNodePitchFactor = param
    pitchShifterNodeLoadStatus = 'unconnect'
    if (pitchShifterNodeTempValue !== 1) {
      connectPitchShifterNode()
    }
  } catch (e) {
    console.error('Failed to load pitch shifter worklet:', e)
    pitchShifterNodeLoadStatus = 'none'
  }
}

export function setPitchShifter(playbackRate: number) {
  effectSettings.pitchShifter.playbackRate = playbackRate
  // Convert playback rate to pitch factor: 1.0 = no change
  pitchShifterNodeTempValue = playbackRate

  switch (pitchShifterNodeLoadStatus) {
    case 'loading':
      break
    case 'none':
      loadPitchShifterNode()
      break
    case 'connected':
      pitchShifterNodePitchFactor!.value = playbackRate
      break
    case 'unconnect':
      connectPitchShifterNode()
      break
  }
}

export function setPitchShifterPlaybackRate(rate: number) {
  setPitchShifter(rate)
}

// ---- Volume / Mute (via audio element) ----

export function setGainVolume(vol: number) {
  if (audio) audio.volume = vol
}

export function setGainMuted(muted: boolean) {
  if (audio) audio.muted = muted
}

// ---- Audio Element Proxy ----

export function getAudioElement() {
  return audio
}

export function getCurrentTime() {
  return audio?.currentTime ?? 0
}

export function setCurrentTime(time: number) {
  if (audio) audio.currentTime = time
}

export function getDuration() {
  return audio?.duration ?? 0
}

// ---- Watchers: react to settings changes ----

function setupWatchers() {
  // Watch EQ changes
  watch(
    () => ({ ...effectSettings.eq }),
    () => {
      if (biquads) applyAllEq()
    },
    { deep: true }
  )

  // Watch panner enabled
  watch(
    () => effectSettings.panner.enabled,
    () => applyPanner()
  )

  // Watch pitch shifter
  watch(
    () => effectSettings.pitchShifter.playbackRate,
    (val) => setPitchShifter(val)
  )
}

// Auto-setup after module is used
let watchersInitialized = false
export function ensureInit() {
  if (!watchersInitialized) {
    setupWatchers()
    watchersInitialized = true
  }
}
