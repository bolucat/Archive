export interface VideoQualitySwitchPlaybackState {
  shouldResume: boolean
  position: number
}

export const captureVideoQualitySwitchPlaybackState = (playing: boolean, position: number): VideoQualitySwitchPlaybackState => ({
  shouldResume: Boolean(playing),
  position: Number.isFinite(position) && position > 0 ? position : 0
})
