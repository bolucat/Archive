import type { MediaAcquisitionFileSnapshot } from '@shared/types/mediaAcquisition'

export function newMediaAcquisitionFiles(before: MediaAcquisitionFileSnapshot[], after: MediaAcquisitionFileSnapshot[]): MediaAcquisitionFileSnapshot[] {
  const existing = new Set(before.map(snapshotKey))
  return after.filter(item => !existing.has(snapshotKey(item)))
}

function snapshotKey(item: MediaAcquisitionFileSnapshot): string {
  return item.id ? `id:${item.id}` : `path:${item.path}`
}
