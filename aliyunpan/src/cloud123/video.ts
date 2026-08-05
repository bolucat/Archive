import message from '../utils/message'
import { getCloud123Token } from './auth'

export type Cloud123TranscodeItem = {
  url: string
  resolution?: string
  duration?: number
  height?: number
  status?: number
  bitRate?: number
  progress?: number
  updateAt?: string
}

export type Cloud123TranscodeResp = {
  code: number
  message: string
  data?: {
    list?: Cloud123TranscodeItem[]
    status?: number
  }
}

const TRANSCODE_URL = 'https://open-api.123pan.com/api/v1/video/transcode/list'

export const apiCloud123TranscodeList = async (
  user_id: string,
  fileId: string
): Promise<{ status: number; list: Cloud123TranscodeItem[] } | string> => {
  const token = await getCloud123Token(user_id)
  if (!token?.access_token) return '未登录 123 云盘'
  if (!fileId) return '参数错误'
  const params = new URLSearchParams({ fileId })
  const url = `${TRANSCODE_URL}?${params.toString()}`
  try {
    const resp = await fetch(url, {
      headers: {
        'Content-Type': 'application/json',
        Platform: 'open_platform',
        Authorization: `Bearer ${token.access_token}`
      }
    })
    if (!resp.ok) return '获取转码列表失败'
    const data = (await resp.json()) as Cloud123TranscodeResp
    if (data?.code !== 0) return data?.message || '获取转码列表失败'
    const list = Array.isArray(data?.data?.list) ? data.data!.list! : []
    const status = Number(data?.data?.status || 0)
    return { status, list }
  } catch (err: any) {
    message.error('获取转码列表失败 ' + (err?.message || ''))
    return '获取转码列表失败'
  }
}
