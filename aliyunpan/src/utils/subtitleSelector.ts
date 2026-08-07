type SubtitleSelectorItem = {
  file_id?: string
  url?: string
  name?: string
  html?: string
}

const subtitleSelectorKey = (item: SubtitleSelectorItem, index: number) => {
  const label = String(item.name || item.html || '').trim().toLocaleLowerCase()
  if (label) return `label:${label}`
  if (item.file_id) return `file:${item.file_id}`
  if (item.url) return `url:${item.url}`
  return `item:${index}`
}

/** Later sources win so a freshly downloaded/selected subtitle replaces its stale directory copy. */
export const dedupeSubtitleSelectors = <T extends SubtitleSelectorItem>(items: T[]): T[] => {
  const unique = new Map<string, T>()
  items.forEach((item, index) => unique.set(subtitleSelectorKey(item, index), item))
  return [...unique.values()]
}
