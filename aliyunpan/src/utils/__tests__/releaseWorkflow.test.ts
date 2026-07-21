import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

describe('release workflow notes', () => {
  it('never publishes an empty release body', () => {
    const workflow = readFileSync(resolve(process.cwd(), '.github/workflows/release.yml'), 'utf8')

    expect(workflow).toContain('git log --format="- %s (%h)"')
    expect(workflow).toContain('if [[ ! -s release-notes.md ]]')
    expect(workflow).toContain('Release notes are empty; refusing to publish')
  })
})
