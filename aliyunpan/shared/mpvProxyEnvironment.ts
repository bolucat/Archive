// FFmpeg reads lowercase no_proxy, even when the shell only defines NO_PROXY.
// BoxPlayer's authenticated subtitle/decryption server must stay on loopback.
export function ensureMpvLoopbackProxyBypass(env: Record<string, string | undefined>): void {
  const entries = [...(env.no_proxy || '').split(','), ...(env.NO_PROXY || '').split(','), 'localhost', '127.0.0.1', '::1']
  const bypass = [...new Set(entries.map(entry => entry.trim()).filter(Boolean))].join(',')
  env.no_proxy = bypass
  env.NO_PROXY = bypass
}
