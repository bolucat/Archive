import type { IPageMusicTrack } from '../../store/appstore'
import { getCachedBeatMap, setCachedBeatMap, type CachedBeatMap, type MineradioBeatEvent } from './BeatMapCache'
import { clamp01, clampRange, makeBiquad, median, percentile, runBiquad } from './beatUtils'

export interface BeatAnalysisResult extends CachedBeatMap {
  cacheHit: boolean
}

function bandAt(arr: Float32Array, idx: number) {
  idx = Math.max(0, Math.min(arr.length - 1, idx | 0))
  const a = arr[Math.max(0, idx - 1)] || 0
  const b = arr[idx] || 0
  const c = arr[Math.min(arr.length - 1, idx + 1)] || 0
  return (a + b * 2 + c) * 0.25
}

function buildBandEnergy(data: Float32Array, sampleRate: number, hop: number) {
  const frames = Math.ceil(data.length / hop)
  const lowEnergy = new Float32Array(frames)
  const hitEnergy = new Float32Array(frames)
  const lowHp = makeBiquad('highpass', 38, 0.72, sampleRate)
  const lowLp = makeBiquad('lowpass', 165, 0.72, sampleRate)
  const hitHp = makeBiquad('highpass', 165, 0.72, sampleRate)
  const hitLp = makeBiquad('lowpass', 2600, 0.72, sampleRate)

  for (let frame = 0; frame < frames; frame += 1) {
    const start = frame * hop
    const end = Math.min(data.length, start + hop)
    let low = 0
    let hit = 0
    for (let i = start; i < end; i += 1) {
      const x = data[i] || 0
      const l = runBiquad(lowLp, runBiquad(lowHp, x))
      const h = runBiquad(hitLp, runBiquad(hitHp, x))
      low += l * l
      hit += h * h
    }
    const len = Math.max(1, end - start)
    lowEnergy[frame] = Math.sqrt(low / len)
    hitEnergy[frame] = Math.sqrt(hit / len)
  }
  return { lowEnergy, hitEnergy }
}

function estimateStep(candidates: Array<{ time: number; power: number }>) {
  if (candidates.length < 3) return 0
  const bin = 0.006
  const hist = new Map<number, number>()
  const gaps: number[] = []
  for (let ai = 0; ai < candidates.length; ai += 1) {
    for (let bi = ai + 1; bi < candidates.length && bi < ai + 10; bi += 1) {
      const rawGap = candidates[bi].time - candidates[ai].time
      if (rawGap < 0.24) continue
      if (rawGap > 2.55) break
      for (let div = 1; div <= 6; div += 1) {
        const gap = rawGap / div
        if (gap < 0.31) break
        if (gap > 0.86) continue
        const weight = Math.sqrt(Math.max(0.001, candidates[ai].power * candidates[bi].power)) / Math.sqrt((bi - ai) * div)
        const key = Math.round(gap / bin)
        hist.set(key, (hist.get(key) || 0) + weight)
        gaps.push(gap)
      }
    }
  }
  let bestKey: number | null = null
  let bestScore = 0
  for (const [key, scoreRaw] of hist.entries()) {
    const score = scoreRaw + (hist.get(key - 1) || 0) * 0.72 + (hist.get(key + 1) || 0) * 0.72
    if (score > bestScore) {
      bestScore = score
      bestKey = key
    }
  }
  return bestKey == null ? median(gaps) : bestKey * bin
}

function makeBeatEvents(candidates: Array<{ time: number; power: number; lowTone: number; raw: number }>, floor: number, step: number, mode: 'mr' | 'dj') {
  const beats: MineradioBeatEvent[] = []
  const strongRef = Math.max(floor + 0.001, percentile(candidates.map((c) => c.power), 0.92))
  for (const cand of candidates) {
    if (cand.power < floor && cand.lowTone < 0.62) continue
    const slot = beats.length % 4
    let combo: MineradioBeatEvent['combo'] = slot === 0 ? 'downbeat' : slot === 1 ? 'push' : slot === 2 ? 'drop' : 'rebound'
    const visualRel = clamp01((cand.power - floor) / Math.max(0.001, strongRef - floor))
    const low = clampRange(0.42 + cand.lowTone * 0.22 + (combo === 'downbeat' ? 0.06 : 0), 0.36, 1)
    const body = clampRange(0.18 + visualRel * 0.34, 0.12, 0.92)
    const snap = clampRange(0.12 + Math.min(1, cand.raw * 80) * 0.28, 0.08, 0.9)
    if (visualRel > 0.84 && combo !== 'downbeat') combo = 'accent'
    const comboLift = combo === 'downbeat' ? 0.1 : combo === 'drop' ? 0.05 : combo === 'accent' ? 0.075 : 0
    const impact = clamp01(visualRel * (mode === 'dj' ? 0.78 : 0.70) + low * 0.18 + comboLift)
    const mass = clamp01(low * 0.72 + body * 0.36 + impact * 0.2)
    const sharpness = clamp01(snap * 0.72 + visualRel * 0.22)
    const camera = impact >= (mode === 'dj' ? 0.105 : 0.13) || combo === 'downbeat'
    beats.push({
      time: Number(cand.time.toFixed(3)),
      strength: clamp01(visualRel),
      confidence: clamp01(0.54 + visualRel * 0.36 + (step > 0 ? 0.08 : 0)),
      impact,
      combo,
      low,
      body,
      snap,
      mass,
      sharpness,
      pulse: impact > 0.16 || combo === 'downbeat',
      camera,
      primary: combo === 'downbeat' || impact > 0.62,
      index: beats.length
    })
  }
  return beats
}

