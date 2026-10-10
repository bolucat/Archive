import { defineStore } from 'pinia'

const STORAGE_KEY = 'unified-library-category-bookmarks-v1'
const defaults = () => ({ favorites: ['library', 'movies', 'tv', 'other', 'music', 'books'], home: [] as string[] })
export default defineStore('unified-library-category-bookmarks', {
  state: defaults,
  actions: {
    ensureLoaded() {
      try {
        const saved = JSON.parse(localStorage.getItem(STORAGE_KEY) || 'null')
        if (!saved) return
        for (const field of ['favorites', 'home'] as const) if (Array.isArray(saved[field])) this[field] = [...new Set<string>(saved[field].filter((id: unknown) => typeof id === 'string'))]
      } catch { /* Preserve defaults when older settings are malformed. */ }
    },
    toggle(field: 'favorites' | 'home', id: string) {
      this[field] = this[field].includes(id) ? this[field].filter(value => value !== id) : [...this[field], id]
      localStorage.setItem(STORAGE_KEY, JSON.stringify({ favorites: this.favorites, home: this.home }))
    }
  }
})
