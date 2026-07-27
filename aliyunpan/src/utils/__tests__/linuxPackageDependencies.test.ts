import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

describe('Linux package dependencies', () => {
  it('does not require http-parser for the Pacman package', () => {
    const config = JSON.parse(readFileSync(resolve(process.cwd(), 'electron-builder.json'), 'utf8'))

    expect(config.pacman.depends).not.toContain('http-parser')
    expect(config.pacman.depends).toEqual(['c-ares', 'ffmpeg', 'gtk3', 'libevent', 'libvpx', 'libxslt', 'libxss', 'minizip', 'nss', 're2', 'snappy', 'libnotify', 'libappindicator-gtk3'])
  })
})
