import { expect, it } from 'vitest'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

it('uses Electron webUtils paths for dragged upload files and rejects empty paths', () => {
  const source = readFileSync(resolve(process.cwd(), 'src/pan/PanRight.vue'), 'utf8')
  expect(source).toContain('window.WebGetPathForFile?.(file)')
  expect(source).toContain('if (!files.length)')
})
