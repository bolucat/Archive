export interface HomeManagementItem {
  id: string
  title: string
  sourceId: string
  sourceTitle?: string
  available: boolean
  child?: boolean
  optIn?: boolean
}

export interface HomeManagementNode {
  id: string
  title: string
  item?: HomeManagementItem
  children?: HomeManagementNode[]
}

export interface HomeManagementGroup {
  id: string
  title: string
  nodes: HomeManagementNode[]
}

export interface UnifiedHomeSettings {
  order: string[]
  hidden: string[]
  titles: Record<string, string>
}

export const defaultHomeSettings = (): UnifiedHomeSettings => ({ order: [], hidden: [], titles: {} })

export const FIXED_HOME_MENU_IDS = ['recent', 'local:unwatched', 'local:daily', 'local:movies', 'local:genres', 'local:tv', 'local:unmatched', 'music']

export function withFixedHomeMenus(items: HomeManagementItem[]) {
  // A fixed entry and its content row can describe the same destination.
  // Keep one identity for the sidebar, management list and home layout.
  items = [...new Map(items.filter(item => item.id !== 'local:favorites').map(item => [item.id, item])).values()]
  const fixed = new Set(FIXED_HOME_MENU_IDS)
  return [
    ...FIXED_HOME_MENU_IDS.flatMap(id => items.filter(item => item.id === id).map(item => ({ ...item, available: true }))),
    ...items.filter(item => !fixed.has(item.id) && item.sourceId === 'local'),
    ...items.filter(item => !fixed.has(item.id) && item.sourceId !== 'local')
  ]
}

export function orderedHomeItems(items: HomeManagementItem[], settings: UnifiedHomeSettings) {
  const rank = new Map(settings.order.map((id, index) => [id, index]))
  return [...items].sort((a, b) => (rank.get(a.id) ?? Number.MAX_SAFE_INTEGER) - (rank.get(b.id) ?? Number.MAX_SAFE_INTEGER))
}

export function visibleHomeItems(items: HomeManagementItem[], settings: UnifiedHomeSettings) {
  return orderedHomeItems(items, settings).filter(item => item.available && (!item.child || item.id.includes(':group:')) && (!item.optIn && !item.child || settings.order.includes(item.id)) && !settings.hidden.includes(item.id))
}

// The sidebar keeps its fixed menus first; the home content starts with source shortcuts.
export function visibleHomeSections(items: HomeManagementItem[], settings: UnifiedHomeSettings) {
  const visible = visibleHomeItems(items, settings)
  if (settings.order.length) return visible
  return [...visible.filter(item => item.id === 'sources'), ...visible.filter(item => item.id !== 'sources')]
}

export function mergeHomeOrder(order: string[], items: HomeManagementItem[]) {
  return [...new Set([...order, ...items.filter(item => !item.optIn && !item.child).map(item => item.id)])]
}

export function moveHomeItem(settings: UnifiedHomeSettings, visibleIds: string[], id: string, destination: string): UnifiedHomeSettings {
  if (id === destination || !visibleIds.includes(id) || !visibleIds.includes(destination)) return settings
  const reordered = visibleIds.filter(value => value !== id)
  reordered.splice(visibleIds.indexOf(destination), 0, id)
  return { ...settings, order: [...reordered, ...settings.order.filter(value => !visibleIds.includes(value))] }
}

export function homeItemTitle(item: HomeManagementItem, settings: UnifiedHomeSettings) {
  return settings.titles[item.id] || item.title
}

export function homeMenuTitle(item: HomeManagementItem, settings: UnifiedHomeSettings) {
  const title = homeItemTitle(item, settings)
  return item.sourceTitle ? `${title} - ${item.sourceTitle}` : title
}
