export const mergeTreeRootsPreservingOrder = <T extends { key?: string | number }>(current: T[], roots: T[]): T[] => {
  const replacements = new Map(roots.map((root) => [root.key, root]))
  const merged = current.map((item) => {
    const replacement = replacements.get(item.key)
    if (!replacement) return item
    replacements.delete(item.key)
    return replacement
  })
  for (const root of roots) {
    if (replacements.has(root.key)) {
      merged.push(root)
      replacements.delete(root.key)
    }
  }
  return merged
}
