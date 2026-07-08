/**
 * StageLyrics — 3D lyrics mesh generator and animator.
 * Renders canvas-based lyric textures onto Three.js planes with
 * glow / sun bloom / spark particles / readability backing.
 * Ported from Mineradio's buildLyricMesh / updateStageLyrics3D.
 */

import * as THREE from 'three'
import { clamp01, clampRange } from './beatUtils'
import type { LyricLine } from '../musicMetadata'
import type { RealtimeBeatState } from './RealtimeBeatEngine'

export interface StageLyricConfig {
  fontFamily: string
  fontSize: number
  fontWeight: string
  primaryColor: string
  highlightColor: string
  glowColor: string
  sparkColor: string
  lineHeightFactor: number
  letterSpacing: number
  glowStrength: number
  beatGlowEnabled: boolean
  glowParticles: boolean
  cameraLock: boolean
}

type ShelfDetailProfile = 'normal' | 'skull'

const SHELF_DETAIL_PROFILES: Record<ShelfDetailProfile, { text: number; glow: number; readability: number; order: number }> = {
  normal: { text: 0.38, glow: 0.18, readability: 0.42, order: 24 },
  skull: { text: 0.3, glow: 0.12, readability: 0.3, order: 20 },
}

const DEFAULT_CONFIG: StageLyricConfig = {
  fontFamily: "'Noto Sans SC', 'PingFang SC', sans-serif",
  fontSize: 42,
  fontWeight: '900',
  primaryColor: '#a9b8c8',
  highlightColor: '#fff0b8',
  glowColor: '#9db8cf',
  sparkColor: '#fff7d2',
  lineHeightFactor: 1.12,
  letterSpacing: 0,
  glowStrength: 0.28,
  beatGlowEnabled: true,
  glowParticles: false,
  cameraLock: false,
}

interface LyricMesh {
  group: THREE.Group
  textMesh: THREE.Mesh
  glowMesh: THREE.Mesh
  sunMesh: THREE.Mesh
  sparks: THREE.Points
  readabilityMesh: THREE.Mesh
  textMat: THREE.ShaderMaterial
  glowMat: THREE.MeshBasicMaterial
  sunMat: THREE.MeshBasicMaterial
  sparkMat: THREE.ShaderMaterial
  readabilityMat: THREE.MeshBasicMaterial
  textWorldW: number
  textWorldH: number
  worldW: number
  worldH: number
  age: number
  state: 'in' | 'out'
  baseSparks: Float32Array
}

export class StageLyrics {
  group: THREE.Group
  private current: LyricMesh | null = null
  private outgoing: LyricMesh[] = []
  private dotTexture: THREE.Texture | null = null
  config: StageLyricConfig = { ...DEFAULT_CONFIG }

  /** Beat-derived glow values, updated externally from RealtimeBeatEngine. */
  beatGlow = 0
  highBloom = 0
  glowFollowX = 0
  glowFollowY = 0
  shelfDetailMix = 0
  shelfDetailProfile: ShelfDetailProfile = 'normal'

  constructor() {
    this.group = new THREE.Group()
    this.group.renderOrder = 38
    this.dotTexture = this.makeDotTexture()
  }

  // ─── canvas helpers ──────────────────────────────────────

  private makeCanvas(w: number, h: number): [HTMLCanvasElement, CanvasRenderingContext2D] {
    const canvas = document.createElement('canvas')
    canvas.width = w
    canvas.height = h
    const ctx = canvas.getContext('2d')!
    return [canvas, ctx]
  }

  private measureText(ctx: CanvasRenderingContext2D, text: string, size: number): TextMetrics {
    ctx.font = `${this.config.fontWeight} ${size}px ${this.config.fontFamily}`
    return ctx.measureText(text)
  }

