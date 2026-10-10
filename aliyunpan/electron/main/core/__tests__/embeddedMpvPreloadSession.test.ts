import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { runInNewContext } from 'node:vm'
import { describe, expect, it, vi } from 'vitest'

describe('MPV preload load session', () => {
  it('sends the same generated session used by both frame filters', async () => {
    const source = readFileSync(resolve('electron/preload/index.ts'), 'utf8')
    const load = source.slice(source.indexOf("let currentMpvSessionId = ''"), source.indexOf('window.WebMpvEmbeddedControl ='))
      .replace('function(data: any)', 'function(data)').replace('catch (error: any)', 'catch (error)')
    const invoke = vi.fn(async (_channel, data) => ({ ok: true, sessionId: data.sessionId }))
    const context = { window: {} as any, ipcRenderer: { invoke }, crypto: { randomUUID: vi.fn().mockReturnValueOnce('generated-one').mockReturnValueOnce('generated-two') }, normalizeMpvEmbeddedLoadData: (data: any) => data }
    runInNewContext(`${load}\nwindow.session = () => currentMpvSessionId`, context)
    await context.window.WebMpvEmbeddedLoad({ url: 'first' })
    expect(invoke).toHaveBeenLastCalledWith('MpvEmbedded:load', { url: 'first', sessionId: 'generated-one' })
    expect(context.window.session()).toBe('generated-one')
    await context.window.WebMpvEmbeddedLoad({ url: 'second' })
    expect(context.window.session()).toBe('generated-two')
    await context.window.WebMpvEmbeddedLoad({ url: 'explicit', sessionId: 'caller-session' })
    expect(invoke).toHaveBeenLastCalledWith('MpvEmbedded:load', { url: 'explicit', sessionId: 'caller-session' })
    expect(context.window.session()).toBe('caller-session')
  })
})
