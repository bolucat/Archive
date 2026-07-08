/**
 * ShelfManager — 3D Playlist Shelf.
 * Creates a rotating 3D band of album/folder/playlist cards in the
 * Three.js scene. Cards show cover art, respond to hover/click/scroll,
 * and can expand to show track details.
 * Ported from Mineradio's makeShelfManager / placeCard / update.
 */

import * as THREE from 'three'
import { clamp01, clampRange } from './beatUtils'

export interface ShelfCard {
  id: string
  title: string
  subtitle: string
  coverUrl: string
  /** Number of tracks in this card. */
  trackCount: number
  /** Data payload — tracks, callback, etc. */
  data: any
}

export interface ShelfConfig {
  mode: 'off' | 'side' | 'stage'
  cameraMode: 'dynamic' | 'static'
  presence: 'auto' | 'always'
  size: number
  offsetX: number
  offsetY: number
  offsetZ: number
  angleY: number
  opacity: number
  bgOpacity: number
  accentColor: string
}

const DEFAULT_CONFIG: ShelfConfig = {
  mode: 'side',
  cameraMode: 'static',
  presence: 'auto',
  size: 1,
  offsetX: 0,
  offsetY: 0,
  offsetZ: 0,
  angleY: -15,
  opacity: 1,
  bgOpacity: 0.9,
  accentColor: '#f4d28a',
}

const SHELF_CARD_WIDTH = 2.05
const SHELF_CARD_HEIGHT = 1.025
const SIDE_SHELF_SAFE_X = 6.00
const SIDE_DETAIL_CARD_OFFSET_X = -2.15
const SIDE_CONTENT_SCALE = 0.78
const SIDE_CARD_GAP_Y = 0.72
const STAGE_CARD_GAP_X = 1.68
const SIDE_DETAIL_VIEWPORT_X = 0.34
const SIDE_DETAIL_VIEWPORT_Y = 0.03
const SIDE_DETAIL_VIEWPORT_DISTANCE = 11.2

function roundRect(ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, r: number) {
  const radius = Math.min(r, w / 2, h / 2)
  ctx.beginPath()
  ctx.moveTo(x + radius, y)
  ctx.arcTo(x + w, y, x + w, y + h, radius)
  ctx.arcTo(x + w, y + h, x, y + h, radius)
  ctx.arcTo(x, y + h, x, y, radius)
  ctx.arcTo(x, y, x + w, y, radius)
  ctx.closePath()
}

function shelfTrackKey(track: any) {
  return track ? `${track.user_id || ''}|${track.drive_id || ''}|${track.file_id || ''}` : ''
}

function truncateCanvasText(ctx: CanvasRenderingContext2D, text: string, maxWidth: number) {
  if (ctx.measureText(text).width <= maxWidth) return text
  const ellipsis = '...'
  let lo = 0
  let hi = text.length
  while (lo < hi) {
    const mid = Math.ceil((lo + hi) / 2)
    if (ctx.measureText(text.slice(0, mid) + ellipsis).width <= maxWidth) lo = mid
    else hi = mid - 1
  }
  return text.slice(0, lo) + ellipsis
}

interface CardMesh {
  group: THREE.Group
  coverPlane: THREE.Mesh
  reflectionPlane: THREE.Mesh
  borderPlane: THREE.Mesh
  glowPlane: THREE.Mesh
  sheenPlane: THREE.Mesh
  titleSprite: THREE.Sprite | null
  card: ShelfCard
  basePosition: THREE.Vector3
  baseRotation: THREE.Euler
  hoverAlpha: number
  expandAlpha: number
  selected: boolean
  expanded: boolean
}

interface ContentRowMesh {
  group: THREE.Group
  plane: THREE.Mesh<THREE.PlaneGeometry, THREE.MeshBasicMaterial>
  track: any
  index: number
}

interface ContentCloseMesh {
  group: THREE.Group
  plane: THREE.Mesh<THREE.PlaneGeometry, THREE.MeshBasicMaterial>
}

interface ContentScrollRailMesh {
  group: THREE.Group
  track: THREE.Mesh<THREE.PlaneGeometry, THREE.MeshBasicMaterial>
  thumb: THREE.Mesh<THREE.PlaneGeometry, THREE.MeshBasicMaterial>
}

type ContentRowAction = 'play' | 'next' | 'collect'

export class ShelfManager {
  group: THREE.Group
  config: ShelfConfig = { ...DEFAULT_CONFIG }
  cards: CardMesh[] = []

  private cardList: ShelfCard[] = []
  private loadedTextures = new Map<string, THREE.Texture>()
  private scrollOffset = 0
  private targetScroll = 0
  private scrollVelocity = 0
  private hoveredIndex = -1
  private selectedIndex = -1
  private needsRebuild = true
  private connectorLines: THREE.LineSegments<THREE.BufferGeometry, THREE.LineBasicMaterial> | null = null
  private connectorParticles: THREE.Points<THREE.BufferGeometry, THREE.PointsMaterial> | null = null
  private floorMirror: THREE.Mesh<THREE.PlaneGeometry, THREE.MeshBasicMaterial> | null = null
  private contentGroup = new THREE.Group()
  private contentBackdrop: THREE.Mesh<THREE.PlaneGeometry, THREE.MeshBasicMaterial> | null = null
  private contentScrollRail: ContentScrollRailMesh | null = null
  private contentRows: ContentRowMesh[] = []
  private contentClose: ContentCloseMesh | null = null
  private contentOpenIndex = -1
  private contentAnchorIndex = -1
  private contentScrollOffset = 0
  private contentTargetScroll = 0
  private contentAlpha = 0
  private contentTargetAlpha = 0
  private hoveredContentIndex = -1
  private dynamicLookHelper = new THREE.Object3D()

  // SFX
  private audioCtx: AudioContext | null = null
  private lastClickSfxAt = 0
  private lastScrollSfxAt = 0
  private presenceAlpha = 1
  private flowPhase = 0

  /** Called when a card is clicked — provides the card data. */
  onCardClick: ((card: ShelfCard) => void) | null = null
  /** Called when a 3D content-list row is clicked. */
  onTrackClick: ((track: any) => void) | null = null
  /** Called when a 3D content-list row requests queue-next. */
  onTrackNext: ((track: any) => void) | null = null
  /** Called when a 3D content-list row requests collect-to-playlist. */
  onTrackCollect: ((track: any) => void) | null = null
  /** Called when the smooth center card changes. */
  onCenterChange: ((card: ShelfCard | null, index: number) => void) | null = null
  /** Called when scrolling past the first/last card can switch collection panes. */
  onBoundaryScroll: ((direction: -1 | 1) => boolean) | null = null
  private lastCenterIndex = -1

  constructor() {
    this.group = new THREE.Group()
    this.group.visible = false
  }

  // ─── data ─────────────────────────────────────────────────

  setCards(cards: ShelfCard[]) {
    const nextSignature = JSON.stringify(cards.map(c => `${c.id}|${c.coverUrl || ''}|${c.title || ''}|${c.subtitle || ''}|${c.trackCount || 0}|${c.data?.activeKey || ''}`))
    const prevSignature = JSON.stringify(this.cardList.map(c => `${c.id}|${c.coverUrl || ''}|${c.title || ''}|${c.subtitle || ''}|${c.trackCount || 0}|${c.data?.activeKey || ''}`))
    if (nextSignature !== prevSignature) {
      this.cardList = [...cards]
      this.targetScroll = clampRange(this.targetScroll, 0, Math.max(0, this.cardList.length - 1))
      this.scrollOffset = clampRange(this.scrollOffset, 0, Math.max(0, this.cardList.length - 1))
      this.needsRebuild = true
    }
  }

