import { readFile, writeFile } from 'node:fs/promises'
import { extname, join, resolve } from 'node:path'
import { homedir } from 'node:os'

import { createAuthStore } from './authStore.mjs'
import { loginWithBrowserOAuth, supportedBrowserLoginProviders } from './browserAuth.mjs'
import { createOperationLogStore, createUndoRenamePlan, createUndoMovePlan } from './operationLog.mjs'
import { dryRunRenamePlan } from './renamePlan.mjs'
import { validateMovePlan, dryRunMovePlan } from './movePlan.mjs'
import { validateTrashPlan, dryRunTrashPlan } from './trashPlan.mjs'
import { createUploadPlanFromLocalPath, dryRunUploadPlan, executeUploadPlan } from './uploadPlan.mjs'
import { EXIT_CODES, classifyError } from './models.mjs'
import { COMMAND_MANIFEST_VERSION, listCommands } from './commandManifest.mjs'
import { PLAN_SCHEMA_VERSION, listPlanSchemas } from './planSchemas.mjs'
import { createAliyunProvider } from '../providers/aliyun.mjs'
import { createPikpakProvider } from '../providers/pikpakProvider.mjs'
import { createDropboxProvider } from '../providers/dropboxProvider.mjs'
import { createOnedriveProvider } from '../providers/onedriveProvider.mjs'
import { createBoxProvider } from '../providers/boxProvider.mjs'
import { createBaiduProvider } from '../providers/baiduProvider.mjs'
import { createDrive115Provider } from '../providers/drive115Provider.mjs'
import { createCloud123Provider } from '../providers/cloud123Provider.mjs'
import { createQuarkProvider } from '../providers/quarkProvider.mjs'
import { createCloud139Provider } from '../providers/cloud139Provider.mjs'
import { createCloud189Provider } from '../providers/cloud189Provider.mjs'

const PROVIDERS = {
  aliyun: createAliyunProvider(),
  pikpak: createPikpakProvider(),
  dropbox: createDropboxProvider(),
  onedrive: createOnedriveProvider(),
  box: createBoxProvider(),
  baidu: createBaiduProvider(),
  '115': createDrive115Provider(),
  cloud123: createCloud123Provider(),
  quark: createQuarkProvider(),
  '139': createCloud139Provider(),
  '189': createCloud189Provider(),
}

function defaultConfigDir() {
  return join(homedir(), '.clouddrive-cli')
}

function hasFlag(argv, flag) {
  if (flag === '--json' && readFormatOption(argv) === 'json') return true
  return argv.includes(flag)
}

function readFormatOption(argv) {
  const raw = readOption(argv, '--format') || readOption(argv, '-f')
  return raw.toLowerCase()
}

function readOption(argv, flag) {
  const index = argv.indexOf(flag)
  if (index < 0) return ''
  return argv[index + 1] || ''
}

function hasOption(argv, flag) {
  return argv.includes(flag)
}

function readOptionWithDefault(argv, flag, fallback) {
  const index = argv.indexOf(flag)
  if (index < 0) return fallback
  return argv[index + 1] ?? ''
}

function readCloudFileId(argv, providerName) {
  return readOptionWithDefault(argv, '--file-id', defaultRootForProvider(providerName))
}

function rejectRemovedPath(argv) {
  if (!hasOption(argv, '--path')) return null
  return fail('--path has been removed. Use --file-id <folder-id>.')
}

function hasHelpFlag(argv) {
  return argv.includes('--help') || argv.includes('-h')
}

function defaultRootForProvider(providerName) {
  if (providerName === '115' || providerName === 'cloud123') return '0'
  if (providerName === 'quark') return 'quark_root'
  if (providerName === '139') return 'cloud139_root'
  if (providerName === '189') return 'cloud189_root'
  if (providerName === 'baidu') return '/'
  if (providerName === 'dropbox') return ''
  if (providerName === 'pikpak') return '*'
  if (providerName === 'onedrive') return 'onedrive_root'
  if (providerName === 'box') return 'box_root'
  return 'root'
}

function jsonOut(value) {
  return `${JSON.stringify(value, null, 2)}\n`
}

function ok(value, json = true) {
  if (json) return { exitCode: EXIT_CODES.SUCCESS, stdout: jsonOut(value), stderr: '' }
  // non-JSON: primitives as plain text, objects/arrays fall back to JSON
  if (typeof value === 'string' || typeof value === 'number' || typeof value === 'boolean') {
    return { exitCode: EXIT_CODES.SUCCESS, stdout: `${String(value)}\n`, stderr: '' }
  }
  return { exitCode: EXIT_CODES.SUCCESS, stdout: jsonOut(value), stderr: '' }
}

function unsupportedUndo(operation) {
  return fail(`Operation "${operation?.type || ''}" cannot be undone. Only rename and move operations are undoable.`, EXIT_CODES.UNSUPPORTED_CAPABILITY)
}

function activeProviders(env = {}) {
  return env.providers || PROVIDERS
}

function summarizeAgentOutput(value) {
  if (Array.isArray(value)) {
    return {
      itemCount: value.length,
      fileCount: value.filter((item) => item?.type === 'file').length,
      folderCount: value.filter((item) => item?.type === 'folder').length,
      totalBytes: value.reduce((sum, item) => sum + (Number(item?.size) || 0), 0),
    }
  }
  if (value && typeof value === 'object') {
    if (Array.isArray(value.items)) {
      return {
        itemCount: value.items.length,
        fileCount: value.items.filter((item) => item?.type === 'file').length,
        folderCount: value.items.filter((item) => item?.type === 'folder').length,
        hasMore: !!value.hasMore,
        nextCursor: value.nextCursor || '',
      }
    }
    const summary = {}
    for (const key of ['total_files', 'total_dirs', 'total_size', 'max_depth', 'path', 'provider', 'driveId']) {
      if (value[key] !== undefined) summary[key] = value[key]
    }
    if (Object.keys(summary).length > 0) return summary
  }
  return { valueType: Array.isArray(value) ? 'array' : typeof value }
}

async function outputOrOk(value, argv, json = true) {
  const outputPath = readOption(argv, '--output')
  if (!outputPath) return ok(value, json)
  await writeFile(outputPath, jsonOut(value), 'utf8')
  return ok({ ok: true, output: outputPath, summary: summarizeAgentOutput(value) }, true)
}

function usage(message) {
  return { exitCode: EXIT_CODES.SUCCESS, stdout: `${message}\n`, stderr: '' }
}

function requireApplyRationale(argv) {
  const rationale = readOption(argv, '--rationale')
  if (!rationale || !rationale.trim()) {
    return { error: '执行 apply 写操作必须提供 --rationale <reason>' }
  }
  return { rationale }
}

function fail(message, exitCode = EXIT_CODES.VALIDATION_ERROR) {
  return { exitCode, stdout: '', stderr: `${message}\n` }
}

function errorCodeForExit(exitCode) {
  if (exitCode === EXIT_CODES.AUTH_ERROR) return 'AUTH_ERROR'
  if (exitCode === EXIT_CODES.PROVIDER_API_ERROR) return 'PROVIDER_API_ERROR'
  if (exitCode === EXIT_CODES.PARTIAL_SUCCESS) return 'PARTIAL_SUCCESS'
  if (exitCode === EXIT_CODES.UNSUPPORTED_CAPABILITY) return 'UNSUPPORTED_CAPABILITY'
  return 'VALIDATION_ERROR'
}

function errorEnvelope(message, exitCode) {
  return {
    ok: false,
    error: {
      code: errorCodeForExit(exitCode),
      message,
      exitCode,
    },
  }
}

