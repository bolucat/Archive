import type { ReedyTool, ToolContext } from '../ToolRegistry'
import { z } from 'zod'
import { hybridSearch } from '../ReedyService'
import { MAX_QUERY_CHARS, MAX_TOP_K, RESULT_SIZE_CAP_CHARS } from '@shared/types/reedy'

function escapeXml(s: string): string {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;')
}

export const lookupPassage: ReedyTool = {
  name: 'lookupPassage',
  description: 'Search the book content for passages matching the given query. Returns relevant text excerpts with CFI anchors.',
  permission: 'read',
  parallelSafe: false,
  timeoutMs: 10_000,
  inputSchema: z.object({
    query: z.string().min(1).max(MAX_QUERY_CHARS).describe('Search query for finding relevant passages'),
    topK: z.number().int().min(1).max(MAX_TOP_K).default(5).describe('Maximum number of passages to return'),
    spoilerBound: z.number().int().optional().describe('Maximum position index for spoiler protection')
  }),

  async run(args: unknown, ctx: ToolContext): Promise<string> {
    const { query, topK, spoilerBound } = args as { query: string; topK: number; spoilerBound?: number }
    if (!query?.trim()) return 'No query provided.'

    const results = hybridSearch(ctx.bookHash, new Float32Array(0), query, topK, spoilerBound)

    if (results.length === 0) {
      return 'No matching passages found.'
    }

    // Serialize results with trust markers
    let output = `<search-results count="${results.length}">\n`
    for (const r of results) {
      const escapedCfi = escapeXml(r.chunk.start_cfi)
      const escapedTitle = escapeXml(r.chunk.chapter_title || '未知章节')
      const escapedText = escapeXml(r.chunk.text)
      output += `<retrieved trust="untrusted" cfi="${escapedCfi}" chapter="${escapedTitle}">${escapedText}</retrieved>\n`
    }
    output += '</search-results>'

    // Clamp result size
    if (output.length > RESULT_SIZE_CAP_CHARS) {
      return output.substring(0, RESULT_SIZE_CAP_CHARS) + '\n<!-- results truncated -->'
    }

    return output
  }
}

export function buildLookupAISDKTool(bookHash: string) {
  return function(opts: any) {
    return lookupPassage.run({ query: opts.query, topK: opts.topK ?? 5, spoilerBound: opts.spoilerBound }, {
      bookHash,
      currentChapter: 0,
      currentPage: 0,
      turnId: '',
      signal: new AbortController().signal
    })
  }
}
