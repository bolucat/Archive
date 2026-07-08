/**
 * RealtimeBeatEngine — live audio spectrum analyser → beat events.
 * Ported from Mineradio's processRealtimeBeatEngine / beatCam / rtBeat.
 * Runs per-frame, consuming Web Audio AnalyserNode frequency data,
 * emitting beat pulses and camera kick values.
 */

import { clamp01, clampRange } from './beatUtils'

export interface BeatEvent {
  time: number
  strength: number
  confidence: number
  primary: boolean
  camera: boolean
  tone: 'snap' | 'body' | 'deep' | 'mixed'
  low: number
  body: number
  snap: number
  mass: number
  sharpness: number
  index: number
}

export interface RealtimeBeatState {
  /** 0-1 beat pulse (smoothed onset energy).  Decays by ~0.90/frame. */
  pulse: number
  /** Raw onset energy this frame (0-1). */
  rawOnset: number
  /** Smoothed bass energy (0-1). */
  bass: number
  /** Smoothed body energy (0-1). */
  body: number
  /** Smoothed snap energy (0-1). */
  snap: number
  /** Number of beat hits detected since last reset. */
  hitCount: number
  /** Milliseconds since last beat hit. */
  msSinceLastHit: number
  /** true when a beat hit was just detected this frame. */
  justHit: boolean
  /** The intensity of the current beat hit. */
  hitStrength: number
  /** Camera kick values (theta, phi, radius, roll). */
  cam: {
    thetaKick: number
    phiKick: number
    radiusKick: number
    rollKick: number
  }
}

const FFT_SIZE = 2048
const BAND_COUNT = FFT_SIZE / 2 // 1024

// Frequency band indices (approximate at 44.1kHz)
// Sub:  0-4    (~0-86Hz)
// Low:  5-10   (~86-215Hz)  
// Body: 11-34  (~215-730Hz)
// Vocal: 35-70 (~730-1500Hz)
// Snap: 71-140 (~1500-3000Hz)
const SUB_START = 0
const SUB_END = 4
const LOW_START = 5
const LOW_END = 10
const BODY_START = 11
const BODY_END = 34
const VOCAL_START = 35
const VOCAL_END = 70
const SNAP_START = 71
const SNAP_END = Math.min(140, BAND_COUNT - 1)

function bandAvg(data: Uint8Array, s: number, e: number): number {
  let sum = 0
  const count = e - s + 1
  for (let i = s; i <= e; i++) sum += data[i]
  return sum / count / 255
}

export class RealtimeBeatEngine {
  private smBass = 0
  private smBody = 0
  private smSnap = 0
  private smSub = 0
  private smVocal = 0
  private prevSub = 0
  private prevLow = 0
  private prevBody = 0
  private prevSnap = 0
  private onsetAvg = 0.012
  private onsetPeak = 0.06
  private subPeak = 0.14
  private lowPeak = 0.18
  private bodyPeak = 0.16
  private snapPeak = 0.14
  private lastHitTime = -10000
  private hitCount = 0
  private pulse = 0
  private justHitFlag = false
  private hitStrength_ = 0
  private rawOnset_ = 0

  // Camera kick
  private thetaKick = 0
  private phiKick = 0
  private radiusKick = 0
  private rollKick = 0

