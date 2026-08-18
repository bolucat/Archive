import type { IAliGetFileModel } from '../aliapi/alimodels'
import type { ProviderListResult } from './providerList'

export type ProviderPaginationState = {
  cursor: string
  requestedCursors: Set<string>
  fileIds: Set<string>
}

export const createProviderPaginationState = (items: IAliGetFileModel[] = []): ProviderPaginationState => ({ cursor: '', requestedCursors: new Set(), fileIds: new Set(items.map(item => item.file_id)) })

export const beginProviderPage = (state: ProviderPaginationState, cursor = state.cursor): string | undefined => {
  if (state.requestedCursors.has(cursor)) return undefined
  state.requestedCursors.add(cursor)
  return cursor
}

export const cancelProviderPage = (state: ProviderPaginationState, cursor: string): void => {
  state.requestedCursors.delete(cursor)
}

export const completeProviderPage = (state: ProviderPaginationState, page: Pick<ProviderListResult, 'items' | 'nextCursor'>): IAliGetFileModel[] => {
  const items = page.items.filter(item => !state.fileIds.has(item.file_id))
  items.forEach(item => state.fileIds.add(item.file_id))
  const nextCursor = String(page.nextCursor || '')
  state.cursor = nextCursor && !state.requestedCursors.has(nextCursor) ? nextCursor : ''
  return items
}

export async function* iterateProviderPages(fetchPage: (cursor: string) => Promise<ProviderListResult> | undefined, shouldStop?: () => boolean): AsyncGenerator<IAliGetFileModel[]> {
  const state = createProviderPaginationState()
  while (!shouldStop?.()) {
    const cursor = beginProviderPage(state)
    if (cursor === undefined) return
    const page = await fetchPage(cursor)
    if (!page) return
    const items = completeProviderPage(state, page)
    if (items.length) yield items
    if (!state.cursor) return
  }
}
