export function createAbortController(): AbortController {
  return new AbortController()
}

export function isAborted(signal?: AbortSignal): boolean {
  return signal?.aborted ?? false
}

export class AbortManager {
  private controllers = new Map<string, AbortController>()

  create(turnId: string): AbortController {
    this.abort(turnId)
    const ctrl = new AbortController()
    this.controllers.set(turnId, ctrl)
    return ctrl
  }

  abort(turnId: string): void {
    const ctrl = this.controllers.get(turnId)
    if (ctrl) {
      ctrl.abort()
      this.controllers.delete(turnId)
    }
  }

  get(turnId: string): AbortController | undefined {
    return this.controllers.get(turnId)
  }

  clear(): void {
    for (const [, ctrl] of this.controllers) {
      ctrl.abort()
    }
    this.controllers.clear()
  }
}
