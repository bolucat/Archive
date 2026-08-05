export const formatOssMultipartETag = (value: string) => {
  const etag = value.trim().replace(/^"|"$/g, '')
  return etag ? `"${etag}"` : ''
}
