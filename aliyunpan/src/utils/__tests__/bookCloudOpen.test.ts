import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

describe('cloud book open contracts', () => {
  it('opens EPUB through the provider-neutral reader path for every drive', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/utils/openfile.ts'), 'utf8')

    expect(source).toContain("if ((file.ext || '').toLowerCase() === 'epub') {")
    expect(source).not.toContain('EPUB_PREVIEW_DRIVES')
    expect(source).toContain("getRawUrl(token.user_id, file.drive_id, file.file_id, getEncType(file), password, file.icon == 'iconweifa', 'other', 'Origin')")
  })

  it('keeps Baidu download headers with the returned book URL', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/cloudbaidu/adapter.ts'), 'utf8')

    expect(source).toContain("'User-Agent': 'pan.baidu.com'")
    expect(source).toContain("Referer: 'https://pan.baidu.com/'")
  })
})
