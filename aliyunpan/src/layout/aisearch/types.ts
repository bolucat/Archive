import type { GlobalSearchResult } from '../../utils/globalSearch'

export interface FileResult {
  name: string
  ext: string
  size: number
  isDir: boolean
  provider: string
  providerName: string
  driveId: string
  fileId: string
  parentFileId: string
  userId: string
  source: string
}

export interface LinkResult {
  type: string
  url: string
  note: string
  password: string
}

export type PartState = 'pending' | 'running' | 'done' | 'error'

export interface TextPart {
  type: 'text'
  text: string
}

export interface ReasoningPart {
  type: 'reasoning'
  text: string
}

export interface ClarificationPart {
  type: 'clarification'
  question: string
  options: string[]
}

export interface SummaryPart {
  type: 'summary'
  text: string
  followups: string[]
}

export interface ToolSearchMyFilesPart {
  type: 'tool-searchMyFiles'
  state: PartState
  input?: { keyword: string }
  output?: { total: number; files: FileResult[] }
  error?: string
}

export interface ToolSearchPanHubPart {
  type: 'tool-searchPanHub'
  state: PartState
  input?: { keyword: string }
  output?: { total: number; links: LinkResult[] }
  error?: string
}

export interface ToolListDrivesPart {
  type: 'tool-listDrives'
  state: 'select'
  drives: { userId: string; name: string; platform: string; driveId: string }[]
}

export interface ToolImportSharePart {
  type: 'tool-importShare'
  state: 'parsing' | 'listing' | 'saving' | 'done' | 'error'
  input?: { url: string; password: string }
  output?: { shareName: string; fileCount: number; savedCount: number; platform: string; asyncStatus?: boolean }
  error?: string
}

export interface ToolDownloadFilesPart {
  type: 'tool-downloadFiles'
  state: 'running' | 'done' | 'error'
  input?: { files: { name: string; fileId: string; driveId: string; userId: string }[] }
  output?: { total: number; success: number }
  error?: string
}

export interface ToolFindDuplicatesPart {
  type: 'tool-findDuplicates'
  state: 'scanning' | 'done' | 'error'
  output?: { totalFiles: number; groups: { name: string; size: number; files: FileResult[] }[] }
  error?: string
}

export interface ToolAnalyzeStoragePart {
  type: 'tool-analyzeStorage'
  state: 'scanning' | 'done' | 'error'
  output?: { drives: { name: string; totalSize: number; fileCount: number; topLarge: FileResult[] }[]; oldestFiles: FileResult[]; unusedFiles: FileResult[] }
  error?: string
}

export interface ToolCategorizeFilesPart {
  type: 'tool-categorizeFiles'
  state: 'planning' | 'done' | 'error'
  output?: { categories: { name: string; pattern: string; fileCount: number; totalSize: number }[] }
  error?: string
}

export interface ToolMoveFilesPart {
  type: 'tool-moveFiles'
  state: 'confirm' | 'running' | 'done' | 'error'
  input?: { files: { name: string; fileId: string; driveId: string; userId: string }[]; targetDir: string }
  output?: { total: number; success: number; failed: number; report?: string }
  error?: string
}

export interface ToolOrganizeFilesPart {
  type: 'tool-organizeFiles'
  state: 'confirm' | 'running' | 'done' | 'error'
  input?: { mode: 'moveToParent' | 'flatten' | 'moveToDir' | 'media'; files: { name: string; fileId: string; driveId: string; userId: string; parentFileId?: string; isDir?: boolean }[]; targetDir: string; plans?: any[] }
  output?: { total: number; success: number; failed: number; report?: string }
  error?: string
}

export interface ToolGetMoviesPart {
  type: 'tool-getMovies'
  state: 'loading' | 'done' | 'error'
  category?: string
  movies?: { id: string; title: string; cover: string; desc: string; url: string }[]
  error?: string
}

export interface ToolDeleteFilesPart {
  type: 'tool-deleteFiles'
  state: 'confirm' | 'running' | 'done' | 'error'
  input?: { files: { name: string; fileId: string; driveId: string; userId: string }[] }
  output?: { total: number; success: number; failed: number }
  error?: string
}

export interface ToolMiaochuanPart {
  type: 'tool-miaochuan'
  state: 'parsing' | 'confirm' | 'running' | 'done' | 'error'
  input?: { parentId?: string; files?: { path: string; name: string; size: number }[] }
  output?: { total: number; success?: number; failed?: number; skipped?: number; report?: string }
  error?: string
}

export interface ToolDirectLinksPart {
  type: 'tool-directLinks'
  state: 'running' | 'done' | 'error'
  input?: { format: 'url' | 'aria2'; files: { name: string; fileId: string; driveId: string; userId: string }[] }
  output?: { total: number; success: number; failed: number; text: string }
  error?: string
}

export interface ToolGuangyaMagnetsPart {
  type: 'tool-guangyaMagnets'
  state: 'confirm' | 'running' | 'done' | 'error'
  input?: { text: string; parentId?: string; magnets: string[] }
  output?: { total: number; success: number; failed: number; report: string }
  error?: string
}

export interface ToolGuangyaEmptyDirsPart {
  type: 'tool-guangyaEmptyDirs'
  state: 'scanning' | 'confirm' | 'running' | 'done' | 'error'
  input?: { rootId?: string; dirs?: { name: string; fileId: string; parentFileId: string; driveId: string; userId: string; path: string }[] }
  output?: { scannedDirs?: number; total: number; success?: number; failed?: number; report: string }
  error?: string
}

export type MessagePart =
  | TextPart
  | ReasoningPart
  | ClarificationPart
  | SummaryPart
  | ToolListDrivesPart
  | ToolSearchMyFilesPart
  | ToolSearchPanHubPart
  | ToolImportSharePart
  | ToolDownloadFilesPart
  | ToolFindDuplicatesPart
  | ToolAnalyzeStoragePart
  | ToolCategorizeFilesPart
  | ToolMoveFilesPart
  | ToolOrganizeFilesPart
  | ToolGetMoviesPart
  | ToolDeleteFilesPart
  | ToolMiaochuanPart
  | ToolDirectLinksPart
  | ToolGuangyaMagnetsPart
  | ToolGuangyaEmptyDirsPart

export interface ChatMessage {
  id: string
  role: 'user' | 'assistant'
  parts: MessagePart[]
}