  setConfig(config: Partial<ShelfConfig>) {
    const prev = this.config
    const next = { ...prev, ...config }
    const needsLayout =
      prev.mode !== next.mode ||
      prev.cameraMode !== next.cameraMode ||
      Math.abs(prev.size - next.size) > 0.001 ||
      Math.abs(prev.offsetX - next.offsetX) > 0.001 ||
      Math.abs(prev.offsetY - next.offsetY) > 0.001 ||
      Math.abs(prev.offsetZ - next.offsetZ) > 0.001 ||
      Math.abs(prev.angleY - next.angleY) > 0.001 ||
      Math.abs(prev.opacity - next.opacity) > 0.001 ||
      Math.abs(prev.bgOpacity - next.bgOpacity) > 0.001 ||
      prev.presence !== next.presence ||
      prev.accentColor !== next.accentColor
    this.config = next
    if (needsLayout) this.needsRebuild = true
  }

  // ─── rebuild ──────────────────────────────────────────────

  private rebuild() {
    // Clear existing
    for (const m of this.cards) this.disposeCardMesh(m)
    this.disposeContentRows()
    this.disposeShelfExtras()
    this.cards = []
    this.group.clear()
    this.group.add(this.contentGroup)
    this.contentOpenIndex = -1

    if (!this.cardList.length || this.config.mode === 'off') {
      this.group.visible = false
      this.notifyCenter(-1)
      return
    }
    this.group.visible = true

    const N = this.cardList.length
    const size = this.config.size
    const spacing = 3.2 / size

    // In side mode, cards go in a vertical spiral on the right
    // In stage mode, they face forward in a horizontal band
    const isSide = this.config.mode === 'side'

    for (let i = 0; i < N; i++) {
      const card = this.cardList[i]
      const t = N > 1 ? i / (N - 1) : 0.5

      // Position in spiral/band
      let x: number, y: number, z: number
      if (isSide) {
        // Right side, vertical spiral
        const radius = 2.8 * size + this.config.offsetZ
        const angleY = THREE.MathUtils.degToRad(this.config.angleY) + t * Math.PI * 0.6
        x = 5 + Math.cos(angleY) * radius + this.config.offsetX
        y = (t - 0.5) * N * spacing + this.config.offsetY
        z = Math.sin(angleY) * radius
      } else {
        // Stage - horizontal band facing camera
        x = (t - 0.5) * N * spacing + this.config.offsetX
        y = this.config.offsetY
        z = 6 + this.config.offsetZ
      }

      const mesh = this.createCardMesh(card, i)
      mesh.basePosition.set(x, y, z)
      mesh.baseRotation.set(0, isSide ? THREE.MathUtils.degToRad(this.config.angleY) : 0, 0)
      mesh.group.position.copy(mesh.basePosition)
      mesh.group.rotation.copy(mesh.baseRotation)

      this.group.add(mesh.group)
      this.cards.push(mesh)

      // Load cover texture
      this.loadCardTexture(mesh, card.coverUrl)
    }

    this.createShelfExtras()
    this.needsRebuild = false
    this.notifyCenter(Math.round(this.scrollOffset))
  }

  // ─── card mesh ────────────────────────────────────────────

  private createCardMesh(card: ShelfCard, _index: number): CardMesh {
    const w = SHELF_CARD_WIDTH
    const h = SHELF_CARD_HEIGHT
    const group = new THREE.Group()
    group.renderOrder = 20

    // Glow (behind)
    const glowGeo = new THREE.PlaneGeometry(w + 0.1, h + 0.1)
    const glowMat = new THREE.MeshBasicMaterial({
      color: new THREE.Color(this.config.accentColor),
      transparent: true,
      opacity: 0,
      depthWrite: false,
      depthTest: false,
      blending: THREE.AdditiveBlending,
      side: THREE.DoubleSide,
    })
    const glowPlane = new THREE.Mesh(glowGeo, glowMat)
    glowPlane.position.z = -0.02
    group.add(glowPlane)

    // Border
    const borderGeo = new THREE.PlaneGeometry(w + 0.06, h + 0.06)
    const borderMat = new THREE.MeshBasicMaterial({
      color: new THREE.Color(this.config.accentColor),
      transparent: true,
      opacity: 0.25,
      depthWrite: false,
      depthTest: true,
      side: THREE.DoubleSide,
    })
    const borderPlane = new THREE.Mesh(borderGeo, borderMat)
    borderPlane.position.z = -0.01
    group.add(borderPlane)

    // Cover
    const coverGeo = new THREE.PlaneGeometry(w, h)
    const coverMat = new THREE.MeshBasicMaterial({
      color: 0x1a1d22,
      transparent: true,
      opacity: this.config.opacity,
      depthWrite: true,
      depthTest: true,
      side: THREE.DoubleSide,
    })
    const coverPlane = new THREE.Mesh(coverGeo, coverMat)
    group.add(coverPlane)

    const sheenGeo = new THREE.PlaneGeometry(w, h)
    const sheenMat = new THREE.MeshBasicMaterial({
      map: this.makeCardSheenTexture(),
      color: 0xffffff,
      transparent: true,
      opacity: 0.16,
      depthWrite: false,
      depthTest: false,
      side: THREE.DoubleSide,
      blending: THREE.AdditiveBlending
    })
    const sheenPlane = new THREE.Mesh(sheenGeo, sheenMat)
    sheenPlane.position.z = 0.012
    sheenPlane.renderOrder = 26
    group.add(sheenPlane)

    const reflectionGeo = new THREE.PlaneGeometry(w, h)
    const reflectionMat = new THREE.MeshBasicMaterial({
      color: 0xffffff,
      transparent: true,
      opacity: 0,
      depthWrite: false,
      depthTest: false,
      side: THREE.DoubleSide,
      blending: THREE.AdditiveBlending
    })
    const reflectionPlane = new THREE.Mesh(reflectionGeo, reflectionMat)
    reflectionPlane.position.set(0, -h * 1.24, -0.015)
    reflectionPlane.scale.y = -0.62
    reflectionPlane.renderOrder = 8
    group.add(reflectionPlane)

    const m: CardMesh = {
      group,
      coverPlane,
      reflectionPlane,
      borderPlane,
      glowPlane,
      sheenPlane,
      titleSprite: null,
      card,
      basePosition: new THREE.Vector3(),
      baseRotation: new THREE.Euler(),
      hoverAlpha: 0,
      expandAlpha: 0,
      selected: false,
      expanded: false,
    }

    return m
  }

  private loadCardTexture(mesh: CardMesh, url: string) {
    if (!url) {
      this.applyGeneratedTexture(mesh)
      return
    }

    const key = `composed:${mesh.card.id}:${url}:${this.config.accentColor}`
    if (this.loadedTextures.has(key)) {
      const tex = this.loadedTextures.get(key)!
      const material = mesh.coverPlane.material as THREE.MeshBasicMaterial
      material.color.set(0xffffff)
      material.map = tex
      material.needsUpdate = true
      const reflectionMaterial = mesh.reflectionPlane.material as THREE.MeshBasicMaterial
      reflectionMaterial.map = tex
      reflectionMaterial.needsUpdate = true
      return
    }

    const loader = new THREE.TextureLoader()
    loader.setCrossOrigin('anonymous')
    loader.load(url, (tex) => {
      this.applyCardCanvasTexture(mesh, tex.image as CanvasImageSource, key)
      tex.dispose()
    }, undefined, () => this.applyGeneratedTexture(mesh))
  }

  private applyGeneratedTexture(mesh: CardMesh) {
    this.applyCardCanvasTexture(mesh, null, `generated:${mesh.card.id}:${this.config.accentColor}`)
  }

