import { readFileSync } from 'fs'
import { resolve } from 'path'
import { describe, expect, it } from 'vitest'

const root = resolve(__dirname, '../../..')
const readSource = (file: string) => readFileSync(resolve(root, file), 'utf8')

describe('app startup loading splash', () => {
  it('uses the BoxPlayer Radio splash instead of the legacy spinner', () => {
    const source = readSource('src/layout/PageLoading.vue')

    expect(source).toContain('BoxPlayer')
    expect(source).toContain('Radio')
    expect(source).toContain('boxplayer-radio-loading')
    expect(source).toContain('boxplayer-radio-loading-wordmark')
    expect(source).toContain('boxplayer-radio-loading-line')
    expect(source).toContain('LOADING')
    expect(source).not.toContain('desktop-loading-img')
    expect(source).not.toContain('rotate360')
    expect(source).not.toContain('alt="loading-img"')
  })
})
