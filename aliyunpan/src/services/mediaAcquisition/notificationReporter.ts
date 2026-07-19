import type { MediaAcquisitionRunView } from '@shared/types/mediaAcquisition'
import useFootStore from '../../store/footstore'

const delivered = new Set<string>()
const TERMINAL_STATUSES = new Set(['completed', 'partial', 'no_coverage', 'failed', 'cancelled'])

function terminalSummary(run: MediaAcquisitionRunView): { status: 'success' | 'error'; message: string } {
  if (run.status === 'completed') return { status: 'success', message: '已入库' }
  if (run.status === 'partial') return { status: 'success', message: '部分入库完成' }
  if (run.status === 'no_coverage') return { status: 'error', message: '暂无可用资源' }
  if (run.status === 'cancelled') return { status: 'error', message: '已取消' }
  return { status: 'error', message: run.errorMessage || '获取失败' }
}

/** Reuse the footer asynchronous-task notification list for terminal Agent events. */
export function deliverMediaAcquisitionTerminalNotification(run: MediaAcquisitionRunView | null): void {
  if (!run || !TERMINAL_STATUSES.has(run.status)) return
  const key = `${run.id}:${run.status}`
  if (delivered.has(key)) return
  delivered.add(key)
  const summary = terminalSummary(run)
  useFootStore().mAddLocalTask(`media-acquisition:${key}`, `媒体获取 · ${run.target.title} · ${summary.message}`, summary.status)
}
