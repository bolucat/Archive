/**
 * Mineradio beat analysis utilities — ported from dj-analyzer.js.
 * Provides shared math helpers used by both BeatAnalyzer (offline) and
 * RealtimeBeatEngine (live).
 */

export function clamp01(v: number): number {
  return Math.max(0, Math.min(1, Number(v) || 0))
}

export function clampRange(v: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, Number(v) || 0))
}

export function percentile(arr: Float32Array | number[], p: number, maxSamples = 16000): number {
  const len = arr.length
  if (!len) return 0.001
  let sample: number[]
  if (len <= maxSamples) {
    sample = Array.from(arr)
  } else {
    sample = new Array(maxSamples)
    const step = (len - 1) / (maxSamples - 1)
    for (let i = 0; i < maxSamples; i++) sample[i] = arr[Math.min(len - 1, Math.floor(i * step))] || 0
  }
  sample.sort((a, b) => a - b)
  return sample[Math.max(0, Math.min(sample.length - 1, Math.floor(sample.length * p)))] || 0.001
}

export function median(vals: number[]): number {
  const filtered = vals.filter((v) => Number.isFinite(v)).sort((a, b) => a - b)
  return filtered.length ? filtered[Math.floor(filtered.length * 0.5)] : 0
}

export interface BiquadState {
  b0: number
  b1: number
  b2: number
  a1: number
  a2: number
  x1: number
  x2: number
  y1: number
  y2: number
}

export function makeBiquad(type: 'lowpass' | 'highpass', freq: number, q: number, sr: number): BiquadState {
  freq = Math.max(8, Math.min(freq, sr * 0.45))
  const w0 = (2 * Math.PI * freq) / sr
  const cos = Math.cos(w0)
  const sin = Math.sin(w0)
  const alpha = sin / (2 * (q || 0.707))
  let b0: number, b1: number, b2: number
  if (type === 'highpass') {
    b0 = (1 + cos) * 0.5
    b1 = -(1 + cos)
    b2 = (1 + cos) * 0.5
  } else {
    b0 = (1 - cos) * 0.5
    b1 = 1 - cos
    b2 = (1 - cos) * 0.5
  }
  const a0 = 1 + alpha
  const a1 = -2 * cos
  const a2 = 1 - alpha
  const inv = 1 / a0
  return { b0: b0 * inv, b1: b1 * inv, b2: b2 * inv, a1: a1 * inv, a2: a2 * inv, x1: 0, x2: 0, y1: 0, y2: 0 }
}

export function runBiquad(st: BiquadState, x: number): number {
  const y = st.b0 * x + st.b1 * st.x1 + st.b2 * st.x2 - st.a1 * st.y1 - st.a2 * st.y2
  st.x2 = st.x1
  st.x1 = x
  st.y2 = st.y1
  st.y1 = y
  return y
}
