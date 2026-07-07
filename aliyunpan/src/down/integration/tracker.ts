export const normalizeTrackerText = (text: string): string => {
  const lines = text
    .split(/[\r\n,]+/)
    .map((l) => l.trim())
    .filter((l) => l.startsWith('http') || l.startsWith('udp') || l.startsWith('ws'))
  const deduped = [...new Set(lines)]
  return deduped.join('\n')
}

export const trackerTextToAriaOption = (text: string): { 'bt-tracker': string } => ({
  'bt-tracker': normalizeTrackerText(text).split('\n').filter(Boolean).join(',')
})

export const fetchTrackerSource = async (url: string): Promise<string> => {
  const response = await fetch(url)
  if (!response.ok) throw new Error(`Tracker 拉取失败: ${response.status}`)
  return normalizeTrackerText(await response.text())
}
