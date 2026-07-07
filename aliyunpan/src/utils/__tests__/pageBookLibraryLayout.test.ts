import { describe, expect, it } from 'vitest'
import fs from 'node:fs'
import path from 'node:path'

const source = fs.readFileSync(path.join(process.cwd(), 'src/layout/PageBookLibrary.vue'), 'utf8')

function cssRule(selector: string): string {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const match = source.match(new RegExp(`${escaped}\\s*\\{([^}]*)\\}`))
  return match?.[1] ?? ''
}

describe('PageBookLibrary tab containment', () => {
  it('keeps Koodo manager chrome inside the app tab pane instead of the viewport', () => {
    expect(cssRule('.manager')).toContain('position:relative')
    expect(cssRule('.sidebar')).not.toContain('position:fixed')
    expect(cssRule('.import-from-local')).not.toContain('position:fixed')
    expect(cssRule('.book-detail')).not.toContain('position:fixed')
  })
})