function maybeJsonError(result, argv) {
  if (!hasFlag(argv, '--json') || result.exitCode === EXIT_CODES.SUCCESS || result.stdout) return result
  const message = (result.stderr || '').trim() || 'Command failed'
  return {
    exitCode: result.exitCode,
    stdout: jsonOut(errorEnvelope(message, result.exitCode)),
    stderr: '',
  }
}

async function readJsonFile(path) {
  if (!path) throw new Error('Missing JSON file path')
  return JSON.parse(await readFile(path, 'utf8'))
}

function readPositiveIntegerOption(argv, flag, fallback) {
  const raw = readOption(argv, flag)
  if (!raw) return fallback
  const value = Number.parseInt(raw, 10)
  return Number.isFinite(value) && value > 0 ? value : fallback
}

function providerCapabilities() {
  return Object.values(PROVIDERS).map((provider) => ({
    id: provider.id,
    displayName: provider.displayName,
    capabilities: provider.capabilities,
  }))
}

function authHelp() {
  return [
    'clouddrive-cli auth <command>',
    '',
    'Commands:',
    '  clouddrive-cli auth list [--json]',
    '  clouddrive-cli auth default <provider> <account-id>',
    '  clouddrive-cli auth import-token --provider <p> --account <id> --token <token.json> [--name <display>] [--default] [--json]',
    '  clouddrive-cli auth login <provider> [--browser chrome] [--redirect-uri <uri>] [--port <n>] [--timeout-ms <n>] [--json]',
    '',
    `Supported login providers: ${supportedBrowserLoginProviders().join(', ')}`,
  ].join('\n') + '\n'
}

function authCommandUsage(subcommand) {
  const usages = {
    list: 'Usage: clouddrive-cli auth list [--json]',
    default: 'Usage: clouddrive-cli auth default <provider> <account-id> [--json]',
    'import-token': 'Usage: clouddrive-cli auth import-token --provider <p> --account <id> --token <token.json> [--name <display>] [--default] [--json]',
    login: `Usage: clouddrive-cli auth login <provider> [--browser chrome] [--redirect-uri <uri>] [--port <n>] [--timeout-ms <n>] [--json]\nSupported login providers: ${supportedBrowserLoginProviders().join(', ')}`,
    check: 'Usage: clouddrive-cli auth check [--provider <p>] [--json]',
  }
  return usages[subcommand] || authHelp().trimEnd()
}

async function handleAuth(argv, env) {
  const store = createAuthStore({ configDir: env.configDir })
  const subcommand = argv[1]
  if (!subcommand || subcommand === 'help' || subcommand === '--help' || subcommand === '-h') {
    return { exitCode: EXIT_CODES.SUCCESS, stdout: authHelp(), stderr: '' }
  }
  if (hasHelpFlag(argv.slice(2))) return usage(authCommandUsage(subcommand))
  if (subcommand === 'list') {
    const accounts = await store.listAccounts()
    if (hasFlag(argv, '--json')) return ok(accounts, true)
    if (accounts.length === 0) {
      return { exitCode: EXIT_CODES.SUCCESS, stdout: '暂无已保存账号\n', stderr: '' }
    }
    const providerWidth = Math.max('Provider'.length, ...accounts.map((a) => String(a.provider || '').length))
    const accountWidth = Math.max('AccountId'.length, ...accounts.map((a) => String(a.accountId || '').length))
    const displayWidth = Math.max('DisplayName'.length, ...accounts.map((a) => String(a.displayName || '').length))
    const lines = [
      `${'Provider'.padEnd(providerWidth)}  ${'AccountId'.padEnd(accountWidth)}  ${'DisplayName'.padEnd(displayWidth)}  Default`,
    ]
    lines.push(`${'─'.repeat(providerWidth)}  ${'─'.repeat(accountWidth)}  ${'─'.repeat(displayWidth)}  ───────`)
    for (const a of accounts) {
      const p = (a.provider || '').padEnd(providerWidth)
      const id = (a.accountId || '').padEnd(accountWidth)
      const name = (a.displayName || '').padEnd(displayWidth)
      const def = a.isDefault ? '  ✓' : ''
      lines.push(`${p}  ${id}  ${name}  ${def}`)
    }
    return { exitCode: EXIT_CODES.SUCCESS, stdout: lines.join('\n') + '\n', stderr: '' }
  }
  if (subcommand === 'default') {
    const provider = argv[2]
    const accountId = argv[3]
    if (!provider || !accountId) return fail('Usage: clouddrive-cli auth default <provider> <account-id>')
    await store.setDefaultAccount(provider, accountId)
    const accounts = await store.listAccounts()
    return ok(accounts.find((a) => a.provider === provider && a.accountId === accountId), hasFlag(argv, '--json'))
  }
  if (subcommand === 'import-token') {
    const provider = readOption(argv, '--provider')
    const accountId = readOption(argv, '--account')
    const displayName = readOption(argv, '--name') || accountId
    const tokenPath = readOption(argv, '--token')
    if (!provider || !accountId || !tokenPath) return fail('Usage: clouddrive-cli auth import-token --provider <p> --account <id> --token <token.json> [--name <display>] [--default] [--json]')
    const token = await readJsonFile(tokenPath)
    await store.saveAccount({ provider, accountId, displayName, token })
    if (hasFlag(argv, '--default')) await store.setDefaultAccount(provider, accountId)
    const accounts = await store.listAccounts()
    return ok(accounts.find((a) => a.provider === provider && a.accountId === accountId), hasFlag(argv, '--json'))
  }
  if (subcommand === 'login') {
    const provider = argv[2]
    if (!provider) return fail(`Usage: clouddrive-cli auth login <provider> [--browser chrome] [--redirect-uri <uri>] [--port <n>] [--timeout-ms <n>] [--json]\nSupported login providers: ${supportedBrowserLoginProviders().join(', ')}`)
    const result = await loginWithBrowserOAuth({
      provider,
      configDir: env.configDir,
      browser: readOption(argv, '--browser'),
      redirectUri: readOption(argv, '--redirect-uri'),
      port: readPositiveIntegerOption(argv, '--port', 0),
      timeoutMs: readPositiveIntegerOption(argv, '--timeout-ms', 120000),
    })
    return ok(result, hasFlag(argv, '--json'))
  }
  if (subcommand === 'check') {
    const providerFilter = readOption(argv, '--provider')
    const isJson = hasFlag(argv, '--json')
    const accounts = await store.listAccounts()
    const filtered = providerFilter ? accounts.filter((a) => a.provider === providerFilter) : accounts
    const results = await Promise.all(filtered.map(async (account) => {
      const full = await store.getAccount(account.provider, account.accountId)
      const token = full?.token
      const provider = PROVIDERS[account.provider]
      if (!token || !provider) return { provider: account.provider, accountId: account.accountId, displayName: account.displayName, status: 'unknown' }
      const expireTime = token.expire_time ? new Date(token.expire_time).getTime() : 0
      const bufferMs = 5 * 60 * 1000
      if (expireTime && expireTime - Date.now() < bufferMs) {
        try {
          const refreshed = await provider.auth.refresh(token)
          await store.saveAccount({ provider: account.provider, accountId: account.accountId, displayName: account.displayName, token: refreshed })
          return { provider: account.provider, accountId: account.accountId, displayName: account.displayName, status: 'expired_refreshed', refreshed: true }
        } catch (e) {
          return { provider: account.provider, accountId: account.accountId, displayName: account.displayName, status: 'expired_unrefreshable', error: e.message }
        }
      }
      return { provider: account.provider, accountId: account.accountId, displayName: account.displayName, status: 'valid' }
    }))
    if (isJson) return ok(results, true)
    const lines = results.map((r) => `${r.provider.padEnd(10)} ${r.accountId.padEnd(36)} ${r.status}${r.error ? ' — ' + r.error : ''}`)
    return { exitCode: EXIT_CODES.SUCCESS, stdout: lines.join('\n') + '\n', stderr: '' }
  }

  return fail(`Unknown auth command: ${subcommand || ''}`.trim())
}

