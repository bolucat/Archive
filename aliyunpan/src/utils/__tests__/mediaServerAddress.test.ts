import { describe, expect, it } from 'vitest'
import { buildMediaServerAddress } from '../mediaServerAddress'
import { readFileSync } from 'node:fs'
import { parse, compileScript, compileTemplate } from 'vue/compiler-sfc'

describe('server add form addresses', () => {
  it.each([
    ['iMac.local', '8096', '', 'auto', 'http://imac.local:8096'],
    ['https://example.com', '8096', '', 'auto', 'https://example.com'],
    ['http://example.com', '8096', '', 'auto', 'http://example.com'],
    ['https://example.com:8920/jellyfin', '8096', '', 'auto', 'https://example.com:8920/jellyfin'],
    ['example.com:8097/emby', '8096', '', 'auto', 'http://example.com:8097/emby'],
    ['example.com', '443', '', 'auto', 'https://example.com'],
    ['example.com', '8920', '/emby', 'on', 'https://example.com:8920/emby'],
    ['[::1]', '8096', '', 'off', 'http://[::1]:8096']
  ])('normalizes %s', (address, port, path, mode, expected) => {
    expect(buildMediaServerAddress(address, port, path, mode as 'auto' | 'on' | 'off').baseUrl).toBe(expected)
  })
  it.each(['0', '65536', 'abc', '-1'])('rejects invalid port %s', port => expect(() => buildMediaServerAddress('host.local', port, '', 'auto')).toThrow())
  it.each(['', 'host name', 'ftp://example.com', 'http://user:pass@example.com', 'http://example.com?token=secret'])('rejects invalid address %s', address => expect(() => buildMediaServerAddress(address, '', '', 'auto')).toThrow())
  it('compiles a fixed-protocol single-column form with functional advanced controls', () => {
    const source = readFileSync('src/components/media-server/AddMediaServerModal.vue', 'utf8')
    const { descriptor, errors } = parse(source)
    expect(errors).toEqual([])
    const script = compileScript(descriptor, { id: 'server-add' })
    const template = compileTemplate({ source: descriptor.template!.content, filename: 'server-add.vue', id: 'server-add', compilerOptions: { bindingMetadata: script.bindings } })
    expect(template.errors).toEqual([])
    expect(source).toContain('v-model="form.httpsMode"')
    expect(source).toContain('v-model="form.libraryMode"')
    expect(source).not.toContain('v-model="form.type"')
    expect(source).not.toContain("emit('update:visible', false)\n}")
  })
})
