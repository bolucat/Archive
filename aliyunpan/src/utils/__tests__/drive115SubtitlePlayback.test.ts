import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

const readSource = (path: string) => readFileSync(resolve(process.cwd(), path), 'utf8')

describe('115 subtitle playback', () => {
  it('routes every authenticated MPV quality through the MPV-only proxy without changing web playback', () => {
    const source = readSource('src/layout/PageVideo.vue')

    expect(source).toContain('const useAuthenticatedMpvProxy = !pageVideo.encType && hasPlaybackHeaders(defaultHeaders)')
    expect(source).toContain("useAuthenticatedMpvProxy ? 'mpv' : ''")
    expect(source).toContain("const resolveRawMpvQualitySource = (data: IRawUrl")
    expect(source).toContain("const defaultUrl = resolveHeaderAwareVideoUrl(defaultQuality.url, defaultHeaders, data.size, defaultQuality.quality || '', useAuthenticatedMpvProxy")
    expect(source).toContain('const mpvHeaders = defaultUrl === defaultQuality.url ? defaultHeaders : undefined')
    expect(source).toContain('headers: mpvHeaders')
    expect(source).toContain("const defaultUrl = resolveHeaderAwareVideoUrl(defaultQuality.url, defaultHeaders, data.size, defaultQuality.quality || '')")
    expect(source).not.toContain('const use115OriginProxy')
    expect(source).not.toContain('proxy_url: directUrl')
  })

  it('proxies web playback whenever a provider requires download headers', () => {
    const source = readSource('src/layout/PageVideo.vue')

    expect(source).toContain('if (!pageVideo.encType && !hasPlaybackHeaders(headers)) return url')
    expect(source).toContain('proxy_headers: hasPlaybackHeaders(headers) ? JSON.stringify(headers) : undefined')
  })

  it('loads the default embedded subtitle and passes its format to Artplayer', () => {
    const source = readSource('src/layout/PageVideo.vue')

    expect(source).toContain('const ext = getSubtitleItemExt(item)')
    expect(source).toContain("await art.subtitle.switch(item.url, { name: item.name, type: ext, escape: false })")
    expect(source).toContain('await loadSubtitleUrlToPlayer(art, embedSubSelector[0])')
    expect(source).toContain('ext: getSubtitleExtension(subtitle.url)')
    expect(source).not.toContain('art.subtitle.url = embedSubSelector[0].url')
  })

  it('uses the shared 115 user-agent when the subtitle proxy requests upstream data', () => {
    const playerSource = readSource('src/layout/PageVideo.vue')
    const source = readSource('src/utils/proxyhelper.ts')

    expect(playerSource).toContain('await DriveFile.ApiFileDownloadUrl(pageVideo.user_id, driveId, item.file_id, 14400, pageVideo.tokenfrom)')
    expect(playerSource).toContain("proxy_kind: 'subtitle'")
    expect(playerSource).toContain('proxy_headers: hasPlaybackHeaders(data.headers) ? JSON.stringify(data.headers) : undefined')
    expect(playerSource).not.toContain('const data = await AliFile.ApiFileDownText(pageVideo.user_id, pageVideo.drive_id, item.file_id')
    expect(source).toContain("import { DRIVE115_DOWN_AGENT } from '@shared/drive115'")
    expect(source).toContain("upstreamHeaders['user-agent'] = DRIVE115_DOWN_AGENT")
  })

  it('preserves provider download headers for web and external-player subtitles', () => {
    const cloud139Source = readSource('src/cloud139/adapter.ts')
    const cloud189Source = readSource('src/cloud189/adapter.ts')
    const playerSource = readSource('src/utils/playerhelper.ts')

    expect(cloud139Source).toContain('headers: cloud139DownloadHeaders()')
    expect(cloud189Source).toContain('headers: cloud189DownloadHeaders()')
    expect(playerSource).toContain("proxy_kind: 'subtitle'")
    expect(playerSource).toContain('proxy_headers: headers && Object.values(headers).some(Boolean) ? JSON.stringify(headers) : undefined')
    expect(playerSource).toContain('subTitleUrl = await resolveExternalSubtitleUrl(token.user_id, subTitleFile)')
    expect(playerSource).toContain('const subtitleUrl = await resolveExternalSubtitleUrl(token.user_id, subTitleFile)')
    expect(playerSource).toContain('buildExternalSubtitleProxyUrl(token.user_id, file.drive_id, file.file_id, subTitleData.url, 0, subTitleData.headers)')
  })

  it('keeps the real cloud parent directory in media-library scan records', () => {
    const scannerSource = readSource('src/utils/mediaScanner.ts')
    const detailSource = readSource('src/components/MediaDetail.vue')

    expect(scannerSource).toContain('parentFileId: item.parent_file_id || folder.file_id')
    expect(detailSource).toContain('driveFile.parentFileId ||')
  })
})
