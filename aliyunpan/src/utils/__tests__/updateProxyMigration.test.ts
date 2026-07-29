import { expect, it } from 'vitest'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

it('migrates the retired mirror.ghproxy.com setting to gh-proxy.com', () => {
  const source = readFileSync(resolve(process.cwd(), 'src/setting/settingstore.ts'), 'utf8')
  expect(source).toContain("setting.uiUpdateProxyUrl === 'https://mirror.ghproxy.com'")
  expect(source).toContain("setting.uiUpdateProxyUrl = 'https://gh-proxy.com'")
  expect(source).toContain('SaveSetting()')
})
