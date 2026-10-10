import type { PosterAction } from '../utils/mediaPosterMenu'
export interface UnifiedLibraryCard {
  key: string
  title: string
  sortValues?: { title: string; fileName?: string; addedAt?: string | number | Date; premiereDate?: string }
  image?: string
  rating?: number
  overview?: string
  certification?: string
  subtitle?: string
  /** Playback progress as a percentage from 0 to 100. */
  progress?: number
  contextMenu?: (event: MouseEvent) => void
  posterMenu?: { server: boolean; tv: boolean; watched: boolean; favorite?: boolean; continuing?: boolean; disabled: PosterAction[]; action: (action: PosterAction) => void }
  action: () => void
}

export interface UnifiedLibraryRow {
  key: string
  title: string
  cards: UnifiedLibraryCard[]
  landscape?: boolean
  grouped?: boolean
  loading?: boolean
  error?: string
  more: () => void
}
