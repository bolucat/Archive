export type PlayerPlatform = 'win32' | 'darwin' | 'linux' | string

export function isMpvCommand(command: string): boolean {
  return (command || '').toLowerCase().includes('mpv')
}

export function shellQuote(value: string): string {
  return `'${String(value).replaceAll("'", "'\\''")}'`
}

export function shellSplit(command: string): string[] {
  const result: string[] = []
  let current = ''
  let quote: '"' | "'" | '' = ''
  let escaped = false

  for (const char of String(command || '').trim()) {
    if (escaped) {
      current += char
      escaped = false
      continue
    }
    if (char === '\\' && quote !== "'") {
      escaped = true
      continue
    }
    if ((char === '"' || char === "'") && (!quote || quote === char)) {
      quote = quote ? '' : char
      continue
    }
    if (!quote && /\s/.test(char)) {
      if (current) {
        result.push(current)
        current = ''
      }
      continue
    }
    current += char
  }

  if (current) result.push(current)
  return result
}

export function buildPlayerCommand(platform: PlayerPlatform, command: string): string {
  if (platform === 'darwin') return `open -a ${shellQuote(command)} ${command.includes('mpv.app') ? '--args ' : ''}`
  if (platform === 'linux' && !command.includes('/') && !command.includes('\\')) return command
  return platform === 'win32' ? `"${command}"` : shellQuote(command)
}

export function buildDirectPlayerInvocation(platform: PlayerPlatform, command: string): { binary: string; args: string[] } {
  if (platform !== 'linux') return { binary: command, args: [] }
  const parts = shellSplit(command)
  return {
    binary: parts[0] || command,
    args: parts.slice(1)
  }
}

export function formatPlayerArg(platform: PlayerPlatform, value: string, directSpawn = false): string {
  if (directSpawn) return String(value)
  return platform === 'win32' ? `"${value}"` : shellQuote(value)
}

export function redactMpvArgs(args: any[]): any[] {
  return args.map((arg) => {
    const value = String(arg)
    if (value.includes('Authorization:')) return value.replace(/Authorization:\s*[^'"]+/i, 'Authorization: [REDACTED]')
    if (/access_token=|x-oss-signature=|X-Amz-Signature=/i.test(value)) return '[REDACTED_URL]'
    return arg
  })
}
