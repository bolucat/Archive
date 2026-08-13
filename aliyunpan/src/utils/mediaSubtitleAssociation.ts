import type { DriveFileItem } from '../types/media'

export type MediaSubtitleFolderScope = 'same-folder' | 'include-subfolders'

const subtitleKey = (file: DriveFileItem) => [file.driveServerId, file.userId, file.driveId, file.id].join(':')

export const associateMediaSubtitles = (
  video: DriveFileItem,
  subtitles: DriveFileItem[],
  folderParents: Map<string, string>,
  scope: MediaSubtitleFolderScope
): DriveFileItem => {
  const folderId = video.parentFileId || ''
  const candidateFolders = new Set([folderId])
  if (scope === 'include-subfolders') {
    for (const [childId, parentId] of folderParents) {
      if (parentId === folderId) candidateFolders.add(childId)
    }
  }
  const associated = subtitles.filter((subtitle) => candidateFolders.has(subtitle.parentFileId || ''))
  const unique = new Map<string, DriveFileItem>()
  associated.forEach((subtitle) => unique.set(subtitleKey(subtitle), subtitle))
  return { ...video, subtitleFiles: [...unique.values()] }
}