function analyzeChannel(data: Float32Array, sampleRate: number, mode: 'mr' | 'dj') {
  const hop = Math.max(512, Math.floor(sampleRate * (mode === 'dj' ? 0.028 : 0.034)))
  const hopSec = hop / sampleRate
  const { lowEnergy, hitEnergy } = buildBandEnergy(data, sampleRate, hop)
  const nFrames = Math.min(lowEnergy.length, hitEnergy.length)
  if (nFrames < 20) return { bpm: 0, peaks: 0, peakTimes: [], beats: [], pulseBeats: [], cameraBeats: [] }

  const lowFloor = Math.max(0.0004, percentile(lowEnergy, 0.22))
  const lowMid = Math.max(lowFloor + 0.0002, percentile(lowEnergy, mode === 'dj' ? 0.54 : 0.58))
  const lowRef = Math.max(lowMid + 0.0002, percentile(lowEnergy, mode === 'dj' ? 0.82 : 0.86))
  const lowCeil = Math.max(lowRef + 0.0004, percentile(lowEnergy, 0.96))
  const hitRef = Math.max(0.0004, percentile(hitEnergy, mode === 'dj' ? 0.82 : 0.86))

  const onset = new Float32Array(nFrames)
  for (let i = 4; i < nFrames; i += 1) {
    const prev = lowEnergy[i - 1] * 0.62 + lowEnergy[i - 2] * 0.28 + lowEnergy[i - 3] * 0.10
    const lowRise = Math.max(0, lowEnergy[i] - prev)
    const wideRise = Math.max(0, (lowEnergy[i] + lowEnergy[i - 1]) * 0.5 - (lowEnergy[i - 3] + lowEnergy[i - 4]) * 0.5)
    const peakRise = Math.max(0, hitEnergy[i] - hitEnergy[i - 2] * 0.84)
    onset[i] = lowRise * (mode === 'dj' ? 1.9 : 1.72) + wideRise * (mode === 'dj' ? 0.96 : 0.86) + peakRise * (mode === 'dj' ? 0.16 : 0.10)
  }

  const winN = Math.max(32, Math.round((mode === 'dj' ? 0.72 : 0.82) / hopSec))
  const minFrameGap = Math.max(8, Math.round((mode === 'dj' ? 0.205 : 0.235) / hopSec))
  const candidates: Array<{ frame: number; time: number; power: number; lowTone: number; raw: number }> = []
  let sumO = 0
  let sqO = 0
  for (let i = 0; i < winN && i < nFrames; i += 1) {
    const o = onset[i] || 0
    sumO += o
    sqO += o * o
  }
  for (let f = winN + 4; f < nFrames - 4; f += 1) {
    const mean = sumO / winN
    const std = Math.sqrt(Math.max(0, sqO / winN - mean * mean))
    const th = mean + std * (mode === 'dj' ? 1.48 : 1.66) + lowRef * 0.0038
    const o = onset[f]
    if (o > th && o >= onset[f - 1] && o > onset[f + 1]) {
      let peakF = f
      let peakScore = o + lowEnergy[f] * 0.10
      for (let pf = f - 2; pf <= f + 3; pf += 1) {
        const score = (onset[pf] || 0) + (lowEnergy[pf] || 0) * 0.10
        if (score > peakScore) {
          peakScore = score
          peakF = pf
        }
      }
      const lowTone = Math.min(2.6, bandAt(lowEnergy, peakF) / lowRef)
      const hitTone = Math.min(2.6, bandAt(hitEnergy, peakF) / hitRef)
      const lowRel = clamp01((bandAt(lowEnergy, peakF) - lowFloor) / Math.max(0.0001, lowCeil - lowFloor))
      const score = (o - th) / Math.max(0.0006, std + mean * 0.38 + lowRef * 0.012)
      if (score > 0.14 && (lowTone > 0.32 || lowRel > 0.22 || hitTone > 0.52)) {
        const power = score * 0.56 + Math.pow(clamp01((lowTone - 0.22) / 1.42), 0.82) * 0.34 + Math.min(1.5, hitTone) * 0.08 + lowRel * 0.10
        const cand = { frame: peakF, time: peakF * hopSec, power, lowTone, raw: o }
        const last = candidates[candidates.length - 1]
        if (last && cand.frame - last.frame < minFrameGap) {
          if (cand.power > last.power) candidates[candidates.length - 1] = cand
        } else {
          candidates.push(cand)
        }
      }
    }
    const old = onset[f - winN] || 0
    const next = onset[f] || 0
    sumO += next - old
    sqO += next * next - old * old
  }

  if (!candidates.length) return { bpm: 0, peaks: 0, peakTimes: [], beats: [], pulseBeats: [], cameraBeats: [] }
  const powers = candidates.map((c) => c.power)
  const p50 = percentile(powers, mode === 'dj' ? 0.42 : 0.50)
  const strong = candidates.filter((c) => c.power >= p50 && c.lowTone > 0.34)
  const step = clampRange(estimateStep(strong.length >= 8 ? strong : candidates) || 0.5, 0.32, 0.86)
  const bpm = Math.round(60 / step)
  const floor = percentile(powers, mode === 'dj' ? 0.24 : 0.30)
  const beats = makeBeatEvents(candidates, floor, step, mode)
  const pulseBeats = beats.filter((b) => b.pulse !== false && (b.impact >= 0.16 || b.combo === 'downbeat'))
  const cameraBeats = beats.filter((b) => b.camera !== false)
  const peakTimes = beats.map((b) => b.time)
  return { bpm, peaks: peakTimes.length, peakTimes, beats, pulseBeats, cameraBeats }
}

