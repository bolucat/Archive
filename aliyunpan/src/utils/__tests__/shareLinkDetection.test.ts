import { describe, expect, it } from 'vitest'
import { canImportShareLink, detectShareLink } from '../shareLinkDetection'

describe('剪贴板分享链接识别', () => {
  it('识别可直接导入的阿里云盘链接和提取码', () => {
    expect(detectShareLink('资源 https://www.alipan.com/s/Abc_def123 提取码: 8xY2')).toEqual({
      provider: 'aliyun',
      providerName: '阿里云盘',
      url: 'https://www.alipan.com/s/Abc_def123',
      password: '8xY2',
      canImport: true
    })
  })

  it('识别百度网盘链接，但不谎称可直接转存', () => {
    expect(detectShareLink('https://pan.baidu.com/s/1abc_DEF?pwd=2x3y）')).toEqual({
      provider: 'baidu',
      providerName: '百度网盘',
      url: 'https://pan.baidu.com/s/1abc_DEF?pwd=2x3y',
      password: '2x3y',
      canImport: false
    })
  })

  it('统一判断现有导入弹窗真正支持的网盘', () => {
    expect(canImportShareLink('pan.quark.cn/s/abc123')).toBe(true)
    expect(canImportShareLink('https://123pan.com/s/abc123')).toBe(false)
    expect(canImportShareLink('https://115.com/s/abc123')).toBe(false)
  })

  it('不会把普通网页当成分享链接', () => {
    expect(detectShareLink('https://example.com/article/123')).toBeUndefined()
  })
})