  private applyCardCanvasTexture(mesh: CardMesh, image: CanvasImageSource | null, key: string) {
    if (this.loadedTextures.has(key)) {
      const tex = this.loadedTextures.get(key)!
      const material = mesh.coverPlane.material as THREE.MeshBasicMaterial
      material.color.set(0xffffff)
      material.map = tex
      material.needsUpdate = true
      const reflectionMaterial = mesh.reflectionPlane.material as THREE.MeshBasicMaterial
      reflectionMaterial.map = tex
      reflectionMaterial.needsUpdate = true
      return
    }

    const canvas = document.createElement('canvas')
    canvas.width = 720
    canvas.height = 360
    const ctx = canvas.getContext('2d')
    if (!ctx) return

    const accent = this.config.accentColor || '#f4d28a'
    const grad = ctx.createLinearGradient(0, 0, 720, 360)
    grad.addColorStop(0, '#1b1d22')
    grad.addColorStop(0.48, '#08090d')
    grad.addColorStop(1, accent)
    ctx.fillStyle = grad
    ctx.fillRect(0, 0, 720, 360)
    ctx.globalAlpha = 0.18
    for (let i = 0; i < 110; i++) {
      const x = Math.random() * 720
      const y = Math.random() * 360
      const s = Math.random() * 3 + 1
      ctx.fillStyle = i % 3 === 0 ? accent : '#ffffff'
      ctx.fillRect(x, y, s, s)
    }
    ctx.globalAlpha = 1
    ctx.fillStyle = 'rgba(255,255,255,.08)'
    roundRect(ctx, 26, 26, 308, 308, 28)
    ctx.fill()
    ctx.save()
    roundRect(ctx, 34, 34, 292, 292, 24)
    ctx.clip()
    if (image) {
      ctx.drawImage(image, 34, 34, 292, 292)
    } else {
      const fallback = ctx.createLinearGradient(34, 34, 326, 326)
      fallback.addColorStop(0, accent)
      fallback.addColorStop(0.55, '#11151d')
      fallback.addColorStop(1, '#000000')
      ctx.fillStyle = fallback
      ctx.fillRect(34, 34, 292, 292)
      ctx.fillStyle = 'rgba(255,255,255,.88)'
      ctx.font = '800 70px -apple-system, BlinkMacSystemFont, "PingFang SC", sans-serif'
      ctx.textAlign = 'center'
      ctx.textBaseline = 'middle'
      ctx.fillText((mesh.card.title || 'B').slice(0, 1).toUpperCase(), 180, 180)
    }
    ctx.restore()
    ctx.fillStyle = 'rgba(0,0,0,.42)'
    ctx.fillRect(360, 0, 360, 360)
    ctx.fillStyle = 'rgba(255,255,255,.94)'
    ctx.font = '800 40px -apple-system, BlinkMacSystemFont, "PingFang SC", sans-serif'
    ctx.textAlign = 'left'
    ctx.textBaseline = 'alphabetic'
    const title = mesh.card.title || 'BoxPlayer'
    const words = title.length > 13 ? title.slice(0, 13) + '...' : title
    ctx.fillText(words, 386, 142)
    ctx.fillStyle = 'rgba(244,210,138,.76)'
    ctx.font = '700 22px -apple-system, BlinkMacSystemFont, "PingFang SC", sans-serif'
    const subtitle = mesh.card.subtitle || '网盘音乐'
    ctx.fillText(subtitle.length > 18 ? subtitle.slice(0, 18) + '...' : subtitle, 388, 184)
    ctx.fillStyle = 'rgba(255,255,255,.50)'
    ctx.font = '700 18px -apple-system, BlinkMacSystemFont, "PingFang SC", sans-serif'
    ctx.fillText(`${mesh.card.trackCount || 0} TRACKS`, 388, 264)

    const texture = new THREE.CanvasTexture(canvas)
    texture.colorSpace = THREE.SRGBColorSpace
    this.loadedTextures.set(key, texture)
    const material = mesh.coverPlane.material as THREE.MeshBasicMaterial
    material.color.set(0xffffff)
    material.map = texture
    material.needsUpdate = true
    const reflectionMaterial = mesh.reflectionPlane.material as THREE.MeshBasicMaterial
    reflectionMaterial.map = texture
    reflectionMaterial.needsUpdate = true
  }

  private makeCardSheenTexture() {
    const canvas = document.createElement('canvas')
    canvas.width = 512
    canvas.height = 256
    const ctx = canvas.getContext('2d')
    if (ctx) {
      ctx.clearRect(0, 0, canvas.width, canvas.height)
      const sweep = ctx.createLinearGradient(0, 0, canvas.width, canvas.height)
      sweep.addColorStop(0, 'rgba(255,255,255,0)')
      sweep.addColorStop(0.28, 'rgba(255,255,255,0.04)')
      sweep.addColorStop(0.42, 'rgba(255,255,255,0.18)')
      sweep.addColorStop(0.56, 'rgba(255,255,255,0.05)')
      sweep.addColorStop(1, 'rgba(255,255,255,0)')
      ctx.fillStyle = sweep
      ctx.fillRect(0, 0, canvas.width, canvas.height)
      const top = ctx.createLinearGradient(0, 0, 0, canvas.height)
      top.addColorStop(0, 'rgba(255,255,255,0.16)')
      top.addColorStop(0.3, 'rgba(255,255,255,0.03)')
      top.addColorStop(1, 'rgba(255,255,255,0)')
      ctx.fillStyle = top
      ctx.fillRect(0, 0, canvas.width, canvas.height)
    }
    const texture = new THREE.CanvasTexture(canvas)
    texture.colorSpace = THREE.SRGBColorSpace
    return texture
  }

  private disposeCardMesh(mesh: CardMesh) {
    if (mesh.titleSprite) {
      mesh.titleSprite.geometry.dispose()
      if (mesh.titleSprite.material instanceof THREE.Material) mesh.titleSprite.material.dispose()
    }
    mesh.coverPlane.geometry.dispose()
    mesh.reflectionPlane.geometry.dispose()
    mesh.borderPlane.geometry.dispose()
    mesh.glowPlane.geometry.dispose()
    mesh.sheenPlane.geometry.dispose()
    ;(mesh.coverPlane.material as THREE.Material).dispose()
    ;(mesh.reflectionPlane.material as THREE.Material).dispose()
    ;(mesh.borderPlane.material as THREE.Material).dispose()
    ;(mesh.glowPlane.material as THREE.Material).dispose()
    const sheenMaterial = mesh.sheenPlane.material as THREE.MeshBasicMaterial
    sheenMaterial.map?.dispose()
    sheenMaterial.dispose()
    mesh.group.clear()
  }

  private disposeShelfExtras() {
    if (this.connectorLines) {
      this.connectorLines.geometry.dispose()
      this.connectorLines.material.dispose()
      this.connectorLines = null
    }
    if (this.connectorParticles) {
      this.connectorParticles.geometry.dispose()
      this.connectorParticles.material.dispose()
      this.connectorParticles = null
    }
    if (this.floorMirror) {
      this.floorMirror.geometry.dispose()
      this.floorMirror.material.dispose()
      this.floorMirror = null
    }
  }

  // ─── update ───────────────────────────────────────────────

