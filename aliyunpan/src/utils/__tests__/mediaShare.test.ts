import { describe, expect, it } from 'vitest'
import { mediaShareText } from '../mediaShare'
import { readFileSync } from 'node:fs'
import { parse, compileScript, compileTemplate } from 'vue/compiler-sfc'
describe('media sharing', () => {
 it('copies only public media details, never file paths or credentials', () => {
  expect(mediaShareText({ id: 'secret-id', title: 'Movie', year: 2024, overview: 'Synopsis', files: [{ path: '/private/video', userId: 'private-account' } as any] })).toBe('Movie\n2024\nSynopsis')
 })
 it('compiles the sharing dialog', () => {
  const file = 'src/components/MediaShareModal.vue'
  const { descriptor, errors } = parse(readFileSync(file, 'utf8'))
  expect(errors).toEqual([])
  const script = compileScript(descriptor, { id: file })
  expect(compileTemplate({ source: descriptor.template!.content, filename: file, id: file, compilerOptions: { bindingMetadata: script.bindings } }).errors).toEqual([])
 })
})