async function resolveToken(env, providerName, accountArg) {
  const store = createAuthStore({ configDir: env.configDir })
  const account = accountArg && accountArg !== 'default'
    ? await store.getAccount(providerName, accountArg)
    : await store.getDefaultAccount(providerName)
  if (!account) {
    const err = new Error(`No account found for provider "${providerName}". Run: clouddrive-cli auth default ${providerName} <account-id>`)
    err.code = 'ERR_NO_ACCOUNT'
    throw err
  }
  const token = account.token || account
  const expireTime = token.expire_time ? new Date(token.expire_time).getTime() : 0
  const bufferMs = 5 * 60 * 1000
  const provider = activeProviders(env)[providerName]
  if (provider?.auth?.refresh && token.refresh_token && expireTime && expireTime - Date.now() < bufferMs) {
    const refreshed = await provider.auth.refresh(token)
    const nextAccount = { ...account, token: { ...token, ...refreshed } }
    await store.saveAccount(nextAccount)
    return nextAccount.token
  }
  return token
}

const VIDEO_EXTS = new Set(['.mp4', '.mkv', '.avi', '.mov', '.m4v', '.wmv', '.flv', '.ts', '.m2ts', '.rmvb', '.webm', '.vob', '.iso'])
const SUBTITLE_EXTS = new Set(['.srt', '.ass', '.ssa', '.sub', '.vtt', '.idx'])
const IMAGE_EXTS = new Set(['.jpg', '.jpeg', '.png', '.gif', '.webp', '.bmp', '.heic', '.tiff'])
const AUDIO_EXTS = new Set(['.mp3', '.flac', '.aac', '.wav', '.ogg', '.m4a', '.wma', '.opus'])
const ARCHIVE_EXTS = new Set(['.zip', '.rar', '.7z', '.tar', '.gz', '.bz2', '.xz'])

function fileCategory(name) {
  const ext = extname(name).toLowerCase()
  if (VIDEO_EXTS.has(ext)) return 'video'
  if (SUBTITLE_EXTS.has(ext)) return 'subtitle'
  if (IMAGE_EXTS.has(ext)) return 'image'
  if (AUDIO_EXTS.has(ext)) return 'audio'
  if (ARCHIVE_EXTS.has(ext)) return 'archive'
  return 'other'
}

function buildTree(rootId, allItems) {
  const byParent = new Map()
  for (const item of allItems) {
    const pid = item.parentFileId || ''
    if (!byParent.has(pid)) byParent.set(pid, [])
    byParent.get(pid).push(item)
  }

  function makeNode(item) {
    const node = {
      fileId: item.fileId,
      name: item.name,
      type: item.type,
    }
    if (item.type === 'file') {
      if (item.size != null) node.size = item.size
      if (item.mimeType) node.mimeType = item.mimeType
    } else {
      const children = (byParent.get(item.fileId) || []).map(makeNode)
      node.children = children
      let totalFiles = 0; let totalFolders = 0; let totalSize = 0
      for (const c of children) {
        if (c.type === 'file') { totalFiles++; totalSize += c.size || 0 }
        else { totalFolders++; totalFiles += c.totalFiles || 0; totalFolders += c.totalFolders || 0; totalSize += c.totalSize || 0 }
      }
      node.totalFiles = totalFiles
      node.totalFolders = totalFolders
      node.totalSize = totalSize
    }
    return node
  }

  const rootLookupKey = rootId === '' ? (byParent.has('/') ? '/' : '') : rootId
  const rootChildren = byParent.get(rootLookupKey) || []
  const rootDisplayName = rootId === 'root' ? 'My Drive' : rootId === '' || rootId === '/' || rootId === '0' || rootId === '*' ? '/' : rootId
  return {
    fileId: rootId,
    name: rootDisplayName,
    type: 'folder',
    children: rootChildren.map(makeNode),
    totalFiles: allItems.filter((i) => i.type === 'file').length,
    totalFolders: allItems.filter((i) => i.type === 'folder').length,
    totalSize: allItems.reduce((s, i) => s + (i.type === 'file' ? (i.size || 0) : 0), 0),
  }
}

function renderTreeText(node, prefix = '', isLast = true) {
  const connector = isLast ? '└─ ' : '├─ '
  const line = `${prefix}${connector}${node.name}${node.type === 'folder' ? '/' : ''}`
  const lines = [line]
  if (node.children) {
    const childPrefix = prefix + (isLast ? '   ' : '│  ')
    node.children.forEach((child, i) => {
      lines.push(...renderTreeText(child, childPrefix, i === node.children.length - 1))
    })
  }
  return lines
}

function buildStats(allItems) {
  const stats = { total_files: 0, total_dirs: 0, total_size: 0, by_category: {}, top_extensions: {} }
  for (const item of allItems) {
    if (item.type === 'folder') { stats.total_dirs++; continue }
    stats.total_files++
    const size = item.size || 0
    stats.total_size += size
    const cat = fileCategory(item.name)
    if (!stats.by_category[cat]) stats.by_category[cat] = { count: 0, size: 0 }
    stats.by_category[cat].count++
    stats.by_category[cat].size += size
    const ext = extname(item.name).toLowerCase() || '(no ext)'
    stats.top_extensions[ext] = (stats.top_extensions[ext] || 0) + 1
  }
  stats.top_extensions = Object.fromEntries(
    Object.entries(stats.top_extensions).sort(([, a], [, b]) => b - a).slice(0, 20)
  )
  return stats
}

function filesCommandUsage(subcommand) {
  const usages = {
    list: 'Usage: clouddrive-cli files list [--provider <p>] [--account <id>] [--drive-id <d>] [--file-id <id>] [--limit <n>] [--cursor <token>] [--json]',
    walk: 'Usage: clouddrive-cli files walk [--provider <p>] [--account <id>] [--drive-id <d>] [--file-id <id>] [--json]',
    tree: 'Usage: clouddrive-cli files tree [--provider <p>] [--account <id>] [--drive-id <d>] [--file-id <id>] [--depth <n>] [--json]',
    stats: 'Usage: clouddrive-cli files stats [--provider <p>] [--account <id>] [--drive-id <d>] [--file-id <id>] [--depth <n>] [--json]',
    info: 'Usage: clouddrive-cli files info --file-id <id> [--provider <p>] [--account <id>] [--drive-id <d>] [--json]',
    download: 'Usage: clouddrive-cli files download --file-id <id> --output <path> [--provider <p>] [--account <id>] [--drive-id <d>] [--json]',
    search: 'Usage: clouddrive-cli files search --name <filename> [--provider <p>] [--account <id>] [--limit <n>] [--json]',
    mkdir: 'Usage: clouddrive-cli files mkdir --name <name> [--parent <id>] [--provider <p>] [--account <id>] [--drive-id <d>] [--json]',
    'rename-apply': 'Usage: clouddrive-cli files rename-apply <plan.json> [--current current.json] [--dry-run] [--json]',
    'move-apply': 'Usage: clouddrive-cli files move-apply <plan.json> [--dry-run] [--json]',
    'trash-apply': 'Usage: clouddrive-cli files trash-apply <plan.json> [--apply] [--json]',
  }
  return usages[subcommand] || 'Usage: clouddrive-cli files <list|walk|tree|stats|info|download|search|mkdir|rename-apply|move-apply|trash-apply> [options]'
}

