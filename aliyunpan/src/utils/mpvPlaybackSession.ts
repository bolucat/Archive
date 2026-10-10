// Own asynchronous subtitle work by playback, not by component lifetime.
export class MpvPlaybackSession {
  private generation = 0
  private subtitles = new Set<string>()

  begin(): number {
    this.subtitles.clear()
    return ++this.generation
  }

  isCurrent(generation: number): boolean {
    return generation === this.generation
  }

  async addSubtitles<T extends { url: string }>(sources: T[], generation: number, add: (source: T) => Promise<void>): Promise<void> {
    for (const source of sources) {
      if (!this.isCurrent(generation)) return
      if (!source.url || this.subtitles.has(source.url)) continue
      this.subtitles.add(source.url)
      try {
        await add(source)
      } catch (error) {
        if (this.isCurrent(generation)) this.subtitles.delete(source.url)
        throw error
      }
    }
  }
}
