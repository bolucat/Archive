export type Drive115UploadTokenItem = {
  endpoint?: string
  AccessKeySecret?: string
  /** Kept for compatibility with early API responses. */
  AccessKeySecrett?: string
  AccessKeyId?: string
  SecurityToken?: string
  Expiration?: string
}

export type Drive115UploadTokenResp = {
  state: boolean
  code: number
  message: string
  data?: Drive115UploadTokenItem | Drive115UploadTokenItem[]
}

export type Drive115OssCallback = {
  callback?: string
  callback_var?: string
}

export const normalizeDrive115OssCallback = (callback?: string | Drive115OssCallback | Drive115OssCallback[], callbackVar?: string): Drive115OssCallback => {
  if (typeof callback === 'string') return { callback, callback_var: callbackVar }
  const value = Array.isArray(callback) ? callback[0] : callback
  return { callback: value?.callback || '', callback_var: value?.callback_var || callbackVar || '' }
}

export const normalizeDrive115UploadTokens = (data: Drive115UploadTokenResp['data']): Drive115UploadTokenItem[] => {
  const tokens = Array.isArray(data) ? data : data ? [data] : []
  return tokens.map((token) => ({ ...token, AccessKeySecret: token.AccessKeySecret || token.AccessKeySecrett }))
}
