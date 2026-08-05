import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

describe('Windows ARM64 package configuration', () => {
  it('builds an ARM64 NSIS installer with its matching engine resources', () => {
    const builder = JSON.parse(readFileSync(resolve(process.cwd(), 'electron-builder.json'), 'utf8'))
    const packageJson = JSON.parse(readFileSync(resolve(process.cwd(), 'package.json'), 'utf8'))

    expect(builder.win.target).toContainEqual({ target: 'nsis', arch: ['x64', 'arm64'] })
    expect(builder.win.extraResources).toContainEqual(expect.objectContaining({ from: './static/engine/win32/${arch}', to: './engine/win32/${arch}' }))
    expect(packageJson.scripts['build:windows:arm64']).toBe('pnpm run build && electron-builder --win --arm64')
  })
})
