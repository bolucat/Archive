import { existsSync, readdirSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { spawnSync } from 'node:child_process'

const root = process.cwd()
const releaseDir = resolve(root, 'release')

function run(args) {
  console.info(`\n> pnpm exec electron-builder ${args.join(' ')}`)
  const result = spawnSync('pnpm', ['exec', 'electron-builder', ...args], {
    cwd: root,
    stdio: 'inherit',
    shell: false
  })

  if (result.status !== 0) {
    process.exit(result.status || 1)
  }
}

function readArEntries(file) {
  const data = readFileSync(file)
  if (data.subarray(0, 8).toString() !== '!<arch>\n') {
    throw new Error(`${file} is not an ar archive`)
  }

  const entries = []
  let offset = 8
  while (offset + 60 <= data.length) {
    const header = data.subarray(offset, offset + 60)
    const name = header.subarray(0, 16).toString().trim()
    const sizeText = header.subarray(48, 58).toString().trim()
    const end = header.subarray(58, 60).toString()
    const size = Number.parseInt(sizeText, 10)
    if (end !== '`\n' || !Number.isFinite(size)) {
      throw new Error(`${file} has an invalid ar header`)
    }
    if (name.startsWith('#1/')) {
      throw new Error(`${file} uses BSD ar extended names; Debian packages require GNU ar format`)
    }
    entries.push(name.replace(/\/$/, ''))
    offset += 60 + size + (size % 2)
  }
  return entries
}

function validateDebArtifacts() {
  if (!existsSync(releaseDir)) {
    throw new Error('release directory was not created')
  }

  const debs = readdirSync(releaseDir)
    .filter((name) => name.endsWith('.deb'))
    .map((name) => resolve(releaseDir, name))

  if (debs.length === 0) {
    throw new Error('no .deb artifacts were created')
  }

  for (const deb of debs) {
    const entries = readArEntries(deb)
    const hasDebianBinary = entries.includes('debian-binary')
    const hasControl = entries.some((entry) => entry.startsWith('control.tar'))
    const hasData = entries.some((entry) => entry.startsWith('data.tar'))
    if (!hasDebianBinary || !hasControl || !hasData) {
      throw new Error(`${deb} is not a valid Debian package: ${entries.join(', ')}`)
    }
  }
}

if (process.platform !== 'linux') {
  run(['--linux', 'AppImage', '--x64', '--arm64'])
  console.warn('\nSkipping Linux deb and pacman targets on non-Linux host.')
  console.warn('electron-builder/fpm can produce invalid .deb archives on macOS because BSD ar is not GNU ar.')
  console.warn('Run this script on Linux or use the Ubuntu release workflow to build .deb artifacts.')
  process.exit(0)
}

run(['--linux'])
validateDebArtifacts()
