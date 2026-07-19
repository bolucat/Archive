function beijingCalendarDate(now: Date): string {
  const parts = new Intl.DateTimeFormat('en-US', { timeZone: 'Asia/Shanghai', year: 'numeric', month: '2-digit', day: '2-digit' }).formatToParts(now)
  const value = (type: string) => parts.find(part => part.type === type)?.value || ''
  return `${value('year')}-${value('month')}-${value('day')}`
}

function validReleaseDate(releaseDate?: string): string | undefined {
  const date = releaseDate?.slice(0, 10)
  return date && /^\d{4}-\d{2}-\d{2}$/.test(date) ? date : undefined
}

export function isMediaAcquisitionMovieUnreleased(releaseDate?: string, now = new Date()): boolean {
  const date = validReleaseDate(releaseDate)
  return !!date && date > beijingCalendarDate(now)
}

export function mediaAcquisitionReleaseAt(releaseDate?: string): number | undefined {
  const date = validReleaseDate(releaseDate)
  return date ? Date.parse(`${date}T00:00:00+08:00`) : undefined
}
