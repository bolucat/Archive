import { describe, expect, it } from 'vitest'
import { documentInsightSourceId, isDocumentInsightFile, MAX_DOCUMENT_INSIGHT_SOURCES, toDocumentInsightSource } from '../../services/documents/insight'

describe('document insight sources', () => {
  it('accepts only supported cloud document formats', () => {
    expect(isDocumentInsightFile({ file_id: '1', name: 'report.pdf' })).toBe(true)
    expect(isDocumentInsightFile({ file_id: '2', name: 'notes.markdown' })).toBe(true)
    expect(isDocumentInsightFile({ file_id: '3', name: 'slides.pptx' })).toBe(false)
    expect(isDocumentInsightFile({ file_id: '4', name: 'folder', isDir: true })).toBe(false)
  })

  it('derives a version-aware source id without download credentials', () => {
    const source = toDocumentInsightSource({ file_id: 'file-1', drive_id: 'drive-1', name: 'report.pdf', size: 12, updated_at: 'v1' }, 'user-1')
    expect(source).toEqual(expect.objectContaining({ userId: 'user-1' }))
    expect(documentInsightSourceId(source!)).toContain('user-1:drive-1:file-1:12:v1')
  })

  it('keeps the multi-source ceiling explicit', () => {
    expect(MAX_DOCUMENT_INSIGHT_SOURCES).toBe(10)
  })
})