function opsCommandUsage(subcommand) {
  const usages = {
    list: 'Usage: clouddrive-cli ops list [--json]',
    show: 'Usage: clouddrive-cli ops show <operation-id> [--json]',
    undo: 'Usage: clouddrive-cli ops undo <operation-id> [--dry-run] [--json]',
  }
  return usages[subcommand] || 'Usage: clouddrive-cli ops <list|show|undo> [options]'
}

function uploadCommandUsage(subcommand) {
  const usages = {
    plan: 'Usage: clouddrive-cli upload plan --local <path> [--provider <p>] [--account <id>] [--remote-parent <id>] [--output <plan.json>] [--json]',
    apply: 'Usage: clouddrive-cli upload apply <plan.json> [--dry-run] [--json]',
  }
  return usages[subcommand] || 'Usage: clouddrive-cli upload <plan|apply> [options]'
}

function fallbackListPage(items, { limit, cursor }) {
  const offset = cursor ? Number.parseInt(cursor, 10) : 0
  const start = Number.isFinite(offset) && offset >= 0 ? offset : 0
  const pageItems = items.slice(start, start + limit)
  const nextOffset = start + pageItems.length
  const nextCursor = nextOffset < items.length ? String(nextOffset) : ''
  return { items: pageItems, nextCursor }
}

function listPageEnvelope({ providerName, driveId, parentFileId, limit, cursor, page }) {
  const nextCursor = page.nextCursor || ''
  return {
    provider: providerName,
    driveId,
    parentFileId,
    limit,
    cursor: cursor || '',
    nextCursor,
    hasMore: !!nextCursor,
    items: page.items || [],
  }
}

