import { getBoxToken } from './dirfilelist'

export const buildBoxThumbnailUrl = (urlTemplate: string): string => {
  return urlTemplate.replace('{+asset_path}', '')
}

export const buildBoxRepresentationInfoUrl = (urlTemplate: string): string => {
  const contentIndex = urlTemplate.indexOf('/content/')
  if (contentIndex < 0) return ''
  return urlTemplate.slice(0, contentIndex).replace(/^https:\/\/[^/]+\/api\/2\.0\//, 'https://api.box.com/2.0/')
}

export const buildBoxThumbnailDataUrl = (bytes: Uint8Array, mimeType = 'image/png'): string => {
  let binary = ''
  const chunkSize = 0x8000
  for (let index = 0; index < bytes.length; index += chunkSize) {
    binary += String.fromCharCode(...bytes.subarray(index, index + chunkSize))
  }
  return `data:${mimeType};base64,${btoa(binary)}`
}

export const apiBoxThumbnail = async (user_id: string, urlTemplate: string): Promise<string> => {
  if (!urlTemplate) return ''
  const token = await getBoxToken(user_id)
  if (!token?.access_token) return ''

  const headers = { Authorization: `Bearer ${token.access_token}` }
  const infoUrl = buildBoxRepresentationInfoUrl(urlTemplate)
  if (!infoUrl) return ''

  let contentUrl = urlTemplate
  for (let attempt = 0; attempt < 8; attempt++) {
    const infoResp = await fetch(infoUrl, { headers })
    const representation = await infoResp.json().catch(() => undefined)
    const state = representation?.status?.state
    if (infoResp.ok && state === 'success') {
      contentUrl = representation?.content?.url_template || urlTemplate
      break
    }
    if (state !== 'none' && state !== 'pending' && infoResp.status !== 202) return ''
    if (attempt === 7) return ''
    await new Promise(resolve => setTimeout(resolve, 500))
  }

  const resp = await fetch(buildBoxThumbnailUrl(contentUrl), { headers })
  const mimeType = resp.headers.get('content-type')?.split(';')[0] || 'image/png'
  if (!resp.ok || !mimeType.startsWith('image/')) return ''
  const bytes = new Uint8Array(await resp.arrayBuffer())
  return bytes.length > 0 ? buildBoxThumbnailDataUrl(bytes, mimeType) : ''
}
