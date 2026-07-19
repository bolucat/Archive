import { AriaRawCall, AriaConnect } from '../../utils/aria2c'
import { normalizeAriaTask, normalizeTaskFiles } from './taskTypes'
import type { DownloadTask, DownloadGlobalStat, DownloadTaskFile } from './taskTypes'

const TASK_FIELDS = [
  'gid', 'status', 'totalLength', 'completedLength', 'uploadLength',
  'downloadSpeed', 'uploadSpeed', 'numSeeders', 'seeder', 'connections',
  'numPieces', 'pieceLength', 'errorCode', 'errorMessage', 'dir',
  'files', 'bittorrent', 'followedBy', 'verifiedLength', 'verifyIntegrityPending'
] as const

export interface MagnetPreview {
  metadataGid: string
  taskGid: string
  name: string
  files: DownloadTaskFile[]
}

interface PrepareMagnetOptions {
  signal?: AbortSignal
  timeoutMs?: number
  pollIntervalMs?: number
}

const wait = (ms: number, signal?: AbortSignal) => new Promise<void>((resolve, reject) => {
  if (signal?.aborted) {
    reject(new DOMException('Aborted', 'AbortError'))
    return
  }
  const timer = setTimeout(resolve, ms)
  signal?.addEventListener('abort', () => {
    clearTimeout(timer)
    reject(new DOMException('Aborted', 'AbortError'))
  }, { once: true })
})

const removeAriaTask = async (gid: string) => {
  if (!gid) return
  try { await AriaRawCall('aria2.forceRemove', gid) } catch {}
  try { await AriaRawCall('aria2.removeDownloadResult', gid) } catch {}
}

export async function discardMagnetPreview(preview?: Pick<MagnetPreview, 'metadataGid' | 'taskGid'> | null): Promise<void> {
  if (!preview) return
  await AriaConnect()
  await Promise.all([...new Set([preview.taskGid, preview.metadataGid])].filter(Boolean).map(removeAriaTask))
}

export async function prepareMagnetFiles(magnet: string, dir: string, options: PrepareMagnetOptions = {}): Promise<MagnetPreview> {
  const connected = await AriaConnect()
  if (!connected) throw new Error('Aria2 未连接')

  const timeoutMs = options.timeoutMs ?? 90000
  const pollIntervalMs = options.pollIntervalMs ?? 500
  const startedAt = Date.now()
  let metadataGid = ''
  let taskGid = ''

  try {
    metadataGid = String(await AriaRawCall('aria2.addUri', [magnet], {
      dir,
      'follow-torrent': 'mem',
      'pause-metadata': 'true'
    }) || '')
    if (!metadataGid) throw new Error('无法创建磁力元数据任务')

    while (Date.now() - startedAt < timeoutMs) {
      if (options.signal?.aborted) throw new DOMException('Aborted', 'AbortError')
      const status = await AriaRawCall('aria2.tellStatus', metadataGid, ['status', 'followedBy', 'errorCode', 'errorMessage'])
      if (status?.status === 'error' || status?.status === 'removed') {
        throw new Error(status?.errorMessage || `磁力解析失败 (${status?.errorCode || 'unknown'})`)
      }
      taskGid = String(status?.followedBy?.[0] || '')
      if (taskGid) {
        await AriaRawCall('aria2.forcePause', taskGid).catch(() => undefined)
        const task = await AriaRawCall('aria2.tellStatus', taskGid, ['files', 'bittorrent'])
        const files = normalizeTaskFiles(task?.files)
        if (files.length) {
          return {
            metadataGid,
            taskGid,
            name: String(task?.bittorrent?.info?.name || ''),
            files
          }
        }
      }
      await wait(pollIntervalMs, options.signal)
    }
    throw new Error('获取磁力文件列表超时，请稍后重试')
  } catch (error) {
    await discardMagnetPreview({ metadataGid, taskGid })
    throw error
  }
}

export const buildSelectFileOption = (indexes: number[]): { 'select-file': string } => ({
  'select-file': indexes
    .filter((index) => Number.isFinite(index) && index > 0)
    .join(',')
})

export const normalizeTaskListResult = (tasks: any[] = []): DownloadTask[] =>
  tasks.map((task) => normalizeAriaTask(task))

export async function getTaskStatus(gid: string): Promise<DownloadTask | null> {
  try {
    await AriaConnect()
    const task = await AriaRawCall('aria2.tellStatus', gid, [...TASK_FIELDS])
    return normalizeAriaTask(task)
  } catch {
    return null
  }
}

export async function getActiveList(): Promise<DownloadTask[]> {
  try {
    await AriaConnect()
    return normalizeTaskListResult(await AriaRawCall('aria2.tellActive', [...TASK_FIELDS]))
  } catch {
    return []
  }
}

export async function getWaitingList(offset = 0, num = 200): Promise<DownloadTask[]> {
  try {
    await AriaConnect()
    return normalizeTaskListResult(await AriaRawCall('aria2.tellWaiting', offset, num, [...TASK_FIELDS]))
  } catch {
    return []
  }
}

export async function getStoppedList(offset = 0, num = 200): Promise<DownloadTask[]> {
  try {
    await AriaConnect()
    return normalizeTaskListResult(await AriaRawCall('aria2.tellStopped', offset, num, [...TASK_FIELDS]))
  } catch {
    return []
  }
}

export async function getTaskFiles(gid: string): Promise<DownloadTaskFile[]> {
  try {
    await AriaConnect()
    return normalizeTaskFiles(await AriaRawCall('aria2.getFiles', gid))
  } catch {
    return []
  }
}

export async function changeTaskSelectedFiles(gid: string, indexes: number[]): Promise<void> {
  await changeTaskOption(gid, buildSelectFileOption(indexes))
}

export async function changeTaskOption(gid: string, options: Record<string, string>): Promise<void> {
  try {
    await AriaConnect()
    await AriaRawCall('aria2.changeOption', gid, options)
  } catch {}
}

export async function pauseTask(gid: string): Promise<void> {
  try {
    await AriaConnect()
    await AriaRawCall('aria2.forcePause', gid)
  } catch {}
}

export async function resumeTask(gid: string): Promise<void> {
  try {
    await AriaConnect()
    await AriaRawCall('aria2.unpause', gid)
  } catch {}
}

export async function removeTask(gid: string): Promise<void> {
  try {
    await AriaConnect()
    await AriaRawCall('aria2.forceRemove', gid)
    await AriaRawCall('aria2.removeDownloadResult', gid)
  } catch {}
}

export async function batchPauseTasks(gids: string[]): Promise<void> {
  await Promise.all(gids.filter(Boolean).map((gid) => pauseTask(gid)))
}

export async function batchResumeTasks(gids: string[]): Promise<void> {
  await Promise.all(gids.filter(Boolean).map((gid) => resumeTask(gid)))
}

export async function batchRemoveTasks(gids: string[]): Promise<void> {
  await Promise.all(gids.filter(Boolean).map((gid) => removeTask(gid)))
}

export async function getGlobalStat(): Promise<DownloadGlobalStat | null> {
  try {
    await AriaConnect()
    return (await AriaRawCall('aria2.getGlobalStat')) as DownloadGlobalStat
  } catch {
    return null
  }
}

export async function changeGlobalOption(options: Record<string, string>): Promise<void> {
  try {
    await AriaConnect()
    await AriaRawCall('aria2.changeGlobalOption', options)
  } catch {}
}
