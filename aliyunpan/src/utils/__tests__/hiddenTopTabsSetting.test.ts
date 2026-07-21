import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

describe('hidden top tabs setting', () => {
  it('uses the checkbox boolean model update instead of reading a DOM event target', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/setting/SettingUI.vue'), 'utf8')

    expect(source).toContain("@update:model-value='(hidden: boolean) => toggleTopTab(tab.key, hidden)'")
    expect(source).not.toContain('event.target.checked')
  })
})