async function handleFiles(argv, env) {
  const subcommand = argv[1]
  if (!subcommand || subcommand === 'help' || subcommand === '--help' || subcommand === '-h') {
    return usage(filesCommandUsage())
  }
  if (hasHelpFlag(argv.slice(2))) return usage(filesCommandUsage(subcommand))

  if (subcommand === 'list' || subcommand === 'walk') {
    const removedPath = rejectRemovedPath(argv)
    if (removedPath) return removedPath
    const providerName = readOption(argv, '--provider') || 'aliyun'
    const accountArg = readOption(argv, '--account') || 'default'
    const fileId = readCloudFileId(argv, providerName)
    const driveId = readOption(argv, '--drive-id') || ''
    const isJson = hasFlag(argv, '--json')
    const usePagination = hasOption(argv, '--limit') || hasOption(argv, '--cursor')
    const limit = readPositiveIntegerOption(argv, '--limit', 100)
    const cursor = readOption(argv, '--cursor')

    const provider = activeProviders(env)[providerName]
    if (!provider) return fail(`Unknown provider: ${providerName}`, EXIT_CODES.UNSUPPORTED_CAPABILITY)

    const token = await resolveToken(env, providerName, accountArg)
    const effectiveDriveId = driveId || token.default_drive_id || token.backup_drive_id

    if (subcommand === 'list') {
      if (usePagination) {
        const page = provider.files.listPage
          ? await provider.files.listPage({ token, driveId: effectiveDriveId, parentFileId: fileId, limit, cursor })
          : fallbackListPage(await provider.files.list({ token, driveId: effectiveDriveId, parentFileId: fileId }), { limit, cursor })
        return outputOrOk(listPageEnvelope({ providerName, driveId: effectiveDriveId, parentFileId: fileId, limit, cursor, page }), argv, true)
      }
      const items = await provider.files.list({ token, driveId: effectiveDriveId, parentFileId: fileId })
      return outputOrOk(items, argv, isJson)
    }

    const allItems = []
    for await (const item of provider.files.walk({ token, driveId: effectiveDriveId, parentFileId: fileId })) {
      allItems.push(item)
    }
    return outputOrOk(allItems, argv, isJson)
  }

  if (subcommand === 'search') {
    const providerName = readOption(argv, '--provider') || 'aliyun'
    const accountArg = readOption(argv, '--account') || 'default'
    const name = readOption(argv, '--name')
    const query = readOption(argv, '--query') || (name ? (providerName === 'aliyun' ? `name match "${name}"` : `name = "${name}"`) : '')
    const limit = readPositiveIntegerOption(argv, '--limit', 100)
    const isJson = hasFlag(argv, '--json')

    if (!query) return fail('Usage: clouddrive-cli files search --name <filename> [--provider <p>] [--account <id>] [--limit <n>] [--json]')

    const provider = activeProviders(env)[providerName]
    if (!provider) return fail(`Unknown provider: ${providerName}`, EXIT_CODES.UNSUPPORTED_CAPABILITY)
    if (!provider.files.search) return fail(`Provider "${providerName}" does not support server-side search`, EXIT_CODES.UNSUPPORTED_CAPABILITY)

    const token = await resolveToken(env, providerName, accountArg)
    const driveId = readOption(argv, '--drive-id') || token.default_drive_id || token.backup_drive_id
    const items = await provider.files.search({ token, driveId, name, query, limit })
    return outputOrOk(items, argv, isJson)
  }

  if (subcommand === 'tree') {
    const removedPath = rejectRemovedPath(argv)
    if (removedPath) return removedPath
    const providerName = readOption(argv, '--provider') || 'aliyun'
    const accountArg = readOption(argv, '--account') || 'default'
    const path = readCloudFileId(argv, providerName)
    const depth = readPositiveIntegerOption(argv, '--depth', 3)
    const isJson = hasFlag(argv, '--json')

    const provider = activeProviders(env)[providerName]
    if (!provider) return fail(`Unknown provider: ${providerName}`, EXIT_CODES.UNSUPPORTED_CAPABILITY)

    const token = await resolveToken(env, providerName, accountArg)
    const driveId = readOption(argv, '--drive-id') || token.default_drive_id || token.backup_drive_id
    const effectivePath = path

    const allItems = []
    for await (const item of provider.files.walk({ token, driveId, parentFileId: effectivePath, maxDepth: depth })) {
      allItems.push(item)
    }

    const node = buildTree(effectivePath, allItems)
    if (isJson || readOption(argv, '--output')) return outputOrOk({ provider: providerName, driveId, rootId: effectivePath, depth, node }, argv, true)

    const rootLabel = node.name === '/' ? '/' : `${node.name}/`
    const lines = [`${rootLabel}  (${node.totalFiles} files, ${node.totalFolders} folders)`]
    node.children.forEach((child, i) => {
      lines.push(...renderTreeText(child, '', i === node.children.length - 1))
    })
    return { exitCode: EXIT_CODES.SUCCESS, stdout: lines.join('\n') + '\n', stderr: '' }
  }

  if (subcommand === 'stats') {
    const removedPath = rejectRemovedPath(argv)
    if (removedPath) return removedPath
    const providerName = readOption(argv, '--provider') || 'aliyun'
    const accountArg = readOption(argv, '--account') || 'default'
    const path = readCloudFileId(argv, providerName)
    const depth = readPositiveIntegerOption(argv, '--depth', 10)
    const isJson = hasFlag(argv, '--json')

    const provider = activeProviders(env)[providerName]
    if (!provider) return fail(`Unknown provider: ${providerName}`, EXIT_CODES.UNSUPPORTED_CAPABILITY)

    const token = await resolveToken(env, providerName, accountArg)
    const driveId = readOption(argv, '--drive-id') || token.default_drive_id || token.backup_drive_id

    const allItems = []
    for await (const item of provider.files.walk({ token, driveId, parentFileId: path, maxDepth: depth })) {
      allItems.push(item)
    }

    const stats = { provider: providerName, driveId, path, max_depth: depth, ...buildStats(allItems) }
    if (isJson || readOption(argv, '--output')) return outputOrOk(stats, argv, true)

    const lines = [
      `Provider: ${stats.provider}  Path: ${stats.path}`,
      `Files: ${stats.total_files}  Dirs: ${stats.total_dirs}  Total size: ${(stats.total_size / 1e9).toFixed(2)} GB`,
      'By category:',
    ]
    for (const [cat, info] of Object.entries(stats.by_category)) {
      lines.push(`  ${cat.padEnd(10)}: ${info.count} files, ${(info.size / 1e6).toFixed(1)} MB`)
    }
    lines.push('Top extensions:', Object.entries(stats.top_extensions).slice(0, 10).map(([ext, n]) => `  ${ext}: ${n}`).join('\n'))
    return { exitCode: EXIT_CODES.SUCCESS, stdout: lines.join('\n') + '\n', stderr: '' }
  }

  if (subcommand === 'info') {
    const providerName = readOption(argv, '--provider') || 'aliyun'
    const accountArg = readOption(argv, '--account') || 'default'
    const fileId = readOption(argv, '--file-id')
    const isJson = hasFlag(argv, '--json')

    if (!fileId) return fail('Usage: clouddrive-cli files info --file-id <id> [--provider <p>] [--drive-id <d>] [--json]')

    const provider = PROVIDERS[providerName]
    if (!provider) return fail(`Unknown provider: ${providerName}`, EXIT_CODES.UNSUPPORTED_CAPABILITY)

    const token = await resolveToken(env, providerName, accountArg)
    const driveId = readOption(argv, '--drive-id') || token.default_drive_id || token.backup_drive_id
    const item = await provider.files.get({ token, driveId, fileId })
    return ok(item, isJson)
  }

  if (subcommand === 'download') {
    const providerName = readOption(argv, '--provider') || 'aliyun'
    const accountArg = readOption(argv, '--account') || 'default'
    const fileId = readOption(argv, '--file-id')
    const outputPath = readOption(argv, '--output')
    const isJson = hasFlag(argv, '--json')

    if (!fileId || !outputPath) return fail('Usage: clouddrive-cli files download --file-id <id> --output <path> [--provider <p>] [--account <id>] [--drive-id <d>] [--json]')

    const provider = activeProviders(env)[providerName]
    if (!provider) return fail(`Unknown provider: ${providerName}`, EXIT_CODES.UNSUPPORTED_CAPABILITY)
    if (!provider.capabilities.downloadFile || !provider.files.downloadFile) return fail(`Provider "${providerName}" does not support CLI download yet`, EXIT_CODES.UNSUPPORTED_CAPABILITY)

    const token = await resolveToken(env, providerName, accountArg)
    const driveId = readOption(argv, '--drive-id') || token.default_drive_id || token.backup_drive_id
    const result = await provider.files.downloadFile({ token, driveId, fileId, outputPath: resolve(outputPath) })
    return ok(result, isJson)
  }

  if (subcommand === 'mkdir') {
    const providerName = readOption(argv, '--provider') || 'aliyun'
    const accountArg = readOption(argv, '--account') || 'default'
    const parentId = readOption(argv, '--parent') || readOption(argv, '--parent-id')
    const parentPath = readOption(argv, '--parent-path')
    const name = readOption(argv, '--name')
    const isJson = hasFlag(argv, '--json')

    if (!name) return fail('Usage: clouddrive-cli files mkdir --name <name> [--parent <id>] [--provider <p>] [--json]')

    const provider = PROVIDERS[providerName]
    if (!provider) return fail(`Unknown provider: ${providerName}`, EXIT_CODES.UNSUPPORTED_CAPABILITY)
    if (!provider.capabilities.mkdir) return fail(`Provider "${providerName}" does not support mkdir`, EXIT_CODES.UNSUPPORTED_CAPABILITY)

    const token = await resolveToken(env, providerName, accountArg)
    const driveId = readOption(argv, '--drive-id') || token.default_drive_id || token.backup_drive_id

    const effectiveParent = parentPath || parentId || defaultRootForProvider(providerName)
    const folder = await provider.files.mkdir({ token, driveId, parentId: effectiveParent, parentPath: effectiveParent, name })
    return ok(folder, isJson)
  }

  if (subcommand === 'move-apply') {
    const planPath = argv[2]
    if (planPath === '--help' || planPath === '-h') return usage('Usage: clouddrive-cli files move-apply <plan.json> [--dry-run] [--json]')
    if (!planPath) return fail('Usage: clouddrive-cli files move-apply <plan.json> [--dry-run] [--json]')
    const plan = await readJsonFile(planPath)

    if (hasFlag(argv, '--dry-run')) {
      const result = dryRunMovePlan(plan)
      return ok(result, hasFlag(argv, '--json'))
    }
    const applyRationale = requireApplyRationale(argv)
    if (applyRationale.error) return fail(applyRationale.error)

    const validation = validateMovePlan(plan)
    if (!validation.ok) return ok({ ok: false, errors: validation.errors, applied: [] }, hasFlag(argv, '--json'))

    const providerName = plan.provider
    const provider = PROVIDERS[providerName]
    if (!provider) return fail(`Unknown provider: ${providerName}`, EXIT_CODES.UNSUPPORTED_CAPABILITY)
    if (!provider.capabilities.move) return fail(`Provider "${providerName}" does not support move`, EXIT_CODES.UNSUPPORTED_CAPABILITY)

    const token = await resolveToken(env, providerName, plan.account_id)
    const driveId = plan.items[0]?.drive_id || token.default_drive_id

    const moves = plan.items.map((item) => ({
      fileId: item.file_id,
      name: item.name,
      type: item.type || 'file',
      toParentId: item.to_parent_file_id,
      fromPath: item.from_path,
      toFolderPath: item.to_folder_path,
      itemType: item.type || 'file',
    }))
    const batchResults = await provider.files.moveBatch({ token, driveId, moves })

    const succeeded = batchResults.filter((r) => r.status === 'success').length
    const failed = batchResults.filter((r) => r.status !== 'success').length

    const operationLog = {
      id: `op_${Date.now()}`,
      type: 'move',
      provider: providerName,
      account_id: plan.account_id,
      rationale: applyRationale.rationale,
      started_at: new Date().toISOString(),
      finished_at: new Date().toISOString(),
      items: batchResults.map((r, i) => ({
        drive_id: driveId,
        file_id: r.fileId,
        name: plan.items[i]?.name || '',
        type: plan.items[i]?.type || 'file',
        from_parent_file_id: plan.items[i]?.from_parent_file_id || '',
        to_parent_file_id: plan.items[i]?.to_parent_file_id || '',
        from_folder_path: plan.items[i]?.from_path ? plan.items[i].from_path.split('/').slice(0, -1).join('/') : '',
        to_path: plan.items[i]?.to_folder_path ? `${plan.items[i].to_folder_path}/${plan.items[i].name}` : '',
        status: r.status,
        error: r.status !== 'success' ? { code: r.code, message: r.message } : undefined,
      })),
    }

    const logStore = createOperationLogStore({ configDir: env.configDir })
    await logStore.save(operationLog)

    const exitCode = failed === 0 ? EXIT_CODES.SUCCESS : succeeded > 0 ? EXIT_CODES.PARTIAL_SUCCESS : EXIT_CODES.PROVIDER_API_ERROR
    const payload = { ok: failed === 0, operationId: operationLog.id, rationale: applyRationale.rationale, succeeded, failed, results: batchResults }
    return { exitCode, stdout: jsonOut(payload), stderr: '' }
  }

  if (subcommand === 'trash-apply') {
    const planPath = argv[2]
    if (!planPath) return fail('Usage: clouddrive-cli files trash-apply <plan.json> [--apply] [--json]')
    const plan = await readJsonFile(planPath)

    const isDryRun = !hasFlag(argv, '--apply')
    const validation = validateTrashPlan(plan)

    if (!validation.ok || isDryRun) {
      const result = dryRunTrashPlan(plan)
      if (isDryRun) {
        const msg = validation.ok
          ? `Dry run: ${result.items.length} item(s) would be trashed. Pass --apply to execute.`
          : 'Plan validation failed'
        return ok({ ok: validation.ok, dry_run: true, items: result.items, errors: result.errors, message: msg }, hasFlag(argv, '--json'))
      }
      return ok({ ok: false, errors: result.errors, applied: [] }, hasFlag(argv, '--json'))
    }
    const applyRationale = requireApplyRationale(argv)
    if (applyRationale.error) return fail(applyRationale.error)

    const providerName = plan.provider
    const provider = PROVIDERS[providerName]
    if (!provider) return fail(`Unknown provider: ${providerName}`, EXIT_CODES.UNSUPPORTED_CAPABILITY)
    if (!provider.capabilities.trash && !provider.capabilities.permanentDelete) {
      return fail(`Provider "${providerName}" does not support trash or delete`, EXIT_CODES.UNSUPPORTED_CAPABILITY)
    }

    const token = await resolveToken(env, providerName, plan.account_id)
    const driveId = plan.items[0]?.drive_id || token.default_drive_id

    const items = plan.items.map((item) => ({
      fileId: item.file_id,
      type: item.type || 'file',
      path: item.path || '',
      itemType: item.type || 'file',
    }))
    const batchResults = await provider.files.trash({ token, driveId, items })

    const succeeded = batchResults.filter((r) => r.status === 'success').length
    const failed = batchResults.filter((r) => r.status !== 'success').length

    const operationLog = {
      id: `op_${Date.now()}`,
      type: 'trash',
      provider: providerName,
      account_id: plan.account_id,
      rationale: applyRationale.rationale,
      started_at: new Date().toISOString(),
      finished_at: new Date().toISOString(),
      items: batchResults.map((r, i) => ({
        drive_id: driveId,
        file_id: r.fileId,
        name: plan.items[i]?.name || '',
        type: plan.items[i]?.type || 'file',
        parent_file_id: plan.items[i]?.parent_file_id || '',
        status: r.status,
        error: r.status !== 'success' ? { code: r.code, message: r.message } : undefined,
      })),
    }

    const logStore = createOperationLogStore({ configDir: env.configDir })
    await logStore.save(operationLog)

    const exitCode = failed === 0 ? EXIT_CODES.SUCCESS : succeeded > 0 ? EXIT_CODES.PARTIAL_SUCCESS : EXIT_CODES.PROVIDER_API_ERROR
    const payload = { ok: failed === 0, operationId: operationLog.id, rationale: applyRationale.rationale, succeeded, failed, results: batchResults }
    return { exitCode, stdout: jsonOut(payload), stderr: '' }
  }

  if (subcommand === 'rename-apply') {
    const planPath = argv[2]
    if (!planPath) return fail('Usage: clouddrive-cli files rename-apply <plan.json> [--current current.json] [--dry-run]')
    const plan = await readJsonFile(planPath)
    const currentPath = readOption(argv, '--current')
    const currentItems = currentPath ? await readJsonFile(currentPath) : []

    if (hasFlag(argv, '--dry-run')) {
      const result = dryRunRenamePlan(plan, currentItems)
      return ok(result, hasFlag(argv, '--json'))
    }
    const applyRationale = requireApplyRationale(argv)
    if (applyRationale.error) return fail(applyRationale.error)

    const providerName = plan.provider
    const provider = PROVIDERS[providerName]
    if (!provider) return fail(`Unknown provider: ${providerName}`, EXIT_CODES.UNSUPPORTED_CAPABILITY)
    if (!provider.capabilities.batchRename) return fail(`Provider "${providerName}" does not support batch rename`, EXIT_CODES.UNSUPPORTED_CAPABILITY)

    const token = await resolveToken(env, providerName, plan.account_id)
    const driveId = plan.items[0]?.drive_id || token.default_drive_id

    const validation = dryRunRenamePlan(plan, currentItems)
    if (!validation.ok) {
      return ok({ ok: false, errors: validation.errors, applied: [] }, hasFlag(argv, '--json'))
    }

    const renames = plan.items.map((item) => ({ fileId: item.file_id, newName: item.new_name }))
    const batchResults = await provider.files.renameBatch({ token, driveId, renames })

    const succeeded = batchResults.filter((r) => r.status === 'success').length
    const failed = batchResults.filter((r) => r.status !== 'success').length

    const operationLog = {
      id: `op_${Date.now()}`,
      type: 'rename',
      provider: providerName,
      account_id: plan.account_id,
      rationale: applyRationale.rationale,
      started_at: new Date().toISOString(),
      finished_at: new Date().toISOString(),
      items: batchResults.map((r, i) => ({
        drive_id: driveId,
        file_id: r.fileId,
        parent_file_id: plan.items[i]?.parent_file_id || '',
        before_name: plan.items[i]?.old_name || '',
        after_name: r.status === 'success' ? r.newName : plan.items[i]?.new_name,
        status: r.status,
        error: r.status !== 'success' ? { code: r.code, message: r.message } : undefined,
      })),
    }

    const logStore = createOperationLogStore({ configDir: env.configDir })
    await logStore.save(operationLog)

    const exitCode = failed === 0 ? EXIT_CODES.SUCCESS : succeeded > 0 ? EXIT_CODES.PARTIAL_SUCCESS : EXIT_CODES.PROVIDER_API_ERROR
    const payload = { ok: failed === 0, operationId: operationLog.id, rationale: applyRationale.rationale, succeeded, failed, results: batchResults }
    return { exitCode, stdout: jsonOut(payload), stderr: '' }
  }

  return fail(`Unknown files command: ${subcommand || ''}`.trim())
}

