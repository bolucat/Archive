import { z } from 'zod'

export interface ReedyTool {
  name: string
  description: string
  permission: 'read' | 'navigate' | 'write'
  parallelSafe: boolean
  timeoutMs: number
  inputSchema: z.ZodType<any>
  run(args: unknown, ctx: ToolContext): Promise<string>
}

export interface ToolContext {
  bookHash: string
  currentChapter: number
  currentPage: number
  currentCfi?: string
  selection?: string
  chapterTitle?: string
  bookTitle?: string
  turnId: string
  signal: AbortSignal
}

export class ToolRegistry {
  private tools = new Map<string, ReedyTool>()

  register(tool: ReedyTool): void {
    this.tools.set(tool.name, tool)
  }

  get(name: string): ReedyTool | undefined {
    return this.tools.get(name)
  }

  list(allowlist?: string[] | null): ReedyTool[] {
    const all = [...this.tools.values()]
    if (!allowlist) return all
    return all.filter(t => allowlist.includes(t.name))
  }
}

export function buildToolDefinitions(
  registry: ToolRegistry,
  allowlist?: string[] | null
): Record<string, any> {
  const tools = registry.list(allowlist)
  const defs: Record<string, any> = {}
  for (const tool of tools) {
    defs[tool.name] = {
      description: tool.description,
      parameters: z.object({}) as any,
      execute: (args: unknown) => tool.run(args, {} as ToolContext)
    }
  }
  return defs
}
