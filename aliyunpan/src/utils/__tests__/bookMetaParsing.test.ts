import { describe, expect, it } from 'vitest'
import { parseBookMeta } from '../bookFilenameMeta'

describe('book filename metadata parsing', () => {
  it('removes catalog prefixes and release suffixes from titles', () => {
    expect(parseBookMeta('0680周鸿祎自述：我的互联网方法论.mobi')).toMatchObject({ title: '周鸿祎自述：我的互联网方法论', author: '未知作者' })
    expect(parseBookMeta('知日-2019更新.mobi')).toMatchObject({ title: '知日', author: '未知作者' })
    expect(parseBookMeta('005315.7天学摄影轻松拍出好照片.mobi')).toMatchObject({ title: '7天学摄影轻松拍出好照片' })
    expect(parseBookMeta('00571980年代的爱情_野夫.mobi')).toMatchObject({ title: '1980年代的爱情', author: '野夫' })
  })

  it('keeps English title-author filenames in the correct order', () => {
    expect(parseBookMeta('The Wailing Wind - Tony Hillerman.mobi')).toMatchObject({ title: 'The Wailing Wind', author: 'Tony Hillerman' })
  })
})
