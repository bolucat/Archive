export const QUICK_FILE_STORAGE_KEY = 'FileQuick-v2'

export interface QuickFilePathNode {
  drive_id: string
  file_id: string
  parent_file_id: string
  name: string
  path?: string
  description?: string
}

export interface QuickFileEntry {
  id: string
  user_id: string
  user_name?: string
  provider: string
  drive_id: string
  drive_name: string
  file_id: string
  parent_file_id: string
  path: string
  title: string
  description?: string
  dir_path: QuickFilePathNode[]
}

export interface LegacyQuickFileEntry {
  key: string
  drive_id: string
  drive_name: string
  title: string
}

export const quickFileId = (user_id: string, drive_id: string, file_id: string) => {
  return [user_id, drive_id, file_id].map(value => encodeURIComponent(value || '')).join(':')
}

export const mergeQuickFiles = (current: QuickFileEntry[], incoming: QuickFileEntry[]): QuickFileEntry[] => {
  const result = [...current]
  for (const item of incoming) {
    const index = result.findIndex(existing => existing.id === item.id)
    if (index >= 0) result[index] = item
    else result.push(item)
  }
  return result
}

export const buildQuickFilePath = (item: QuickFileEntry): QuickFilePathNode[] => {
  const path = (item.dir_path || []).map(node => ({ ...node }))
  if (!path.some(node => node.file_id === item.file_id)) {
    path.push({
      drive_id: item.drive_id,
      file_id: item.file_id,
      parent_file_id: item.parent_file_id || '',
      name: item.title,
      path: item.path,
      description: item.description || ''
    })
  }
  return path
}

export const migrateLegacyQuickFiles = (current: QuickFileEntry[], user_id: string, legacy: LegacyQuickFileEntry[], provider: string, user_name = user_id): QuickFileEntry[] => {
  return mergeQuickFiles(current, legacy.map(item => ({
    id: quickFileId(user_id, item.drive_id, item.key),
    user_id,
    user_name,
    provider,
    drive_id: item.drive_id,
    drive_name: item.drive_name,
    file_id: item.key,
    parent_file_id: '',
    path: item.key.startsWith('/') ? item.key : '',
    title: item.title || item.key,
    description: '',
    dir_path: []
  })))
}
