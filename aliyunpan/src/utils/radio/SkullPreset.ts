import * as THREE from 'three'

export const SKULL_PRESET_ASSET = '/assets/@mineradio/skull-decimation-points.bin'

function createSkullMaterial(accentColor = '#f4d28a') {
  void accentColor
  return new THREE.PointsMaterial({ size: 0.045, vertexColors: true, transparent: true, opacity: 0.78, depthWrite: false, blending: THREE.AdditiveBlending })
}

function colorizeSkull(count: number, accentColor = '#f4d28a') {
  const colors = new Float32Array(count * 3)
  const accent = new THREE.Color(accentColor)
  const bone = new THREE.Color('#d9d2bf')
  for (let i = 0; i < count; i += 1) {
    const c = bone.clone().lerp(accent, Math.random() * 0.28)
    colors[i * 3] = c.r
    colors[i * 3 + 1] = c.g
    colors[i * 3 + 2] = c.b
  }
  return colors
}

export function applySkullPointBuffer(points: THREE.Points<THREE.BufferGeometry, THREE.PointsMaterial>, buffer: ArrayBuffer, accentColor = '#f4d28a') {
  const floats = new Float32Array(buffer)
  const count = Math.floor(floats.length / 3)
  if (count < 64) return false
  const positions = new Float32Array(count * 3)
  let maxAbs = 0.001
  for (let i = 0; i < count * 3; i += 1) maxAbs = Math.max(maxAbs, Math.abs(floats[i] || 0))
  const scale = maxAbs > 8 ? 3.8 / maxAbs : 1
  for (let i = 0; i < count; i += 1) {
    positions[i * 3] = (floats[i * 3] || 0) * scale
    positions[i * 3 + 1] = (floats[i * 3 + 1] || 0) * scale - 0.35
    positions[i * 3 + 2] = (floats[i * 3 + 2] || 0) * scale + 0.2
  }
  const geometry = new THREE.BufferGeometry()
  geometry.setAttribute('position', new THREE.BufferAttribute(positions, 3))
  geometry.setAttribute('color', new THREE.BufferAttribute(colorizeSkull(count, accentColor), 3))
  points.geometry.dispose()
  points.geometry = geometry
  points.material.size = 0.035
  points.material.opacity = 0.84
  return true
}

export function createSkullPreset(accentColor = '#f4d28a') {
  const count = 1400
  const positions = new Float32Array(count * 3)
  for (let i = 0; i < count; i += 1) {
    const t = Math.random() * Math.PI * 2
    const v = Math.random() * 2 - 1
    const head = Math.random() < 0.72
    const rx = head ? 2.35 : 1.28
    const ry = head ? 2.9 : 1.05
    const rz = head ? 1.62 : 0.92
    const yBase = head ? 0.36 : -2.15
    let x = Math.cos(t) * Math.sqrt(1 - v * v) * rx
    let y = v * ry + yBase
    let z = Math.sin(t) * Math.sqrt(1 - v * v) * rz
    const eye = (Math.hypot(x - 0.72, y - 0.55) < 0.42 || Math.hypot(x + 0.72, y - 0.55) < 0.42) && z > 0
    const nose = Math.abs(x) < 0.22 && y > -0.2 && y < 0.46 && z > 0.58
    if (eye || nose) z -= 0.96
    x += (Math.random() - 0.5) * 0.12
    y += (Math.random() - 0.5) * 0.12
    z += (Math.random() - 0.5) * 0.12
    positions[i * 3] = x
    positions[i * 3 + 1] = y
    positions[i * 3 + 2] = z + 0.8
  }
  const geometry = new THREE.BufferGeometry()
  geometry.setAttribute('position', new THREE.BufferAttribute(positions, 3))
  geometry.setAttribute('color', new THREE.BufferAttribute(colorizeSkull(count, accentColor), 3))
  const material = createSkullMaterial(accentColor)
  const points = new THREE.Points(geometry, material)
  points.renderOrder = 8
  return points
}
