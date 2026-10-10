import { afterEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import useRegistry from '../../store/mediaServerRegistry'
import { saveMediaServerEndpoint, removeMediaServerEndpoint } from '../mediaServerEndpoints'

afterEach(() => vi.unstubAllGlobals())
describe('server endpoint management', () => {
  it('creates named and unnamed endpoints while preserving primary selection', () => {
    expect(saveMediaServerEndpoint({}, null, ' LAN ', 'https://media.example/emby/')).toEqual({ backupAddresses: { LAN: 'https://media.example/emby' }, selectedLineName: '' })
    expect(saveMediaServerEndpoint({}, null, '', 'http://192.168.1.2:8096')).toEqual({ backupAddresses: { '192.168.1.2:8096': 'http://192.168.1.2:8096' }, selectedLineName: '' })
  })
  it('renames selected endpoints without losing selection and removes selected endpoints safely', () => {
    const server = { backupAddresses: { Old: 'http://old.example', Other: 'https://other.example' }, selectedLineName: 'Old' }
    const updated = saveMediaServerEndpoint(server, 'Old', 'New', 'https://new.example')
    expect(updated).toEqual({ backupAddresses: { Other: 'https://other.example', New: 'https://new.example' }, selectedLineName: 'New' })
    expect(removeMediaServerEndpoint(updated, 'New')).toEqual({ backupAddresses: { Other: 'https://other.example' }, selectedLineName: '' })
    expect(server.selectedLineName).toBe('Old')
  })
  it.each(['not a URL', 'file:///tmp/server', 'https://user:secret@example.com', 'https://example.com?token=secret', 'https://example.com#fragment'])('rejects unsafe or invalid address %s', url => {
    expect(() => saveMediaServerEndpoint({}, null, 'Test', url)).toThrow()
  })
  it('rejects duplicate and reserved names without overwriting an existing address', () => {
    expect(() => saveMediaServerEndpoint({ backupAddresses: { LAN: 'https://old.example' } }, null, 'lan', 'https://new.example')).toThrow('同名')
    expect(() => saveMediaServerEndpoint({}, null, '__primary__', 'https://example.com')).toThrow()
  })
  it('switches actual connection configuration and synchronizes the selected endpoint to the main process', () => {
    const sync = vi.fn()
    vi.stubGlobal('window', { MsImageCacheSyncConfig: sync })
    vi.stubGlobal('localStorage', { getItem: () => null, setItem: vi.fn() })
    setActivePinia(createPinia())
    const registry = useRegistry()
    const server = registry.addServer({ name: 'Test', type: 'emby', baseUrl: 'https://primary.example', backupAddresses: { LAN: 'http://lan.example' } })
    registry.setCurrentServerLine('LAN')
    expect(registry.currentServer?.baseUrl).toBe('http://lan.example')
    expect(sync.mock.calls.at(-1)?.[0][0].baseUrl).toBe('http://lan.example')
    expect(registry.currentServerRecord?.baseUrl).toBe('https://primary.example')
    registry.updateServer(server.id, removeMediaServerEndpoint(registry.currentServerRecord!, 'LAN'))
    expect(registry.currentServer?.baseUrl).toBe('https://primary.example')
    expect(sync.mock.calls.at(-1)?.[0][0].baseUrl).toBe('https://primary.example')
  })
})
