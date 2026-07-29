const githubDownloadHosts = new Set(['github.com', 'raw.githubusercontent.com', 'objects.githubusercontent.com', 'github-releases.githubusercontent.com'])

export function buildUpdateProxyUrl(proxyUrl: string, downloadUrl: string): string {
  try {
    const target = new URL(downloadUrl)
    const proxy = new URL(proxyUrl)
    if (!githubDownloadHosts.has(target.hostname) || !['http:', 'https:'].includes(proxy.protocol)) return downloadUrl
    return `${proxy.toString().replace(/\/+$/, '')}/${target.toString()}`
  } catch {
    return downloadUrl
  }
}
