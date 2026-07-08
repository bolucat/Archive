/**
 * BloomEffect — Thin wrapper around Three.js UnrealBloomPass + EffectComposer.
 * Provides a toggleable bloom post-processing pass on the existing WebGLRenderer.
 */

import * as THREE from 'three'
import { EffectComposer } from 'three/examples/jsm/postprocessing/EffectComposer.js'
import { RenderPass } from 'three/examples/jsm/postprocessing/RenderPass.js'
import { UnrealBloomPass } from 'three/examples/jsm/postprocessing/UnrealBloomPass.js'

export class BloomEffect {
  composer: EffectComposer | null = null
  private bloomPass: UnrealBloomPass | null = null
  private renderPass: RenderPass | null = null
  enabled = false

  /** Attach to an existing renderer + scene + camera. Call after scene is ready. */
  attach(renderer: THREE.WebGLRenderer, scene: THREE.Scene, camera: THREE.PerspectiveCamera) {
    this.dispose()
    this.renderPass = new RenderPass(scene, camera)

    this.bloomPass = new UnrealBloomPass(
      new THREE.Vector2(renderer.domElement.width, renderer.domElement.height),
      0.62,   // strength
      0.40,   // radius
      0.85,   // threshold
    )

    this.composer = new EffectComposer(renderer)
    this.composer.addPass(this.renderPass)
    this.composer.addPass(this.bloomPass)
  }

  updateStrength(value: number) {
    if (this.bloomPass) {
      this.bloomPass.strength = value
    }
  }

  resize(w: number, h: number) {
    if (this.bloomPass) {
      this.bloomPass.resolution.set(w, h)
    }
    if (this.composer) {
      this.composer.setSize(w, h)
    }
  }

  render(delta: number) {
    if (this.enabled && this.composer) {
      this.composer.render(delta)
      return true
    }
    return false
  }

  dispose() {
    if (this.bloomPass) {
      this.bloomPass.dispose()
      this.bloomPass = null
    }
    if (this.renderPass) {
      this.renderPass.dispose()
      this.renderPass = null
    }
    if (this.composer) {
      this.composer.dispose()
      this.composer = null
    }
  }
}
