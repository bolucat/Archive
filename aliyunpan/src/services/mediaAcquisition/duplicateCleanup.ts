import AliFileCmd from '../../aliapi/filecmd'
import type { MediaAcquisitionDuplicateGroup } from './duplicatePolicy'
import { listMediaAcquisitionTargetFiles } from './targetSnapshot'
import type { MediaAcquisitionRunView } from '@shared/types/mediaAcquisition'

export async function trashVerifiedMediaAcquisitionDuplicates(run: MediaAcquisitionRunView, groups: MediaAcquisitionDuplicateGroup[]): Promise<{ removed: number; errors: string[] }> {
  const candidates = groups.flatMap(group => group.deleteCandidates)
  if (!candidates.length) return { removed: 0, errors: [] }
  const current = new Map((await listMediaAcquisitionTargetFiles(run.target)).map(file => [file.id, file]))
  const changed = candidates.find(file => {
    const latest = current.get(file.id)
    return !latest || latest.name !== file.name || Number(latest.size || 0) !== Number(file.size || 0)
  })
  if (changed) throw new Error(`目录已变化，未执行清理：${changed.name}`)

  const errors = await AliFileCmd.ApiTrashBatch(run.target.targetUserId, run.target.targetDriveId, candidates.map(file => file.id))
  return { removed: candidates.length - errors.length, errors }
}
