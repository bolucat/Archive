export const engineBinMap: Record<string, string> = {
  darwin: 'aria2c',
  win32: 'aria2c.exe',
  linux: 'aria2c'
}

export const engineArchMap: Record<string, Record<string, string>> = {
  darwin: { x64: 'x64', arm64: 'arm64' },
  win32:  { ia32: 'ia32', x64: 'x64', arm64: 'x64' },
  linux:  { x64: 'x64', arm: 'armv7l', arm64: 'arm64' }
}
