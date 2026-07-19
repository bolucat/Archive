import { existsSync, readFileSync, rmSync } from 'node:fs'
import { createServer, type IncomingHttpHeaders, type Server } from 'node:http'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { afterEach, describe, expect, it } from 'vitest'
import mpvAPI from '../../module/node-mpv'

const integrationDescribe = process.env.BOXPLAYER_MPV_INTEGRATION === '1' ? describe : describe.skip

const waitFor = async <T>(read: () => Promise<T>, accept: (value: T) => boolean, timeout = 5000): Promise<T> => {
  const startedAt = Date.now()
  let lastError: unknown
  while (Date.now() - startedAt < timeout) {
    try {
      const value = await read()
      if (accept(value)) return value
    } catch (error) {
      lastError = error
    }
    await new Promise((resolve) => setTimeout(resolve, 100))
  }
  throw lastError || new Error(`Condition was not met within ${timeout}ms`)
}

integrationDescribe('real MPV runtime', () => {
  let mpv: any
  let server: Server | undefined
  let socketPath = ''

  afterEach(async () => {
    try {
      if (mpv?.running) await mpv.quit()
    } catch {
      mpv?.mpvPlayer?.kill?.('SIGKILL')
    }
    server?.closeAllConnections?.()
    await new Promise<void>((resolve) => server?.close(() => resolve()) || resolve())
    if (socketPath && existsSync(socketPath)) rmSync(socketPath, { force: true })
    mpv = undefined
    server = undefined
    socketPath = ''
  })

  it('decodes video and preserves authenticated HTTP headers over JSON IPC', async () => {
    const binary = process.env.MPV_BIN || 'mpv'
    const mediaPath = process.env.MPV_TEST_MEDIA
    if (!mediaPath || !existsSync(mediaPath)) throw new Error(`MPV_TEST_MEDIA does not exist: ${mediaPath || '(empty)'}`)

    const media = readFileSync(mediaPath)
    const requests: IncomingHttpHeaders[] = []
    server = createServer((request, response) => {
      requests.push(request.headers)
      const match = /^bytes=(\d+)-(\d*)$/i.exec(String(request.headers.range || ''))
      const start = match ? Math.min(Number(match[1]), media.length - 1) : 0
      const requestedEnd = match?.[2] ? Number(match[2]) : media.length - 1
      const end = Math.max(start, Math.min(requestedEnd, media.length - 1))
      response.writeHead(match ? 206 : 200, {
        'Accept-Ranges': 'bytes',
        'Content-Length': end - start + 1,
        'Content-Type': 'video/mp4',
        ...(match ? { 'Content-Range': `bytes ${start}-${end}/${media.length}` } : {})
      })
      if (request.method === 'HEAD') response.end()
      else response.end(media.subarray(start, end + 1))
    })
    await new Promise<void>((resolve) => server!.listen(0, '127.0.0.1', resolve))
    const address = server.address()
    const port = typeof address === 'object' && address ? address.port : 0

    socketPath = path.join(tmpdir(), `boxplayer-mpv-integration-${process.pid}-${Date.now()}.sock`)
    mpv = new mpvAPI({
      binary,
      socket: socketPath,
      auto_restart: false,
      start_timeout: 8000,
      spawnOptions: { detached: false, shell: false, windowsVerbatimArguments: false }
    }, [
      '--no-config',
      '--vo=null',
      '--ao=null',
      '--pause',
      '--user-agent=BoxPlayer-Linux-CI',
      '--referrer=https://www.aliyundrive.com/',
      '--http-header-fields=Authorization: Bearer boxplayer-ci-token',
      `http://127.0.0.1:${port}/boxplayer-mpv-test.mp4`
    ])

    await mpv.start()
    const duration = await waitFor(() => mpv.getProperty('duration'), (value) => Number(value) > 0)
    const videoCodec = await waitFor(() => mpv.getProperty('video-codec'), (value) => Boolean(value))
    const width = await waitFor(() => mpv.getProperty('video-params/w'), (value) => Number(value) > 0)

    expect(mpv.running).toBe(true)
    expect(Number(duration)).toBeGreaterThan(0)
    expect(String(videoCodec)).not.toBe('')
    expect(Number(width)).toBe(320)
    expect(requests.length).toBeGreaterThan(0)
    expect(requests.every((headers) => headers.authorization === 'Bearer boxplayer-ci-token')).toBe(true)
    expect(requests.every((headers) => headers['user-agent'] === 'BoxPlayer-Linux-CI')).toBe(true)
    expect(requests.every((headers) => headers.referer === 'https://www.aliyundrive.com/')).toBe(true)
  }, 15000)

  it('fails promptly when the configured MPV binary is missing', async () => {
    socketPath = path.join(tmpdir(), `boxplayer-mpv-missing-${process.pid}-${Date.now()}.sock`)
    mpv = new mpvAPI({
      binary: path.join(tmpdir(), 'boxplayer-mpv-does-not-exist'),
      socket: socketPath,
      auto_restart: false,
      start_timeout: 2000,
      spawnOptions: { detached: false, shell: false, windowsVerbatimArguments: false }
    })

    const startedAt = Date.now()
    await expect(mpv.start()).rejects.toMatchObject({ errcode: 2 })
    expect(Date.now() - startedAt).toBeLessThan(2000)
  }, 5000)
})
