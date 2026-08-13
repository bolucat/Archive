type SettingItem = {
  name: string
  selector?: unknown[]
  [key: string]: unknown
}

type ArtplayerSettings = {
  active?: unknown[] | null
  find: (name: string) => SettingItem | null | undefined
  update: (target: SettingItem) => SettingItem
  render: (selector: unknown[]) => void
}

/** Rebuild a setting without losing the nested panel the user is currently viewing. */
export const updateSettingPreservingActivePanel = (settings: ArtplayerSettings, target: SettingItem): SettingItem => {
  const current = settings.find(target.name)
  const restoreNestedPanel = !!current?.selector && settings.active === current.selector
  const updated = settings.update(target)
  if (restoreNestedPanel && updated.selector) settings.render(updated.selector)
  return updated
}
