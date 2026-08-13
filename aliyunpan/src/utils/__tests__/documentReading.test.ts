import { describe, expect, it } from 'vitest'
import { buildPdfReadingUnits } from '../../services/documents'

const chunk = (page: number, text = `第 ${page} 页的有效正文，包含足够的内容用于阅读单元。`.repeat(4)) => ({
  id: `chunk-${page}`,
  book_hash: 'document:test',
  section_index: page - 1,
  chapter_title: `第 ${page} 页`,
  start_cfi: `page:${page}`,
  end_cfi: `page:${page}`,
  position_index: page - 1,
  text,
  token_count: 10
})

describe('buildPdfReadingUnits', () => {
  it('uses real PDF page locations and caps units at 20 pages', () => {
    const plan = buildPdfReadingUnits(Array.from({ length: 45 }, (_, index) => chunk(index + 1)))
    expect(plan.totalPages).toBe(45)
    expect(plan.units.map(unit => [unit.startPage, unit.endPage])).toEqual([[1, 20], [21, 40], [41, 45]])
  })

  it('marks a unit containing only non-body text as skipped', () => {
    const plan = buildPdfReadingUnits([chunk(1, '目录\n第一章\n第二章'), chunk(2, '参考文献')])
    expect(plan.units).toHaveLength(1)
    expect(plan.units[0].skipReason).toBeTruthy()
  })
})
