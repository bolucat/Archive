import { expect, it } from 'vitest'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

it('does not reject WMA raw download urls as preview-only audio', () => {
  const source = readFileSync(resolve(process.cwd(), 'src/utils/proxyhelper.ts'), 'utf8')
  expect(source).not.toContain("return '不支持预览的加密音频格式'")
})