  private makeDotTexture(): THREE.Texture {
    const [c, ctx] = this.makeCanvas(64, 64)
    const g = ctx.createRadialGradient(32, 32, 0, 32, 32, 16)
    g.addColorStop(0, 'rgba(255,255,255,0.9)')
    g.addColorStop(0.5, 'rgba(255,255,255,0.2)')
    g.addColorStop(1, 'rgba(255,255,255,0)')
    ctx.fillStyle = g
    ctx.fillRect(0, 0, 64, 64)
    const t = new THREE.CanvasTexture(c)
    t.needsUpdate = true
    return t
  }

  // ─── lyric texture ──────────────────────────────────────

  private makeLyricTexture(text: string) {
    const size = this.config.fontSize
    const letterSpacing = this.config.letterSpacing * Math.max(1, size)
    const lineHeight = size * this.config.lineHeightFactor

    const lines = text.split('\n')
    const ctxTemp = document.createElement('canvas').getContext('2d')!
    let maxW = 0
    for (const line of lines) {
      this.measureText(ctxTemp, line, size)
      const w = ctxTemp.measureText(line).width + letterSpacing * (line.length - 1)
      if (w > maxW) maxW = w
    }

    const pad = size * 1.8
    const w = Math.ceil(maxW + pad * 2)
    const h = Math.ceil(lines.length * lineHeight + pad * 2)

    const [canvas, ctx] = this.makeCanvas(w, h)
    ctx.textBaseline = 'top'
    ctx.textAlign = 'left'
    ctx.font = `${this.config.fontWeight} ${size}px ${this.config.fontFamily}`

    // Draw text with letter spacing
    let y = pad
    for (const line of lines) {
      let x = pad
      for (let i = 0; i < line.length; i++) {
        ctx.fillText(line[i], x, y)
        x += ctx.measureText(line[i]).width + letterSpacing
      }
      y += lineHeight
    }

    return { canvas, w, h, maxW, lines: lines.length, lineHeight }
  }

  // ─── lyric shader material ─────────────────────────────────

  private makeTextShaderMaterial(tex: THREE.Texture): THREE.ShaderMaterial {
    return new THREE.ShaderMaterial({
      uniforms: {
        uMap: { value: tex },
        uProgress: { value: 0 },
        uColor: { value: new THREE.Color(this.config.primaryColor) },
        uHighlight: { value: new THREE.Color(this.config.highlightColor) },
        uAlpha: { value: 1 },
        uTime: { value: 0 },
      },
      vertexShader: `
        varying vec2 vUv;
        void main() {
          vUv = uv;
          gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
        }
      `,
      fragmentShader: `
        precision highp float;
        varying vec2 vUv;
        uniform sampler2D uMap;
        uniform float uProgress;
        uniform float uAlpha;
        uniform float uTime;
        uniform vec3 uColor;
        uniform vec3 uHighlight;
        void main() {
          vec4 tex = texture2D(uMap, vUv);
          float t = vUv.y / uProgress;
          float highlight = 1.0 - smoothstep(0.0, 0.04, abs(t - 1.0));
          vec3 base = mix(uColor, uHighlight, highlight);
          gl_FragColor = vec4(base, tex.a * uAlpha);
        }
      `,
      transparent: true,
      depthWrite: false,
      depthTest: false,
    })
  }

  // ─── build lyric mesh ──────────────────────────────────────