  update(dt: number, camera: THREE.Camera) {
    if (this.needsRebuild) this.rebuild()
    if (!this.cards.length) return
    const cameraPos = camera.position

    const maxScroll = Math.max(0, this.cards.length - 1)
    if (Math.abs(this.scrollVelocity) > 0.001) {
      this.targetScroll = clampRange(this.targetScroll + this.scrollVelocity, 0, maxScroll)
      this.scrollVelocity *= 0.84
    } else {
      this.scrollVelocity = 0
    }
    this.scrollOffset += (this.targetScroll - this.scrollOffset) * 0.12
    this.flowPhase = (this.flowPhase + dt * 0.72) % 1
    this.notifyCenter(Math.round(this.scrollOffset))

    const collapsed = this.config.presence === 'auto' && this.config.mode === 'side' && this.hoveredIndex < 0 && this.selectedIndex < 0
    const presenceTarget = collapsed ? 0.38 : 1
    this.presenceAlpha += (presenceTarget - this.presenceAlpha) * 0.1
    const collapse = 1 - this.presenceAlpha
    this.group.position.x = collapse * 2.2
    this.group.scale.setScalar(0.72 + this.presenceAlpha * 0.28)
    this.group.visible = this.presenceAlpha > 0.04

    for (let i = 0; i < this.cards.length; i++) {
      const m = this.cards[i]
      const isHovered = i === this.hoveredIndex
      const isSelected = i === this.selectedIndex
      const delta = i - this.scrollOffset
      const absD = Math.abs(delta)

      // Hover animation
      const hoverTarget = (isHovered || isSelected) ? 1 : 0
      m.hoverAlpha += (hoverTarget - m.hoverAlpha) * 0.15
      const expandTarget = (m.expanded || (absD < 0.42 && this.contentOpenIndex === i)) ? 1 : 0
      m.expandAlpha += (expandTarget - m.expandAlpha) * 0.16
      const hoverScale = 1 + m.hoverAlpha * 0.08 + m.expandAlpha * 0.1
      const passiveDim = this.config.presence === 'always' && this.config.mode === 'side' && !isHovered && !isSelected ? 0.76 : 1
      const glowAlpha = m.hoverAlpha * 0.35 + m.expandAlpha * 0.24

      m.group.scale.setScalar(hoverScale)
      ;(m.glowPlane.material as THREE.MeshBasicMaterial).opacity = glowAlpha
      ;(m.borderPlane.material as THREE.MeshBasicMaterial).opacity = (0.18 + this.config.bgOpacity * 0.14 + m.hoverAlpha * 0.3 + m.expandAlpha * 0.22) * passiveDim
      ;(m.coverPlane.material as THREE.MeshBasicMaterial).opacity = this.config.opacity * passiveDim
      ;(m.sheenPlane.material as THREE.MeshBasicMaterial).opacity = (0.10 + m.hoverAlpha * 0.08 + m.expandAlpha * 0.12) * passiveDim

      // Mineradio-style center-relative layout: every card is placed by its
      // distance to the smooth center, so the first/last card can still become
      // the visual center instead of staying tied to an absolute list position.
      const targetPos = new THREE.Vector3()
      let targetScale = hoverScale
      const distanceOpacity = absD < 0.5 ? 1 : Math.max(0.18, 1 - absD * 0.30)
      if (this.config.mode === 'side') {
        const detailMode = this.contentOpenIndex >= 0
        const detailCardScale = detailMode ? 0.66 : 1
        m.group.visible = detailMode ? false : absD <= 5.5
        targetPos.set(
          SIDE_SHELF_SAFE_X + (detailMode ? SIDE_DETAIL_CARD_OFFSET_X : 0) + absD * 0.05 + this.config.offsetX,
          this.config.offsetY - delta * SIDE_CARD_GAP_Y,
          1.08 - absD * 0.16 + this.config.offsetZ + m.expandAlpha * 0.18
        )
        targetScale *= (absD < 0.5 ? 1.02 + m.expandAlpha * 0.12 : Math.max(0.50, 0.98 - absD * 0.13)) * detailCardScale * this.config.size
        m.group.rotation.x += (-delta * 0.042 - m.group.rotation.x) * 0.12
        m.group.rotation.y += (THREE.MathUtils.degToRad(this.config.angleY + m.expandAlpha * 3) - m.group.rotation.y) * 0.12
      } else {
        m.group.visible = absD <= 5.5
        targetPos.set(
          this.config.offsetX + delta * STAGE_CARD_GAP_X,
          -2.2 + this.config.offsetY + m.expandAlpha * 0.12,
          1 + this.config.offsetZ - Math.min(2, absD) * 0.55 + m.expandAlpha * 0.26
        )
        targetScale *= (absD < 0.5 ? 1.20 + m.expandAlpha * 0.13 : Math.max(0.45, 1 - absD * 0.22)) * this.config.size
        m.group.rotation.x += (0.10 - absD * 0.04 + m.expandAlpha * 0.035 - m.group.rotation.x) * 0.12
        m.group.rotation.y += (-delta * 0.22 - m.group.rotation.y) * 0.12
      }
      m.group.position.lerp(targetPos, 0.12)
      m.group.scale.setScalar(targetScale)
      m.group.renderOrder = 60 + Math.round((6 - Math.min(6, absD)) * 10)
      ;(m.coverPlane.material as THREE.MeshBasicMaterial).opacity *= distanceOpacity
      ;(m.sheenPlane.material as THREE.MeshBasicMaterial).opacity *= distanceOpacity * this.presenceAlpha
      const reflectionMaterial = m.reflectionPlane.material as THREE.MeshBasicMaterial
      m.reflectionPlane.visible = this.config.mode === 'stage' && absD <= 3.4
      reflectionMaterial.opacity = (this.config.mode === 'stage' ? Math.max(0, 0.16 - absD * 0.035 + m.expandAlpha * 0.05) : 0) * this.presenceAlpha * this.config.bgOpacity
      ;(m.borderPlane.material as THREE.MeshBasicMaterial).opacity *= distanceOpacity

      // Face camera slightly
      if (this.config.cameraMode === 'dynamic') {
        this.dynamicLookHelper.position.copy(m.group.position)
        this.dynamicLookHelper.rotation.copy(m.group.rotation)
        this.dynamicLookHelper.lookAt(cameraPos.x, m.group.position.y, cameraPos.z)
        m.group.quaternion.slerp(this.dynamicLookHelper.quaternion, 0.16)
      }
    }
    this.updateShelfExtras()
    this.updateContentList(camera)
  }

  private createShelfExtras() {
    const maxSegments = Math.max(1, this.cards.length - 1)
    const geometry = new THREE.BufferGeometry()
    geometry.setAttribute('position', new THREE.BufferAttribute(new Float32Array(maxSegments * 6), 3))
    geometry.setDrawRange(0, 0)
    const material = new THREE.LineBasicMaterial({
      color: new THREE.Color(this.config.accentColor),
      transparent: true,
      opacity: 0.18,
      depthWrite: false,
      depthTest: true,
      blending: THREE.AdditiveBlending
    })
    this.connectorLines = new THREE.LineSegments(geometry, material)
    this.connectorLines.renderOrder = 18
    this.group.add(this.connectorLines)

    const particleGeometry = new THREE.BufferGeometry()
    particleGeometry.setAttribute('position', new THREE.BufferAttribute(new Float32Array(Math.max(1, maxSegments) * 18), 3))
    particleGeometry.setDrawRange(0, 0)
    const particleMaterial = new THREE.PointsMaterial({
      color: new THREE.Color(this.config.accentColor),
      transparent: true,
      opacity: 0.38,
      size: 0.045,
      depthWrite: false,
      depthTest: true,
      blending: THREE.AdditiveBlending
    })
    this.connectorParticles = new THREE.Points(particleGeometry, particleMaterial)
    this.connectorParticles.renderOrder = 19
    this.group.add(this.connectorParticles)

    const mirrorMaterial = new THREE.MeshBasicMaterial({
      color: new THREE.Color(this.config.accentColor),
      transparent: true,
      opacity: 0.07,
      depthWrite: false,
      depthTest: false,
      side: THREE.DoubleSide,
      blending: THREE.AdditiveBlending
    })
    this.floorMirror = new THREE.Mesh(new THREE.PlaneGeometry(7.2, 1.9), mirrorMaterial)
    this.floorMirror.rotation.x = -Math.PI / 2
    this.floorMirror.position.set(this.config.offsetX, -3.05 + this.config.offsetY * 0.35, 0.55 + this.config.offsetZ)
    this.floorMirror.renderOrder = 12
    this.group.add(this.floorMirror)
  }