async function handleOps(argv, env) {
  const store = createOperationLogStore({ configDir: env.configDir })
  const subcommand = argv[1]
  if (!subcommand || subcommand === 'help' || subcommand === '--help' || subcommand === '-h') {
    return usage(opsCommandUsage())
  }
  if (hasHelpFlag(argv.slice(2))) return usage(opsCommandUsage(subcommand))

  if (subcommand === 'list') {
    return ok(await store.list(), hasFlag(argv, '--json'))
  }

  if (subcommand === 'show') {
    const operation = await store.get(argv[2])
    if (!operation) return fail(`Unknown operation: ${argv[2] || ''}`.trim())
    return ok(operation, hasFlag(argv, '--json'))
  }

  if (subcommand === 'undo') {
    const opId = argv[2]
    if (!opId) return fail('Usage: clouddrive-cli ops undo <operation-id> [--dry-run] [--json]')

    const operation = await store.get(opId)
    if (!operation) return fail(`Unknown operation: ${opId}`, EXIT_CODES.VALIDATION_ERROR)

    if (operation.type === 'move') {
      const undoPlan = createUndoMovePlan(operation)
      if (hasFlag(argv, '--dry-run')) {
        const dryResult = dryRunMovePlan(undoPlan)
        return ok({ undoPlan, dryRun: dryResult }, hasFlag(argv, '--json'))
      }
      const applyRationale = requireApplyRationale(argv)
      if (applyRationale.error) return fail(applyRationale.error)
      const providerName = operation.provider
      const provider = activeProviders(env)[providerName]
      if (!provider) return fail(`Unknown provider: ${providerName}`, EXIT_CODES.UNSUPPORTED_CAPABILITY)
      if (!provider.capabilities.move) return fail(`Provider "${providerName}" does not support move`, EXIT_CODES.UNSUPPORTED_CAPABILITY)
      const token = await resolveToken(env, providerName, operation.account_id)
      const driveId = (operation.items[0]?.drive_id) || token.default_drive_id
      const moves = undoPlan.items.map((item) => ({
        fileId: item.file_id, name: item.name, type: item.type || 'file',
        toParentId: item.to_parent_file_id, fromPath: item.from_path, toFolderPath: item.to_folder_path,
      }))
      const batchResults = await provider.files.moveBatch({ token, driveId, moves })
      const succeeded = batchResults.filter((r) => r.status === 'success').length
      const failed = batchResults.filter((r) => r.status !== 'success').length
      const undoLog = { id: `op_${Date.now()}`, type: 'move', provider: providerName, account_id: operation.account_id, rationale: applyRationale.rationale, started_at: new Date().toISOString(), finished_at: new Date().toISOString(), source_undo_of: opId, items: batchResults.map((r, i) => ({ drive_id: driveId, file_id: r.fileId, name: undoPlan.items[i]?.name || '', type: undoPlan.items[i]?.type || 'file', from_parent_file_id: undoPlan.items[i]?.from_parent_file_id || '', to_parent_file_id: undoPlan.items[i]?.to_parent_file_id || '', status: r.status, error: r.status !== 'success' ? { code: r.code, message: r.message } : undefined })) }
      await store.save(undoLog)
      const exitCode = failed === 0 ? EXIT_CODES.SUCCESS : succeeded > 0 ? EXIT_CODES.PARTIAL_SUCCESS : EXIT_CODES.PROVIDER_API_ERROR
      return { exitCode, stdout: jsonOut({ ok: failed === 0, undoOperationId: undoLog.id, sourceOperationId: opId, rationale: applyRationale.rationale, succeeded, failed, results: batchResults }), stderr: '' }
    }

    if (operation.type !== 'rename') return unsupportedUndo(operation)

    const undoPlan = createUndoRenamePlan(operation)

    if (hasFlag(argv, '--dry-run')) {
      const dryResult = dryRunRenamePlan(undoPlan, [])
      return ok({ undoPlan, dryRun: dryResult }, hasFlag(argv, '--json'))
    }
    const applyRationale = requireApplyRationale(argv)
    if (applyRationale.error) return fail(applyRationale.error)

    const providerName = operation.provider
    const provider = PROVIDERS[providerName]
    if (!provider) return fail(`Unknown provider: ${providerName}`, EXIT_CODES.UNSUPPORTED_CAPABILITY)
    if (!provider.capabilities.batchRename) return fail(`Provider "${providerName}" does not support batch rename`, EXIT_CODES.UNSUPPORTED_CAPABILITY)

    const token = await resolveToken(env, providerName, operation.account_id)
    const driveId = (operation.items[0]?.drive_id) || token.default_drive_id

    const renames = undoPlan.items.map((item) => ({ fileId: item.file_id, newName: item.new_name }))
    const batchResults = await provider.files.renameBatch({ token, driveId, renames })

    const succeeded = batchResults.filter((r) => r.status === 'success').length
    const failed = batchResults.filter((r) => r.status !== 'success').length

    const undoLog = {
      id: `op_${Date.now()}`,
      type: 'rename',
      provider: providerName,
      account_id: operation.account_id,
      rationale: applyRationale.rationale,
      started_at: new Date().toISOString(),
      finished_at: new Date().toISOString(),
      source_undo_of: opId,
      items: batchResults.map((r, i) => ({
        drive_id: driveId,
        file_id: r.fileId,
        parent_file_id: undoPlan.items[i]?.parent_file_id || '',
        before_name: undoPlan.items[i]?.old_name || '',
        after_name: r.status === 'success' ? r.newName : undoPlan.items[i]?.new_name,
        status: r.status,
        error: r.status !== 'success' ? { code: r.code, message: r.message } : undefined,
      })),
    }
    await store.save(undoLog)

    const exitCode = failed === 0 ? EXIT_CODES.SUCCESS : succeeded > 0 ? EXIT_CODES.PARTIAL_SUCCESS : EXIT_CODES.PROVIDER_API_ERROR
    const payload = { ok: failed === 0, undoOperationId: undoLog.id, sourceOperationId: opId, rationale: applyRationale.rationale, succeeded, failed, results: batchResults }
    return { exitCode, stdout: jsonOut(payload), stderr: '' }
  }

  return fail(`Unknown ops command: ${subcommand || ''}`.trim())
}

