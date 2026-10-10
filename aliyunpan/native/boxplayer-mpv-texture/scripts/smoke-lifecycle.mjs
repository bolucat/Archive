// Native-only regression: no Electron process, profile, window or cloud account.
import assert from 'node:assert/strict'
import { createRequire } from 'node:module'
import { createServer } from 'node:http'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const native = createRequire(import.meta.url)(path.join(root, 'build/Release/mpv_texture.node'))
const mpv = native.mpvTexture || native
const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
async function until(predicate, label, timeout = 5000) {
  const end = Date.now() + timeout
  while (!predicate() && Date.now() < end) await delay(20)
  assert.ok(predicate(), label)
}
let response
let frames = 0
const held = []
let holdTextures = false
const server = createServer((_request, reply) => { response = reply })
await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve))
const url = `http://127.0.0.1:${server.address().port}/slow.srt`
try {
  await mpv.create({ width: 1280, height: 720, hwdec: 'no' })
  mpv.onFrame((frame) => {
    frames++
    if (holdTextures && frame.release) held.push(frame)
    else frame.release?.()
  })
  await mpv.load(path.join(root, '../../e2e/assets/boxplayer-e2e.mkv'))
  await until(() => frames > 1, 'first frames arrive')
  const subtitle = mpv.addSubtitle(url, 'slow subtitle')
  const subtitleResult = subtitle.then(() => 'loaded', () => 'cancelled')
  await until(() => Boolean(response), 'subtitle request reaches server')
  const before = frames
  let ticks = 0
  const timer = setInterval(() => { mpv.getTrackStatus(); ticks++ }, 20)
  await delay(600)
  clearInterval(timer)
  assert.ok(ticks >= 10, 'native track request must not block JS event loop')
  assert.ok(frames > before, 'video must continue while a subtitle URL is pending')
  response.end('1\n00:00:01,000 --> 00:00:10,000\nSubtitle\n')
  assert.equal(await subtitleResult, 'loaded')
  // GPU pools must stop reusing occupied slots. Software fallback has no leases.
  holdTextures = true
  await delay(250)
  const gpuLeases = held.length
  if (process.env.BOXPLAYER_MPV_REQUIRE_GPU === '1') assert.ok(gpuLeases > 0, 'GPU texture path is required; software fallback is not acceptance')
  if (held.length) {
    assert.ok(held.length <= 3, 'GPU producer must not overwrite retained slots')
    const count = frames
    await delay(150)
    assert.equal(frames, count)
  }
  holdTextures = false
  for (const frame of held.splice(0)) frame.release()
  const beforeRelease = frames
  await until(() => frames > beforeRelease, 'rendering resumes after texture release')
  response = null
  const pending = mpv.addSubtitle(`${url}?close`, 'cancelled').catch(() => {})
  await until(() => Boolean(response), 'second subtitle request reaches server')
  const start = Date.now()
  await mpv.destroy()
  assert.ok(Date.now() - start < 2000, 'closing a pending subtitle must complete promptly')
  await pending
  await mpv.create({ headless: true })
  await mpv.destroy()
  console.log(`Native lifecycle OK: responsive slow subtitles, ${frames} frames, ${gpuLeases} GPU leases, safe close/recreate`)
} finally {
  response?.end()
  server.closeAllConnections()
  server.close()
  for (const frame of held) frame.release()
  await mpv.destroy()
}
