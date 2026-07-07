import { describe, expect, it } from 'vitest'
import { buildBookAnnotationsFileName, buildBookAnnotationsMarkdown } from '../bookAnnotationExport'

describe('bookAnnotationExport', () => {
  it('exports notes and bookmarks as markdown ordered by reading progress', () => {
    const markdown = buildBookAnnotationsMarkdown({
      book: { title: '测试书', file_name: 'book.epub', author: '作者' },
      exportedAt: new Date('2026-06-01T09:05:00'),
      notes: [
        {
          id: 'n2',
          book_id: 'b1',
          user_id: 'u1',
          drive_id: 'd1',
          file_id: 'f1',
          kind: 'note',
          text: 'second',
          note: 'memo',
          chapter: 'B',
          chapter_index: 1,
          position: { percentage: 0.8 },
          range: '',
          color: 0,
          tags: [],
          created_at: 1,
          updated_at: 1
        },
        {
          id: 'n1',
          book_id: 'b1',
          user_id: 'u1',
          drive_id: 'd1',
          file_id: 'f1',
          kind: 'highlight',
          text: 'first',
          note: '',
          chapter: 'A',
          chapter_index: 0,
          position: { percentage: 0.1 },
          range: '',
          color: 0,
          tags: [],
          created_at: 1,
          updated_at: 1
        }
      ],
      bookmarks: [
        {
          id: 'bm2',
          book_id: 'b1',
          user_id: 'u1',
          drive_id: 'd1',
          file_id: 'f1',
          label: 'later',
          chapter: 'B',
          position: { percentage: 0.9 },
          percentage: 0.9,
          created_at: 1,
          updated_at: 1
        },
        {
          id: 'bm1',
          book_id: 'b1',
          user_id: 'u1',
          drive_id: 'd1',
          file_id: 'f1',
          label: 'early',
          chapter: 'A',
          position: { percentage: 0.2 },
          percentage: 0.2,
          created_at: 1,
          updated_at: 1
        }
      ]
    })

    expect(markdown).toContain('# 测试书')
    expect(markdown).toContain('作者：作者')
    expect(markdown).toContain('导出时间：2026-06-01 09:05')
    expect(markdown.indexOf('> first')).toBeLessThan(markdown.indexOf('> second'))
    expect(markdown.indexOf('20% · A · early')).toBeLessThan(markdown.indexOf('90% · B · later'))
    expect(markdown).toContain('备注：memo')
  })

  it('builds a safe markdown filename', () => {
    expect(buildBookAnnotationsFileName({ title: 'A/B:C*D?', file_name: 'book.epub' })).toBe('A_B_C_D_-书摘.md')
  })
})
