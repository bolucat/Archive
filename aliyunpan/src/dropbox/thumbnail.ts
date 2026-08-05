import message from '../utils/message'
import { getDropboxToken } from './dirfilelist'

const DROPBOX_CONTENT_API_HOST = 'https://content.dropboxapi.com/2'

export type DropboxThumbnailFormat = 'jpeg' | 'png'
export type DropboxThumbnailSize = 'w32h32' | 'w64h64' | 'w128h128' | 'w256h256' | 'w480h320' | 'w640h480' | 'w960h640' | 'w1024h768' | 'w2048h1536'

export const buildDropboxThumbnailArg = (fileId: string, format: DropboxThumbnailFormat = 'jpeg', size: DropboxThumbnailSize = 'w256h256') => ({
  resource: {
    '.tag': 'path',
    path: fileId
  },
  format,
  size,
  mode: 'strict'
})

export const buildDropboxThumbnailDataUrl = (bytes: Uint8Array, format: DropboxThumbnailFormat = 'jpeg'): string => {
  let binary = ''
  const chunkSize = 0x8000
  for (let i = 0; i < bytes.length; i += chunkSize) {
    binary += String.fromCharCode(...bytes.subarray(i, i + chunkSize))
  }
  return `data:image/${format};base64,${btoa(binary)}`
}

export const buildDropboxThumbnailBatchBody = (fileIds: string[], format: DropboxThumbnailFormat = 'jpeg', size: DropboxThumbnailSize = 'w256h256') => ({
  entries: fileIds.map((path) => ({ path, format, size, mode: 'strict', quality: 'quality_80' }))
})

type DropboxThumbnailBatchResponse = {
  entries?: Array<{ '.tag'?: string; metadata?: { id?: string }; thumbnail?: string }>
}

export const apiDropboxThumbnail = async (
  user_id: string,
  fileId: string,
  format: DropboxThumbnailFormat = 'jpeg',
  size: DropboxThumbnailSize = 'w256h256'
): Promise<string> => {
  const token = await getDropboxToken(user_id)
  if (!token?.access_token) {
    message.error('未登录 Dropbox')
    return ''
  }
  const resp = await fetch(`${DROPBOX_CONTENT_API_HOST}/files/get_thumbnail_v2`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${token.access_token}`,
      'Dropbox-API-Arg': JSON.stringify(buildDropboxThumbnailArg(fileId, format, size))
    },
    body: ''
  })
  if (!resp.ok) return ''
  const bytes = new Uint8Array(await resp.arrayBuffer())
  return bytes.length > 0 ? buildDropboxThumbnailDataUrl(bytes, format) : ''
}

export const apiDropboxThumbnails = async (user_id: string, fileIds: string[], format: DropboxThumbnailFormat = 'jpeg', size: DropboxThumbnailSize = 'w256h256'): Promise<Map<string, string>> => {
  const token = await getDropboxToken(user_id)
  const result = new Map<string, string>()
  if (!token?.access_token) return result
  for (let index = 0; index < fileIds.length; index += 25) {
    const ids = fileIds.slice(index, index + 25)
    const resp = await fetch(`${DROPBOX_CONTENT_API_HOST}/files/get_thumbnail_batch`, {
      method: 'POST',
      headers: { Authorization: `Bearer ${token.access_token}`, 'Content-Type': 'application/json' },
      body: JSON.stringify(buildDropboxThumbnailBatchBody(ids, format, size))
    })
    const data = await resp.json().catch(() => undefined) as DropboxThumbnailBatchResponse | undefined
    if (!resp.ok) continue
    for (const entry of data?.entries || []) {
      if (entry['.tag'] !== 'success' || !entry.metadata?.id || !entry.thumbnail) continue
      result.set(entry.metadata.id, `data:image/${format};base64,${entry.thumbnail}`)
    }
  }
  return result
}
