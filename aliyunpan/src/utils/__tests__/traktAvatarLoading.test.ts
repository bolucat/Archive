import { describe, expect, it } from 'vitest'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { parse, compileScript, compileTemplate } from 'vue/compiler-sfc'

describe('Trakt avatar loading', () => {
  it('keeps the silhouette visible until the remote avatar loads, including errors and URL changes', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/components/TraktAvatar.vue'), 'utf8')
    expect(source).toContain('backgroundImage: `url(${placeholder})`')
    expect(source).toContain('v-if="src && !failed"')
    expect(source).toContain('@load="loaded = true"')
    expect(source).toContain('loaded.value = false')
    expect(source).toContain('.trakt-avatar img.loaded{opacity:1}')
    const { descriptor } = parse(source)
    const script = compileScript(descriptor, { id: 'TraktAvatar' })
    expect(compileTemplate({ source: descriptor.template!.content, filename: 'TraktAvatar.vue', id: 'TraktAvatar', compilerOptions: { bindingMetadata: script.bindings } }).errors).toEqual([])
  })
})
