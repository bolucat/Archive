import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

describe('Quark download headers', () => {
  it('keeps provider headers for downloads while allowing browser playback to use its session interceptor', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/utils/proxyhelper.ts'), 'utf8')

    expect(source).toContain("const useQuarkSessionInterceptor = (preview_type === 'video' || preview_type === 'audio')")
    expect(source).toContain('if (downUrl.headers && !useQuarkSessionInterceptor) data.headers = downUrl.headers')

    const ariaSource = readFileSync(resolve(process.cwd(), 'src/utils/aria2c.ts'), 'utf8')
    expect(ariaSource).toContain("isQuarkDownload ? 'https://pan.quark.cn/'")
    expect(ariaSource).toContain('isQuarkDownload ? QUARK_DOWNLOAD_AGENT')
    expect(ariaSource).toContain("proxy_kind: 'quark-download'")
    expect(ariaSource).toContain('proxy_headers: JSON.stringify({ ...resolvedDownloadHeaders, ...(info.downloadHeaders || {}) })')
    expect(ariaSource).toContain("file.Down.DownUrl = ''")

    expect(source).toContain("proxy_kind !== 'mpv' && proxy_kind !== 'quark-download'")
    expect(source).toContain("if (!upstreamHeaders.range) upstreamHeaders.range = 'bytes=0-'")

    const openFileSource = readFileSync(resolve(process.cwd(), 'src/utils/openfile.ts'), 'utf8')
    expect(openFileSource).toContain('proxy_headers: JSON.stringify(rawData.headers || {})')
  })
})
