import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { createAuthStore } from '../core/authStore.mjs'
import { runBoxPlayerCli } from '../core/commands.mjs'
import { createOperationLogStore } from '../core/operationLog.mjs'

const tempDirs: string[] = []

async function makeTempDir() {
  const dir = await mkdtemp(join(tmpdir(), 'clouddrive-cli-command-'))
  tempDirs.push(dir)
  return dir
}

async function writeJson(path: string, value: unknown) {
  await writeFile(path, `${JSON.stringify(value, null, 2)}\n`, 'utf8')
}

afterEach(async () => {
  vi.restoreAllMocks()
  while (tempDirs.length) {
    const dir = tempDirs.pop()
    if (dir) await rm(dir, { recursive: true, force: true })
  }
})

describe('BoxPlayer CLI commands', () => {
  it('lists command metadata as machine-readable JSON', async () => {
    const configDir = await makeTempDir()
    const result = await runBoxPlayerCli(['list', '--format', 'json'], { configDir })

    expect(result.exitCode).toBe(0)
    const commands = JSON.parse(result.stdout)
    expect(commands).toEqual(expect.arrayContaining([
      expect.objectContaining({
        group: 'auth',
        name: 'list',
        command: 'auth list',
        access: 'read',
        args: expect.any(Array),
        examples: expect.arrayContaining([expect.stringContaining('clouddrive-cli auth list')]),
        safety: expect.objectContaining({ dryRunRequired: false }),
      }),
      expect.objectContaining({
        group: 'files',
        name: 'rename-apply',
        command: 'files rename-apply',
        access: 'write',
        requiresDryRun: true,
        undoable: true,
      }),
      expect.objectContaining({
        command: 'files walk',
        largeOutput: true,
        examples: expect.arrayContaining([expect.stringContaining('clouddrive-cli files walk')]),
      }),
      expect.objectContaining({
        group: 'schema',
        command: 'schema plans',
        access: 'read',
        output: 'PlanSchema[]',
      }),
      expect.objectContaining({
        command: 'upload apply',
        safety: expect.objectContaining({ dryRunRequired: true, destructive: false }),
        providerRequirements: expect.objectContaining({ capability: 'uploadFile' }),
      }),
      expect.objectContaining({
        command: 'files download',
        access: 'read',
        providerRequirements: expect.objectContaining({ capability: 'downloadFile' }),
        examples: expect.arrayContaining([expect.stringContaining('clouddrive-cli files download')]),
      }),
      expect.objectContaining({
        command: 'files list',
        options: expect.arrayContaining([
          expect.objectContaining({ name: 'file-id' }),
          expect.objectContaining({ name: 'limit' }),
          expect.objectContaining({ name: 'cursor' }),
        ]),
      }),
    ]))
    const filesList = commands.find((command: { command: string }) => command.command === 'files list')
    expect(filesList.options).not.toEqual(expect.arrayContaining([expect.objectContaining({ name: 'path' })]))
    expect(commands.map((command: { command: string }) => command.command)).not.toEqual(expect.arrayContaining([
      'media rename-plan',
      'media organize-plan',
    ]))
  })

  it('prints plan schemas as machine-readable JSON', async () => {
    const configDir = await makeTempDir()
    const result = await runBoxPlayerCli(['schema', 'plans', '--json'], { configDir })

    expect(result.exitCode).toBe(0)
    const body = JSON.parse(result.stdout)
    expect(body.version).toBe(1)
    expect(body.plans).toEqual(expect.arrayContaining([
      expect.objectContaining({
        name: 'rename',
        command: 'files rename-apply',
        requiresDryRun: true,
        applyRequiresRationale: true,
        itemFields: expect.arrayContaining([expect.objectContaining({ name: 'file_id', required: true })]),
      }),
      expect.objectContaining({
        name: 'upload',
        command: 'upload apply',
        example: expect.objectContaining({ operation: 'upload' }),
      }),
    ]))
  })

  it('supports --format json as an alias for --json', async () => {
    const configDir = await makeTempDir()
    const store = createAuthStore({ configDir })
    await store.saveAccount({
      provider: 'aliyun',
      accountId: 'aliyun_demo',
      displayName: 'Demo',
      token: { accessToken: 'secret' },
    })

    const result = await runBoxPlayerCli(['auth', 'list', '--format', 'json'], { configDir })

    expect(result.exitCode).toBe(0)
    expect(JSON.parse(result.stdout)).toEqual([
      { provider: 'aliyun', accountId: 'aliyun_demo', displayName: 'Demo', isDefault: false },
    ])
  })

  it('returns structured JSON errors when JSON output is requested', async () => {
    const configDir = await makeTempDir()
    const result = await runBoxPlayerCli(['files', 'list', '--provider', 'nonexistent', '--json'], { configDir })

    expect(result.exitCode).toBe(5)
    expect(result.stderr).toBe('')
    expect(JSON.parse(result.stdout)).toEqual({
      ok: false,
      error: {
        code: 'UNSUPPORTED_CAPABILITY',
        message: 'Unknown provider: nonexistent',
        exitCode: 5,
      },
    })
  })

  it('prints auth help when no auth subcommand is provided', async () => {
    const configDir = await makeTempDir()
    const result = await runBoxPlayerCli(['auth'], { configDir })

    expect(result.exitCode).toBe(0)
    expect(result.stdout).toContain('clouddrive-cli auth login <provider>')
    expect(result.stdout).toContain('aliyun')
    expect(result.stdout).toContain('dropbox')
    expect(result.stdout).toContain('box')
    expect(result.stdout).toContain('123')
    expect(result.stdout).toContain('115')
    expect(result.stderr).toBe('')
  })

  it('prints auth list as JSON without secrets', async () => {
    const configDir = await makeTempDir()
    const store = createAuthStore({ configDir })
    await store.saveAccount({
      provider: 'aliyun',
      accountId: 'aliyun_demo',
      displayName: 'Demo',
      token: { accessToken: 'secret' },
    })

    const result = await runBoxPlayerCli(['auth', 'list', '--json'], { configDir })

    expect(result).toEqual({
      exitCode: 0,
      stdout: `${JSON.stringify([{ provider: 'aliyun', accountId: 'aliyun_demo', displayName: 'Demo', isDefault: false }], null, 2)}\n`,
      stderr: '',
    })
  })

  it('prints full account ids in auth list text output', async () => {
    const configDir = await makeTempDir()
    const store = createAuthStore({ configDir })
    const longAccountId = 'aliyun_25fd55383d5a4bb5a7319ad66c4c7e75'
    await store.saveAccount({
      provider: 'aliyun',
      accountId: longAccountId,
      displayName: 'Demo',
      token: { accessToken: 'secret' },
    })
    await store.setDefaultAccount('aliyun', longAccountId)

    const result = await runBoxPlayerCli(['auth', 'list'], { configDir })

    expect(result.exitCode).toBe(0)
    expect(result.stdout).toContain(longAccountId)
    expect(result.stdout).not.toContain('aliyun_25fd55383d5a4bb5a7319ad66c  ')
  })

  it('sets a default account', async () => {
    const configDir = await makeTempDir()
    const store = createAuthStore({ configDir })
    await store.saveAccount({
      provider: 'aliyun',
      accountId: 'aliyun_demo',
      displayName: 'Demo',
      token: { accessToken: 'secret' },
    })

    const result = await runBoxPlayerCli(['auth', 'default', 'aliyun', 'aliyun_demo', '--json'], { configDir })

    expect(result.exitCode).toBe(0)
    expect(JSON.parse(result.stdout)).toEqual({
      provider: 'aliyun',
      accountId: 'aliyun_demo',
      displayName: 'Demo',
      isDefault: true,
    })
  })

  it('imports an auth token for standalone CLI installs', async () => {
    const configDir = await makeTempDir()
    const tokenPath = join(configDir, 'token.json')
    await writeJson(tokenPath, {
      access_token: 'access',
      refresh_token: 'refresh',
      user_id: 'u1',
      user_name: 'Standalone User',
    })

    const result = await runBoxPlayerCli([
      'auth', 'import-token',
      '--provider', 'aliyun',
      '--account', 'aliyun_standalone',
      '--name', 'Standalone User',
      '--token', tokenPath,
      '--default',
      '--json',
    ], { configDir })

    expect(result.exitCode).toBe(0)
    expect(JSON.parse(result.stdout)).toEqual({
      provider: 'aliyun',
      accountId: 'aliyun_standalone',
      displayName: 'Standalone User',
      isDefault: true,
    })
  })

  it('dry-runs a rename plan from files', async () => {
    const configDir = await makeTempDir()
    const planPath = join(configDir, 'plan.json')
    const currentPath = join(configDir, 'current.json')
    await writeJson(planPath, {
      version: 1,
      operation: 'rename',
      provider: 'aliyun',
      account_id: 'aliyun_demo',
      items: [
        {
          drive_id: 'drive',
          file_id: 'file-1',
          parent_file_id: 'parent',
          old_name: 'A.mkv',
          new_name: 'B.mkv',
        },
      ],
    })
    await writeJson(currentPath, [{ fileId: 'file-1', name: 'A.mkv' }])

    const result = await runBoxPlayerCli(['files', 'rename-apply', planPath, '--current', currentPath, '--dry-run', '--json'], { configDir })

    expect(result.exitCode).toBe(0)
    expect(JSON.parse(result.stdout)).toMatchObject({
      ok: true,
      changes: [{ file_id: 'file-1', before_name: 'A.mkv', after_name: 'B.mkv' }],
      errors: [],
    })
  })

  it('lists operations as JSON', async () => {
    const configDir = await makeTempDir()
    const store = createOperationLogStore({ configDir })
    await store.save({
      id: 'op_test',
      type: 'rename',
      provider: 'aliyun',
      account_id: 'aliyun_demo',
      started_at: '2026-05-14T00:00:00.000Z',
      finished_at: '2026-05-14T00:00:01.000Z',
      items: [],
    })

    const result = await runBoxPlayerCli(['ops', 'list', '--json'], { configDir })

    expect(result.exitCode).toBe(0)
    expect(JSON.parse(result.stdout)).toEqual([
      {
        id: 'op_test',
        type: 'rename',
        provider: 'aliyun',
        account_id: 'aliyun_demo',
        started_at: '2026-05-14T00:00:00.000Z',
        finished_at: '2026-05-14T00:00:01.000Z',
        successCount: 0,
        failureCount: 0,
      },
    ])
  })

  it('returns an error for unknown commands', async () => {
    const configDir = await makeTempDir()
    const result = await runBoxPlayerCli(['wat'], { configDir })

    expect(result).toEqual({
      exitCode: 1,
      stdout: '',
      stderr: '未知命令: wat\n运行 clouddrive-cli --help 查看可用命令\n',
    })
  })

  it('ops undo --dry-run generates inverse plan without applying', async () => {
    const configDir = await makeTempDir()
    const store = createOperationLogStore({ configDir })
    await store.save({
      id: 'op_undo_test',
      type: 'rename',
      provider: 'aliyun',
      account_id: 'aliyun_demo',
      started_at: '2026-05-14T00:00:00.000Z',
      finished_at: '2026-05-14T00:00:01.000Z',
      items: [
        { drive_id: 'd1', file_id: 'f1', parent_file_id: 'p1', before_name: 'Old.mkv', after_name: 'New.mkv', status: 'success' },
      ],
    })

    const result = await runBoxPlayerCli(['ops', 'undo', 'op_undo_test', '--dry-run', '--json'], { configDir })

    expect(result.exitCode).toBe(0)
    const body = JSON.parse(result.stdout)
    expect(body.undoPlan.items[0].old_name).toBe('New.mkv')
    expect(body.undoPlan.items[0].new_name).toBe('Old.mkv')
    expect(body.undoPlan.source_operation_id).toBe('op_undo_test')
  })

  it('ops undo returns exitCode 1 for unknown operation', async () => {
    const configDir = await makeTempDir()
    const result = await runBoxPlayerCli(['ops', 'undo', 'nonexistent', '--json'], { configDir })
    expect(result.exitCode).toBe(1)
    expect(result.stderr).toBe('')
    expect(JSON.parse(result.stdout).error.message).toMatch(/Unknown operation/)
  })

  it('auth error returns exitCode 2', async () => {
    const configDir = await makeTempDir()
    const result = await runBoxPlayerCli(['files', 'list', '--provider', 'aliyun', '--json'], { configDir })
    expect(result.exitCode).toBe(2)
    expect(result.stderr).toBe('')
    expect(JSON.parse(result.stdout).error.message).toMatch(/No account found/)
  })

  it('unknown provider returns exitCode 5', async () => {
    const configDir = await makeTempDir()
    const result = await runBoxPlayerCli(['files', 'list', '--provider', 'nonexistent', '--json'], { configDir })
    expect(result.exitCode).toBe(5)
  })

  it('does not expose non cloud-drive commands', async () => {
    const configDir = await makeTempDir()
    const listed = await runBoxPlayerCli(['list', '--json'], { configDir })
    const commands = JSON.parse(listed.stdout).map((command: { command: string }) => command.command)
    expect(commands).not.toEqual(expect.arrayContaining([
      'media scan',
      'media match',
      'docs read',
      'docs convert',
      'organize analyze',
      'organize plan',
      'organize apply',
    ]))

    for (const argv of [
      ['media', 'scan', '--json'],
      ['docs', 'read', 'README.md', '--json'],
      ['organize', 'analyze', '--json'],
    ]) {
      const result = await runBoxPlayerCli(argv, { configDir })
      expect(result.exitCode, argv.join(' ')).toBe(1)
      expect(result.stderr, argv.join(' ')).toBe('')
      expect(JSON.parse(result.stdout).error.message).toMatch(/未知命令/)
    }
  })

  it('prints provider capabilities as JSON', async () => {
    const configDir = await makeTempDir()

    const result = await runBoxPlayerCli(['providers', 'capabilities', '--json'], { configDir })

    expect(result.exitCode).toBe(0)
    const body = JSON.parse(result.stdout)
    expect(body.find((provider: { id: string }) => provider.id === 'aliyun')).toMatchObject({
      id: 'aliyun',
      displayName: 'Aliyun Drive',
      capabilities: {
        batchRename: true,
        recursiveWalk: true,
        uploadFile: true,
        downloadFile: true,
        mkdir: true,
        move: true,
      },
    })
    expect(body.find((provider: { id: string }) => provider.id === 'cloud123')).toMatchObject({
      id: 'cloud123',
      displayName: '123网盘',
      capabilities: {
        recursiveWalk: true,
        serverSideSearch: true,
        uploadFile: true,
        mkdir: true,
        move: true,
      },
    })
    expect(body.find((provider: { id: string }) => provider.id === 'quark')).toMatchObject({
      id: 'quark',
      displayName: '夸克网盘',
      capabilities: {
        recursiveWalk: true,
        serverSideSearch: true,
        uploadFile: false,
        mkdir: true,
        move: true,
      },
    })
    expect(body.find((provider: { id: string }) => provider.id === '139')).toMatchObject({
      id: '139',
      displayName: '139云盘',
      capabilities: {
        recursiveWalk: true,
        uploadFile: false,
        mkdir: true,
        move: true,
      },
    })
    expect(body.find((provider: { id: string }) => provider.id === '189')).toMatchObject({
      id: '189',
      displayName: '天翼云盘',
      capabilities: {
        recursiveWalk: true,
        uploadFile: false,
        mkdir: true,
        move: true,
      },
    })
  })

  it('creates and dry-runs an upload plan from a local directory', async () => {
    const configDir = await makeTempDir()
    const localDir = await makeTempDir()
    const outputPath = join(configDir, 'upload-plan.json')
    await mkdir(join(localDir, 'Season 01'))
    await writeFile(join(localDir, 'Season 01', 'Episode 01.mkv'), 'video', 'utf8')

    const planned = await runBoxPlayerCli([
      'upload', 'plan',
      '--local', localDir,
      '--provider', 'aliyun',
      '--account', 'default',
      '--remote-parent', 'root',
      '--output', outputPath,
      '--json',
    ], { configDir })

    expect(planned.exitCode).toBe(0)
    expect(JSON.parse(planned.stdout)).toMatchObject({
      operation: 'upload',
      provider: 'aliyun',
      remote_parent_file_id: 'root',
    })
    expect(JSON.parse(await readFile(outputPath, 'utf8')).items).toHaveLength(2)

    const dryRun = await runBoxPlayerCli(['upload', 'apply', outputPath, '--dry-run', '--json'], { configDir })

    expect(dryRun.exitCode).toBe(0)
    expect(JSON.parse(dryRun.stdout)).toMatchObject({
      ok: true,
      fileCount: 1,
      folderCount: 1,
      totalBytes: 5,
      errors: [],
    })
  })

  it('writes large read command output to a file and returns an agent summary', async () => {
    const configDir = await makeTempDir()
    const outputPath = join(configDir, 'stats.json')
    const store = createAuthStore({ configDir })
    await store.saveAccount({
      provider: 'aliyun',
      accountId: 'u1',
      displayName: 'Aliyun',
      token: { user_id: 'u1', default_drive_id: 'drive' },
    })
    await store.setDefaultAccount('aliyun', 'u1')

    const result = await runBoxPlayerCli([
      'files', 'stats',
      '--provider', 'aliyun',
      '--account', 'default',
      '--file-id', 'root',
      '--output', outputPath,
      '--json',
    ], {
      configDir,
      providers: {
        aliyun: {
          id: 'aliyun',
          capabilities: { recursiveWalk: true },
          files: {
            async *walk() {
              yield { type: 'file', name: 'movie.mkv', size: 10 }
            },
          },
        },
      } as any,
    })

    expect(result.exitCode).toBe(0)
    expect(JSON.parse(result.stdout)).toMatchObject({
      ok: true,
      output: outputPath,
      summary: { total_files: 1, total_size: 10 },
    })
    expect(JSON.parse(await readFile(outputPath, 'utf8'))).toMatchObject({
      total_files: 1,
      total_size: 10,
    })
  })

  it('refreshes an expired token before reading files', async () => {
    const configDir = await makeTempDir()
    const store = createAuthStore({ configDir })
    await store.saveAccount({
      provider: 'aliyun',
      accountId: 'u1',
      displayName: 'Aliyun',
      token: {
        access_token: 'expired-access',
        refresh_token: 'refresh-token',
        default_drive_id: 'drive',
        expire_time: '2020-01-01T00:00:00.000Z',
      },
    })
    await store.setDefaultAccount('aliyun', 'u1')

    const result = await runBoxPlayerCli([
      'files', 'list',
      '--provider', 'aliyun',
      '--account', 'default',
      '--file-id', 'root',
      '--json',
    ], {
      configDir,
      providers: {
        aliyun: {
          id: 'aliyun',
          auth: {
            async refresh(token: { refresh_token: string }) {
              expect(token.refresh_token).toBe('refresh-token')
              return {
                access_token: 'fresh-access',
                refresh_token: 'fresh-refresh',
                default_drive_id: 'drive',
                expire_time: '2999-01-01T00:00:00.000Z',
              }
            },
          },
          files: {
            async list({ token }: { token: { access_token: string } }) {
              expect(token.access_token).toBe('fresh-access')
              return [{ type: 'folder', fileId: 'folder-1', name: 'Media' }]
            },
          },
        },
      } as any,
    })

    expect(result.exitCode).toBe(0)
    expect(JSON.parse(result.stdout)).toEqual([{ type: 'folder', fileId: 'folder-1', name: 'Media' }])
    const saved = await store.getAccount('aliyun', 'u1')
    expect(saved?.token).toMatchObject({
      access_token: 'fresh-access',
      refresh_token: 'fresh-refresh',
    })
  })

  it('accepts --file-id as the cloud folder id for files list', async () => {
    const configDir = await makeTempDir()
    const store = createAuthStore({ configDir })
    await store.saveAccount({
      provider: 'aliyun',
      accountId: 'u1',
      displayName: 'Aliyun',
      token: { user_id: 'u1', default_drive_id: 'drive' },
    })
    await store.setDefaultAccount('aliyun', 'u1')

    const result = await runBoxPlayerCli([
      'files', 'list',
      '--provider', 'aliyun',
      '--account', 'default',
      '--file-id', 'folder-1',
      '--json',
    ], {
      configDir,
      providers: {
        aliyun: {
          id: 'aliyun',
          files: {
            async list({ parentFileId }: { parentFileId: string }) {
              expect(parentFileId).toBe('folder-1')
              return [{ type: 'file', fileId: 'file-1', name: 'Movie.mkv' }]
            },
          },
        },
      } as any,
    })

    expect(result.exitCode).toBe(0)
    expect(JSON.parse(result.stdout)).toEqual([{ type: 'file', fileId: 'file-1', name: 'Movie.mkv' }])
  })

  it('downloads one cloud file through the provider download capability', async () => {
    const configDir = await makeTempDir()
    const outputPath = join(configDir, 'downloads', 'movie.mkv')
    const store = createAuthStore({ configDir })
    await store.saveAccount({
      provider: 'aliyun',
      accountId: 'u1',
      displayName: 'Aliyun',
      token: { user_id: 'u1', default_drive_id: 'drive' },
    })
    await store.setDefaultAccount('aliyun', 'u1')

    const result = await runBoxPlayerCli([
      'files', 'download',
      '--provider', 'aliyun',
      '--account', 'default',
      '--file-id', 'file-1',
      '--output', outputPath,
      '--json',
    ], {
      configDir,
      providers: {
        aliyun: {
          id: 'aliyun',
          capabilities: { downloadFile: true },
          files: {
            async downloadFile({ token, driveId, fileId, outputPath: target }: { token: any; driveId: string; fileId: string; outputPath: string }) {
              expect(token.user_id).toBe('u1')
              expect(driveId).toBe('drive')
              expect(fileId).toBe('file-1')
              await mkdir(join(target, '..'), { recursive: true })
              await writeFile(target, 'movie-bytes', 'utf8')
              return { ok: true, provider: 'aliyun', accountId: token.user_id, driveId, fileId, output: target, size: 11 }
            },
          },
        },
      } as any,
    })

    expect(result.exitCode).toBe(0)
    expect(JSON.parse(result.stdout)).toMatchObject({
      ok: true,
      provider: 'aliyun',
      fileId: 'file-1',
      output: outputPath,
    })
    expect(await readFile(outputPath, 'utf8')).toBe('movie-bytes')
  })

  it('rejects download when a provider does not declare download capability', async () => {
    const configDir = await makeTempDir()
    const store = createAuthStore({ configDir })
    await store.saveAccount({
      provider: 'pikpak',
      accountId: 'u1',
      displayName: 'PikPak',
      token: { user_id: 'u1' },
    })
    await store.setDefaultAccount('pikpak', 'u1')

    const result = await runBoxPlayerCli([
      'files', 'download',
      '--provider', 'pikpak',
      '--file-id', 'file-1',
      '--output', join(configDir, 'file.bin'),
      '--json',
    ], {
      configDir,
      providers: {
        pikpak: {
          id: 'pikpak',
          capabilities: { downloadFile: false },
          files: {},
        },
      } as any,
    })

    expect(result.exitCode).toBe(5)
    expect(JSON.parse(result.stdout)).toEqual({
      ok: false,
      error: {
        code: 'UNSUPPORTED_CAPABILITY',
        message: 'Provider "pikpak" does not support CLI download yet',
        exitCode: 5,
      },
    })
  })

  it('prints files list help without reading the cloud drive', async () => {
    const configDir = await makeTempDir()
    const result = await runBoxPlayerCli(['files', 'list', '--help'], {
      configDir,
      providers: {
        aliyun: {
          id: 'aliyun',
          files: {
            async list() {
              throw new Error('files.list should not be called for --help')
            },
          },
        },
      } as any,
    })

    expect(result.exitCode).toBe(0)
    expect(result.stderr).toBe('')
    expect(result.stdout).toContain('Usage: clouddrive-cli files list')
    expect(result.stdout).toContain('--file-id <id>')
    expect(result.stdout).toContain('--limit <n>')
    expect(result.stdout).toContain('--cursor <token>')
  })

  it('prints help for every subcommand without executing it', async () => {
    const configDir = await makeTempDir()
    const helpCases: Array<{ argv: string[]; contains: string }> = [
      { argv: ['auth', 'list', '--help'], contains: 'clouddrive-cli auth list' },
      { argv: ['auth', 'default', '--help'], contains: 'clouddrive-cli auth default' },
      { argv: ['auth', 'import-token', '--help'], contains: 'clouddrive-cli auth import-token' },
      { argv: ['auth', 'login', '--help'], contains: 'clouddrive-cli auth login' },
      { argv: ['auth', 'check', '--help'], contains: 'clouddrive-cli auth check' },
      { argv: ['settings', 'show', '--help'], contains: 'clouddrive-cli settings show' },
      { argv: ['providers', 'capabilities', '--help'], contains: 'clouddrive-cli providers capabilities' },
      { argv: ['list', '--help'], contains: 'clouddrive-cli list' },
      { argv: ['schema', 'commands', '--help'], contains: 'clouddrive-cli schema commands' },
      { argv: ['schema', 'plans', '--help'], contains: 'clouddrive-cli schema plans' },
      { argv: ['files', 'list', '--help'], contains: 'clouddrive-cli files list' },
      { argv: ['files', 'walk', '--help'], contains: 'clouddrive-cli files walk' },
      { argv: ['files', 'tree', '--help'], contains: 'clouddrive-cli files tree' },
      { argv: ['files', 'stats', '--help'], contains: 'clouddrive-cli files stats' },
      { argv: ['files', 'info', '--help'], contains: 'clouddrive-cli files info' },
      { argv: ['files', 'download', '--help'], contains: 'clouddrive-cli files download' },
      { argv: ['files', 'search', '--help'], contains: 'clouddrive-cli files search' },
      { argv: ['files', 'mkdir', '--help'], contains: 'clouddrive-cli files mkdir' },
      { argv: ['files', 'rename-apply', '--help'], contains: 'clouddrive-cli files rename-apply' },
      { argv: ['files', 'move-apply', '--help'], contains: 'clouddrive-cli files move-apply' },
      { argv: ['files', 'trash-apply', '--help'], contains: 'clouddrive-cli files trash-apply' },
      { argv: ['upload', 'plan', '--help'], contains: 'clouddrive-cli upload plan' },
      { argv: ['upload', 'apply', '--help'], contains: 'clouddrive-cli upload apply' },
      { argv: ['ops', 'list', '--help'], contains: 'clouddrive-cli ops list' },
      { argv: ['ops', 'show', '--help'], contains: 'clouddrive-cli ops show' },
      { argv: ['ops', 'undo', '--help'], contains: 'clouddrive-cli ops undo' },
    ]

    for (const helpCase of helpCases) {
      const result = await runBoxPlayerCli(helpCase.argv, { configDir })
      expect(result.exitCode, helpCase.argv.join(' ')).toBe(0)
      expect(result.stderr, helpCase.argv.join(' ')).toBe('')
      expect(result.stdout, helpCase.argv.join(' ')).toContain(helpCase.contains)
    }
  })

  it('rejects removed media plan commands', async () => {
    const configDir = await makeTempDir()

    for (const subcommand of ['rename-plan', 'organize-plan']) {
      const result = await runBoxPlayerCli(['media', subcommand, '--json'], { configDir })
      expect(result.exitCode).toBe(1)
      expect(JSON.parse(result.stdout)).toMatchObject({
        error: { message: expect.stringContaining('未知命令: media') },
      })
    }
  })

  it('rejects --path for files list', async () => {
    const configDir = await makeTempDir()

    const result = await runBoxPlayerCli([
      'files', 'list',
      '--provider', 'aliyun',
      '--path', 'root',
      '--json',
    ], { configDir })

    expect(result.exitCode).toBe(1)
    expect(JSON.parse(result.stdout)).toMatchObject({
      ok: false,
      error: {
        code: 'VALIDATION_ERROR',
        message: '--path has been removed. Use --file-id <folder-id>.',
      },
    })
  })

  it('returns a paginated files list page when limit is provided', async () => {
    const configDir = await makeTempDir()
    const store = createAuthStore({ configDir })
    await store.saveAccount({
      provider: 'aliyun',
      accountId: 'u1',
      displayName: 'Aliyun',
      token: { user_id: 'u1', default_drive_id: 'drive' },
    })
    await store.setDefaultAccount('aliyun', 'u1')

    const result = await runBoxPlayerCli([
      'files', 'list',
      '--provider', 'aliyun',
      '--account', 'default',
      '--file-id', 'folder-1',
      '--limit', '2',
      '--cursor', 'page-2',
      '--json',
    ], {
      configDir,
      providers: {
        aliyun: {
          id: 'aliyun',
          files: {
            async listPage({ parentFileId, limit, cursor }: { parentFileId: string; limit: number; cursor: string }) {
              expect(parentFileId).toBe('folder-1')
              expect(limit).toBe(2)
              expect(cursor).toBe('page-2')
              return {
                items: [{ type: 'file', fileId: 'file-2', name: 'Episode 02.mkv' }],
                nextCursor: 'page-3',
              }
            },
          },
        },
      } as any,
    })

    expect(result.exitCode).toBe(0)
    expect(JSON.parse(result.stdout)).toEqual({
      provider: 'aliyun',
      driveId: 'drive',
      parentFileId: 'folder-1',
      limit: 2,
      cursor: 'page-2',
      nextCursor: 'page-3',
      hasMore: true,
      items: [{ type: 'file', fileId: 'file-2', name: 'Episode 02.mkv' }],
    })
  })

  it('creates and dry-runs an upload plan from a single local file', async () => {
    const configDir = await makeTempDir()
    const localDir = await makeTempDir()
    const filePath = join(localDir, 'README.md')
    const outputPath = join(configDir, 'upload-file-plan.json')
    await writeFile(filePath, 'hello', 'utf8')

    const planned = await runBoxPlayerCli([
      'upload', 'plan',
      '--local', filePath,
      '--provider', 'cloud123',
      '--account', 'default',
      '--remote-parent', '0',
      '--output', outputPath,
      '--json',
    ], { configDir })

    expect(planned.exitCode).toBe(0)
    expect(JSON.parse(planned.stdout).items[0]).toMatchObject({ type: 'file', relative_path: '', target_name: 'README.md' })

    const dryRun = await runBoxPlayerCli(['upload', 'apply', outputPath, '--dry-run', '--json'], { configDir })

    expect(dryRun.exitCode).toBe(0)
    expect(JSON.parse(dryRun.stdout)).toMatchObject({
      ok: true,
      fileCount: 1,
      folderCount: 0,
      totalBytes: 5,
      errors: [],
    })
  })

  it('records rationale on upload dry-runs for agent auditability', async () => {
    const configDir = await makeTempDir()
    const planPath = join(configDir, 'upload-plan.json')
    await writeJson(planPath, {
      version: 1,
      operation: 'upload',
      provider: 'onedrive',
      account_id: 'default',
      local_root: configDir,
      remote_parent_file_id: 'onedrive_root',
      conflict: 'skip',
      items: [],
    })

    const dryRun = await runBoxPlayerCli([
      'upload', 'apply', planPath,
      '--dry-run',
      '--rationale', 'User asked to back up selected local files.',
      '--json',
    ], { configDir })

    expect(dryRun.exitCode).toBe(0)
    expect(JSON.parse(dryRun.stdout)).toMatchObject({
      ok: true,
      rationale: 'User asked to back up selected local files.',
    })
  })

  it('returns a clear error when undoing a non-undoable operation', async () => {
    const configDir = await makeTempDir()
    const store = createOperationLogStore({ configDir })
    await store.save({
      id: 'op_trash',
      type: 'trash',
      provider: 'aliyun',
      account_id: 'acc',
      started_at: '2026-01-01T00:00:00.000Z',
      finished_at: '2026-01-01T00:00:00.000Z',
      items: [],
    })

    const result = await runBoxPlayerCli(['ops', 'undo', 'op_trash', '--json'], { configDir })

    expect(result.exitCode).toBe(5)
    expect(JSON.parse(result.stdout)).toMatchObject({
      ok: false,
      error: { code: 'UNSUPPORTED_CAPABILITY', message: expect.stringContaining('Only rename and move operations are undoable') },
    })
  })

  it('requires rationale for real apply operations', async () => {
    const configDir = await makeTempDir()
    const planPath = join(configDir, 'upload-plan.json')
    await writeJson(planPath, {
      version: 1,
      operation: 'upload',
      provider: 'onedrive',
      account_id: 'default',
      local_root: configDir,
      remote_parent_file_id: 'onedrive_root',
      conflict: 'skip',
      items: [],
    })

    const result = await runBoxPlayerCli(['upload', 'apply', planPath, '--json'], { configDir })

    expect(result.exitCode).toBe(1)
    expect(JSON.parse(result.stdout)).toMatchObject({
      ok: false,
      error: { message: expect.stringContaining('--rationale') },
    })
  })

  it('prints help for positional plan commands without reading --help as a file', async () => {
    const configDir = await makeTempDir()
    const result = await runBoxPlayerCli(['files', 'move-apply', '--help'], { configDir })

    expect(result.exitCode).toBe(0)
    expect(result.stdout).toContain('Usage: clouddrive-cli files move-apply')
    expect(result.stderr).toBe('')
  })

  it('applies a small upload plan through the provider upload adapter', async () => {
    const configDir = await makeTempDir()
    const localDir = await makeTempDir()
    const filePath = join(localDir, 'hello.txt')
    const planPath = join(configDir, 'onedrive-upload-plan.json')
    await writeFile(filePath, 'hello', 'utf8')
    const store = createAuthStore({ configDir })
    await store.saveAccount({
      provider: 'onedrive',
      accountId: 'u1',
      displayName: 'OneDrive',
      token: {
        user_id: 'u1',
        access_token: 'access-token',
        default_drive_id: 'onedrive',
      },
    })
    await store.setDefaultAccount('onedrive', 'u1')
    await writeJson(planPath, {
      version: 1,
      operation: 'upload',
      provider: 'onedrive',
      account_id: 'default',
      local_root: localDir,
      remote_parent_file_id: 'onedrive_root',
      conflict: 'skip',
      items: [
        { type: 'file', local_path: filePath, relative_path: '', target_name: 'hello.txt', size: 5 },
      ],
    })
    vi.stubGlobal('fetch', vi.fn(async () => new Response(JSON.stringify({
      id: 'remote-file',
      name: 'hello.txt',
      size: 5,
      file: {},
      parentReference: { id: 'onedrive_root' },
    }), { status: 201, headers: { 'Content-Type': 'application/json' } })))

    const applied = await runBoxPlayerCli(['upload', 'apply', planPath, '--rationale', 'User asked to upload this file.', '--json'], { configDir })

    expect(applied.exitCode).toBe(0)
    expect(JSON.parse(applied.stdout)).toMatchObject({
      ok: true,
      rationale: 'User asked to upload this file.',
      succeeded: 1,
      failed: 0,
      results: [{ type: 'file', fileId: 'remote-file', status: 'success' }],
    })
  })
})
