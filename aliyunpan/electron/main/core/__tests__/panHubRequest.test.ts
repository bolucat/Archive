import { describe, expect, it, vi } from 'vitest'
import { requestPanHub } from '../panHubRequest'
import { readFileSync } from 'node:fs'

describe('PanHub main-process request proxy', () => {
  it('allows only the configured PanHub HTTPS origin', async () => {
    const fetchImpl = vi.fn(async () => new Response(JSON.stringify({ status: 'ok' }), { status: 200 }))

    await expect(requestPanHub({ url: 'https://api.xbyvideohub.com/api/health' }, fetchImpl)).resolves.toEqual({
      ok: true,
      status: 200,
      data: { status: 'ok' }
    })
    await expect(requestPanHub({ url: 'https://example.com/api/health' }, fetchImpl)).rejects.toThrow('不允许的 PanHub 请求地址')
    expect(fetchImpl).toHaveBeenCalledTimes(1)
  })

  it('registers the request proxy with the main IPC event handler', () => {
    const source = readFileSync(new URL('../ipcEvent.ts', import.meta.url), 'utf8')
    expect(source).toContain("ipcMain.handle('PanHub:request'")
    expect(source).toContain('requestPanHub(data)')
    expect(source).not.toContain('requestPanHub(data, net.fetch)')
    expect(source).toContain('this.handlePanHubRequest()')
  })
})