  show(text: string, progress = 0) {
    this.clearCurrent()
    const { canvas, w, h, maxW, lines, lineHeight } = this.makeLyricTexture(text)
    const tex = new THREE.CanvasTexture(canvas)
    tex.needsUpdate = true

    const worldW = 6.1
    const worldH = worldW * (h / w)
    const textWorldW = worldW * (maxW / w)
    const textWorldH = worldH

    const group = new THREE.Group()
    group.renderOrder = 42
    group.position.set(0, 0.2, 1.46)

    // Text plane
    const textGeo = new THREE.PlaneGeometry(worldW, worldH)
    const textMat = this.makeTextShaderMaterial(tex)
    const textMesh = new THREE.Mesh(textGeo, textMat)
    textMesh.renderOrder = 43
    group.add(textMesh)

    // Glow (additive)
    const glowGeo = new THREE.PlaneGeometry(worldW * 1.1, worldH * 1.06)
    const glowTex = this.makeGlowTexture(canvas, w, h, maxW, lines, lineHeight)
    const glowMat = new THREE.MeshBasicMaterial({
      map: glowTex,
      transparent: true,
      opacity: 0,
      depthWrite: false,
      depthTest: false,
      blending: THREE.AdditiveBlending,
      color: new THREE.Color(this.config.glowColor),
      side: THREE.DoubleSide,
    })
    const glowMesh = new THREE.Mesh(glowGeo, glowMat)
    glowMesh.renderOrder = 41
    group.add(glowMesh)

    // Sun bloom (additive)
    const sunWorldW = Math.max(textWorldW + worldH * 1.1, textWorldW * 1.18)
    const sunWorldH = Math.max(worldH, worldH + textWorldW * 0.07)
    const sunGeo = new THREE.PlaneGeometry(sunWorldW, sunWorldH)
    const sunMat = new THREE.MeshBasicMaterial({
      map: this.getSunBloomTexture(),
      transparent: true,
      opacity: 0,
      depthWrite: false,
      depthTest: false,
      blending: THREE.AdditiveBlending,
      color: new THREE.Color(this.config.glowColor),
      side: THREE.DoubleSide,
    })
    const sunMesh = new THREE.Mesh(sunGeo, sunMat)
    sunMesh.renderOrder = 40
    sunMesh.scale.set(0.78, 0.58, 1)
    group.add(sunMesh)

    // Readability backing
    const readabilityMat = new THREE.MeshBasicMaterial({
      map: this.makeReadabilityTexture(canvas, w, h),
      transparent: true,
      opacity: 0,
      depthWrite: false,
      depthTest: false,
      side: THREE.DoubleSide,
    })
    const readabilityMesh = new THREE.Mesh(textGeo.clone(), readabilityMat)
    readabilityMesh.renderOrder = 42
    readabilityMesh.position.set(0, 0, -0.012)
    group.add(readabilityMesh)

    // Sparks
    const sparkCount = 64
    const pgeo = new THREE.BufferGeometry()
    const ppos = new Float32Array(sparkCount * 3)
    const pseed = new Float32Array(sparkCount)
    for (let i = 0; i < sparkCount; i++) {
      const angle = Math.random() * Math.PI * 2
      const rx = textWorldW * 0.6
      const ry = worldH * 0.5
      ppos[i * 3] = Math.cos(angle) * rx + (Math.random() - 0.5) * textWorldW * 0.12
      ppos[i * 3 + 1] = Math.sin(angle) * ry + (Math.random() - 0.5) * worldH * 0.14
      ppos[i * 3 + 2] = (Math.random() - 0.5) * 0.24
      pseed[i] = Math.random() * 1000
    }
    pgeo.setAttribute('position', new THREE.BufferAttribute(ppos, 3))
    pgeo.setAttribute('seed', new THREE.BufferAttribute(pseed, 1))
    const sparkMat = new THREE.ShaderMaterial({
      uniforms: {
        uMap: { value: this.dotTexture },
        uSize: { value: 0.052 },
        uOpacity: { value: 0 },
        uColor: { value: new THREE.Color(this.config.sparkColor) },
      },
      vertexShader: `
        attribute float seed;
        uniform float uSize;
        varying float vSeed;
        void main() {
          vSeed = seed;
          float jitter = 0.6 + fract(sin(seed * 19.17) * 43758.5) * 1.2;
          gl_PointSize = uSize * jitter * 80.0;
          gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
        }
      `,
      fragmentShader: `
        precision highp float;
        uniform sampler2D uMap;
        uniform vec3 uColor;
        uniform float uOpacity;
        varying float vSeed;
        void main() {
          vec4 tex = texture2D(uMap, gl_PointCoord);
          float twinkle = 0.72 + fract(sin(vSeed * 7.31) * 91.7) * 0.28;
          gl_FragColor = vec4(uColor * twinkle, tex.a * uOpacity);
        }
      `,
      transparent: true,
      depthWrite: false,
      depthTest: false,
      blending: THREE.AdditiveBlending,
    })
    const sparks = new THREE.Points(pgeo, sparkMat)
    sparks.renderOrder = 44
    group.add(sparks)

    const mesh: LyricMesh = {
      group, textMesh, glowMesh, sunMesh, sparks, readabilityMesh,
      textMat, glowMat, sunMat, sparkMat, readabilityMat,
      textWorldW, textWorldH, worldW, worldH,
      age: 0, state: 'in', baseSparks: new Float32Array(ppos),
    }

    this.updateProgress(mesh, progress)
    this.group.add(group)
    this.current = mesh
  }

