export type BookManagerView =
  | 'home'
  | 'recent'
  | 'favorites'
  | 'shelves'
  | 'notes'
  | 'highlights'
  | 'bookmarks'
  | 'trash'
  | 'folders'
  | 'formats'
  | 'stats'
  | 'shelf'
  | 'folder'
  | 'format'

export type BookManagerSortMode = 'title' | 'author' | 'added' | 'recent' | 'readingTime' | 'progress' | 'size'
export type BookManagerSortOrder = 'asc' | 'desc'
export type BookViewMode = 'grid' | 'list' | 'cover'

export interface BookManagerTab {
  key: BookManagerView
  label: string
}

export interface BookShelfGroup {
  id: string
  name: string
  book_ids: string[]
  created_at: number
  updated_at: number
}
