export interface MatchInfo {
  hot: MatchHotItem[]
  banner: MatchBannerItem[]
  preview: MatchPreviewItem[]
}

export enum MatchHotItemStatus {
  NotStarted = 1,
  InProgress,
  Finished,
}

export interface MatchHotItem {
  index: number
  cid: number
  desc: string
  status: MatchHotItemStatus
  name: string
  jump_url: string
}

export interface MatchBannerItem {
  index: number
  cover: string
  jump_url: string
}

export interface MatchPreviewItem {
  index: number
  cid: number
  name: string
  desc: string
  fav_status: number
  jump_url: string
  // spell-checker: disable-next-line
  stime: string
}
