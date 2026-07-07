import { defineStore } from 'pinia'
import { IPageMusic } from './appstore'

export interface MusicPlayerState {
  title: string
  artist: string
  album: string
  coverUrl: string
  isPlaying: boolean
  isLoading: boolean
  currentTime: number
  duration: number
  progressPercent: number
  hasTrack: boolean
}

export type MusicPlayerCommand = 'toggle' | 'prev' | 'next'

export default defineStore('musicplayer', {
  state: () => ({
    panelVisible: false,
    commandSeq: 0,
    command: '' as MusicPlayerCommand | '',
    loadSeq: 0,
    pendingLoad: null as IPageMusic | null,
    state: {
      title: '',
      artist: '',
      album: '',
      coverUrl: '',
      isPlaying: false,
      isLoading: false,
      currentTime: 0,
      duration: 0,
      progressPercent: 0,
      hasTrack: false
    } as MusicPlayerState
  }),
  actions: {
    updateState(state: MusicPlayerState) {
      this.state = state
    },
    sendCommand(command: MusicPlayerCommand) {
      this.command = command
      this.commandSeq += 1
    },
    loadMusic(pageMusic: IPageMusic) {
      this.pendingLoad = pageMusic
      this.loadSeq += 1
    },
    togglePanel() {
      this.panelVisible = !this.panelVisible
    },
    showPanel() {
      this.panelVisible = true
    },
    hidePanel() {
      this.panelVisible = false
    }
  }
})