  private updateShelfExtras() {
    if (!this.connectorLines || !this.connectorParticles || !this.floorMirror) return
    const connectorMaterial = this.connectorLines.material
    connectorMaterial.color.set(this.config.accentColor)
    connectorMaterial.opacity = (this.config.mode === 'off' ? 0 : 0.16 * this.presenceAlpha * this.config.bgOpacity)
    const visibleCards = this.cards.filter(card => card.group.visible).sort((a, b) => a.group.position.y - b.group.position.y)
    const positionAttr = this.connectorLines.geometry.getAttribute('position') as THREE.BufferAttribute
    const positions = positionAttr.array as Float32Array
    let segmentCount = 0
    for (let i = 0; i < visibleCards.length - 1; i += 1) {
      const a = visibleCards[i].group.position
      const b = visibleCards[i + 1].group.position
      const base = segmentCount * 6
      positions[base] = a.x
      positions[base + 1] = a.y
      positions[base + 2] = a.z - 0.06
      positions[base + 3] = b.x
      positions[base + 4] = b.y
      positions[base + 5] = b.z - 0.06
      segmentCount += 1
    }
    this.connectorLines.geometry.setDrawRange(0, segmentCount * 2)
    positionAttr.needsUpdate = true
    this.connectorLines.visible = this.contentOpenIndex < 0 && segmentCount > 0 && this.presenceAlpha > 0.08

    const particleAttr = this.connectorParticles.geometry.getAttribute('position') as THREE.BufferAttribute
    const particlePositions = particleAttr.array as Float32Array
    let particleCount = 0
    for (let i = 0; i < visibleCards.length - 1; i += 1) {
      const a = visibleCards[i].group.position
      const b = visibleCards[i + 1].group.position
      for (let p = 0; p < 3; p += 1) {
        const t = (this.flowPhase + p / 3 + i * 0.11) % 1
        const base = particleCount * 3
        particlePositions[base] = THREE.MathUtils.lerp(a.x, b.x, t)
        particlePositions[base + 1] = THREE.MathUtils.lerp(a.y, b.y, t)
        particlePositions[base + 2] = THREE.MathUtils.lerp(a.z - 0.03, b.z - 0.03, t)
        particleCount += 1
      }
    }
    this.connectorParticles.geometry.setDrawRange(0, particleCount)
    particleAttr.needsUpdate = true
    this.connectorParticles.visible = this.contentOpenIndex < 0 && particleCount > 0 && this.presenceAlpha > 0.12
    this.connectorParticles.material.color.set(this.config.accentColor)
    this.connectorParticles.material.opacity = 0.32 * this.presenceAlpha * this.config.bgOpacity
    this.connectorParticles.material.size = 0.038 + this.config.size * 0.014

    this.floorMirror.visible = this.config.mode === 'stage' && this.presenceAlpha > 0.08
    this.floorMirror.position.set(this.config.offsetX, -3.05 + this.config.offsetY * 0.35, 0.55 + this.config.offsetZ)
    this.floorMirror.scale.setScalar(this.config.size)
    this.floorMirror.material.color.set(this.config.accentColor)
    this.floorMirror.material.opacity = 0.08 * this.presenceAlpha * this.config.bgOpacity
  }

  // ─── interaction ──────────────────────────────────────────

  /** Raycast against all cards. Returns index or -1. */
  raycast(raycaster: THREE.Raycaster): number {
    if (!this.cards.length) return -1
    const visibleCards = this.cards
      .map((card, index) => ({ card, index }))
      .filter(({ card }) => card.group.visible && card.coverPlane.visible)
      .sort((a, b) => (b.card.group.renderOrder || 0) - (a.card.group.renderOrder || 0))
    const meshes = visibleCards.map(c => c.card.coverPlane)
    const intersects = raycaster.intersectObjects(meshes)
    if (intersects.length) {
      const meshIndex = meshes.indexOf(intersects[0].object as THREE.Mesh)
      return visibleCards[meshIndex]?.index ?? -1
    }
    return -1
  }

  pickAtScreen(camera: THREE.Camera, width: number, height: number, sx: number, sy: number, pad = 72): number {
    if (!this.cards.length || width <= 0 || height <= 0) return -1
    const ordered = this.cards
      .map((card, index) => ({ card, index }))
      .filter(({ card }) => card.group.visible)
      .sort((a, b) => (b.card.group.renderOrder || 0) - (a.card.group.renderOrder || 0))
    const halfW = SHELF_CARD_WIDTH / 2
    const halfH = SHELF_CARD_HEIGHT / 2
    const corners = [
      new THREE.Vector3(-halfW, -halfH, 0),
      new THREE.Vector3(halfW, -halfH, 0),
      new THREE.Vector3(halfW, halfH, 0),
      new THREE.Vector3(-halfW, halfH, 0)
    ]
    for (const { card, index } of ordered) {
      card.coverPlane.updateMatrixWorld(true)
      let minX = Infinity
      let minY = Infinity
      let maxX = -Infinity
      let maxY = -Infinity
      for (const base of corners) {
        const p = base.clone().applyMatrix4(card.coverPlane.matrixWorld).project(camera)
        const x = (p.x + 1) * width * 0.5
        const y = (1 - p.y) * height * 0.5
        minX = Math.min(minX, x)
        minY = Math.min(minY, y)
        maxX = Math.max(maxX, x)
        maxY = Math.max(maxY, y)
      }
      if (sx >= minX - pad && sx <= maxX + pad && sy >= minY - pad && sy <= maxY + pad) return index
    }
    return -1
  }

  setHovered(index: number) {
    this.hoveredIndex = index
  }

  setHoveredContent(index: number) {
    this.hoveredContentIndex = index
  }

  click(index: number) {
    if (index < 0 || index >= this.cards.length) return
    const centerIndex = Math.round(this.targetScroll)
    if (index !== centerIndex) {
      this.closeContent()
      this.targetScroll = clampRange(index, 0, this.cards.length - 1)
      this.selectedIndex = index
      this.cards.forEach((c, i) => { c.expanded = i === index })
      this.notifyCenter(index)
      this.playExpandSfx()
      return
    }
    if (this.contentOpenIndex === index) {
      this.closeContent()
      this.selectedIndex = index
      this.playClickSfx()
      return
    }
    this.selectedIndex = index
    this.cards.forEach((c, i) => { c.expanded = i === index })
    this.openContent(index)
    this.playClickSfx()
  }

  scrollBy(step: number) {
    if (!this.cards.length) return
    if (Math.abs(step) < 0.001) return
    const direction = step > 0 ? 1 : -1
    const impulse = clampRange(Math.abs(step), 1, 2.4) * direction
    const next = Math.round(this.targetScroll) + direction
    if ((next < 0 || next >= this.cards.length) && this.onBoundaryScroll?.(direction)) {
      this.closeContent()
      this.scrollVelocity = 0
      this.playExpandSfx()
      return
    }
    if (next !== Math.round(this.targetScroll)) this.closeContent()
    this.scrollVelocity = clampRange(this.scrollVelocity + impulse * 0.18, -0.85, 0.85)
    this.targetScroll = clampRange(this.targetScroll + impulse * 0.46, 0, this.cards.length - 1)
    this.selectedIndex = Math.round(this.targetScroll)
    this.cards.forEach((c, i) => { c.expanded = i === this.selectedIndex })
    this.playScrollSfx()
  }

  scroll(deltaY: number) {
    this.scrollBy(deltaY === 0 ? 0 : clampRange(deltaY / 120, -2.4, 2.4))
  }

  next() {
    this.scrollBy(1)
  }

  prev() {
    this.scrollBy(-1)
  }

  getCenterIdx() {
    return Math.round(this.scrollOffset)
  }

  getCenterCard() {
    const index = this.getCenterIdx()
    return this.cards[index]?.card || null
  }

  openContent(index: number) {
    if (index < 0 || index >= this.cards.length) return
    const tracks = this.cards[index].card.data?.tracks
    if (!Array.isArray(tracks) || !tracks.length) {
      this.closeContent()
      return
    }
    this.contentOpenIndex = index
    this.contentAnchorIndex = index
    this.contentTargetAlpha = 1
    this.selectedIndex = index
    this.cards.forEach((c, i) => { c.expanded = i === index })
    this.disposeContentRows()
    this.contentGroup.visible = true
    const activeKey = this.cards[index].card.data?.activeKey || ''
    const activeIndex = activeKey ? tracks.findIndex((track: any) => shelfTrackKey(track) === activeKey) : -1
    this.contentTargetScroll = activeIndex >= 0 ? activeIndex : 0
    this.contentScrollOffset = this.contentTargetScroll
    this.contentBackdrop = this.createContentBackdrop(this.cards[index].card, tracks.length)
    this.contentGroup.add(this.contentBackdrop)
    this.contentScrollRail = this.createContentScrollRail()
    this.contentGroup.add(this.contentScrollRail.group)
    for (let i = 0; i < tracks.length; i += 1) {
      const row = this.createContentRow(tracks[i], i, activeKey)
      this.contentGroup.add(row.group)
      this.contentRows.push(row)
    }
    this.contentClose = this.createContentCloseControl()
    this.contentGroup.add(this.contentClose.group)
    this.playExpandSfx()
  }

