export interface BuildAriaAddOptionsInput {
  gid: string
  dir: string
  split: number
  referer: string
  userAgent: string
  headers: string[]
  outFileName: string
  sourceType: string
  selectFile?: string
  allProxy?: string
}

export const buildAriaAddOptions = (input: BuildAriaAddOptionsInput): Record<string, any> => {
  const isBtSource = input.sourceType === 'magnet' || input.sourceType === 'torrent' || input.sourceType === 'torrent-url'
  const options: Record<string, any> = {
    gid: input.gid,
    dir: input.dir,
    split: input.split
  }

  if (!isBtSource) {
    options.out = input.outFileName
    if (input.referer) options.referer = input.referer
    if (input.userAgent) options['user-agent'] = input.userAgent
    if (input.headers.length) options.header = input.headers
  }

  if (input.allProxy) options['all-proxy'] = input.allProxy
  if (isBtSource && input.selectFile) options['select-file'] = input.selectFile
  if (input.sourceType === 'torrent-url') options['follow-torrent'] = 'mem'
  return options
}
