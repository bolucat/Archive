import path from 'path'

export type DownloadTaskStatus =
  | 'active' | 'waiting' | 'paused' | 'error' | 'complete' | 'removed' | string

export interface DownloadTaskFile {
  index: number
  path: string
  name: string
  length: number
  completedLength: number
  selected: boolean
}

export interface DownloadPeer {
  peerId: string
  ip: string
  port: number
  bitfield: string
  amChoking: boolean
  peerChoking: boolean
  downloadSpeed: number
  uploadSpeed: number
  seeder: boolean
}

export interface DownloadBitTorrent {
  infoHash?: string
  numSeeders?: number
  seeder?: string
  mode?: string
  announceList?: string[][]
  info?: { name: string }
}

export interface DownloadTask {
  gid: string
  status: DownloadTaskStatus
  totalLength: number
  completedLength: number
  uploadLength: number
  downloadSpeed: number
  uploadSpeed: number
  numSeeders: number
  seeder: boolean
  connections: number
  numPieces: number
  pieceLength: number
  dir: string
  files: DownloadTaskFile[]
  peers: DownloadPeer[]
  bittorrent?: DownloadBitTorrent
  errorCode: string
  errorMessage: string
}

export interface DownloadGlobalStat {
  downloadSpeed: string
  uploadSpeed: string
  numActive: string
  numWaiting: string
  numStopped: string
  numStoppedTotal: string
}

const toNumber = (value: unknown): number => {
  const parsed = Number(value || 0)
  return Number.isFinite(parsed) ? parsed : 0
}

const toBoolean = (value: unknown): boolean => value === true || value === 'true'

export const normalizeTaskFiles = (files: any[] = []): DownloadTaskFile[] =>
  files.map((file) => ({
    index: toNumber(file.index),
    path: String(file.path || ''),
    name: path.basename(String(file.path || '')),
    length: toNumber(file.length),
    completedLength: toNumber(file.completedLength),
    selected: toBoolean(file.selected)
  }))

export const normalizePeers = (peers: any[] = []): DownloadPeer[] =>
  peers.map((peer) => ({
    peerId: String(peer.peerId || ''),
    ip: String(peer.ip || ''),
    port: toNumber(peer.port),
    bitfield: String(peer.bitfield || ''),
    amChoking: toBoolean(peer.amChoking),
    peerChoking: toBoolean(peer.peerChoking),
    downloadSpeed: toNumber(peer.downloadSpeed),
    uploadSpeed: toNumber(peer.uploadSpeed),
    seeder: toBoolean(peer.seeder)
  }))

export const normalizeAriaTask = (task: any): DownloadTask => ({
  gid: String(task?.gid || ''),
  status: String(task?.status || ''),
  totalLength: toNumber(task?.totalLength),
  completedLength: toNumber(task?.completedLength),
  uploadLength: toNumber(task?.uploadLength),
  downloadSpeed: toNumber(task?.downloadSpeed),
  uploadSpeed: toNumber(task?.uploadSpeed),
  numSeeders: toNumber(task?.numSeeders),
  seeder: toBoolean(task?.seeder),
  connections: toNumber(task?.connections),
  numPieces: toNumber(task?.numPieces),
  pieceLength: toNumber(task?.pieceLength),
  dir: String(task?.dir || ''),
  files: normalizeTaskFiles(task?.files),
  peers: normalizePeers(task?.peers),
  bittorrent: task?.bittorrent,
  errorCode: String(task?.errorCode || ''),
  errorMessage: String(task?.errorMessage || '')
})
