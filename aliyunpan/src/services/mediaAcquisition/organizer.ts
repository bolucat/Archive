import type { MediaAcquisitionFileSnapshot, MediaAcquisitionRunView, MediaAcquisitionTarget } from '@shared/types/mediaAcquisition'
import AliFileCmd from '../../aliapi/filecmd'
import { getMediaAcquisitionCapability, normalizeMediaAcquisitionPlatform, normalizeMediaAcquisitionRootFolder } from './capabilities'
import { buildMediaAcquisitionLeafPath, isMediaAcquisitionMatchingSidecar, isMediaAcquisitionPrimaryVideoName } from './organizerPolicy'
import { selectRequestedEpisodeFiles } from './duplicatePolicy'
import { listMediaAcquisitionDirectoryEntries } from './targetSnapshot'

export interface MediaAcquisitionOrganizeResult {
  movedCount: number
  fullyMoved: boolean
  folderId?: string
  folderPath?: string
  files: MediaAcquisitionFileSnapshot[]
  message: string
}

export async function organizeMediaAcquisitionFiles(run: MediaAcquisitionRunView, landedFiles: MediaAcquisitionFileSnapshot[]): Promise<MediaAcquisitionOrganizeResult> {
  const capability = getMediaAcquisitionCapability(run.target.targetPlatform)
  if (!capability || !landedFiles.length) return { movedCount: 0, fullyMoved: true, files: landedFiles, message: '无需整理' }

  // Some providers, including Quark, can omit size in an otherwise valid list
  // item. The verifier treats that as landed; keep the same rule here and only
  // reject files explicitly reported as zero bytes.
  const availableVideoFiles = landedFiles.filter(file => isMediaAcquisitionPrimaryVideoName(file.name) && hasMediaAcquisitionContent(file))
  const videoFiles = selectRequestedEpisodeFiles(run.target, availableVideoFiles)
  if (!videoFiles.length) return { movedCount: 0, fullyMoved: false, files: landedFiles, message: '未发现可整理的视频文件' }
  const selectedVideoIds = new Set(videoFiles.map(file => file.id))
  const files = landedFiles.filter(file => hasMediaAcquisitionContent(file) && (selectedVideoIds.has(file.id) || isMediaAcquisitionMatchingSidecar(file.name, videoFiles.map(video => video.name))))

  const leaf = await ensureMediaAcquisitionLeafFolder(run.target)
  const needMove = files.filter(file => file.parentId !== leaf.id)
  if (!needMove.length) {
    const removedWrappers = run.target.mediaType === 'movie' ? await flattenMovieWrapperDirectories(run.target, leaf.id) : 0
    return { movedCount: 0, fullyMoved: true, folderId: leaf.id, folderPath: leaf.path, files, message: removedWrappers ? `新增文件已在目标目录，已清理 ${removedWrappers} 个电影包装目录` : '新增文件已在目标目录' }
  }

  const success = await AliFileCmd.ApiMoveBatch(run.target.targetUserId, run.target.targetDriveId, needMove.map(file => file.id), run.target.targetDriveId, leaf.id)
  const successIds = new Set(success)
  const movedFiles = files.map(file => successIds.has(file.id) ? { ...file, parentId: leaf.id, path: `${leaf.path}/${file.name}`.replace(/\/+/g, '/') } : file)
  const fullyMoved = successIds.size === needMove.length
  const removedWrappers = fullyMoved && run.target.mediaType === 'movie' ? await flattenMovieWrapperDirectories(run.target, leaf.id) : 0
  return {
    movedCount: successIds.size,
    fullyMoved,
    folderId: leaf.id,
    folderPath: leaf.path,
    files: movedFiles,
    message: fullyMoved ? `已整理 ${successIds.size} 个文件到 ${leaf.path}${removedWrappers ? `，已清理 ${removedWrappers} 个电影包装目录` : ''}` : `仅整理 ${successIds.size}/${needMove.length} 个文件到 ${leaf.path}，暂存目录将保留供重试`
  }
}

export async function ensureMediaAcquisitionLeafFolder(target: MediaAcquisitionTarget): Promise<{ id: string; path: string }> {
  const platform = normalizeMediaAcquisitionPlatform(target.targetPlatform)
  let parentId = normalizeMediaAcquisitionRootFolder(platform, target.targetParentFileId)
  let parentPath = ''
  for (const segment of buildMediaAcquisitionLeafPath(target)) {
    const existing = (await listMediaAcquisitionDirectoryEntries(target, parentId, parentPath)).find(entry => entry.isDir && entry.name === segment)
    if (existing) {
      parentId = existing.id
      parentPath = existing.path
      continue
    }
    const created = await AliFileCmd.ApiCreatNewForder(target.targetUserId, target.targetDriveId, parentId, segment, '', 'refuse')
    if (!created.file_id) {
      const retried = (await listMediaAcquisitionDirectoryEntries(target, parentId, parentPath)).find(entry => entry.isDir && entry.name === segment)
      if (retried) {
        parentId = retried.id
        parentPath = retried.path
        continue
      }
      throw new Error(created.error || `无法创建 ${segment}`)
    }
    parentId = normalizeCreatedFolderId(platform, created.file_id)
    parentPath = `${parentPath}/${segment}`.replace(/\/+/g, '/')
  }
  return { id: parentId, path: parentPath || '/' }
}

export function ensureMediaAcquisitionSeasonFolder(target: MediaAcquisitionTarget, seasonNumber: number): Promise<{ id: string; path: string }> {
  return ensureMediaAcquisitionLeafFolder({ ...target, seasonNumber })
}

function normalizeCreatedFolderId(platform: string, fileId: string): string {
  if (platform === '115' && fileId === '0') return 'drive115_root'
  return fileId
}

function hasMediaAcquisitionContent(file: MediaAcquisitionFileSnapshot): boolean {
  return file.size === undefined || file.size > 0
}

/** Movie imports land in the final directory; resource-pack wrapper folders are disposable after flattening. */
async function flattenMovieWrapperDirectories(target: MediaAcquisitionTarget, movieDirectoryId: string): Promise<number> {
  const wrappers = (await listMediaAcquisitionDirectoryEntries(target, movieDirectoryId)).filter(entry => entry.isDir)
  if (!wrappers.length) return 0
  const removed = await AliFileCmd.ApiTrashBatch(target.targetUserId, target.targetDriveId, wrappers.map(wrapper => wrapper.id))
  return removed.length
}