  updateProgress(mesh: LyricMesh, progress: number) {
    mesh.textMat.uniforms.uProgress.value = clamp01(progress)
  }

  updateCurrentProgress(progress: number) {
    if (!this.current) return
    this.updateProgress(this.current, progress)
  }

  // ─── glow texture ────────────────────────────────────────

  private makeGlowTexture(canvas: HTMLCanvasElement, w: number, h: number, textW: number, _lines: number, _lineH: number): THREE.Texture {
    const [gc, gctx] = this.makeCanvas(w, h)
    // Copy text canvas, blur by drawing scaled-down then scaled-up
    gctx.filter = 'blur(8px)'
    gctx.drawImage(canvas, 0, 0)
    gctx.fillStyle = this.config.glowColor + '44'
    gctx.fillRect(0, 0, w, h)
    gctx.globalCompositeOperation = 'source-atop'
    gctx.fillStyle = this.config.glowColor + '88'
    gctx.fillRect(0, 0, w, h)
    const t = new THREE.CanvasTexture(gc)
    t.needsUpdate = true
    return t
  }

  private makeReadabilityTexture(canvas: HTMLCanvasElement, w: number, h: number): THREE.Texture {
    const [rc, rctx] = this.makeCanvas(w, h)
    rctx.drawImage(canvas, 0, 0)
    rctx.globalCompositeOperation = 'source-in'
    rctx.fillStyle = '#000000'
    rctx.fillRect(0, 0, w, h)
    const t = new THREE.CanvasTexture(rc)
    t.needsUpdate = true
    return t
  }

  private sunBloomTex: THREE.Texture | null = null
  private getSunBloomTexture(): THREE.Texture {
    if (this.sunBloomTex) return this.sunBloomTex
    const [c, ctx] = this.makeCanvas(256, 64)
    const g = ctx.createRadialGradient(128, 32, 8, 128, 32, 128)
    g.addColorStop(0, 'rgba(255,255,255,0.6)')
    g.addColorStop(0.3, 'rgba(255,255,255,0.15)')
    g.addColorStop(1, 'rgba(255,255,255,0)')
    ctx.fillStyle = g
    ctx.fillRect(0, 0, 256, 64)
    this.sunBloomTex = new THREE.CanvasTexture(c)
    this.sunBloomTex.needsUpdate = true
    return this.sunBloomTex
  }

  // ─── animation update ─────────────────────────────────────

