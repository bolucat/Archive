export const getNow =
  typeof performance === 'object' && performance.now
    ? performance.now.bind(performance)
    : Date.now.bind(Date)

export class TimeoutTools {
  private nextTick: (cb: FrameRequestCallback) => number
  private cancelNextTick: (id: number) => void
  private invokeTime = 0
  private animationFrameId: number | null = null
  private timeoutId: number | null = null
  private callback: ((diff: number) => void) | null = null
  private thresholdTime: number

  constructor(thresholdTime = 80) {
    this.nextTick = window.requestAnimationFrame.bind(window)
    this.cancelNextTick = window.cancelAnimationFrame.bind(window)
    this.thresholdTime = thresholdTime
  }

  run() {
    this.animationFrameId = this.nextTick(() => {
      this.animationFrameId = null
      const diff = this.invokeTime - getNow()
      if (diff > 0) {
        if (diff < this.thresholdTime) return this.run()
        this.timeoutId = window.setTimeout(() => {
          this.timeoutId = null
          this.run()
        }, diff - this.thresholdTime)
        return
      }
      this.callback?.(diff)
    })
  }

  start(callback: (diff: number) => void, timeout = 0) {
    this.callback = callback
    this.invokeTime = getNow() + timeout
    this.run()
  }

  clear() {
    if (this.animationFrameId !== null) {
      this.cancelNextTick(this.animationFrameId)
      this.animationFrameId = null
    }
    if (this.timeoutId !== null) {
      window.clearTimeout(this.timeoutId)
      this.timeoutId = null
    }
  }
}