  closeContent() {
    const hadContent = this.contentOpenIndex >= 0 || this.contentRows.length > 0
    this.contentOpenIndex = -1
    this.hoveredContentIndex = -1
    this.contentTargetAlpha = 0
    if (hadContent) {
      this.cards.forEach(c => { c.expanded = false })
      this.playClickSfx()
    }
    return hadContent
  }

  hasOpenContent() {
    return this.contentOpenIndex >= 0 && this.contentRows.length > 0
  }

  hasVisibleContent() {
    return this.contentRows.length > 0 && (this.contentOpenIndex >= 0 || this.contentAlpha > 0.04 || this.contentTargetAlpha > 0)
  }

  scrollContent(deltaY: number) {
    if (!this.contentRows.length) return
    const next = Math.round(this.contentTargetScroll) + (deltaY > 0 ? 1 : -1)
    this.contentTargetScroll = clampRange(next, 0, Math.max(0, this.contentRows.length - 1))
    this.playScrollSfx()
  }

  clickContentRow(index: number, action: ContentRowAction = 'play') {
    const row = this.contentRows[index]
    if (!row) return false
    this.contentTargetScroll = clampRange(index, 0, Math.max(0, this.contentRows.length - 1))
    this.contentScrollOffset = this.contentTargetScroll
    this.updateContentList()
    if (action === 'next') {
      if (this.onTrackNext) this.onTrackNext(row.track)
      this.playExpandSfx()
      return true
    }
    if (action === 'collect') {
      if (this.onTrackCollect) this.onTrackCollect(row.track)
      this.playExpandSfx()
      return true
    }
    if (this.onTrackClick) this.onTrackClick(row.track)
    this.playClickSfx()
    return true
  }

  pickContentAtScreen(camera: THREE.Camera, width: number, height: number, sx: number, sy: number, pad = 10): number {
    return this.pickContentActionAtScreen(camera, width, height, sx, sy, pad)?.index ?? -1
  }

  pickContentActionAtScreen(camera: THREE.Camera, width: number, height: number, sx: number, sy: number, pad = 10): { index: number; action: ContentRowAction } | null {
    if (!this.hasOpenContent() || width <= 0 || height <= 0) return null
    const halfW = 2.75
    const halfH = 0.30
    const corners = [
      new THREE.Vector3(-halfW, -halfH, 0),
      new THREE.Vector3(halfW, -halfH, 0),
      new THREE.Vector3(halfW, halfH, 0),
      new THREE.Vector3(-halfW, halfH, 0)
    ]
    const rows = this.contentRows
      .filter(row => row.group.visible)
      .sort((a, b) => (b.group.renderOrder || 0) - (a.group.renderOrder || 0))
    for (const row of rows) {
      row.plane.updateMatrixWorld(true)
      let minX = Infinity
      let minY = Infinity
      let maxX = -Infinity
      let maxY = -Infinity
      for (const base of corners) {
        const p = base.clone().applyMatrix4(row.plane.matrixWorld).project(camera)
        const x = (p.x + 1) * width * 0.5
        const y = (1 - p.y) * height * 0.5
        minX = Math.min(minX, x)
        minY = Math.min(minY, y)
        maxX = Math.max(maxX, x)
        maxY = Math.max(maxY, y)
      }
      if (sx >= minX - pad && sx <= maxX + pad && sy >= minY - pad && sy <= maxY + pad) {
        const relX = (sx - minX) / Math.max(1, maxX - minX)
        const action: ContentRowAction = relX > 0.93 ? 'play' : relX > 0.865 ? 'collect' : relX > 0.79 ? 'next' : 'play'
        return { index: row.index, action }
      }
    }
    return null
  }

  pickContentCloseAtScreen(camera: THREE.Camera, width: number, height: number, sx: number, sy: number, pad = 8): boolean {
    if (!this.hasOpenContent() || !this.contentClose || width <= 0 || height <= 0) return false
    const halfW = 0.41
    const halfH = 0.13
    const corners = [
      new THREE.Vector3(-halfW, -halfH, 0),
      new THREE.Vector3(halfW, -halfH, 0),
      new THREE.Vector3(halfW, halfH, 0),
      new THREE.Vector3(-halfW, halfH, 0)
    ]
    this.contentClose.plane.updateMatrixWorld(true)
    let minX = Infinity
    let minY = Infinity
    let maxX = -Infinity
    let maxY = -Infinity
    for (const base of corners) {
      const p = base.clone().applyMatrix4(this.contentClose.plane.matrixWorld).project(camera)
      const x = (p.x + 1) * width * 0.5
      const y = (1 - p.y) * height * 0.5
      minX = Math.min(minX, x)
      minY = Math.min(minY, y)
      maxX = Math.max(maxX, x)
      maxY = Math.max(maxY, y)
    }
    return sx >= minX - pad && sx <= maxX + pad && sy >= minY - pad && sy <= maxY + pad
  }

  pickContentPanelAtScreen(camera: THREE.Camera, width: number, height: number, sx: number, sy: number, pad = 18): boolean {
    if (!this.hasOpenContent() || !this.contentBackdrop || width <= 0 || height <= 0) return false
    const halfW = 2.95
    const halfH = 2.275
    const corners = [
      new THREE.Vector3(-halfW, -halfH, 0),
      new THREE.Vector3(halfW, -halfH, 0),
      new THREE.Vector3(halfW, halfH, 0),
      new THREE.Vector3(-halfW, halfH, 0)
    ]
    this.contentBackdrop.updateMatrixWorld(true)
    let minX = Infinity
    let minY = Infinity
    let maxX = -Infinity
    let maxY = -Infinity
    for (const base of corners) {
      const p = base.clone().applyMatrix4(this.contentBackdrop.matrixWorld).project(camera)
      const x = (p.x + 1) * width * 0.5
      const y = (1 - p.y) * height * 0.5
      minX = Math.min(minX, x)
      minY = Math.min(minY, y)
      maxX = Math.max(maxX, x)
      maxY = Math.max(maxY, y)
    }
    return sx >= minX - pad && sx <= maxX + pad && sy >= minY - pad && sy <= maxY + pad
  }

  private notifyCenter(index: number) {
    const next = clampRange(index, -1, this.cards.length - 1)
    if (next === this.lastCenterIndex) return
    this.lastCenterIndex = next
    if (this.onCenterChange) this.onCenterChange(next >= 0 ? this.cards[next]?.card || null : null, next)
  }

  // ─── SFX ──────────────────────────────────────────────────

  private playClickSfx() {
    const now = performance.now()
    if (now - this.lastClickSfxAt < 80) return
    this.lastClickSfxAt = now
    this.beepCluster([
      [780, 0.032, 'square', 0.042, 0],
      [410, 0.038, 'triangle', 0.024, 24],
    ])
  }

  private playScrollSfx() {
    const now = performance.now()
    if (now - this.lastScrollSfxAt < 110) return
    this.lastScrollSfxAt = now
    const base = 300 + Math.min(360, Math.abs(this.targetScroll - this.scrollOffset) * 110)
    this.beepCluster([
      [base, 0.022, 'square', 0.016, 0],
      [base * 1.42, 0.018, 'triangle', 0.01, 18],
    ])
  }

  private playExpandSfx() {
    this.beepCluster([
      [260, 0.05, 'sawtooth', 0.024, 0],
      [520, 0.04, 'triangle', 0.018, 34],
      [910, 0.028, 'square', 0.011, 66],
    ])
  }

