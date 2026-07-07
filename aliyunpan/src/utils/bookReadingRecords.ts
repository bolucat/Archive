const LS_KEY = 'bookLibrary.readingRecords'

export interface ReadingRecord {
  date: string   // 'YYYY-MM-DD'
  minutes: number
  bookId: string
  bookTitle: string
}

export interface DailyStats {
  date: string
  minutes: number
}

function todayKey(): string {
  const d = new Date()
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
}

export function loadReadingRecords(): ReadingRecord[] {
  try {
    const raw = localStorage.getItem(LS_KEY)
    return raw ? JSON.parse(raw) : []
  } catch {
    return []
  }
}

function saveReadingRecords(records: ReadingRecord[]) {
  try {
    localStorage.setItem(LS_KEY, JSON.stringify(records))
  } catch {}
}

/** 记录一次阅读会话（每分钟调用一次） */
export function recordReadingMinute(bookId: string, bookTitle: string) {
  const records = loadReadingRecords()
  const today = todayKey()
  // 找今天的记录（同一本书）
  const existing = records.find((r) => r.date === today && r.bookId === bookId)
  if (existing) {
    existing.minutes += 1
  } else {
    records.push({ date: today, minutes: 1, bookId, bookTitle })
  }
  saveReadingRecords(records)
}

/** 获取指定日期范围的每日阅读时长汇总 */
export function getDailyStats(daysBack: number, endDate?: string): DailyStats[] {
  const records = loadReadingRecords()
  const end = endDate ? new Date(endDate) : new Date()
  const result: DailyStats[] = []

  for (let i = daysBack - 1; i >= 0; i--) {
    const d = new Date(end)
    d.setDate(end.getDate() - i)
    const key = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
    const minutes = records.filter((r) => r.date === key).reduce((s, r) => s + r.minutes, 0)
    result.push({ date: key, minutes })
  }

  return result
}

/** 获取热力图数据 */
export function getHeatmapStats(weeksBack: number, endDate?: string): DailyStats[] {
  return getDailyStats(weeksBack * 7, endDate)
}

/** 计算总阅读时长（分钟） */
export function getTotalReadingMinutes(): number {
  return loadReadingRecords().reduce((s, r) => s + r.minutes, 0)
}

/** 获取最长连续阅读天数 */
export function getLongestStreak(): number {
  const records = loadReadingRecords()
  const dates = new Set(records.map((r) => r.date))
  if (!dates.size) return 0

  let maxStreak = 0
  let currentStreak = 0
  const today = new Date()

  for (let i = 365; i >= 0; i--) {
    const d = new Date(today)
    d.setDate(today.getDate() - i)
    const key = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
    if (dates.has(key)) {
      currentStreak++
      if (currentStreak > maxStreak) maxStreak = currentStreak
    } else {
      currentStreak = 0
    }
  }

  return maxStreak
}

/** 获取有效阅读天数 */
export function getActiveReadingDays(): number {
  const records = loadReadingRecords()
  return new Set(records.map((r) => r.date)).size
}

/** 清除超过指定天数的记录 */
export function pruneOldRecords(maxDays = 365) {
  const records = loadReadingRecords()
  const cutoff = new Date()
  cutoff.setDate(cutoff.getDate() - maxDays)
  const cutoffKey = `${cutoff.getFullYear()}-${String(cutoff.getMonth() + 1).padStart(2, '0')}-${String(cutoff.getDate()).padStart(2, '0')}`
  saveReadingRecords(records.filter((r) => r.date >= cutoffKey))
}
