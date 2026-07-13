export type PlayerPlatform = 'win32' | 'darwin' | 'linux' | string

export function isMpvCommand(command: string): boolean {
  return (command || '').toLowerCase().includes('mpv')
}

export function shellQuote(value: string): string {
  return `'${String(value).replaceAll("'", "'\\''")}'`
}

export function buildPlayerCommand(platform: PlayerPlatform, command: string): string {
  if (platform === 'darwin') return `open -a ${shellQuote(command)} ${command.includes('mpv.app') ? '--args ' : ''}`
  if (platform === 'linux' && !command.includes('/') && !command.includes('\\')) return command
  return platform === 'win32' ? `"${command}"` : shellQuote(command)
}

export function redactMpvArgs(args: any[]): any[] {
  return args.map((arg) => {
    const value = String(arg)
    if (value.includes('Authorization:')) return value.replace(/Authorization:\s*[^'"]+/i, 'Authorization: [REDACTED]')
    if (/access_token=|x-oss-signature=|X-Amz-Signature=/i.test(value)) return '[REDACTED_URL]'
    return arg
  })
}
