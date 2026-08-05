import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

describe('file-open provider account routing', () => {
  it('does not trust a file user ID from another provider', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/utils/openfile.ts'), 'utf8')

    expect(source).toContain('return resolveDriveFileToken(file as IAliGetFileModel & { user_id?: string }, useUserStore().user_id)')
    expect(source).toContain('custom_playlist: buildSiblingVideoPlaylist(file, token.user_id, options?.customPlaylist)')
  })
})
