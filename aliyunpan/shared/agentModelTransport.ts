export interface AgentModelEndpointInput {
  providerName: string
  endpoint: string
  cloudBaseUrl: string
  /** Legacy renderer configurations may deliberately point at a private HTTP gateway. */
  allowInsecureHttp?: boolean
}

/**
 * One provider-neutral endpoint rule for Pi's OpenAI-compatible transport.
 * Credentials remain outside this module and must never be persisted with an
 * Agent workflow or plan.
 */
export function resolveAgentModelEndpoint(input: AgentModelEndpointInput): string {
  const source = input.providerName === 'boxplayer-cloud' ? input.cloudBaseUrl : input.endpoint
  const raw = source?.trim()
  if (!raw) throw new Error(input.providerName === 'boxplayer-cloud' ? 'BoxPlayer AI 服务地址未配置' : 'AI endpoint is not configured')
  let url: URL
  try { url = new URL(raw) } catch { throw new Error('AI endpoint is invalid') }
  const localHttp = url.protocol === 'http:' && ['localhost', '127.0.0.1', '::1'].includes(url.hostname)
  if (url.protocol !== 'https:' && !localHttp && !input.allowInsecureHttp) throw new Error('AI endpoint 必须使用 HTTPS；本机模型可使用 HTTP')
  if (!['http:', 'https:'].includes(url.protocol)) throw new Error('AI endpoint 必须使用 HTTP 或 HTTPS')
  return url.toString().replace(/\/+$/, '') + (input.providerName === 'boxplayer-cloud' ? '/v1' : '')
}
