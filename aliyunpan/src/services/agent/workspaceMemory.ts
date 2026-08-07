import type { MemoryRow } from '../reedy/types'
import { ReedyClient } from '../reedy/ReedyClient'
import UserDAL from '../../user/userdal'

const MEMORY_LIMIT = 12
const client = new ReedyClient()

export type WorkspaceMemory = MemoryRow

function hasReedyBridge(): boolean {
  return typeof window !== 'undefined' && typeof (window as any).ReedyInvoke === 'function'
}

async function workspaceScopeKey(): Promise<string> {
  const users = await UserDAL.GetUserListFromDB().catch(() => [])
  const accountIds = users
    .filter(user => user?.user_id && user?.access_token)
    .map(user => user.user_id)
    .sort()
  // There is no BoxPlayer account login. Scope memory to the local set of
  // connected drive accounts so two desktop profiles cannot share preferences.
  return `boxplayer-ai-workspace:${accountIds.join('|') || 'guest'}`
}

export async function listWorkspaceMemories(limit = MEMORY_LIMIT): Promise<WorkspaceMemory[]> {
  if (!hasReedyBridge()) return []
  return client.listMemories('user', await workspaceScopeKey(), limit)
}

export async function rememberWorkspaceFact(key: string, summary: string, sourceMessageId?: string): Promise<WorkspaceMemory | null> {
  if (!hasReedyBridge() || !key.trim() || !summary.trim()) return null
  return client.writeMemory({ scope: 'user', scope_key: await workspaceScopeKey(), key: key.trim().slice(0, 128), summary: summary.trim().slice(0, 2000), source_message_id: sourceMessageId })
}

export async function forgetWorkspaceMemory(id: string): Promise<boolean> {
  if (!hasReedyBridge()) return false
  return client.deleteMemory(id)
}

export function buildWorkspaceMemoryContext(memories: WorkspaceMemory[]): string {
  if (!memories.length) return '暂无长期记忆。'
  return memories.map(memory => `- ${memory.key}: ${memory.summary}`).join('\n')
}
