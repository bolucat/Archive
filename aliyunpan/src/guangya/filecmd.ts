import { guangyaApiParentId, guangyaRequest } from './dirfilelist'

export const apiGuangyaMkdir = async (user_id: string, parentId: string, name: string): Promise<{ file_id: string; error: string }> => {
  try {
    const data = await guangyaRequest(user_id, '/nd.bizuserres.s/v1/file/create_dir', {
      dirName: name,
      parentId: guangyaApiParentId(parentId)
    })
    const item = data?.data || data || {}
    const fileId = item.fileId || item.id || item.dirId || item.dirID || ''
    return { file_id: String(fileId || ''), error: fileId ? '' : '新建文件夹失败' }
  } catch (error: any) {
    return { file_id: '', error: error?.message || '新建文件夹失败' }
  }
}

export const apiGuangyaRename = async (user_id: string, fileId: string, name: string): Promise<{ success: boolean; name: string; parent_file_id: string; isDir: boolean }> => {
  try {
    const data = await guangyaRequest(user_id, '/nd.bizuserres.s/v1/file/rename', { fileId, newName: name })
    const item = data?.data || data || {}
    return { success: true, name: item.name || item.fileName || name, parent_file_id: item.parentId || item.parentFileId || '', isDir: !!(item.isDir || item.dir || item.isFolder) }
  } catch {
    return { success: false, name, parent_file_id: '', isDir: false }
  }
}

export const apiGuangyaTrashBatch = async (user_id: string, ids: string[]): Promise<string[]> => {
  if (!ids.length) return []
  try {
    await guangyaRequest(user_id, '/nd.bizuserres.s/v1/file/delete_file', { fileIds: ids })
    return ids
  } catch {
    return []
  }
}

export const apiGuangyaMoveBatch = async (user_id: string, ids: string[], toParentId: string): Promise<string[]> => {
  if (!ids.length) return []
  try {
    await guangyaRequest(user_id, '/nd.bizuserres.s/v1/file/move_file', {
      fileIds: ids,
      parentId: guangyaApiParentId(toParentId)
    })
    return ids
  } catch {
    return []
  }
}

export const apiGuangyaCopyBatch = async (user_id: string, ids: string[], toParentId: string): Promise<string[]> => {
  if (!ids.length) return []
  try {
    await guangyaRequest(user_id, '/nd.bizuserres.s/v1/file/copy_file', {
      fileIds: ids,
      parentId: guangyaApiParentId(toParentId)
    })
    return ids
  } catch {
    return []
  }
}

