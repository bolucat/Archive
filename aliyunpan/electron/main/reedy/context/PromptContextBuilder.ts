import { CHAR_PER_TOKEN, PROMPT_SAFETY_MARGIN_TOKENS } from '@shared/types/reedy'

export function estimateTokens(text: string): number {
  return Math.ceil(text.length / CHAR_PER_TOKEN)
}

export interface PromptLayer {
  readonly name: string
  readonly renderPriority: number
  readonly shrinkPriority: number
  readonly expendable: boolean
  render(): string | null
  shrink(level: number): string | null
}

export class PromptContextBuilder {
  private layers: PromptLayer[] = []
  private contextWindow: number
  private reservedOutput: number
  private safetyMargin: number

  constructor(contextWindow: number, reservedOutput: number, safetyMargin: number = PROMPT_SAFETY_MARGIN_TOKENS) {
    this.contextWindow = contextWindow
    this.reservedOutput = reservedOutput
    this.safetyMargin = safetyMargin
  }

  addLayer(layer: PromptLayer): void {
    this.layers.push(layer)
    this.layers.sort((a, b) => a.renderPriority - b.renderPriority)
  }

  build(): string {
    const budget = this.contextWindow - this.reservedOutput - this.safetyMargin
    const shrinkLevels = new Map<string, number>()

    while (true) {
      const parts: string[] = []
      for (const layer of this.layers) {
        const level = shrinkLevels.get(layer.name) ?? 0
        const rendered = level === 0 ? layer.render() : layer.shrink(level)
        if (rendered) parts.push(rendered)
      }
      const fullPrompt = parts.join('\n\n')
      if (estimateTokens(fullPrompt) <= budget) {
        return fullPrompt
      }

      // Shrink most expendable layer first
      let shrunk = false
      const expendable = this.layers
        .filter(l => l.expendable)
        .sort((a, b) => a.shrinkPriority - b.shrinkPriority)

      for (const layer of expendable) {
        const currentLevel = shrinkLevels.get(layer.name) ?? 0
        const nextLevel = currentLevel + 1
        const shrunkOutput = layer.shrink(nextLevel)
        if (shrunkOutput !== null) {
          shrinkLevels.set(layer.name, nextLevel)
          shrunk = true
          break
        }
      }

      if (!shrunk) {
        // Can't shrink any further, return as is
        return fullPrompt
      }
    }
  }
}