  update(frequencyData: Uint8Array, dtMs: number, isPlaying: boolean): RealtimeBeatState {
    if (!isPlaying || dtMs <= 0) {
      this.decay(0.88)
      return this.snapshot(0)
    }

    const sub = bandAvg(frequencyData, SUB_START, SUB_END)
    const low = bandAvg(frequencyData, LOW_START, LOW_END)
    const body = bandAvg(frequencyData, BODY_START, BODY_END)
    const vocal = bandAvg(frequencyData, VOCAL_START, VOCAL_END)
    const snap = bandAvg(frequencyData, SNAP_START, SNAP_END)

    // Smoothing
    const smK = 0.28
    this.smSub += (sub - this.smSub) * smK
    this.smBass += (low - this.smBass) * smK
    this.smBody += (body - this.smBody) * smK
    this.smVocal += (vocal - this.smVocal) * smK
    this.smSnap += (snap - this.smSnap) * smK

    // Onset detection: how much did sub/low rise?
    const subRise = Math.max(0, this.smSub - this.prevSub)
    const lowRise = Math.max(0, this.smBass - this.prevLow)
    const bodyRise = Math.max(0, this.smBody - this.prevBody)
    const snapRise = Math.max(0, this.smSnap - this.prevSnap)
    const onset = subRise * 0.44 + lowRise * 0.32 + bodyRise * 0.12 + snapRise * 0.05
    this.rawOnset_ = clamp01(onset * 3.5)

    // Adaptive threshold
    this.onsetAvg += (onset - this.onsetAvg) * 0.018
    this.onsetPeak += ((onset > this.onsetPeak ? onset : this.onsetPeak * 0.995) - this.onsetPeak) * 0.035

    const thresh = this.onsetAvg * 2.2 + this.onsetPeak * 0.42 + 0.016
    const minIntervalMs = 220
    const dtSinceLast = performance.now() - this.lastHitTime

    this.justHitFlag = false
    if (onset > thresh && dtSinceLast > minIntervalMs && low > 0.015) {
      this.justHitFlag = true
      this.lastHitTime = performance.now()
      this.hitCount++
      this.hitStrength_ = clamp01((onset - thresh) / Math.max(0.002, this.onsetPeak * 0.8))

      // Camera kick
      const kick = this.hitStrength_ * 1.8
      this.thetaKick += (Math.random() - 0.5) * kick * 0.5
      this.phiKick += (Math.random() - 0.5) * kick * 0.42
      this.radiusKick = Math.max(this.radiusKick, kick * 0.32)
      this.rollKick += (Math.random() - 0.5) * kick * 0.35

      // Pulse
      this.pulse = Math.min(1, this.pulse + onset * 8 + this.hitStrength_ * 0.24)
    }

    // Update peaks
    this.subPeak += (Math.max(this.subPeak * 0.9995, sub) - this.subPeak) * 0.04
    this.lowPeak += (Math.max(this.lowPeak * 0.9995, low) - this.lowPeak) * 0.04
    this.bodyPeak += (Math.max(this.bodyPeak * 0.9995, body) - this.bodyPeak) * 0.04
    this.snapPeak += (Math.max(this.snapPeak * 0.9995, snap) - this.snapPeak) * 0.04

    // Decay
    this.decay(0.90)

    // Update prev values
    this.prevSub = this.smSub
    this.prevLow = this.smBass
    this.prevBody = this.smBody
    this.prevSnap = this.smSnap

    return this.snapshot(dtMs)
  }

  private decay(rate: number) {
    this.pulse *= rate
    this.thetaKick *= 0.88
    this.phiKick *= 0.88
    this.radiusKick *= 0.82
    this.rollKick *= 0.85
    this.hitStrength_ *= 0.92
    this.rawOnset_ *= 0.94
  }

  private snapshot(dtMs: number): RealtimeBeatState {
    const now = performance.now()
    return {
      pulse: clamp01(this.pulse),
      rawOnset: clamp01(this.rawOnset_),
      bass: clamp01(this.smBass),
      body: clamp01(this.smBody),
      snap: clamp01(this.smSnap),
      hitCount: this.hitCount,
      msSinceLastHit: now - this.lastHitTime,
      justHit: this.justHitFlag,
      hitStrength: clamp01(this.hitStrength_),
      cam: {
        thetaKick: clampRange(this.thetaKick, -1, 1),
        phiKick: clampRange(this.phiKick, -1, 1),
        radiusKick: clamp01(this.radiusKick),
        rollKick: clampRange(this.rollKick, -1, 1),
      },
    }
  }

  reset() {
    this.smBass = 0
    this.smBody = 0
    this.smSnap = 0
    this.smSub = 0
    this.smVocal = 0
    this.prevSub = 0
    this.prevLow = 0
    this.prevBody = 0
    this.prevSnap = 0
    this.onsetAvg = 0.012
    this.onsetPeak = 0.06
    this.subPeak = 0.14
    this.lowPeak = 0.18
    this.bodyPeak = 0.16
    this.snapPeak = 0.14
    this.lastHitTime = -10000
    this.hitCount = 0
    this.pulse = 0
    this.justHitFlag = false
    this.hitStrength_ = 0
    this.rawOnset_ = 0
    this.thetaKick = 0
    this.phiKick = 0
    this.radiusKick = 0
    this.rollKick = 0
  }
}
