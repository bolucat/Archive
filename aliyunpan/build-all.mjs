import { mkdirSync, writeFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { spawnSync } from 'node:child_process'

const root = process.cwd()
const toolDir = resolve(root, '.build-tools')

let resolvedPython = ''

function run(command, args, options = {}) {
  console.info(`\n> ${[command, ...args].join(' ')}`)
  const env = {
    ...process.env,
    CSC_IDENTITY_AUTO_DISCOVERY: 'true',
    PATH: `${toolDir}:${process.env.PATH || ''}`,
    ...options.env,
  }
  if (resolvedPython) {
    env.PYTHON_PATH = resolvedPython
  }
  const result = spawnSync(command, args, {
    cwd: root,
    stdio: 'inherit',
    shell: false,
    env,
  })

  if (result.status !== 0) {
    process.exit(result.status || 1)
  }
}

function canUsePython(command) {
  return spawnSync(command, ['-c', 'import plistlib; import xml.parsers.expat'], { stdio: 'ignore' }).status === 0
}

function resolveAbs(command) {
  const r = spawnSync('/bin/sh', ['-c', `command -v ${command}`], { encoding: 'utf8' })
  return r.status === 0 ? r.stdout.trim() : ''
}

function ensurePythonShim() {
  const candidates = [
    '/usr/bin/python3',
    process.env.PYTHON,
    'python3',
    'python',
  ].filter(Boolean)

  let python = ''
  for (const candidate of candidates) {
    if (!canUsePython(candidate)) continue
    python = candidate.startsWith('/') ? candidate : resolveAbs(candidate)
    if (python) break
  }
  if (!python) return

  resolvedPython = python
  mkdirSync(toolDir, { recursive: true })
  const script = `#!/bin/sh\nexec "${python}" "$@"\n`
  for (const name of ['python', 'python3']) {
    writeFileSync(resolve(toolDir, name), script, { mode: 0o755 })
  }
}

ensurePythonShim()

run('node', ['version.mjs'])
run('pnpm', ['exec', 'vue-tsc', '--noEmit'])
run('pnpm', ['exec', 'vite', 'build'])

run('pnpm', ['exec', 'electron-builder', '--linux', 'AppImage', '--x64', '--arm64'])
run('pnpm', ['exec', 'electron-builder', '--linux', 'deb', '--x64', '--arm64'])
run('pnpm', ['exec', 'electron-builder', '--linux', 'pacman', '--x64', '--arm64'])
run('pnpm', ['exec', 'electron-builder', '--win'])
run('pnpm', ['exec', 'electron-builder', '--mac'])
