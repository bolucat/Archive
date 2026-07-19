import type { DriveOperation, ProviderCapabilityManifest } from './providerCapabilityTypes'

export const AGENT_TOOL_CAPABILITY_REQUIREMENTS: Record<string, DriveOperation | null> = {
  listDrives: null,
  getConnectedDriveCapabilities: null,
  getBoxPlayerCapabilities: null,
  openBoxPlayerModule: null,
  rememberPreference: null,
  searchMyFiles: 'files.search',
  // Current storage/category analysis is implemented through global file search.
  analyzeStorage: 'files.search',
  categorizeFiles: 'files.search',
  findDuplicates: 'files.list',
  scanDriveLargeFiles: 'files.list',
  scanDriveEmptyDirs: 'files.list',
  deleteDriveEmptyDirs: 'trash.move',
  downloadFiles: 'files.download',
  exportDirectLinks: 'links.direct',
  moveFiles: 'files.move',
  organizeFiles: 'files.move',
  mediaOrganizeFiles: 'files.move',
  deleteFiles: 'trash.move'
}

export function unsupportedAgentToolMessage(manifest: ProviderCapabilityManifest, toolName: string): string | null {
  const capability = AGENT_TOOL_CAPABILITY_REQUIREMENTS[toolName]
  if (!capability || manifest.operations[capability]) return null
  const reason = manifest.notes[0] || '该平台尚未向 Agent 开放此能力'
  return `${manifest.name} 不支持 Agent 操作“${toolName}”（需要 ${capability}）：${reason}`
}
