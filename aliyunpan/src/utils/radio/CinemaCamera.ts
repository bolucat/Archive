/**
 * CinemaCamera — beat-driven cinematic camera shake.
 * Uses RealtimeBeatState.cam (thetaKick, phiKick, radiusKick, rollKick)
 * to create subtle camera movement synced to the beat.
 */

import * as THREE from 'three'
import type { RealtimeBeatState } from './RealtimeBeatEngine'
import { clamp01, clampRange } from './beatUtils'

export interface CinemaState {
  targetTheta: number
  targetPhi: number
  targetRadius: number
  targetRoll: number
  currentTheta: number
  currentPhi: number
  currentRadius: number
  currentRoll: number
  shakeIntensity: number
  energyAvg: number
  lowAvg: number
}

export class CinemaCamera {
  state: CinemaState = {
    targetTheta: 0, targetPhi: 0, targetRadius: 0, targetRoll: 0,
    currentTheta: 0, currentPhi: 0, currentRadius: 0, currentRoll: 0,
    shakeIntensity: 0.5,
    energyAvg: 0,
    lowAvg: 0,
  }

  enabled = true

  /** Call every frame with current beat state. Returns delta camera pos/rot offsets. */
  update(beat: RealtimeBeatState, dt: number, intensity: number): {
    x: number; y: number; z: number; roll: number; pitch: number; yaw: number
  } {
    this.state.shakeIntensity = intensity

    // Smooth energy tracking
    const energy = beat.bass * 0.7 + beat.body * 0.3
    this.state.energyAvg += (energy - this.state.energyAvg) * 0.08
    this.state.lowAvg += (beat.bass - this.state.lowAvg) * 0.08

    // Accumulate beat kicks as camera offset targets
    const kickScale = intensity * (this.state.energyAvg * 0.6 + 0.4)

    if (beat.justHit && this.enabled) {
      const kick = (beat.hitStrength * 0.5 + beat.pulse * 0.3) * kickScale
      this.state.targetTheta += (Math.random() - 0.5) * kick * 0.40
      this.state.targetPhi += (Math.random() - 0.5) * kick * 0.36
      this.state.targetRadius += kick * 0.18
      this.state.targetRoll += (Math.random() - 0.5) * kick * 0.28
      // Clamp targets
      this.state.targetTheta = clampRange(this.state.targetTheta, -0.9, 0.9)
      this.state.targetPhi = clampRange(this.state.targetPhi, -0.7, 0.7)
      this.state.targetRadius = clamp01(this.state.targetRadius)
      this.state.targetRoll = clampRange(this.state.targetRoll, -0.6, 0.6)
    }

    // Smooth follow
    const follow = 0.12
    this.state.currentTheta += (this.state.targetTheta - this.state.currentTheta) * follow
    this.state.currentPhi += (this.state.targetPhi - this.state.currentPhi) * follow
    this.state.currentRadius += (this.state.targetRadius - this.state.currentRadius) * follow
    this.state.currentRoll += (this.state.targetRoll - this.state.currentRoll) * follow

    // Decay targets
    const decay = 0.92
    this.state.targetTheta *= decay
    this.state.targetPhi *= decay
    this.state.targetRadius *= decay
    this.state.targetRoll *= decay

    // Compute offsets
    const scale = intensity * 0.7
    return {
      x: this.state.currentRadius * scale * 0.8,
      y: this.state.currentPhi * scale * 1.2,
      z: this.state.currentRadius * scale * 0.5,
      roll: this.state.currentRoll * scale,
      pitch: this.state.currentTheta * scale * 0.8,
      yaw: this.state.currentPhi * scale * 0.5,
    }
  }

  reset() {
    this.state.targetTheta = 0
    this.state.targetPhi = 0
    this.state.targetRadius = 0
    this.state.targetRoll = 0
    this.state.currentTheta = 0
    this.state.currentPhi = 0
    this.state.currentRadius = 0
    this.state.currentRoll = 0
  }
}
