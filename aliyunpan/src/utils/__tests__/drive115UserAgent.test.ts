import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'
import { DRIVE115_DOWN_AGENT } from '@shared/drive115'

const readSource = (path: string) => readFileSync(resolve(process.cwd(), path), 'utf8')

describe('115 download user-agent', () => {
  it('uses the shared user-agent for both URL acquisition and actual download requests', () => {
    const downloadSource = readSource('src/cloud115/download.ts')
    const launchSource = readSource('electron/main/launch.ts')

    expect(DRIVE115_DOWN_AGENT).toContain('aDrive/4.12.0')
    expect(downloadSource).toContain("'User-Agent': DRIVE115_DOWN_AGENT")
    expect(launchSource).toContain("import { DRIVE115_DOWN_AGENT } from '@shared/drive115'")
    expect(launchSource).toContain("'user-agent': DRIVE115_DOWN_AGENT")
    expect(launchSource).not.toContain('DEFAULT_DOWN_AGENT')
  })
})
