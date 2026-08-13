import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

describe('cloud139 folder commands', () => {
  it('uses the new personal-cloud folder creation payload', async () => {
    const source = readFileSync(resolve(process.cwd(), 'src/cloud139/filecmd.ts'), 'utf8')

    expect(source).toContain("cloud139Request(user_id, '/file/create', {")
    expect(source).toContain('name,')
    expect(source).toContain("description: '',")
    expect(source).toContain("type: 'folder',")
    expect(source).toContain("fileRenameMode: 'force_rename'")
  })
})
