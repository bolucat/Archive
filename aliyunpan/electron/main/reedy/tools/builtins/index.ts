import type { ReedyTool } from '../ToolRegistry'
import { z } from 'zod'

export const getReadingContext: ReedyTool = {
  name: 'getReadingContext',
  description: 'Get the user\'s current reading position including chapter, page, and CFI location.',
  permission: 'read',
  parallelSafe: true,
  timeoutMs: 500,
  inputSchema: z.object({}),
  async run(_: unknown, ctx) {
    return JSON.stringify({
      chapter: ctx.currentChapter,
      page: ctx.currentPage,
      cfi: ctx.currentCfi || 'unknown',
      chapterTitle: ctx.chapterTitle || 'Unknown',
      bookTitle: ctx.bookTitle || 'Unknown'
    })
  }
}

export const getSelection: ReedyTool = {
  name: 'getSelection',
  description: 'Get the user\'s currently selected text, if any.',
  permission: 'read',
  parallelSafe: true,
  timeoutMs: 500,
  inputSchema: z.object({}),
  async run(_: unknown, ctx) {
    if (ctx.selection?.trim()) {
      return ctx.selection
    }
    return 'No text currently selected.'
  }
}

export const navigateToCfi: ReedyTool = {
  name: 'navigateToCfi',
  description: 'Navigate the reader to a specific CFI location.',
  permission: 'navigate',
  parallelSafe: false,
  timeoutMs: 2000,
  inputSchema: z.object({
    cfi: z.string().min(1).describe('CFI anchor to navigate to')
  }),
  async run(args: unknown) {
    // Navigation is handled by renderer via IPC
    const { cfi } = args as { cfi: string }
    return `Navigating to CFI: ${cfi}`
  }
}

export const createHighlight: ReedyTool = {
  name: 'createHighlight',
  description: 'Create a highlight annotation in the current book.',
  permission: 'write',
  parallelSafe: false,
  timeoutMs: 2000,
  inputSchema: z.object({
    text: z.string().min(1).describe('Text to highlight'),
    cfi: z.string().optional().describe('CFI location of the highlight'),
    note: z.string().optional().describe('Optional note for the highlight')
  }),
  async run(args: unknown) {
    const { text, cfi, note } = args as { text: string; cfi?: string; note?: string }
    return `Highlight created: "${text.slice(0, 50)}..."${cfi ? ` at ${cfi}` : ''}${note ? ` with note: "${note}"` : ''}`
  }
}

export const createNote: ReedyTool = {
  name: 'createNote',
  description: 'Create a note annotation in the current book.',
  permission: 'write',
  parallelSafe: false,
  timeoutMs: 2000,
  inputSchema: z.object({
    content: z.string().min(1).describe('Note content'),
    cfi: z.string().optional().describe('CFI location to attach the note to')
  }),
  async run(args: unknown) {
    const { content, cfi } = args as { content: string; cfi?: string }
    return `Note created${cfi ? ` at ${cfi}` : ''}: "${content.slice(0, 100)}"`
  }
}

export const addCitation: ReedyTool = {
  name: 'addCitation',
  description: 'Record a citation referencing a specific passage in the book.',
  permission: 'read',
  parallelSafe: true,
  timeoutMs: 1000,
  inputSchema: z.object({
    cfi: z.string().min(1).describe('CFI anchor for the cited passage'),
    text: z.string().optional().describe('The cited text'),
    chapter: z.string().optional().describe('Chapter title for the citation')
  }),
  async run(args: unknown) {
    const { cfi, text, chapter } = args as { cfi: string; text?: string; chapter?: string }
    return `Citation recorded: cfi=${cfi} chapter="${chapter || '未知'}" text="${(text || '').slice(0, 50)}"`
  }
}
