export interface MagnetImportResult {
  total: number
  success: number
  skipped: number
  failed: number
  tasks: { magnet: string; taskId: string; fileId: string }[]
  failures: { magnet: string; reason: string }[]
  report: string
}

const MAGNET_RE = /magnet:\?[^\s"'<>]+/ig

export const getMagnetBtih = (magnet: string): string => {
  try {
    const query = magnet.slice(magnet.indexOf('?') + 1)
    const params = new URLSearchParams(query)
    const xt = params.get('xt') || ''
    const match = xt.match(/btih:([a-z0-9]+)/i)
    return (match?.[1] || '').toLowerCase()
  } catch {
    return ''
  }
}

export const getMagnetDisplayName = (magnet: string): string => {
  try {
    const query = magnet.slice(magnet.indexOf('?') + 1)
    const params = new URLSearchParams(query)
    const dn = params.get('dn')
    if (dn) return decodeURIComponent(dn).trim()
  } catch {}
  const btih = getMagnetBtih(magnet)
  return btih ? `磁力_${btih.slice(0, 12)}` : '磁力资源'
}

export const extractMagnetLinks = (text: string): string[] => {
  const seen = new Set<string>()
  const out: string[] = []
  const raw = String(text || '')
  const matches = raw.match(MAGNET_RE) || []
  for (const match of matches) {
    const magnet = String(match || '').trim().replace(/[),.;]+$/g, '')
    const key = getMagnetBtih(magnet) || magnet.toLowerCase()
    if (!magnet || seen.has(key)) continue
    seen.add(key)
    out.push(magnet)
  }
  return out
}

export const importGuangyaMagnets = async (userId: string, parentId: string, text: string): Promise<MagnetImportResult> => {
  const magnets = extractMagnetLinks(text)
  const result: MagnetImportResult = { total: magnets.length, success: 0, skipped: 0, failed: 0, tasks: [], failures: [], report: '' }
  const { apiGuangyaOfflineCreate } = await import('../../guangya/offline')
  for (const magnet of magnets) {
    const task = await apiGuangyaOfflineCreate(userId, magnet, getMagnetDisplayName(magnet), parentId || 'guangya_root')
    if (task.error) {
      result.failed += 1
      result.failures.push({ magnet, reason: task.error })
      continue
    }
    result.success += 1
    result.tasks.push({ magnet, taskId: task.taskId, fileId: task.fileId })
  }
  result.report = `磁力云添加完成：成功 ${result.success}/${result.total}，失败 ${result.failed}${result.failures.length ? `\n失败示例：${result.failures.slice(0, 5).map(item => `${getMagnetDisplayName(item.magnet)}(${item.reason})`).join('；')}` : ''}`
  return result
}