async function handleSettings(argv, env) {
  const subcommand = argv[1]
  if (subcommand === 'help' || subcommand === '--help' || subcommand === '-h') {
    return usage('Usage: clouddrive-cli settings show [--json]')
  }
  if (subcommand === 'show' && hasHelpFlag(argv.slice(2))) {
    return usage('Usage: clouddrive-cli settings show [--json]')
  }
  if (!subcommand || subcommand === 'show' || subcommand === '--json') {
    const isJson = hasFlag(argv, '--json') || subcommand === '--json'
    const store = createAuthStore({ configDir: env.configDir })
    const accounts = await store.listAccounts()
    const byProvider = {}
    for (const a of accounts) {
      if (!byProvider[a.provider]) byProvider[a.provider] = 0
      byProvider[a.provider]++
    }
    const defaults = {}
    for (const p of Object.keys(PROVIDERS)) {
      const def = await store.getDefaultAccount(p).catch(() => null)
      if (def) defaults[p] = def.accountId
    }
    const payload = {
      config_dir: env.configDir,
      total_accounts: accounts.length,
      accounts_by_provider: byProvider,
      default_accounts: defaults,
      providers: Object.keys(PROVIDERS),
    }
    if (isJson) return ok(payload, true)
    const lines = [
      `Config dir:  ${payload.config_dir}`,
      `Total accounts: ${payload.total_accounts}`,
      'Accounts by provider:',
      ...Object.entries(byProvider).map(([p, n]) => `  ${p}: ${n}${defaults[p] ? ` (default: ${defaults[p]})` : ''}`),
    ]
    return { exitCode: EXIT_CODES.SUCCESS, stdout: lines.join('\n') + '\n', stderr: '' }
  }
  return fail(`Unknown settings command: ${subcommand || ''}`.trim())
}

async function handleProviders(argv) {
  const subcommand = argv[1]
  if (!subcommand || subcommand === 'help' || subcommand === '--help' || subcommand === '-h') {
    return { exitCode: EXIT_CODES.SUCCESS, stdout: 'clouddrive-cli providers <command>\n\nCommands:\n  clouddrive-cli providers capabilities [--json]\n', stderr: '' }
  }
  if (hasHelpFlag(argv.slice(2))) return usage('Usage: clouddrive-cli providers capabilities [--json]')
  if (subcommand !== 'capabilities') return fail(`Unknown providers command: ${subcommand || ''}`.trim())
  return ok(providerCapabilities(), hasFlag(argv, '--json'))
}

async function handleList(argv) {
  if (hasHelpFlag(argv.slice(1))) return usage('Usage: clouddrive-cli list [--group <name>] [--json]')
  const group = readOption(argv, '--group')
  const commands = listCommands({ group })
  if (hasFlag(argv, '--json')) return ok(commands, true)
  const lines = ['Command                         Access  Description']
  lines.push('──────────────────────────────  ──────  ─────────────────────────────────────────')
  for (const command of commands) {
    lines.push(`${command.command.padEnd(30)}  ${command.access.padEnd(6)}  ${command.description}`)
  }
  return { exitCode: EXIT_CODES.SUCCESS, stdout: lines.join('\n') + '\n', stderr: '' }
}

