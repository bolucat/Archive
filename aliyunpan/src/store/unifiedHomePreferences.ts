import { defineStore } from 'pinia'
import { defaultHomeSettings, type UnifiedHomeSettings } from '../utils/unifiedHomeManagement'

export const UNIFIED_HOME_PREFERENCES_KEY = 'unified-media-home-preferences'

export default defineStore('unified-media-home-preferences', {
  state: defaultHomeSettings,
  actions: {
    ensureLoaded() {
      try {
        const data = JSON.parse(localStorage.getItem(UNIFIED_HOME_PREFERENCES_KEY) || '{}')
        this.$patch({
          order: Array.isArray(data.order) ? data.order.filter((id: unknown) => typeof id === 'string') : [],
          hidden: Array.isArray(data.hidden) ? data.hidden.filter((id: unknown) => typeof id === 'string') : [],
          titles: Object.fromEntries(Object.entries(data.titles || {}).filter(([, value]) => typeof value === 'string')) as Record<string, string>
        })
      } catch { this.$patch(defaultHomeSettings()) }
    },
    apply(settings: UnifiedHomeSettings) {
      this.$patch({ order: [...settings.order], hidden: [...settings.hidden], titles: { ...settings.titles } })
      localStorage.setItem(UNIFIED_HOME_PREFERENCES_KEY, JSON.stringify(settings))
    },
    restoreOrder() {
      this.apply({ order: [], hidden: [], titles: { ...this.titles } })
    },
    rename(id: string, title: string) {
      const name = title.trim()
      if (name) this.apply({ order: [...this.order], hidden: [...this.hidden], titles: { ...this.titles, [id]: name } })
    }
  }
})