  update(dt: number, beat: RealtimeBeatState) {
    if (!this.current) return

    const mesh = this.current
    mesh.age += dt
    const detailMix = clamp01(this.shelfDetailMix)
    const detailProfile = SHELF_DETAIL_PROFILES[this.shelfDetailProfile]
    const textAlpha = 1 - detailMix * (1 - detailProfile.text)
    const glowAlpha = 1 - detailMix * (1 - detailProfile.glow)
    const readabilityAlpha = 1 - detailMix * (1 - detailProfile.readability)
    this.group.renderOrder = detailMix > 0.04 ? detailProfile.order : 38
    mesh.group.renderOrder = detailMix > 0.04 ? detailProfile.order + 1 : 42
    mesh.textMesh.renderOrder = detailMix > 0.04 ? detailProfile.order + 2 : 43
    mesh.textMat.uniforms.uAlpha.value = textAlpha

    // Fade in
    if (mesh.state === 'in' && mesh.age < 0.48) {
      const t = clamp01(mesh.age / 0.48)
      const ease = 1 - Math.pow(1 - t, 3)
      mesh.group.scale.setScalar(0.96 + ease * 0.04)
      mesh.glowMat.opacity = ease * this.config.glowStrength * glowAlpha
      mesh.sunMat.opacity = ease * 0.18 * glowAlpha
      mesh.readabilityMat.opacity = ease * 0.86 * readabilityAlpha
      mesh.sparkMat.uniforms.uOpacity.value = this.config.glowParticles ? ease * 0.5 * glowAlpha : 0
    } else {
      mesh.readabilityMat.opacity = 0.86 * readabilityAlpha
      if (!this.config.beatGlowEnabled) {
        mesh.glowMat.opacity = this.config.glowStrength * glowAlpha
        mesh.sunMat.opacity = 0.18 * glowAlpha
      }
      mesh.sparkMat.uniforms.uOpacity.value = this.config.glowParticles ? 0.5 * glowAlpha : 0
    }

    // Beat glow
    if (this.config.beatGlowEnabled) {
      const tgt = beat.pulse * 1.22 + beat.cam.radiusKick * 1.85
      this.beatGlow += (tgt - this.beatGlow) * 0.28
      const solarBloom = clamp01(0.18 + this.beatGlow * 1.18 + this.config.glowStrength)
      this.highBloom += (solarBloom - this.highBloom) * 0.12
      mesh.sunMat.opacity = clampRange(this.highBloom * 0.45, 0, 0.6) * glowAlpha
      mesh.glowMat.opacity = clampRange(this.config.glowStrength + this.beatGlow * 0.2, 0, 0.85) * glowAlpha
    }

    // Process outgoing
    for (let i = this.outgoing.length - 1; i >= 0; i--) {
      const out = this.outgoing[i]
      out.age += dt
      if (out.state === 'out' && out.age > 0.42) {
        this.disposeOne(out)
        this.outgoing.splice(i, 1)
      } else {
        const t = clamp01(out.age / 0.42)
        out.glowMat.opacity *= 0.92
        out.sunMat.opacity *= 0.92
        out.sparkMat.uniforms.uOpacity.value *= 0.9
        out.group.scale.setScalar(0.96 * (1 - t))
      }
    }
  }

  clear() {
    this.clearCurrent()
    while (this.outgoing.length) {
      this.disposeOne(this.outgoing.pop()!)
    }
  }

  private clearCurrent() {
    if (this.current) {
      this.current.state = 'out'
      this.outgoing.push(this.current)
      this.current = null
    }
  }

  private disposeOne(mesh: LyricMesh) {
    this.group.remove(mesh.group)
    mesh.textMat.uniforms.uMap.value?.dispose?.()
    mesh.glowMat.map?.dispose()
    mesh.readabilityMat.map?.dispose()
    mesh.textMat.dispose()
    mesh.glowMat.dispose()
    mesh.sunMat.dispose()
    mesh.sparkMat.dispose()
    mesh.readabilityMat.dispose()
    mesh.textMesh.geometry.dispose()
    mesh.glowMesh.geometry.dispose()
    mesh.sunMesh.geometry.dispose()
    mesh.sparks.geometry.dispose()
    mesh.readabilityMesh.geometry.dispose()
    mesh.group.clear()
  }

  dispose() {
    this.clear()
    this.dotTexture?.dispose()
    this.sunBloomTex?.dispose()
    this.group.clear()
  }
}
