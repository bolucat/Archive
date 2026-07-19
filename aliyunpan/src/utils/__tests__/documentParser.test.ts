import { describe, expect, it } from 'vitest'
import { MAX_DOCUMENT_BYTES, parseDocument } from '../../services/documents/parser'
import { chunkSection } from '../../services/ai/utils/chunker'

describe('local document analysis', () => {
  it('parses UTF-8 text without uploading it', async () => {
    const data = new TextEncoder().encode('第一段\n\n第二段').buffer
    const parsed = await parseDocument('notes.md', data)

    expect(parsed.totalChars).toBe(8)
    expect(parsed.sections).toEqual([{ index: 0, title: '正文', text: '第一段\n\n第二段', location: 'document:body' }])
  })

  it('rejects unsupported files and files above the local limit', async () => {
    await expect(parseDocument('sheet.xlsx', new ArrayBuffer(10))).rejects.toThrow('unsupported_document_type')
    await expect(parseDocument('large.txt', new ArrayBuffer(MAX_DOCUMENT_BYTES + 1))).rejects.toThrow('document_too_large')
  })

  it('uses 500-character chunks with overlap for retrieval', () => {
    const chunks = chunkSection('测试句子。'.repeat(220), 0, '正文', 'source-1')

    expect(chunks.length).toBeGreaterThan(1)
    expect(Math.max(...chunks.map(chunk => chunk.text.length))).toBeLessThanOrEqual(550)
  })
})
