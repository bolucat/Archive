import { describe, expect, it } from 'vitest'
import { readFileSync } from 'fs'
import path from 'path'

const root = path.resolve(__dirname, '../../..')

const readJson = (file: string) => JSON.parse(readFileSync(path.join(root, file), 'utf-8'))

describe('Windows MPV bundle packaging config', () => {
  it('does not package the MPV directory with the Windows application', () => {
    const builder = readJson('electron-builder.json')
    const engineResource = builder.win.extraResources.find((entry: { from?: string; to?: string }) => entry.from === './static/engine/win32/${arch}' && entry.to === './engine/win32/${arch}')

    expect(engineResource?.filter).toContain('!mpv{,/**}')
  })
})
