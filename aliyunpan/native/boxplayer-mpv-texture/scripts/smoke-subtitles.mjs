// Native-only regression: does not launch Electron or touch the user's profile.
import assert from 'node:assert/strict'
import { mkdtempSync, writeFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { createRequire } from 'node:module'
import { fileURLToPath } from 'node:url'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..')
const addon = createRequire(import.meta.url)(path.join(root, 'native/boxplayer-mpv-texture/build/Release/mpv_texture.node'))
const mpv = addon.mpvTexture || addon
const directory = mkdtempSync(path.join(tmpdir(), 'boxplayer-subtitles-'))
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms))
const waitFor = async check => {
  for (let count = 0; count < 100; count++) { if (check()) return; await sleep(50) }
  throw new Error('Timed out waiting for subtitle tracks')
}
const chinese = path.join(directory, 'chinese.srt')
const english = path.join(directory, 'english.vtt')
writeFileSync(chinese, Buffer.from('\ufeff1\n00:00:00,000 --> 00:00:10,000\n中文字幕字体回归验证\n', 'utf16le'))
writeFileSync(english, 'WEBVTT\n\n00:00.000 --> 00:10.000\nEnglish secondary subtitle\nSecond line\n')
process.env.BOXPLAYER_MPV_AUDIO_OUTPUT = 'null'
let frames = 0
try {
  await mpv.create({ width: 640, height: 360, hwdec: 'no', fontsDir: path.join(root, 'src/assets/fonts') })
  mpv.onFrame(frame => { frames++; frame.release?.() })
  mpv.onStatus(() => {})
  await mpv.load(path.join(root, 'e2e/assets/mpv-sample.mp4'))
  await waitFor(() => frames > 0)
  await mpv.addSubtitle(chinese, 'CJK UTF16')
  await mpv.addSubtitle(english, 'English UTF8')
  await waitFor(() => mpv.getTrackStatus().tracks.some(track => track.title === 'English UTF8'))
  const tracks = mpv.getTrackStatus().tracks
  const primary = tracks.find(track => track.title === 'CJK UTF16').id
  const secondary = tracks.find(track => track.title === 'English UTF8').id
  mpv.setSubtitleTrack(primary)
  mpv.setVideoProperty('secondary-sid', String(secondary))
  await waitFor(() => mpv.getTrackStatus().subtitleId === primary && mpv.getTrackStatus().secondarySubtitleId === secondary)
  await waitFor(() => mpv.getTrackStatus().secondarySubtitleLines === 2)
  mpv.setVideoProperty('secondary-sub-pos', '96')
  mpv.setVideoProperty('sub-pos', '80')
  // Adding a third track must not confuse the primary/secondary selected flags.
  await mpv.addSubtitle(english, 'Another subtitle')
  mpv.setSubtitleTrack(primary)
  await waitFor(() => mpv.getTrackStatus().subtitleId === primary)
  assert.equal(mpv.getTrackStatus().secondarySubtitleId, secondary)
  mpv.setVideoProperty('secondary-sid', 'no')
  mpv.setSubtitleTrack(secondary)
  await waitFor(() => mpv.getTrackStatus().subtitleId === secondary && mpv.getTrackStatus().secondarySubtitleId === -1)
  console.log(`Subtitle UTF16/UTF8 loading and independent slots OK (${frames} frames)`)
} finally {
  await mpv.destroy()
  rmSync(directory, { recursive: true, force: true })
}
