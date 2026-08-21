import { EventEmitter } from 'node:events'
import { beforeEach, describe, expect, it, vi } from 'vitest'

const request = vi.hoisted(() => vi.fn())

vi.mock('https', () => ({ default: { request } }))
vi.mock('../auth', () => ({ getBaiduToken: vi.fn() }))

import { apiBaiduUploadPart, buildBaiduUploadPath } from '../upload'
import { resolveBaiduUploadParentPath } from '../filecmd'

describe('apiBaiduUploadPart', () => {
  beforeEach(() => request.mockReset())

  it('accepts the md5-only success response returned by PCS upload nodes', async () => {
    request.mockImplementation((_options: unknown, onResponse: (response: EventEmitter & { statusCode: number }) => void) => {
      const response = Object.assign(new EventEmitter(), { statusCode: 200 })
      const clientRequest = Object.assign(new EventEmitter(), {
        write: vi.fn(),
        end: () => {
          onResponse(response)
          response.emit('data', Buffer.from(JSON.stringify({ md5: 'abc123', request_id: 1 })))
          response.emit('end')
        }
      })
      return clientRequest
    })

    await expect(apiBaiduUploadPart('https://d.pcs.baidu.com', 'token', '/test.mp4', 'upload-id', 0, Buffer.from('part'))).resolves.toBe(true)
  })
})

describe('Baidu upload destinations', () => {
  it('uses the cached absolute path instead of a numeric folder id', () => {
    const parentPath = resolveBaiduUploadParentPath('987654', 'baidu_fsid:987654;baidu_path:/Books/Inbox', [], undefined)

    expect(buildBaiduUploadPath(parentPath, 'demo.epub')).toBe('/Books/Inbox/demo.epub')
  })
})
