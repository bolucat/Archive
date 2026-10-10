import type { MediaServerMediaInfoCard } from '../types/mediaServerContent'

export const mediaFileSummary = (fileLabel: string | undefined, cards: MediaServerMediaInfoCard[]): string => {
  const video = cards.find(card => card.kind === 'video' && card.selected) || cards.find(card => card.kind === 'video')
  const audio = cards.find(card => card.kind === 'audio' && card.selected) || cards.find(card => card.kind === 'audio')
  const value = (card: MediaServerMediaInfoCard | undefined, label: string) => card?.rows.find(row => row.label === label)?.value
  const codec = (raw: string | undefined) => {
    const name = raw?.toUpperCase()
    return name === 'H264' || name === 'AVC' ? 'H.264' : name === 'H265' || name === 'HEVC' ? 'H.265' : name
  }
  const size = fileLabel?.match(/\d+(?:\.\d+)?\s*(?:TB|GB|MB|KB|B)\b/i)?.[0].replace(/(\d)([A-Z])/i, '$1 $2')
  const dimensions = value(video, '分辨率')?.match(/(\d+)\s*[x×]\s*(\d+)/i)
  const resolution = dimensions ? '(' + dimensions[2] + 'p)' : value(video, '分辨率')
  const layout = value(audio, '布局')
  const channels = value(audio, 'Channels')
  const channelLabel = layout?.match(/\d+\.\d+/)?.[0] || ({ '1': '1.0', '2': '2.0', '6': '5.1', '8': '7.1' } as Record<string, string>)[channels || ''] || channels
  const audioLabel = [codec(value(audio, 'Codec')), channelLabel].filter(Boolean).join(' ')
  const bitrate = value(video, '比特率')?.replace(/(\d)(Mbps|kbps)/i, '$1 $2')
  const frameRate = Number(value(video, '帧率'))
  return [size, codec(value(video, 'Codec')), resolution, audioLabel, bitrate, frameRate > 0 ? Number(frameRate.toFixed(2)) + ' fps' : undefined].filter(Boolean).join('   ')
}
