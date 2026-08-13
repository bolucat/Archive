import { describe, expect, it } from 'vitest'
import type { IBookItem } from '../../types/book'
import { groupBookWorks } from '../bookWorks'

const makeBook = (id: string, ext: string, title = '多谈谈问题'): IBookItem => ({
  id,
  user_id: 'user',
  drive_id: 'drive',
  file_id: id,
  parent_file_id: 'root',
  file_name: `${title}.${ext.toLowerCase()}`,
  ext,
  size: 1,
  category: 'doc',
  title,
  author: '作者',
  scanned_at: 1
})

describe('groupBookWorks', () => {
  it('groups formats of the same work and prefers EPUB for opening', () => {
    const works = groupBookWorks([makeBook('pdf', 'PDF'), makeBook('mobi', 'MOBI'), makeBook('epub', 'EPUB')])
    expect(works).toHaveLength(1)
    expect(works[0].book.id).toBe('epub')
    expect(works[0].variants.map((book) => book.ext)).toEqual(['EPUB', 'MOBI', 'PDF'])
  })

  it('does not merge different titles', () => {
    expect(groupBookWorks([makeBook('one', 'EPUB'), makeBook('two', 'MOBI', '另一部书')])).toHaveLength(2)
  })
})