async function handleSchema(argv) {
  const subcommand = argv[1]
  if (subcommand === 'help' || subcommand === '--help' || subcommand === '-h') {
    return usage('Usage: clouddrive-cli schema <commands|plans> [--group <name>] [--name <plan>]')
  }
  if (subcommand === 'commands' && hasHelpFlag(argv.slice(2))) {
    return usage('Usage: clouddrive-cli schema commands [--group <name>]')
  }
  if (subcommand === 'plans' && hasHelpFlag(argv.slice(2))) {
    return usage('Usage: clouddrive-cli schema plans [--name <rename|move|trash|upload>]')
  }
  if (!subcommand || subcommand === 'commands') {
    return ok({
      version: COMMAND_MANIFEST_VERSION,
      commands: listCommands({ group: readOption(argv, '--group') }),
    }, true)
  }
  if (subcommand === 'plans') {
    return ok({
      version: PLAN_SCHEMA_VERSION,
      plans: listPlanSchemas({ name: readOption(argv, '--name') }),
    }, true)
  }
  return fail(`Unknown schema command: ${subcommand || ''}`.trim())
}

async function handleUpload(argv, env) {
  const subcommand = argv[1]
  if (!subcommand || subcommand === 'help' || subcommand === '--help' || subcommand === '-h') {
    return usage(uploadCommandUsage())
  }
  if (hasHelpFlag(argv.slice(2))) return usage(uploadCommandUsage(subcommand))

  if (subcommand === 'plan') {
    const localPath = readOption(argv, '--local')
    const provider = readOption(argv, '--provider') || 'aliyun'
    const accountId = readOption(argv, '--account') || 'default'
    const remoteParentFileId = readOption(argv, '--remote-parent') || 'root'
    const outputPath = readOption(argv, '--output')
    const conflict = readOption(argv, '--conflict') || 'skip'

    if (!localPath) return fail('Usage: clouddrive-cli upload plan --local <path> [--provider <p>] [--account <id>] [--remote-parent <id>] [--output <plan.json>] [--json]')
    if (!PROVIDERS[provider]) return fail(`Unknown provider: ${provider}`, EXIT_CODES.UNSUPPORTED_CAPABILITY)

    const plan = await createUploadPlanFromLocalPath({ localPath, provider, accountId, remoteParentFileId, conflict })
    if (outputPath) await writeFile(outputPath, `${JSON.stringify(plan, null, 2)}\n`, 'utf8')
    return ok(plan, hasFlag(argv, '--json') || !outputPath)
  }

  if (subcommand === 'apply') {
    const planPath = argv[2]
    if (!planPath) return fail('Usage: clouddrive-cli upload apply <plan.json> [--dry-run] [--json]')
    const plan = await readJsonFile(planPath)
    const rationale = readOption(argv, '--rationale')
    const dryRun = dryRunUploadPlan(plan)
    if (rationale) dryRun.rationale = rationale
    if (hasFlag(argv, '--dry-run')) return ok(dryRun, hasFlag(argv, '--json'))
    if (!dryRun.ok) return ok(dryRun, hasFlag(argv, '--json'))
    const applyRationale = requireApplyRationale(argv)
    if (applyRationale.error) return fail(applyRationale.error)

    const provider = activeProviders(env)[plan.provider]
    if (!provider) return fail(`Unknown provider: ${plan.provider}`, EXIT_CODES.UNSUPPORTED_CAPABILITY)
    if (!provider.capabilities.uploadFile) {
      return fail(`Provider "${plan.provider}" does not support CLI upload yet`, EXIT_CODES.UNSUPPORTED_CAPABILITY)
    }
    const token = await resolveToken(env, plan.provider, plan.account_id)
    const driveId = plan.drive_id || token.default_drive_id || token.backup_drive_id || plan.provider
    const result = await executeUploadPlan(plan, { provider, token, driveId })
    result.rationale = applyRationale.rationale
    const exitCode = result.ok ? EXIT_CODES.SUCCESS : result.succeeded > 0 ? EXIT_CODES.PARTIAL_SUCCESS : EXIT_CODES.PROVIDER_API_ERROR
    return { exitCode, stdout: jsonOut(result), stderr: '' }
  }

  return fail(`Unknown upload command: ${subcommand || ''}`.trim())
}

function printHelp() {
  return {
    exitCode: EXIT_CODES.SUCCESS,
    stdout: `clouddrive-cli — BoxPlayer 命令行工具

用法：
  clouddrive-cli <命令> [子命令] [选项]

命令：
  list [--group <name>] [--json]
                                机器可发现的命令清单
  schema commands [--group <name>]
                                输出命令 schema/manifest
  schema plans [--name <plan>]
                                输出 move/rename/trash/upload plan JSON schema

  auth
    list                        列出所有已保存账号
    default <provider> <id>     设置默认账号
    import-token --provider <p> --account <id> --token <token.json>
                                导入独立 CLI token
    login <provider> [--browser chrome]
                                使用浏览器 OAuth 登录独立 CLI

  providers
    capabilities [--json]       列出 provider 能力矩阵

  files
    list    --provider <p> --account <id> [--drive-id <d>] [--file-id <id>]
            [--limit <n>] [--cursor <token>] [--json]
                                列出目录文件
    walk    --provider <p> --account <id> [--drive-id <d>] [--file-id <id>] [--json]
                                递归遍历文件
    tree    --provider <p> [--file-id <id>] [--depth 3] [--json]
                                输出目录树摘要（更适合 AI 上下文）
    stats   --provider <p> [--file-id <id>] [--depth 10] [--json]
                                统计目录大小/文件数/视频数
    info    --file-id <id> --provider <p> [--drive-id <d>] [--json]
                                查看文件详情
    search  --name <filename> [--provider <p>] [--account <id>] [--limit <n>] [--json]
                                按文件名搜索（服务端搜索）
    mkdir   --name <name> [--parent <id>] [--provider <p>] [--json]
                                创建目录
    move-apply <plan.json> [--dry-run] [--json]
                                执行移动计划
    trash-apply <plan.json> [--apply] [--json]
                                移动到回收站（默认 dry-run，加 --apply 执行）
    rename-apply <plan.json> [--current <f>] [--dry-run] [--json]
                                执行重命名计划

  upload
    plan --local <path> [--provider <p>] [--account <id>]
         [--remote-parent <id>] [--output <plan.json>] [--json]
                                生成本地到网盘的上传计划
    apply <plan.json> [--dry-run] [--json]
                                校验或执行上传计划

  ops
    list                        列出操作历史
    show <op-id>                查看操作详情
    undo <op-id> [--dry-run]    撤销支持的移动/重命名操作

  help, --help, -h              显示此帮助信息

支持的 provider：aliyun · cloud123 · 115 · 139 · 189 · quark · pikpak · dropbox · onedrive · box · baidu

示例：
  clouddrive-cli auth list
  clouddrive-cli list --format json
  clouddrive-cli providers capabilities --json
  clouddrive-cli files list --provider aliyun --json
`,
    stderr: '',
  }
}


export async function runBoxPlayerCli(argv, env = {}) {
  const runtime = {
    ...env,
    configDir: env.configDir || process.env.CLOUDDRIVE_CLI_CONFIG_DIR || defaultConfigDir(),
  }
  try {
    const command = argv[0]
    let result
    if (!command || command === 'help' || command === '--help' || command === '-h') result = printHelp()
    else if (command === 'list') result = await handleList(argv)
    else if (command === 'schema') result = await handleSchema(argv)
    else if (command === 'auth') result = await handleAuth(argv, runtime)
    else if (command === 'providers') result = await handleProviders(argv)
    else if (command === 'files') result = await handleFiles(argv, runtime)
    else if (command === 'upload') result = await handleUpload(argv, runtime)
    else if (command === 'ops') result = await handleOps(argv, runtime)
    else if (command === 'settings') result = await handleSettings(argv, runtime)
    else result = fail(`未知命令: ${command}\n运行 clouddrive-cli --help 查看可用命令`)
    return maybeJsonError(result, argv)
  } catch (error) {
    return maybeJsonError(fail(error?.message || 'Unknown error', classifyError(error)), argv)
  }
}
