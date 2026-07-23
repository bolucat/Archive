import { describe, expect, it } from 'vitest'
import { setLocale, t } from '../../i18n'

describe('application locale', () => {
  it('switches settings labels between Simplified Chinese and English', () => {
    setLocale('zh-CN')
    expect(t('settings.uploadConflict')).toBe('上传时遇到重名文件')

    setLocale('en-US')
    expect(t('settings.uploadConflict')).toBe('When a file already exists')
  })
})