  private createContentRow(track: any, index: number, activeKey: string): ContentRowMesh {
    const canvas = document.createElement('canvas')
    canvas.width = 1440
    canvas.height = 176
    const ctx = canvas.getContext('2d')
    if (ctx) {
      const isActive = !!activeKey && shelfTrackKey(track) === activeKey
      const accent = this.config.accentColor || '#f4d28a'
      ctx.clearRect(0, 0, canvas.width, canvas.height)
      const bg = ctx.createLinearGradient(0, 0, canvas.width, 0)
      bg.addColorStop(0, isActive ? 'rgba(0,245,212,.28)' : 'rgba(255,255,255,.08)')
      bg.addColorStop(0.54, 'rgba(9,11,16,.62)')
      bg.addColorStop(1, 'rgba(244,210,138,.08)')
      ctx.fillStyle = bg
      roundRect(ctx, 10, 12, canvas.width - 20, canvas.height - 24, 32)
      ctx.fill()
      ctx.strokeStyle = isActive ? 'rgba(0,245,212,.58)' : 'rgba(255,255,255,.13)'
      ctx.lineWidth = 3
      roundRect(ctx, 10, 12, canvas.width - 20, canvas.height - 24, 32)
      ctx.stroke()
      ctx.fillStyle = isActive ? '#00f5d4' : 'rgba(244,210,138,.82)'
      ctx.font = '800 42px -apple-system, BlinkMacSystemFont, "PingFang SC", sans-serif'
      ctx.textAlign = 'right'
      ctx.textBaseline = 'middle'
      ctx.fillText(String(index + 1).padStart(2, '0'), 120, 88)
      ctx.textAlign = 'left'
      ctx.fillStyle = 'rgba(255,255,255,.94)'
      ctx.font = '800 44px -apple-system, BlinkMacSystemFont, "PingFang SC", sans-serif'
      const name = track?.file_name || track?.name || '未知曲目'
      ctx.fillText(truncateCanvasText(ctx, name, 930), 160, 74)
      ctx.fillStyle = 'rgba(255,255,255,.45)'
      ctx.font = '700 26px -apple-system, BlinkMacSystemFont, "PingFang SC", sans-serif'
      ctx.fillText(truncateCanvasText(ctx, track?.artist || track?.description || 'BoxPlayer Radio', 900), 160, 120)
      ctx.fillStyle = 'rgba(255,255,255,.56)'
      ctx.font = '800 22px -apple-system, BlinkMacSystemFont, "PingFang SC", sans-serif'
      ctx.textAlign = 'right'
      ctx.fillText('NEXT', 1205, 92)
      ctx.fillStyle = 'rgba(255,255,255,.48)'
      ctx.fillText('SAVE', 1310, 92)
      ctx.fillStyle = accent
      ctx.fillText('PLAY', 1410, 92)
    }
    const texture = new THREE.CanvasTexture(canvas)
    texture.colorSpace = THREE.SRGBColorSpace
    const material = new THREE.MeshBasicMaterial({
      map: texture,
      transparent: true,
      opacity: 0,
      depthWrite: false,
      depthTest: false,
      side: THREE.DoubleSide
    })
    const plane = new THREE.Mesh(new THREE.PlaneGeometry(5.5, 0.60), material)
    const group = new THREE.Group()
    group.add(plane)
    group.visible = false
    return { group, plane, track, index }
  }

  private createContentBackdrop(card: ShelfCard, trackCount: number) {
    const canvas = document.createElement('canvas')
    canvas.width = 1280
    canvas.height = 980
    const ctx = canvas.getContext('2d')
    if (ctx) {
      const accent = this.config.accentColor || '#f4d28a'
      ctx.clearRect(0, 0, canvas.width, canvas.height)
      const bg = ctx.createLinearGradient(0, 0, canvas.width, canvas.height)
      bg.addColorStop(0, 'rgba(255,255,255,.12)')
      bg.addColorStop(0.36, 'rgba(18,20,27,.76)')
      bg.addColorStop(1, 'rgba(0,0,0,.82)')
      ctx.fillStyle = bg
      roundRect(ctx, 18, 18, canvas.width - 36, canvas.height - 36, 44)
      ctx.fill()
      ctx.strokeStyle = 'rgba(255,255,255,.18)'
      ctx.lineWidth = 4
      roundRect(ctx, 18, 18, canvas.width - 36, canvas.height - 36, 44)
      ctx.stroke()
      ctx.strokeStyle = accent
      ctx.globalAlpha = 0.34
      ctx.lineWidth = 3
      roundRect(ctx, 34, 34, canvas.width - 68, canvas.height - 68, 34)
      ctx.stroke()
      ctx.globalAlpha = 1
      ctx.fillStyle = accent
      ctx.font = '900 34px -apple-system, BlinkMacSystemFont, "PingFang SC", sans-serif'
      ctx.textAlign = 'left'
      ctx.textBaseline = 'alphabetic'
      ctx.fillText(`${trackCount} TRACKS`, 76, 116)
      ctx.fillStyle = 'rgba(255,255,255,.96)'
      ctx.font = '900 54px -apple-system, BlinkMacSystemFont, "PingFang SC", sans-serif'
      const title = card.title || '当前队列'
      ctx.fillText(truncateCanvasText(ctx, title, 820), 76, 186)
      ctx.fillStyle = 'rgba(255,255,255,.52)'
      ctx.font = '760 28px -apple-system, BlinkMacSystemFont, "PingFang SC", sans-serif'
      const subtitle = card.subtitle || 'BoxPlayer Radio'
      ctx.fillText(truncateCanvasText(ctx, subtitle, 880), 78, 236)
      const line = ctx.createLinearGradient(74, 286, canvas.width - 74, 286)
      line.addColorStop(0, accent)
      line.addColorStop(0.52, 'rgba(255,255,255,.18)')
      line.addColorStop(1, 'rgba(255,255,255,0)')
      ctx.fillStyle = line
      ctx.fillRect(76, 284, canvas.width - 152, 3)
    }
    const texture = new THREE.CanvasTexture(canvas)
    texture.colorSpace = THREE.SRGBColorSpace
    const material = new THREE.MeshBasicMaterial({
      map: texture,
      transparent: true,
      opacity: 0,
      depthWrite: false,
      depthTest: false,
      side: THREE.DoubleSide
    })
    const backdrop = new THREE.Mesh(new THREE.PlaneGeometry(5.9, 4.55), material)
    backdrop.position.set(0, -0.04, -0.10)
    backdrop.renderOrder = 82
    return backdrop
  }

  private createContentCloseControl(): ContentCloseMesh {
    const canvas = document.createElement('canvas')
    canvas.width = 220
    canvas.height = 72
    const ctx = canvas.getContext('2d')
    if (ctx) {
      const accent = this.config.accentColor || '#f4d28a'
      ctx.clearRect(0, 0, canvas.width, canvas.height)
      const bg = ctx.createLinearGradient(0, 0, canvas.width, 0)
      bg.addColorStop(0, 'rgba(255,255,255,.12)')
      bg.addColorStop(1, 'rgba(0,0,0,.58)')
      ctx.fillStyle = bg
      roundRect(ctx, 8, 8, canvas.width - 16, canvas.height - 16, 22)
      ctx.fill()
      ctx.strokeStyle = 'rgba(255,255,255,.22)'
      ctx.lineWidth = 2
      roundRect(ctx, 8, 8, canvas.width - 16, canvas.height - 16, 22)
      ctx.stroke()
      ctx.fillStyle = accent
      ctx.font = '900 18px -apple-system, BlinkMacSystemFont, "PingFang SC", sans-serif'
      ctx.textAlign = 'center'
      ctx.textBaseline = 'middle'
      ctx.fillText('BACK', 110, 37)
    }
    const texture = new THREE.CanvasTexture(canvas)
    texture.colorSpace = THREE.SRGBColorSpace
    const material = new THREE.MeshBasicMaterial({
      map: texture,
      transparent: true,
      opacity: 0,
      depthWrite: false,
      depthTest: false,
      side: THREE.DoubleSide
    })
    const plane = new THREE.Mesh(new THREE.PlaneGeometry(0.82, 0.26), material)
    const group = new THREE.Group()
    group.add(plane)
    group.visible = false
    return { group, plane }
  }

