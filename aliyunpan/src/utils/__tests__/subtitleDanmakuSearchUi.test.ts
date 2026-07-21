import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const readSource = (path: string) => readFileSync(resolve(process.cwd(), path), 'utf8')

describe('player subtitle and danmaku search UI', () => {
  it('keeps web-player search dialogs compact and exposes result context', () => {
    const player = readSource('src/layout/PageVideo.vue')

    expect(player).toContain('width: 920')
    expect(player).toContain('danmaku-result-toolbar')
    expect(player).toContain('danmaku-result-poster')
    expect(player).toContain('按片名或 TMDB ID 查找并加载字幕')
    expect(player).toContain('downloadingFileId.value === subtitle.fileId')
  })

  it('uses the same result hierarchy in the embedded MPV subtitle dialog', () => {
    const player = readSource('src/components/MpvEmbeddedSurface.vue')

    expect(player).toContain('mpv-subtitle-modal-subtitle')
    expect(player).toContain('mpv-subtitle-result-toolbar')
    expect(player).toContain('downloadingFileId.value === subtitle.fileId')
  })
})
