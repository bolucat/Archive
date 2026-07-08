/**
 * FreeCamera — WASD + mouse free-roam camera controller for Three.js scenes.
 * Ported from Mineradio's toggleFreeCamera / updateFreeCamera.
 */

import * as THREE from 'three'

export class FreeCamera {
  enabled = false
  private keys = new Set<string>()
  private mouse = { x: 0, y: 0, prevX: 0, prevY: 0 }
  private yaw = 0
  private pitch = 0
  private velocity = new THREE.Vector3()
  private baseSpeed = 3.5
  private sensitivity = 0.002

  private onKey = (e: KeyboardEvent) => { this.keys.add(e.code); if (e.code === 'KeyR') this.toggle(); if (e.code === 'KeyK') this.resetPosition() }
  private onKeyUp = (e: KeyboardEvent) => { this.keys.delete(e.code) }
  private onMouse = (e: MouseEvent) => {
    this.mouse.prevX = this.mouse.x
    this.mouse.prevY = this.mouse.y
    this.mouse.x = e.clientX
    this.mouse.y = e.clientY
  }

  private targetPos = new THREE.Vector3(0, 0, 18)
  private currentPos = new THREE.Vector3(0, 0, 18)

  attach() {
    document.addEventListener('keydown', this.onKey)
    document.addEventListener('keyup', this.onKeyUp)
    document.addEventListener('mousemove', this.onMouse)
  }

  detach() {
    document.removeEventListener('keydown', this.onKey)
    document.removeEventListener('keyup', this.onKeyUp)
    document.removeEventListener('mousemove', this.onMouse)
  }

  toggle() { this.enabled = !this.enabled }

  private resetPosition() {
    this.targetPos.set(0, 0, 18)
    this.currentPos.set(0, 0, 18)
    this.yaw = 0
    this.pitch = 0
  }

  update(dt: number, camera: THREE.PerspectiveCamera) {
    if (!this.enabled) return

    const dx = this.mouse.x - this.mouse.prevX
    const dy = this.mouse.y - this.mouse.prevY
    this.mouse.prevX = this.mouse.x
    this.mouse.prevY = this.mouse.y

    this.yaw -= dx * this.sensitivity
    this.pitch -= dy * this.sensitivity
    this.pitch = Math.max(-1.4, Math.min(1.4, this.pitch))

    const speed = this.baseSpeed * dt * (this.keys.has('ShiftLeft') || this.keys.has('ShiftRight') ? 2.5 : 1)
    const forward = new THREE.Vector3(0, 0, -1).applyQuaternion(new THREE.Quaternion().setFromEuler(new THREE.Euler(this.pitch, this.yaw, 0)))
    const right = new THREE.Vector3(1, 0, 0).applyQuaternion(new THREE.Quaternion().setFromEuler(new THREE.Euler(0, this.yaw, 0)))

    if (this.keys.has('KeyW')) this.targetPos.add(forward.clone().multiplyScalar(speed))
    if (this.keys.has('KeyS')) this.targetPos.add(forward.clone().multiplyScalar(-speed))
    if (this.keys.has('KeyA')) this.targetPos.add(right.clone().multiplyScalar(-speed))
    if (this.keys.has('KeyD')) this.targetPos.add(right.clone().multiplyScalar(speed))
    if (this.keys.has('Space')) this.targetPos.y += speed
    if (this.keys.has('ControlLeft') || this.keys.has('ControlRight')) this.targetPos.y -= speed

    // Q/E tilt
    if (this.keys.has('KeyQ')) this.yaw -= 0.8 * dt
    if (this.keys.has('KeyE')) this.yaw += 0.8 * dt

    this.currentPos.lerp(this.targetPos, 0.1)
    camera.position.copy(this.currentPos)
    const lookTarget = this.currentPos.clone().add(forward)
    camera.lookAt(lookTarget)
  }

  dispose() { this.detach() }
}
