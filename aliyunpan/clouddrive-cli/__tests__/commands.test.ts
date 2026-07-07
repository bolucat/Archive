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
        command: 'upload apply',
        safety: expect.objectContaining({ dryRunRequired: true, destructive: false }),
        providerRequirements: expect.objectContaining({ capability: 'uploadFile' }),
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

  it('docs read returns a local document as AI context JSON', async () => {
    const configDir = await makeTempDir()
    const docPath = join(configDir, 'context.md')
    await writeFile(docPath, '# Context\n\nUse this as background for the rename task.\n', 'utf8')

    const result = await runBoxPlayerCli(['docs', 'read', docPath, '--json'], { configDir })

    expect(result.exitCode).toBe(0)
    expect(JSON.parse(result.stdout)).toEqual({
      path: docPath,
      format: 'markdown',
      chars: 55,
      truncated: false,
      content: '# Context\n\nUse this as background for the rename task.\n',
    })
  })

  it('docs read supports max character truncation and text output', async () => {
    const configDir = await makeTempDir()
    const docPath = join(configDir, 'notes.txt')
    await writeFile(docPath, 'abcdefg', 'utf8')

    const result = await runBoxPlayerCli(['docs', 'read', docPath, '--max-chars', '4'], { configDir })

    expect(result).toEqual({
      exitCode: 0,
      stdout: 'abcd\n',
      stderr: '',
    })
  })

  it('docs read converts PDFs through OpenDataLoader before returning AI context', async () => {
    const configDir = await makeTempDir()
    const docPath = join(configDir, 'report.pdf')
    await writeFile(docPath, '%PDF-1.7 fake fixture', 'utf8')
    const convertCalls: unknown[] = []

    const result = await runBoxPlayerCli(['docs', 'read', docPath, '--max-chars', '12', '--json'], {
      configDir,
      openDataLoaderPdf: {
        async convert(paths: string[], options: { outputDir: string; format: string; quiet: boolean }) {
          convertCalls.push({ paths, options })
          await writeFile(join(options.outputDir, 'report.md'), 'Converted PDF Markdown content', 'utf8')
        },
      },
    } as any)

    expect(result.exitCode).toBe(0)
    expect(convertCalls).toHaveLength(1)
    expect(convertCalls[0]).toMatchObject({
      paths: [docPath],
      options: {
        format: 'markdown',
        quiet: true,
      },
    })
    expect(JSON.parse(result.stdout)).toEqual({
      path: docPath,
      format: 'pdf',
      sourceFormat: 'markdown',
      chars: 30,
      truncated: true,
      content: 'Converted PD',
    })
  })

  it('docs convert exposes the full OpenDataLoader option surface', async () => {
    const configDir = await makeTempDir()
    const docPath = join(configDir, 'report.pdf')
    const outputDir = join(configDir, 'out')
    await mkdir(outputDir, { recursive: true })
    await writeFile(docPath, '%PDF-1.7 fake fixture', 'utf8')
    const convertCalls: unknown[] = []

    const result = await runBoxPlayerCli([
      'docs', 'convert', docPath,
      '--output', outputDir,
      '--pdf-format', 'json,html,pdf,markdown,tagged-pdf,text',
      '--pdf-password', 'secret',
      '--pdf-content-safety-off', 'hidden-text,off-page',
      '--pdf-sanitize',
      '--pdf-keep-line-breaks',
      '--pdf-replace-invalid-chars', '?',
      '--pdf-use-struct-tree',
      '--pdf-table-method', 'cluster',
      '--pdf-reading-order', 'off',
      '--pdf-markdown-page-separator', '--- Page %page-number% ---',
      '--pdf-markdown-with-html',
      '--pdf-text-page-separator', '=== %page-number% ===',
      '--pdf-html-page-separator', '<hr data-page="%page-number%">',
      '--pdf-image-output', 'embedded',
      '--pdf-image-format', 'jpeg',
      '--pdf-image-dir', join(outputDir, 'images'),
      '--pdf-pages', '1,3,5-7',
      '--pdf-include-header-footer',
      '--pdf-detect-strikethrough',
      '--pdf-hybrid', 'hancom-ai',
      '--pdf-hybrid-mode', 'full',
      '--pdf-hybrid-url', 'http://127.0.0.1:5002',
      '--pdf-hybrid-timeout', '2500',
      '--pdf-hybrid-fallback',
      '--pdf-hybrid-hancom-ai-regionlist-strategy', 'list-only',
      '--pdf-hybrid-hancom-ai-ocr-strategy', 'force',
      '--pdf-hybrid-hancom-ai-image-cache', 'disk',
      '--pdf-threads', '4',
      '--json',
    ], {
      configDir,
      openDataLoaderPdf: {
        async convert(paths: string[], options: any) {
          convertCalls.push({ paths, options })
          await writeFile(join(outputDir, 'report.md'), 'Converted', 'utf8')
          await writeFile(join(outputDir, 'report.json'), '{"ok":true}', 'utf8')
        },
      },
    } as any)

    expect(result.exitCode).toBe(0)
    expect(convertCalls).toEqual([{
      paths: [docPath],
      options: {
        outputDir,
        format: 'json,html,pdf,markdown,tagged-pdf,text',
        quiet: true,
        password: 'secret',
        contentSafetyOff: 'hidden-text,off-page',
        sanitize: true,
        keepLineBreaks: true,
        replaceInvalidChars: '?',
        useStructTree: true,
        tableMethod: 'cluster',
        readingOrder: 'off',
        markdownPageSeparator: '--- Page %page-number% ---',
        markdownWithHtml: true,
        textPageSeparator: '=== %page-number% ===',
        htmlPageSeparator: '<hr data-page="%page-number%">',
        imageOutput: 'embedded',
        imageFormat: 'jpeg',
        imageDir: join(outputDir, 'images'),
        pages: '1,3,5-7',
        includeHeaderFooter: true,
        detectStrikethrough: true,
        hybrid: 'hancom-ai',
        hybridMode: 'full',
        hybridUrl: 'http://127.0.0.1:5002',
        hybridTimeout: '2500',
        hybridFallback: true,
        hybridHancomAiRegionlistStrategy: 'list-only',
        hybridHancomAiOcrStrategy: 'force',
        hybridHancomAiImageCache: 'disk',
        threads: '4',
      },
    }])
    expect(JSON.parse(result.stdout)).toMatchObject({
      path: docPath,
      outputDir,
      formats: ['json', 'html', 'pdf', 'markdown', 'tagged-pdf', 'text'],
      files: expect.arrayContaining([
        expect.objectContaining({ name: 'report.md' }),
        expect.objectContaining({ name: 'report.json' }),
      ]),
    })
  })

  it('docs read rejects missing paths', async () => {
    const configDir = await makeTempDir()

    const result = await runBoxPlayerCli(['docs', 'read'], { configDir })

    expect(result).toEqual({
      exitCode: 1,
      stdout: '',
      stderr: 'Usage: clouddrive-cli docs read <path> [--max-chars <n>] [--pdf-format <markdown|text|json|html>] [--pdf-pages <pages>] [--json]\n',
    })
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
      { argv: ['docs', 'read', '--help'], contains: 'clouddrive-cli docs read' },
      { argv: ['files', 'list', '--help'], contains: 'clouddrive-cli files list' },
      { argv: ['files', 'walk', '--help'], contains: 'clouddrive-cli files walk' },
      { argv: ['files', 'tree', '--help'], contains: 'clouddrive-cli files tree' },
      { argv: ['files', 'stats', '--help'], contains: 'clouddrive-cli files stats' },
      { argv: ['files', 'info', '--help'], contains: 'clouddrive-cli files info' },
      { argv: ['files', 'search', '--help'], contains: 'clouddrive-cli files search' },
      { argv: ['files', 'mkdir', '--help'], contains: 'clouddrive-cli files mkdir' },
      { argv: ['files', 'rename-apply', '--help'], contains: 'clouddrive-cli files rename-apply' },
      { argv: ['files', 'move-apply', '--help'], contains: 'clouddrive-cli files move-apply' },
      { argv: ['files', 'trash-apply', '--help'], contains: 'clouddrive-cli files trash-apply' },
      { argv: ['media', 'scan', '--help'], contains: 'clouddrive-cli media scan' },
      { argv: ['media', 'match', '--help'], contains: 'clouddrive-cli media match' },
      { argv: ['upload', 'plan', '--help'], contains: 'clouddrive-cli upload plan' },
      { argv: ['upload', 'apply', '--help'], contains: 'clouddrive-cli upload apply' },
      { argv: ['organize', 'analyze', '--help'], contains: 'clouddrive-cli organize analyze' },
      { argv: ['organize', 'plan', '--help'], contains: 'clouddrive-cli organize plan' },
      { argv: ['organize', 'apply', '--help'], contains: 'clouddrive-cli organize apply' },
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
        error: { message: `Unknown media command: ${subcommand}` },
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

  it('analyzes file exports and dry-runs an organize plan', async () => {
    const configDir = await makeTempDir()
    const filesPath = join(configDir, 'files.json')
    const analysisPath = join(configDir, 'analysis.json')
    const planPath = join(configDir, 'organize-plan.json')
    await writeJson(filesPath, [
      { provider: 'aliyun', accountId: 'acc', driveId: 'drive', fileId: 'movies', parentFileId: 'root', name: 'Movies', type: 'folder' },
      { provider: 'aliyun', accountId: 'acc', driveId: 'drive', fileId: 'f1', parentFileId: 'root', name: 'Film.2024.mkv', type: 'file', size: 10 },
      { provider: 'aliyun', accountId: 'acc', driveId: 'drive', fileId: 'f2', parentFileId: 'root', name: 'Show.S01E01.mkv', type: 'file', size: 20 },
    ])

    const analyzed = await runBoxPlayerCli([
      'organize', 'analyze',
      '--input', filesPath,
      '--provider', 'aliyun',
      '--account', 'acc',
      '--file-id', 'root',
      '--output', analysisPath,
      '--json',
    ], { configDir })

    expect(analyzed.exitCode).toBe(0)
    expect(JSON.parse(analyzed.stdout).stats).toMatchObject({ totalItems: 3, videoCount: 2 })

    const planned = await runBoxPlayerCli([
      'organize', 'plan',
      '--analysis', analysisPath,
      '--output', planPath,
      '--json',
    ], { configDir })

    expect(planned.exitCode).toBe(0)
    expect(JSON.parse(planned.stdout).actions.map((a: { type: string }) => a.type)).toEqual(['mkdir', 'move', 'move'])

    const dryRun = await runBoxPlayerCli(['organize', 'apply', planPath, '--dry-run', '--json'], { configDir })

    expect(dryRun.exitCode).toBe(0)
    expect(JSON.parse(dryRun.stdout)).toMatchObject({
      ok: true,
      actionCount: 3,
      counts: { mkdir: 1, move: 2, rename: 0, copy: 0, trash: 0 },
    })

    const summarized = await runBoxPlayerCli(['organize', 'apply', planPath, '--dry-run', '--summary', '--json'], { configDir })

    expect(summarized.exitCode).toBe(0)
    expect(JSON.parse(summarized.stdout)).toMatchObject({
      ok: true,
      actionCount: 3,
      counts: { mkdir: 1, move: 2, rename: 0, copy: 0, trash: 0 },
      moveTargets: { 'folder:Movies': 1, 'folder:TV Shows': 1 },
    })
    expect(JSON.parse(summarized.stdout).actions).toBeUndefined()
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

    const applied = await runBoxPlayerCli(['upload', 'apply', planPath, '--json'], { configDir })

    expect(applied.exitCode).toBe(0)
    expect(JSON.parse(applied.stdout)).toMatchObject({
      ok: true,
      succeeded: 1,
      failed: 0,
      results: [{ type: 'file', fileId: 'remote-file', status: 'success' }],
    })
  })
})
