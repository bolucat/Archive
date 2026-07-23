import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'
import { supportsAliyunAutoSign } from '../../user/autoSignPolicy'
import type { ITokenInfo } from '../../user/userstore'

describe('automatic sign-in provider policy', () => {
  it.each<ITokenInfo['tokenfrom']>(['cloud123', '115', '139', '189', 'guangya', 'baidu', 'pikpak', 'quark', 'dropbox', 'onedrive', 'box', 'webdav', 'alist'])(
    'does not send %s accounts to the Aliyun sign-in API',
    tokenfrom => expect(supportsAliyunAutoSign({ tokenfrom })).toBe(false)
  )

  it.each<ITokenInfo['tokenfrom']>(['aliyun', 'unknown'])('keeps %s accounts eligible for legacy Aliyun sign-in', tokenfrom => {
    expect(supportsAliyunAutoSign({ tokenfrom })).toBe(true)
  })

  it('guards the startup auto-sign call site with the provider policy', () => {
    const source = readFileSync(new URL('../../user/userdal.ts', import.meta.url), 'utf8')
    const method = source.slice(source.indexOf('static async UserAutoSign'), source.indexOf('\n  }\n}', source.indexOf('static async UserAutoSign')))
    expect(method).toContain('if (!supportsAliyunAutoSign(token))')
    expect(method.indexOf('supportsAliyunAutoSign(token)')).toBeLessThan(method.indexOf('AliUser.ApiUserSign(token)'))
  })
})
