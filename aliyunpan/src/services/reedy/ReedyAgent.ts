import { streamText, tool, stepCountIs } from 'ai'
import { reedyClient } from './ReedyClient'
import { getAIProvider } from '../ai/providers'
import type { AISettings, ScoredChunk } from '../ai/types'
import type { LanguageModel, EmbeddingModel } from 'ai'
import { z } from 'zod'
import { MAX_QUERY_CHARS, MAX_TOP_K } from './types'

export interface ReedyChatConfig {
  model: LanguageModel
  embeddingModel?: EmbeddingModel
  system: string
  messages: Array<{ role: string; content: string }>
  bookHash: string
  bookTitle?: string
  chapterTitle?: string
  currentChapter?: number
  currentPage?: number
  currentCfi?: string
  selection?: string
  maxSteps?: number
  signal?: AbortSignal
  toolAllowlist?: string[] | null
}

export interface ReedyStreamCallbacks {
  onToken: (text: string) => void
  onToolCall?: (name: string, args: unknown) => void
  onToolResult?: (name: string, ok: boolean, result: string) => void
  onCitation?: (cfi: string, chapter: string, text: string) => void
  onStepFinish?: (step: number) => void
  onDone: () => void
  onError: (err: string) => void
}

export async function runReedyStream(config: ReedyChatConfig, callbacks: ReedyStreamCallbacks): Promise<void> {
  console.log('[Reedy][Agent] runReedyStream start', {
    bookHash: config.bookHash,
    bookTitle: config.bookTitle,
    chapterTitle: config.chapterTitle,
    currentChapter: config.currentChapter,
    hasEmbeddingModel: !!config.embeddingModel,
    messageCount: config.messages.length,
    maxSteps: config.maxSteps || 5
  })

  const tools: Record<string, any> = {}

  // lookupPassage tool
  if (!config.toolAllowlist || config.toolAllowlist.includes('lookupPassage')) {
    tools.lookupPassage = {
      description:
        'REQUIRED first tool when the user asks about book content, current chapter, plot, characters, themes, or anything inside the book. Search the currently open book for passages matching the given query. Returns up to topK relevant text excerpts with CFI position anchors. Always call this before answering any question about the book.',
      inputSchema: z.object({
        query: z.string().min(1).max(MAX_QUERY_CHARS).describe('Search query for finding relevant passages in the book'),
        topK: z.number().int().min(1).max(MAX_TOP_K).default(5).describe('Maximum number of passages to return')
      }),
      execute: async (args: { query: string; topK: number }) => {
        const { query, topK } = args
        console.log('[Reedy][Agent] lookupPassage called', { query: query.slice(0, 80), topK })
        callbacks.onToolCall?.('lookupPassage', { query, topK })

        try {
          // Check once per session if embeddings exist for this book
          if (embeddingAvailable === null) {
            const meta = await reedyClient.getMeta(config.bookHash)
            embeddingAvailable = meta !== null && meta.embedding_dim > 0
            console.log('[Reedy][Agent] embeddingAvailable:', embeddingAvailable)
          }

          let results: any[]
          if (config.embeddingModel && embeddingAvailable) {
            const { embed } = await import('ai')
            try {
              console.log('[Reedy][Agent] embedding query...')
              const embResult = await embed({ model: config.embeddingModel, value: query })
              console.log('[Reedy][Agent] embedding success, dim:', embResult.embedding?.length)
              if (embResult.embedding?.length) {
                results = await reedyClient.search(config.bookHash, embResult.embedding, query, topK)
              } else {
                console.warn('[Reedy][Agent] embedding empty, falling back to FTS')
                results = await reedyClient.search(config.bookHash, new Float32Array(0), query, topK)
              }
            } catch (embErr: any) {
              console.warn('[Reedy][Agent] embedding failed:', embErr?.message || embErr)
              results = await reedyClient.search(config.bookHash, new Float32Array(0), query, topK)
            }
          } else {
            console.warn('[Reedy][Agent] embedding skipped (no embeddings available), using FTS only')
            results = await reedyClient.search(config.bookHash, new Float32Array(0), query, topK)
          }

          console.log('[Reedy][Agent] search returned', results?.length ?? 0, 'results')

          if (!results || results.length === 0) {
            callbacks.onToolResult?.('lookupPassage', true, 'No matching passages found.')
            return 'No matching passages found in the book.'
          }

          // Build structured result
          let output = `<search-results count="${results.length}">\n`
          for (const r of results) {
            const chunk = r.chunk
            const cfi = chunk?.start_cfi || chunk?.id || 'unknown'
            const chapter = chunk?.chapter_title || '未知章节'
            const text = chunk?.text || ''
            const escaped = escapeXml(text)

            callbacks.onCitation?.(cfi, chapter, text)

            output += `<retrieved trust="untrusted" cfi="${escapeXml(cfi)}" chapter="${escapeXml(chapter)}">${escaped}</retrieved>\n`
          }
          output += '</search-results>'

          const resultText = output.length > 6000 ? output.slice(0, 6000) + '\n<!-- results truncated -->' : output
          console.log('[Reedy][Agent] lookupPassage result length:', resultText.length)
          callbacks.onToolResult?.('lookupPassage', true, `Found ${results.length} passages`)
          return resultText
        } catch (e: any) {
          console.error('[Reedy][Agent] lookupPassage error:', e)
          callbacks.onToolResult?.('lookupPassage', false, e?.message || 'Search failed')
          return `Search error: ${e?.message || 'unknown error'}`
        }
      }
    }
  }

  // addCitation tool
  if (!config.toolAllowlist || config.toolAllowlist.includes('addCitation')) {
    tools.addCitation = {
      description: 'Record a citation referencing a specific passage in the book.',
      inputSchema: z.object({
        cfi: z.string().min(1).describe('CFI anchor for the cited passage'),
        text: z.string().optional().describe('The cited text (brief excerpt)'),
        chapter: z.string().optional().describe('Chapter title for the citation')
      }),
      execute: async (args: { cfi: string; text?: string; chapter?: string }) => {
        callbacks.onCitation?.(args.cfi, args.chapter || '未知章节', args.text || '')
        return `Citation recorded for ${args.cfi}`
      }
    }
  }

  // getReadingContext tool
  if (!config.toolAllowlist || config.toolAllowlist.includes('getReadingContext')) {
    tools.getReadingContext = {
      description: 'ONLY use for questions about reading progress, position, or page location. Do NOT use for questions about book content, plot, characters, or themes — for those, call lookupPassage instead.',
      inputSchema: z.object({}),
      execute: async () => {
        const ctx = {
          bookHash: config.bookHash,
          bookTitle: config.bookTitle || '未知书籍',
          chapterTitle: config.chapterTitle || '未知章节',
          currentChapter: config.currentChapter ?? 0,
          currentPage: config.currentPage ?? 0,
          currentCfi: config.currentCfi || 'unknown',
          selection: config.selection || '',
          note: 'Use lookupPassage to search for book content. getReadingContext only returns position metadata.'
        }
        console.log('[Reedy][Agent] getReadingContext called, returning:', ctx)
        return JSON.stringify(ctx)
      }
    }
  }

  console.log('[Reedy][Agent] tools registered:', Object.keys(tools))
  console.log('[Reedy][Agent] system prompt length:', config.system.length)

  let embeddingAvailable: boolean | null = null

  try {
    const result = streamText({
      model: config.model,
      system: config.system,
      messages: config.messages as any,
      tools,
      stopWhen: stepCountIs(config.maxSteps || 5),
      abortSignal: config.signal
    })

    let stepIndex = 0
    for await (const part of result.fullStream) {
      console.log('[Reedy][Agent] stream part:', part.type)
      switch (part.type) {
        case 'text-delta':
          if (part.text.length > 0) callbacks.onToken(part.text)
          break
        case 'tool-call':
          console.log('[Reedy][Agent] tool-call:', { name: (part as any).toolName, args: (part as any).args })
          break
        case 'tool-result':
          console.log('[Reedy][Agent] tool-result:', { name: (part as any).toolName, ok: !(part as any).error })
          break
        case 'finish-step':
          console.log('[Reedy][Agent] finish-step:', stepIndex)
          callbacks.onStepFinish?.(stepIndex)
          stepIndex++
          break
        case 'finish':
          console.log('[Reedy][Agent] finish:', (part as any).finishReason)
          break
        case 'error':
          console.error('[Reedy][Agent] stream error part:', part.error)
          callbacks.onError(`AI stream error: ${JSON.stringify(part.error)}`)
          return
      }
    }

    console.log('[Reedy][Agent] stream done')
    callbacks.onDone()
  } catch (e: any) {
    console.error('[Reedy][Agent] stream exception:', e)
    callbacks.onError(e?.message || 'Stream failed')
  }
}

function escapeXml(s: string): string {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;')
}