  private createContentScrollRail(): ContentScrollRailMesh {
    const trackMat = new THREE.MeshBasicMaterial({
      color: 0xffffff,
      transparent: true,
      opacity: 0,
      depthWrite: false,
      depthTest: false,
      side: THREE.DoubleSide
    })
    const thumbMat = new THREE.MeshBasicMaterial({
      color: new THREE.Color(this.config.accentColor),
      transparent: true,
      opacity: 0,
      depthWrite: false,
      depthTest: false,
      side: THREE.DoubleSide,
      blending: THREE.AdditiveBlending
    })
    const track = new THREE.Mesh(new THREE.PlaneGeometry(0.035, 2.72), trackMat)
    const thumb = new THREE.Mesh(new THREE.PlaneGeometry(0.055, 0.56), thumbMat)
    const group = new THREE.Group()
    group.add(track)
    group.add(thumb)
    group.visible = false
    return { group, track, thumb }
  }

  private updateContentList(camera?: THREE.Camera) {
    if (!this.contentRows.length) return
    this.contentAlpha += (this.contentTargetAlpha - this.contentAlpha) * 0.18
    if (this.contentTargetAlpha <= 0 && this.contentAlpha < 0.025) {
      this.disposeContentRows()
      this.contentGroup.visible = false
      return
    }
    this.contentScrollOffset += (this.contentTargetScroll - this.contentScrollOffset) * 0.18
    const isSide = this.config.mode === 'side'
    if (isSide && camera) {
      const viewportAnchor = new THREE.Vector3(SIDE_DETAIL_VIEWPORT_X, SIDE_DETAIL_VIEWPORT_Y, 0.5).unproject(camera)
      const viewportDirection = viewportAnchor.sub(camera.position).normalize()
      this.contentGroup.position.copy(camera.position).add(viewportDirection.multiplyScalar(SIDE_DETAIL_VIEWPORT_DISTANCE + this.config.offsetZ * 0.18))
      this.contentGroup.position.x += this.config.offsetX * 0.12
      this.contentGroup.position.y += this.config.offsetY * 0.12
      this.contentGroup.quaternion.slerp(camera.quaternion, 0.22)
    } else {
      this.contentGroup.position.set(this.config.offsetX, -0.98 + this.config.offsetY, 1.10 + this.config.offsetZ)
      if (this.config.cameraMode === 'dynamic' && camera) this.contentGroup.quaternion.slerp(camera.quaternion, 0.18)
      else this.contentGroup.rotation.set(0.08, 0, 0)
    }
    this.contentGroup.scale.setScalar(this.config.size * (isSide ? SIDE_CONTENT_SCALE : 0.92))
    this.contentGroup.visible = true
    const ease = Math.sin(clamp01(this.contentAlpha) * Math.PI * 0.5)
    this.contentGroup.position.y += (1 - ease) * 0.18
    if (this.contentBackdrop) {
      this.contentBackdrop.visible = ease > 0.025
      this.contentBackdrop.position.set(0, -0.08, -0.16)
      this.contentBackdrop.scale.setScalar(0.98 + ease * 0.02)
      this.contentBackdrop.renderOrder = 82
      this.contentBackdrop.material.opacity = 0.86 * ease * this.presenceAlpha
    }
    if (this.contentScrollRail) {
      const maxScroll = Math.max(1, this.contentRows.length - 1)
      const progress = clampRange(this.contentScrollOffset / maxScroll, 0, 1)
      const railSpan = 2.24
      this.contentScrollRail.group.visible = ease > 0.04 && this.contentRows.length > 1
      this.contentScrollRail.group.position.set(2.34, -0.84, 0.08)
      this.contentScrollRail.group.renderOrder = 146
      this.contentScrollRail.track.material.opacity = 0.18 * ease * this.presenceAlpha
      this.contentScrollRail.thumb.position.y = railSpan * 0.5 - progress * railSpan
      this.contentScrollRail.thumb.material.color.set(this.config.accentColor)
      this.contentScrollRail.thumb.material.opacity = 0.72 * ease * this.presenceAlpha
    }
    for (const row of this.contentRows) {
      const delta = row.index - this.contentScrollOffset
      const absD = Math.abs(delta)
      const isHovered = row.index === this.hoveredContentIndex
      const rowIntro = clamp01(this.contentAlpha * 1.4 - Math.min(0.48, row.index * 0.045))
      const rowEase = Math.sin(rowIntro * Math.PI * 0.5)
      row.group.visible = absD <= 3.8
      row.group.position.set((1 - rowEase) * (isSide ? 0.42 : -0.26), -0.82 - delta * (isSide ? 0.74 : 0.42), 0.04 - (1 - rowEase) * 0.12)
      row.group.scale.setScalar((isHovered ? 1.025 : 1) * (0.98 + rowEase * 0.02))
      row.group.renderOrder = 90 + Math.round((5 - Math.min(5, absD)) * 10) + (isHovered ? 8 : 0)
      row.plane.material.opacity = Math.min(1, Math.max(0.16, 0.88 - absD * 0.15) + (isHovered ? 0.12 : 0)) * this.presenceAlpha * ease * rowEase
    }
    if (this.contentClose) {
      this.contentClose.group.visible = ease > 0.04
      this.contentClose.group.position.set(2.00, 1.84, 0.10)
      this.contentClose.group.scale.setScalar(0.98 + ease * 0.04)
      this.contentClose.group.renderOrder = 148
      this.contentClose.plane.material.opacity = 0.84 * ease * this.presenceAlpha
    }
  }

  private disposeContentRows() {
    for (const row of this.contentRows) {
      row.plane.geometry.dispose()
      const material = row.plane.material
      material.map?.dispose()
      material.dispose()
      row.group.clear()
    }
    this.contentRows = []
    if (this.contentBackdrop) {
      this.contentBackdrop.geometry.dispose()
      const material = this.contentBackdrop.material
      material.map?.dispose()
      material.dispose()
      this.contentBackdrop = null
    }
    if (this.contentScrollRail) {
      this.contentScrollRail.track.geometry.dispose()
      this.contentScrollRail.thumb.geometry.dispose()
      this.contentScrollRail.track.material.dispose()
      this.contentScrollRail.thumb.material.dispose()
      this.contentScrollRail.group.clear()
      this.contentScrollRail = null
    }
    if (this.contentClose) {
      this.contentClose.plane.geometry.dispose()
      const material = this.contentClose.plane.material
      material.map?.dispose()
      material.dispose()
      this.contentClose.group.clear()
      this.contentClose = null
    }
    this.contentGroup.clear()
    this.contentAlpha = 0
  }

  private beep(freq: number, duration: number, type: OscillatorType, vol: number) {
    try {
      if (!this.audioCtx) this.audioCtx = new AudioContext()
      const osc = this.audioCtx.createOscillator()
      const gain = this.audioCtx.createGain()
      osc.type = type
      osc.frequency.value = freq
      gain.gain.setValueAtTime(vol, this.audioCtx.currentTime)
      gain.gain.exponentialRampToValueAtTime(0.001, this.audioCtx.currentTime + duration)
      osc.connect(gain)
      gain.connect(this.audioCtx.destination)
      osc.start()
      osc.stop(this.audioCtx.currentTime + duration)
    } catch { /* ignore audio context errors */ }
  }

  private beepCluster(notes: Array<[number, number, OscillatorType, number, number]>) {
    for (const [freq, duration, type, vol, delayMs] of notes) {
      window.setTimeout(() => this.beep(freq, duration, type, vol), delayMs)
    }
  }

  // ─── dispose ──────────────────────────────────────────────

  dispose() {
    for (const m of this.cards) this.disposeCardMesh(m)
    this.disposeShelfExtras()
    this.cards = []
    this.group.clear()
    this.loadedTextures.forEach(t => t.dispose())
    this.loadedTextures.clear()
    if (this.audioCtx) {
      this.audioCtx.close().catch(() => {})
      this.audioCtx = null
    }
  }
}