function abortError() {
  const err = new Error('节奏分析已停止')
  err.name = 'AbortError'
  return err
}

function throwIfAborted(signal?: AbortSignal) {
  if (signal?.aborted) throw abortError()
}

async function decodeAudioDataWithAbort(ctx: AudioContext, buf: ArrayBuffer, signal?: AbortSignal) {
  throwIfAborted(signal)
  if (!signal) return ctx.decodeAudioData(buf.slice(0))
  let onAbort: (() => void) | null = null
  const aborted = new Promise<AudioBuffer>((_, reject) => {
    onAbort = () => reject(abortError())
    signal.addEventListener('abort', onAbort, { once: true })
  })
  try {
    return await Promise.race([ctx.decodeAudioData(buf.slice(0)), aborted])
  } finally {
    if (onAbort) signal.removeEventListener('abort', onAbort)
  }
}

export async function analyzeAudioElementBeat(audioEl: HTMLAudioElement, track: IPageMusicTrack | undefined | null, mode: 'mr' | 'dj', duration = 0, signal?: AbortSignal): Promise<BeatAnalysisResult> {
  const cached = await getCachedBeatMap(track, mode, duration)
  if (cached) return { ...cached, cacheHit: true }
  if (!audioEl.src) throw new Error('当前音频没有可分析地址')
  throwIfAborted(signal)
  const res = await fetch(audioEl.src, { signal })
  const buf = await res.arrayBuffer()
  const ctx = new AudioContext()
  try {
    const audio = await decodeAudioDataWithAbort(ctx, buf, signal)
    throwIfAborted(signal)
    const ch0 = audio.getChannelData(0)
    const ch1 = audio.numberOfChannels > 1 ? audio.getChannelData(1) : null
    const mixed = ch1 ? new Float32Array(ch0.length) : ch0
    if (ch1) {
      for (let i = 0; i < ch0.length; i += 1) mixed[i] = (ch0[i] + ch1[i]) * 0.5
    }
    const result = analyzeChannel(mixed, audio.sampleRate, mode)
    await setCachedBeatMap(track, mode, audio.duration || duration, result)
    return { ...result, mode, duration: audio.duration || duration, updatedAt: Date.now(), cacheHit: false }
  } finally {
    await ctx.close().catch(() => {})
  }
}
